import Foundation
import Observation

/// Everything a later refine needs from a finished search: the request that
/// ran, plus where a follow-up scan must pick up (`remaining` seeds starting
/// at `resumeFrom`) to complete its seed-space coverage.
public struct BaseRun: Codable, Sendable {
    public let request: SearchRequest
    public let resumeFrom: Int64
    public let remaining: Int64
    public init(request: SearchRequest, resumeFrom: Int64, remaining: Int64) {
        self.request = request; self.resumeFrom = resumeFrom; self.remaining = remaining
    }
}

/// Every loaded or discovered seed, with its original recipe and selection source.
public struct TargetState: Codable, Sendable {
    public let request: SearchRequest
    /// All saved seeds in discovery order.
    public var seeds: [String]
    public var recipes: [String: SeedResult]
    public var sources: [String: SearchRequest] = [:]
    public init(request: SearchRequest, seeds: [String], recipes: [String: SeedResult] = [:]) {
        self.request = request; self.seeds = seeds; self.recipes = recipes
    }
}

public struct RefineProgress: Sendable {
    public let checked: Int
    public let total: Int
}

@MainActor @Observable
public final class SearchController {
    public private(set) var state: SearchState?
    /// The listed results, capped at `resultCap` rows: a refine of a grown
    /// Target Set can keep far more survivors than the display holds, and an
    /// uncapped SwiftUI table is what a 5,000-row hang is made of. The run's
    /// full result set lives in `collected`.
    public private(set) var results: [SeedResult] = []
    public private(set) var scannedSeeds: Int64 = 0
    public private(set) var totalSeeds: Int64 = 0
    public private(set) var matchProbability: Double?
    public var probabilityLabel: String {
        return NumberFormat.probabilityPercent(matchProbability)
    }
    public private(set) var seedsPerSecond: Double = 0
    public private(set) var elapsed: TimeInterval = 0
    public private(set) var errorCode: Int64 = 0
    public private(set) var message: String?
    public private(set) var isRunning = false
    public private(set) var isPreparing = false
    public private(set) var refineProgress: RefineProgress?
    /// The last finished (completed or cancelled) run, ready to be refined.
    public private(set) var baseRun: BaseRun?
    /// The session's Target, if one has been established — see
    /// docs/search-semantics.md. Only `clearResults()` discards it.
    public private(set) var target: TargetState?
    /// How many previous results survived the last refine; nil after a fresh search.
    public private(set) var refinedKept: Int?
    /// The size of the full saved pool checked by the last search.
    public private(set) var refinedOf: Int?
    /// Whether the current results were restored from an imported file
    /// rather than produced by a search.
    public private(set) var isImported = false
    /// Imported entries dropped as duplicates or beyond the result cap.
    public private(set) var importedDropped = 0
    /// The query that produced the current results, snapshotted at search
    /// start (or import) so an export never reflects later editor changes.
    public private(set) var exportQuery: SavedQuery?
    public var selectedSeed: String?

    /// How many rows the displayed list holds at most.
    public static let resultCap = 1_024

    private let engine: any SeedFinderEngine
    private var session: (any SeedFinderSearchSession)?
    private var task: Task<Void, Never>?
    /// Every unique seed of the current run — filter survivors plus scanned
    /// finds — in discovery order and uncapped, unlike the displayed
    /// `results`. Every discovery also joins the saved pool.
    private var collected: [String] = []
    private var collectedRecipes: [String: SeedResult] = [:]

    private let checkpointURL: URL?
    private let checkpointInterval: Duration
    private var pendingSearch: PendingSearchCheckpoint?
    private var stopRequested = false
    private var interruptionRequested = false
    private var resumeWhenSettled = false
    private var resumedScanned: Int64 = 0
    private var resumedElapsed: TimeInterval = 0
    private var resumedGoal: Int?

    /// Supplying a private application-support URL enables atomic recovery of
    /// results, recipes and fully drained traversal checkpoints. The host app
    /// calls `interrupt()` before suspension and `resumeInterrupted()` when visible.
    public init(engine: any SeedFinderEngine = ProductionSeedFinderEngine(),
                checkpointURL: URL? = nil, checkpointInterval: Duration = .seconds(15)) {
        self.engine = engine
        self.checkpointURL = checkpointURL
        self.checkpointInterval = checkpointInterval
        restoreCheckpoint()
    }

