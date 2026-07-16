import Foundation
import Observation

@MainActor @Observable
public final class SearchController {
    public private(set) var state: SearchState?
    public private(set) var baseResults: [SeedResult] = []
    public private(set) var scannedSeeds: Int64 = 0
    public private(set) var totalSeeds: Int64 = 0
    public private(set) var matchProbability: Double?
    public private(set) var seedsPerSecond: Double = 0
    public private(set) var elapsed: TimeInterval = 0
    public private(set) var errorCode: Int64 = 0
    public private(set) var message: String?
    public private(set) var isRunning = false
    public private(set) var isFiltering = false
    public private(set) var filterMessage: String?
    public private(set) var isImportedList = false
    public var seedSearchText = ""
    public var selectedSeed: String?

    private let engine: any SeedFinderEngine
    private var semanticResults: [SeedResult]?
    private var session: (any SeedFinderSearchSession)?
    private var task: Task<Void, Never>?
    private var filterTask: Task<Void, Never>?
    private var filterGeneration = 0

    public init(engine: any SeedFinderEngine = ProductionSeedFinderEngine()) { self.engine = engine }
    public var timeToSeed: TimeInterval? {
        guard let matchProbability, seedsPerSecond > 0 else { return nil }
        return 1 / matchProbability / seedsPerSecond
    }
    public var results: [SeedResult] {
        let source = semanticResults ?? baseResults
        let search = normalizedSeedSearch(seedSearchText)
        guard !search.isEmpty else { return source }
        return source.filter { normalizedSeedSearch($0.seed).contains(search) }
    }
    public var reachedResultCap: Bool { !isImportedList && baseResults.count >= 1_024 }
    public var hasSemanticFilter: Bool { semanticResults != nil }
    public var hasActiveFilter: Bool {
        hasSemanticFilter || !normalizedSeedSearch(seedSearchText).isEmpty
    }
    /// The engine completes an unsatisfiable plan before scanning any seed,
    /// which would otherwise be indistinguishable from a malfunction.
    public var isImpossibleQuery: Bool {
        state == .completed && scannedSeeds == 0 && baseResults.isEmpty
    }

    public func start(_ request: SearchRequest) {
        guard !isFiltering else { return }
        cancelFilter()
        task?.cancel(); baseResults = []; semanticResults = nil; seedSearchText = ""
        isImportedList = false; selectedSeed = nil
        scannedSeeds = 0; totalSeeds = 0; matchProbability = nil; seedsPerSecond = 0; elapsed = 0
        errorCode = 0; message = nil; state = .running; isRunning = true
        task = Task { [weak self] in
            guard let self else { return }
            let searchStart = ContinuousClock.now
            do {
                let session = try await engine.startSearch(request)
                self.session = session
                var previousCount: Int64 = 0
                var previousTime = ContinuousClock.now
                while !Task.isCancelled {
                    let batch = try await session.poll(1_024)
                    self.baseResults.append(contentsOf: batch)
                    let status = try await session.status()
                    let now = ContinuousClock.now
                    let totalDuration = searchStart.duration(to: now).components
                    self.elapsed = Double(totalDuration.seconds) + Double(totalDuration.attoseconds) / 1e18
                    let seconds = Double(previousTime.duration(to: now).components.attoseconds) / 1e18
                        + Double(previousTime.duration(to: now).components.seconds)
                    if seconds > 0 {
                        let instantRate = Double(max(0, status.scannedSeeds - previousCount)) / seconds
                        self.seedsPerSecond = self.seedsPerSecond == 0 ? instantRate : self.seedsPerSecond * 0.7 + instantRate * 0.3
                    }
                    previousCount = status.scannedSeeds; previousTime = now
                    self.scannedSeeds = status.scannedSeeds; self.totalSeeds = status.totalSeeds
                    self.matchProbability = status.matchProbability > 0 ? status.matchProbability : nil
                    self.errorCode = status.errorCode; self.state = status.state
                    if status.state != .running {
                        let finalBatch = try await session.poll(1_024)
                        self.baseResults.append(contentsOf: finalBatch)
                        break
                    }
                    try await Task.sleep(for: .milliseconds(150))
                }
                await session.close()
            } catch is CancellationError {
                await self.session?.cancel(); await self.session?.close()
                self.state = .cancelled
            } catch {
                await self.session?.close(); self.state = .failed; self.message = error.localizedDescription
            }
            self.session = nil; self.isRunning = false
        }
    }

    public func filterSeeds(matching request: SearchRequest) {
        guard !isRunning, !isFiltering, !baseResults.isEmpty else { return }
        filterGeneration += 1
        let generation = filterGeneration
        let snapshot = baseResults
        isFiltering = true
        filterMessage = nil
        filterTask = Task { [weak self] in
            guard let self else { return }
            do {
                let matches = try await engine.filterSeeds(snapshot.map(\.seed), matching: request)
                try Task.checkCancellation()
                guard generation == filterGeneration else { return }
                let matchingSeeds = Set(matches.map(\.seed))
                semanticResults = snapshot.compactMap { result in
                    matchingSeeds.contains(result.seed)
                        ? SeedResult(seed: result.seed, matchedRequirements: request.requirements.count)
                        : nil
                }
                if let selectedSeed, !matchingSeeds.contains(selectedSeed) { self.selectedSeed = nil }
            } catch is CancellationError {
                // A new search or imported list superseded this filter.
            } catch {
                guard generation == filterGeneration else { return }
                filterMessage = error.localizedDescription
            }
            if generation == filterGeneration {
                isFiltering = false
                filterTask = nil
            }
        }
    }

    public func clearFilter() {
        guard !isFiltering else { return }
        cancelFilter()
        semanticResults = nil
        seedSearchText = ""
        filterMessage = nil
    }

    public func replaceWithImportedSeeds(_ seeds: [String]) throws {
        guard !isRunning, !isFiltering else { return }
        let normalized = try SeedListCodec.decode(SeedListCodec.encode(seeds))
        cancelFilter()
        task?.cancel()
        baseResults = normalized.map { SeedResult(seed: $0, matchedRequirements: 0) }
        semanticResults = nil
        seedSearchText = ""
        selectedSeed = nil
        state = nil
        scannedSeeds = 0
        totalSeeds = 0
        matchProbability = nil
        seedsPerSecond = 0
        elapsed = 0
        errorCode = 0
        message = nil
        filterMessage = nil
        isImportedList = true
    }

    public func cancel() {
        guard isRunning else { return }
        Task { await session?.cancel() }
    }

    private func cancelFilter() {
        filterGeneration += 1
        filterTask?.cancel()
        filterTask = nil
        isFiltering = false
        filterMessage = nil
    }
}

private func normalizedSeedSearch(_ value: String) -> String {
    value.filter { $0 != "-" && !$0.isWhitespace }
        .uppercased(with: Locale(identifier: "en_US_POSIX"))
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