    public var timeToSeed: TimeInterval? {
        guard let matchProbability, seedsPerSecond > 0 else { return nil }
        return 1 / matchProbability / seedsPerSecond
    }
    public var reachedResultCap: Bool { results.count >= Self.resultCap }
    public var foundCount: Int { collected.count }
    public var hasPendingSearch: Bool { pendingSearch != nil }
    /// The interrupted search's board can differ from the saved editor draft
    /// and from the previous result query while refinement is still pending.
    public var pendingQuery: SavedQuery? {
        guard let request = pendingSearch?.request else { return nil }
        return SavedQuery(requirements: request.requirements, maximumDepth: request.maximumDepth,
            requireBlacksmith: request.requireBlacksmith, excludeBlacksmithRewards: request.excludeBlacksmithRewards,
            wandmakerQuest: request.wandmakerQuest, challenges: request.challenges,
            autoApplyTrinket: request.autoApplyTrinket, arcaneResin: request.arcaneResin,
            arcaneResinFilter: request.arcaneResinFilter, arcaneResinAuto: request.arcaneResinAuto,
            floorRequirements: request.floorRequirements)
    }
    /// The engine completes an unsatisfiable plan before scanning any seed,
    /// which would otherwise be indistinguishable from a malfunction.
    public var isImpossibleQuery: Bool {
        state == .completed && scannedSeeds == 0 && results.isEmpty
    }
    public private(set) var impossibleReason: String?

    /// Replaces the results with seeds restored from an imported results
    /// file and remembers the query that produced them for later export. The
    /// shared import rule — deduplicate, then cap at the result limit — is the
    /// engine's, applied while decoding the file, so `seeds` is taken as given
    /// and `dropped` is what that step removed. Callers must ensure no search
    /// is running.
    public func loadImported(seeds: [String], dropped: Int = 0, query: SavedQuery, trinkets: [String?] = []) {
        guard !isRunning else { return }
        results = seeds.enumerated().map { index, seed in SeedResult(seed: seed, matchedRequirements: query.slotCount, selectedTrinket: index < trinkets.count ? trinkets[index] : nil) }
        collectedRecipes = Dictionary(uniqueKeysWithValues: results.map { ($0.seed, $0) })
        collected = seeds
        importedDropped = dropped
        exportQuery = query
        scannedSeeds = 0; totalSeeds = 0; matchProbability = nil; seedsPerSecond = 0; elapsed = 0
        errorCode = 0; message = nil; state = nil; isImported = true; selectedSeed = nil
        impossibleReason = nil
        // Imported results carry no traversal state, so the previous
        // search's base run no longer describes the listed seeds.
        baseRun = nil; refinedKept = nil; refinedOf = nil
        // Imports add to the retained pool; they provide no scan cursor.
        let request = try? SearchRequest(
            requirements: query.requirements, maximumDepth: query.maximumDepth,
            requireBlacksmith: query.requireBlacksmith,
            excludeBlacksmithRewards: query.excludeBlacksmithRewards,
            wandmakerQuest: query.wandmakerQuest,
            challenges: query.challenges, autoApplyTrinket: query.autoApplyTrinket, arcaneResin: query.arcaneResin, arcaneResinFilter: query.arcaneResinFilter, arcaneResinAuto: query.arcaneResinAuto, floorRequirements: query.floorRequirements)
        if let request { remember(results, source: request) }
        pendingSearch = nil
        saveCheckpoint()
    }

    /// Search checks every retained seed and then looks for more matches.
    public func start(_ request: SearchRequest, workers: Int = WorkerPersistence.unset) {
        guard !isRunning else { return }
        let pool = target
        let encoded = try? QueryDocument.encode(request)
        let previous = baseRun.flatMap { (try? QueryDocument.encode($0.request)) == encoded ? $0 : nil }
        task?.cancel(); resetProgress()
        stopRequested = false; interruptionRequested = false; resumeWhenSettled = false
        resumedScanned = 0; resumedElapsed = 0; resumedGoal = nil
        pendingSearch = PendingSearchCheckpoint(request: request, workers: workers)
        impossibleReason = nil
        if target == nil { target = TargetState(request: request, seeds: []) }
        saveCheckpoint()
        task = Task { [weak self] in
            guard let self else { return }
            defer { self.finishedTask() }
            do {
                self.impossibleReason = await Task.detached {
                    encoded.flatMap { try? QueryAnalysis.impossibilityReason($0) }
                }.value
                try Task.checkCancellation()
                var groups: [(source: SearchRequest, recipes: [SeedResult])] = []
                for seed in pool?.seeds ?? [] {
                    let source = pool?.sources[seed] ?? pool?.request ?? request
                    let recipe = pool?.recipes[seed] ?? SeedResult(seed: seed, matchedRequirements: source.slotCount)
                    let key = try QueryDocument.encode(source)
                    if let index = try groups.firstIndex(where: { try QueryDocument.encode($0.source) == key }) {
                        groups[index].recipes.append(recipe)
                    } else { groups.append((source, [recipe])) }
                }
                var kept: [SeedResult] = []
                var checked = 0
                self.isPreparing = false
                if let pool, !pool.seeds.isEmpty {
                    self.refineProgress = RefineProgress(checked: 0, total: pool.seeds.count)
                }
                for group in groups {
                    let chunkSize = checkpointURL == nil ? max(1, group.recipes.count) : 24
                    for offset in stride(from: 0, to: group.recipes.count, by: chunkSize) {
                        try Task.checkCancellation()
                        let chunk = Array(group.recipes[offset..<min(offset + chunkSize, group.recipes.count)])
                        kept += try await engine.filterRecipes(request, base: group.source, recipes: chunk)
                        checked += chunk.count
                        self.refineProgress = RefineProgress(checked: checked, total: pool?.seeds.count ?? checked)
                    }
                }
                try Task.checkCancellation()
                self.refineProgress = nil
                self.collected = kept.map(\.seed)
                self.collectedRecipes = Dictionary(uniqueKeysWithValues: kept.map { ($0.seed, $0) })
                self.results = Array(kept.prefix(Self.resultCap))
                self.refinedKept = pool == nil ? nil : kept.count; self.refinedOf = pool?.seeds.count
                self.exportQuery = SavedQuery(
                    requirements: request.requirements, maximumDepth: request.maximumDepth,
                    requireBlacksmith: request.requireBlacksmith, excludeBlacksmithRewards: request.excludeBlacksmithRewards,
                    wandmakerQuest: request.wandmakerQuest, challenges: request.challenges,
                    autoApplyTrinket: request.autoApplyTrinket, arcaneResin: request.arcaneResin,
                    arcaneResinFilter: request.arcaneResinFilter, arcaneResinAuto: request.arcaneResinAuto, floorRequirements: request.floorRequirements)
                self.isImported = false; self.importedDropped = 0
                if let previous, previous.remaining == 0 {
                    self.state = .completed; self.isRunning = false
                    return
                }
                await self.run(request, alreadyShown: Set(kept.map(\.seed)), workers: workers) { engine in
                    if let previous {
                        return try await engine.startResumedSearch(request, resumeFrom: previous.resumeFrom,
                            scanLen: previous.remaining, workers: workers)
                    }
                    return try await engine.startSearch(request, workers: workers)
                }
            } catch is CancellationError {
                self.state = .cancelled; self.isRunning = false
            } catch {
                self.state = .failed; self.message = error.localizedDescription; self.isRunning = false
            }
        }
    }

    /// Whether there is anything for `clearResults()` to discard.
    public var canClearResults: Bool {
        !isRunning && (!results.isEmpty || state != nil || baseRun != nil
            || exportQuery != nil || target != nil)
    }

    /// Discards the saved pool, visible matches, and scan cursor. Ignored while searching.
    public func clearResults() {
        guard !isRunning else { return }
        results = []; collected = []; collectedRecipes = [:]; selectedSeed = nil; exportQuery = nil
        isImported = false; importedDropped = 0
        baseRun = nil; refinedKept = nil; refinedOf = nil
        target = nil
        scannedSeeds = 0; totalSeeds = 0; matchProbability = nil; seedsPerSecond = 0; elapsed = 0
        errorCode = 0; message = nil; state = nil
        impossibleReason = nil
        pendingSearch = nil
        saveCheckpoint()
    }

    /// Opening another query clears its displayed matches without discarding
    /// the saved pool or the coverage of the preceding search.
    public func clearDisplayedResults() {
        guard !isRunning else { return }
        results = []; collected = []; collectedRecipes = [:]; selectedSeed = nil; exportQuery = nil
        isImported = false; importedDropped = 0; refinedKept = nil; refinedOf = nil
        scannedSeeds = 0; totalSeeds = 0; matchProbability = nil; seedsPerSecond = 0; elapsed = 0
        errorCode = 0; message = nil; state = nil; pendingSearch = nil
        impossibleReason = nil
        saveCheckpoint()
    }

    public func cancel() {
        guard isRunning else { return }
        stopRequested = true; interruptionRequested = false; resumeWhenSettled = false
        // Commit Stop before draining so a killed process cannot restart it.
        pendingSearch = nil
        saveCheckpoint()
        if let session {
            Task { await session.cancel() }
        } else {
            // No native session yet (refine's filter phase): cancel the
            // controller task so the awaited filter throws CancellationError.
            task?.cancel()
        }
    }

    /// Cooperatively closes native workers before iOS suspension. Only a
    /// settled session's resume hint is safe: an active hint can skip chunks.
    public func interrupt() {
        guard isRunning else { return }
        interruptionRequested = true; resumeWhenSettled = false
        if let session { Task { await session.cancel() } }
        else { task?.cancel() }
    }

    /// Restarts an interrupted search when the host becomes visible. Returning
    /// while workers are draining never opens a second native session.
    public func resumeInterrupted() {
        guard let pendingSearch else { return }
        if isRunning {
            if interruptionRequested { resumeWhenSettled = true }
        } else {
            let previousScanned = scannedSeeds, previousElapsed = elapsed, previousTotal = totalSeeds
            start(pendingSearch.request, workers: pendingSearch.workers)
            resumedScanned = previousScanned; resumedElapsed = previousElapsed
            resumedGoal = pendingSearch.resultGoal
            self.pendingSearch?.resultGoal = pendingSearch.resultGoal
            scannedSeeds = previousScanned; elapsed = previousElapsed; totalSeeds = previousTotal
            saveCheckpoint()
        }
    }

    public func waitUntilSettled() async {
        await task?.value
    }

    private func resetProgress() {
        scannedSeeds = 0; totalSeeds = 0; matchProbability = nil; seedsPerSecond = 0; elapsed = 0
        errorCode = 0; message = nil; state = .running; isRunning = true
        isPreparing = true; refineProgress = nil
    }

    /// Runs one native session's poll loop, appending results not already in
    /// `alreadyShown`. A run that stops cleanly (completed or cancelled)
    /// records its resume hint as the new base run; a failure clears it.
    private func run(_ request: SearchRequest, alreadyShown: Set<String>, workers: Int,
                     startSession: (any SeedFinderEngine) async throws -> any SeedFinderSearchSession) async {
        let searchStart = ContinuousClock.now
        var shown = alreadyShown
        let goal = resumedGoal ?? (shown.count >= Self.resultCap ? shown.count + Self.resultCap : Self.resultCap)
        pendingSearch?.resultGoal = goal
        var scannedBefore = resumedScanned
        var originalTotal = totalSeeds
        saveCheckpoint()
        do {
            isPreparing = true
            var session = try await startSession(engine)
            self.session = session
            try Task.checkCancellation()
            isPreparing = false
            var previousCount: Int64 = 0
            var previousTime = ContinuousClock.now
            var checkpointStarted = ContinuousClock.now
            var checkpointing = false
            while !Task.isCancelled {
                let batch = try await session.poll(1_024)
                self.append(batch, excluding: &shown, source: request)
                let status = try await session.status()
                let now = ContinuousClock.now
                let totalDuration = searchStart.duration(to: now).components
                self.elapsed = resumedElapsed + Double(totalDuration.seconds) + Double(totalDuration.attoseconds) / 1e18
                let interval = previousTime.duration(to: now).components
                let seconds = Double(interval.seconds) + Double(interval.attoseconds) / 1e18
                if seconds > 0 {
                    let instantRate = Double(max(0, status.scannedSeeds - previousCount)) / seconds
                    self.seedsPerSecond = self.seedsPerSecond == 0 ? instantRate : self.seedsPerSecond * 0.7 + instantRate * 0.3
                }
                previousCount = status.scannedSeeds; previousTime = now
                if originalTotal == 0 { originalTotal = status.totalSeeds }
                self.scannedSeeds = min(originalTotal, scannedBefore + status.scannedSeeds)
                self.totalSeeds = originalTotal
                self.matchProbability = status.matchProbability > 0 ? status.matchProbability : nil
                self.errorCode = status.errorCode; self.state = status.state
                if status.state == .running {
                    if stopRequested || interruptionRequested || shown.count >= goal {
                        await session.cancel()
                    } else if checkpointURL != nil && checkpointStarted.duration(to: now) >= checkpointInterval {
                        checkpointing = true
                        await session.cancel()
                    }
                } else {
                    // Cancellation joins workers, but any number of batches can
                    // still be queued. Drain every one before persisting coverage.
                    while true {
                        let finalBatch = try await session.poll(1_024)
                        if finalBatch.isEmpty { break }
                        self.append(finalBatch, excluding: &shown, source: request)
                    }
                    if status.state == .failed {
                        self.baseRun = nil
                        self.message = status.errorCode == 2_001
                            ? "A native world-generation worker stopped unexpectedly."
                            : "The native search stopped with error \(status.errorCode)."
                        break
                    }
                    let hint = try await session.resumeHint()
                    self.baseRun = BaseRun(request: request, resumeFrom: hint.position, remaining: hint.remaining)
                    let shouldContinue = !stopRequested && !interruptionRequested
                        && hint.remaining > 0 && shown.count < goal
                        && (checkpointing || (status.state == .completed && status.scannedSeeds > 0))
                    if shouldContinue {
                        self.state = .running
                        saveCheckpoint()
                        await session.close()
                        scannedBefore = self.scannedSeeds
                        session = try await engine.startResumedSearch(request, resumeFrom: hint.position,
                            scanLen: hint.remaining, workers: workers)
                        self.session = session
                        previousCount = 0; previousTime = .now; checkpointStarted = .now; checkpointing = false
                        continue
                    }
                    self.state = stopRequested ? .cancelled
                        : hint.remaining == 0 || shown.count >= goal ? .completed : status.state
                    break
                }
                try await Task.sleep(for: .milliseconds(150))
            }
            await session.close()
        } catch is CancellationError {
            await self.session?.cancel(); await self.session?.close()
            self.state = .cancelled
            // Keep the preceding safe cursor; no hint is read from an active
            // native session, so abrupt cancellation may only repeat work.
        } catch {
            await self.session?.close(); self.state = .failed; self.message = error.localizedDescription
            self.baseRun = nil
        }
        self.session = nil; self.isRunning = false
    }

    private func finishedTask() {
        isRunning = false
        isPreparing = false; refineProgress = nil
        if !interruptionRequested || stopRequested || state == .failed || state == .completed {
            pendingSearch = nil
        } else if pendingSearch != nil {
            message = "Search interrupted. It will continue when you reopen the app."
        }
        saveCheckpoint()
        if resumeWhenSettled, pendingSearch != nil {
            // The preceding task must return before a new task replaces it.
            Task { [weak self] in self?.resumeInterrupted() }
        }
    }

    private func saveCheckpoint() {
        guard let checkpointURL else { return }
        let snapshot = SearchCheckpoint(results: results,
            collected: collected.compactMap { collectedRecipes[$0] }, query: exportQuery,
            target: target, baseRun: baseRun, pending: pendingSearch, state: state,
            scanned: scannedSeeds, total: totalSeeds, elapsed: elapsed,
            probability: matchProbability, isImported: isImported, importedDropped: importedDropped,
            errorCode: errorCode, message: message)
        do { try snapshot.save(to: checkpointURL) }
        catch { message = "Could not save search progress: \(error.localizedDescription)" }
    }

    private func restoreCheckpoint() {
        guard let checkpointURL else { return }
        do {
            guard let saved = try SearchCheckpoint.load(from: checkpointURL) else { return }
            results = Array(saved.results.prefix(Self.resultCap)); exportQuery = saved.query
            collected = saved.collected.map(\.seed)
            collectedRecipes = Dictionary(saved.collected.map { ($0.seed, $0) }, uniquingKeysWith: { first, _ in first })
            target = saved.target; baseRun = saved.baseRun; pendingSearch = saved.pending
            state = saved.state == .running && saved.pending == nil ? .cancelled : saved.state
            scannedSeeds = saved.scanned; totalSeeds = saved.total; elapsed = saved.elapsed
            matchProbability = saved.probability; isImported = saved.isImported
            importedDropped = saved.importedDropped; errorCode = saved.errorCode; message = saved.message
            if saved.engine != SearchCheckpoint.engineIdentifier {
                baseRun = nil; pendingSearch = nil; state = nil
                message = "The engine changed. Saved results were restored; start a search to recheck them."
            }
        } catch {
            message = "The saved search could not be restored: \(error.localizedDescription)"
        }
    }

    private func remember(_ entries: [SeedResult], source: SearchRequest) {
        var pool = target ?? TargetState(request: source, seeds: [])
        var known = Set(pool.seeds)
        for entry in entries where known.insert(entry.seed).inserted {
            pool.seeds.append(entry.seed)
            pool.recipes[entry.seed] = entry
            pool.sources[entry.seed] = source
        }
        target = pool
    }

    private func append(_ batch: [SeedResult], excluding shown: inout Set<String>, source: SearchRequest) {
        remember(batch, source: source)
        guard !batch.isEmpty else { return }
        let fresh = batch.filter { shown.insert($0.seed).inserted }
        guard !fresh.isEmpty else { return }
        collected.append(contentsOf: fresh.map(\.seed))
        for result in fresh { collectedRecipes[result.seed] = result }
        // Only the display is capped; everything delivered stays collected
        // for the Target and later refines.
        if results.count < Self.resultCap {
            results.append(contentsOf: fresh.prefix(Self.resultCap - results.count))
        }
    }
}

public enum NumberFormat {
    public static func si(_ value: Double) -> String {
        let units = [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")]
        for (scale, suffix) in units where value >= scale {
            let scaled = value / scale
            return String(format: scaled >= 100 ? "%.0f%@" : scaled >= 10 ? "%.1f%@" : "%.2f%@", scaled, suffix)
        }
        return String(format: "%.0f", value)
    }
    public static func duration(_ seconds: TimeInterval?) -> String {
        guard let seconds, seconds.isFinite else { return "—" }
        let total = Int(seconds.rounded())
        if total < 60 { return "\(total)s" }
        if total < 3_600 { return "\(total / 60)m \(total % 60)s" }
        return "\(total / 3_600)h \((total % 3_600) / 60)m"
    }
    public static func probabilityPercent(_ probability: Double?) -> String {
        guard let probability, probability > 0 else { return "estimating…" }
        let percent = probability * 100
        var exponent = Int(floor(log10(percent)))
        var mantissa = percent / pow(10, Double(exponent))
        if mantissa >= 9.95 { mantissa = 1; exponent += 1 }
        return String(format: "%.1fx10^%d%%", mantissa, exponent)
    }
    public static func estimateDuration(_ seconds: TimeInterval?) -> String {
        guard let seconds, seconds.isFinite else { return "estimating…" }
        let value: Double
        let unit: String
        if seconds < 60 { value = seconds; unit = "second" }
        else if seconds < 3_600 { value = seconds / 60; unit = "minute" }
        else if seconds < 86_400 { value = seconds / 3_600; unit = "hour" }
        else { value = seconds / 86_400; unit = "day" }
        let suffix = value >= 0.95 && value < 1.05 ? "" : "s"
        return String(format: "%.1f %@%@", value, unit, suffix)
    }
    public static func seedRate(_ value: Double) -> String {
        guard value > 0 else { return "—" }
        if value >= 1e6 { return String(format: "%.1fM", value / 1e6) }
        if value >= 1e3 { return String(format: "%.1fk", value / 1e3) }
        return String(format: "%.0f", value)
    }
}
