import Foundation
import XCTest
@testable import SeedSeekerKit

/// A scripted session: each poll drains one batch, the status turns terminal
/// once every batch is delivered, and the resume hint is fixed up front.
private final class FakeSearchSession: SeedFinderSearchSession, @unchecked Sendable {
    private let lock = NSLock()
    private var batches: [[SeedResult]]
    private let finalState: SearchState
    private let hint: ResumeHint
    private(set) var closed = false

    init(batches: [[SeedResult]], finalState: SearchState = .completed, hint: ResumeHint) {
        self.batches = batches; self.finalState = finalState; self.hint = hint
    }
    func poll(_ maximum: Int) async throws -> [SeedResult] {
        lock.withLock { batches.isEmpty ? [] : batches.removeFirst() }
    }
    func status() async throws -> SearchStatus {
        lock.withLock {
            SearchStatus(state: batches.isEmpty ? finalState : .running,
                         scannedSeeds: 0, totalSeeds: 0, errorCode: 0, matchProbability: 0)
        }
    }
    func resumeHint() async throws -> ResumeHint { hint }
    func cancel() async {}
    func close() async { lock.withLock { closed = true } }
}

private final class FakeEngine: SeedFinderEngine, @unchecked Sendable {
    private let lock = NSLock()
    var startSessions: [FakeSearchSession] = []
    var resumedSessions: [FakeSearchSession] = []
    var filterResult: [String] = []
    var filterError: Error?
    var filterDelay: Duration?
    private(set) var filteredSeeds: [[String]] = []
    private(set) var resumedCalls: [(resumeFrom: Int64, scanLen: Int64)] = []
    private(set) var freshCalls = 0
    private(set) var filterSources: [SearchRequest] = []
    /// The worker count each native start was handed, in call order: the
    /// controller must pass the device-local preference through untouched to
    /// both the fresh scan and a refine's resumed remainder.
    private(set) var startWorkers: [Int] = []
    private(set) var resumedWorkers: [Int] = []

    /// An unscripted call must not trap — tests assert on the call counters to
    /// tell a fresh scan from a refine, so an unexpected one has to survive
    /// long enough to be reported.
    private func nextSession(_ queue: inout [FakeSearchSession]) -> FakeSearchSession {
        queue.isEmpty ? FakeSearchSession(batches: [], hint: ResumeHint(position: 0, remaining: 0))
                      : queue.removeFirst()
    }
    func startSearch(_ request: SearchRequest, workers: Int) async throws -> any SeedFinderSearchSession {
        lock.withLock {
            freshCalls += 1
            startWorkers.append(workers)
            return nextSession(&startSessions)
        }
    }
    func startResumedSearch(_ request: SearchRequest, resumeFrom: Int64, scanLen: Int64,
                            workers: Int) async throws -> any SeedFinderSearchSession {
        lock.withLock {
            resumedCalls.append((resumeFrom, scanLen))
            resumedWorkers.append(workers)
            return nextSession(&resumedSessions)
        }
    }
    func filterSeeds(_ request: SearchRequest, seeds: [String]) async throws -> [String] {
        if let filterDelay { try await Task.sleep(for: filterDelay) }
        if let filterError { throw filterError }
        return lock.withLock {
            filteredSeeds.append(seeds)
            return filterResult
        }
    }
    func filterRecipes(_ request: SearchRequest, base: SearchRequest, recipes: [SeedResult]) async throws -> [SeedResult] {
        lock.withLock { filterSources.append(base) }
        let matches = try await filterSeeds(request, seeds: recipes.map(\.seed))
        return recipes.filter { matches.contains($0.seed) }
    }
    func scoutSeed(_ seed: String, challenges: Int) async throws -> ScoutWorld {
        throw SeedFinderEngineError.invalidArgument
    }
}

@MainActor
final class RefineSearchTests: XCTestCase {
    private func wandRequest(count: Int) throws -> SearchRequest {
        try SearchRequest(requirements: (1...count).map { key in
            try ItemRequirement(key: Int64(key), item: nil,
                                upgrade: key == 1 ? 3 : 0, kind: .wand,
                                upgradeMatch: key == 1 ? .exactly : .any)
        })
    }
    private func wand(key: Int64, upgrade: Int) throws -> ItemRequirement {
        try ItemRequirement(key: key, item: nil, upgrade: upgrade, kind: .wand,
                            upgradeMatch: upgrade == 0 ? .any : .exactly)
    }
    private func result(_ seed: String, matched: Int = 1) -> SeedResult {
        SeedResult(seed: seed, matchedRequirements: matched)
    }
    private func ringRequest(count: Int = 1) throws -> SearchRequest {
        try SearchRequest(requirements: (1...count).map { key in
            try ItemRequirement(key: Int64(100 + key), item: nil, upgrade: 0, kind: .ring,
                                upgradeMatch: .any)
        })
    }
    private func waitUntilIdle(_ controller: SearchController) async throws {
        let deadline = ContinuousClock.now + .seconds(5)
        while controller.isRunning && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertFalse(controller.isRunning)
    }

    func testEveryQueryChecksTheWholePoolAndDiscoveriesKeepTheirSources() async throws {
        let engine = FakeEngine()
        let controller = SearchController(engine: engine)
        let a = try wandRequest(count: 1)
        let b = try ringRequest()
        engine.startSessions = [FakeSearchSession(batches: [[result("AAA-AAA-AAA"), result("AAA-AAA-AAB")]], hint: ResumeHint(position: 500, remaining: 100))]
        controller.start(a)
        try await waitUntilIdle(controller)
        engine.filterResult = ["AAA-AAA-AAB"]
        engine.startSessions = [FakeSearchSession(batches: [[result("AAA-AAA-AAC")]], hint: ResumeHint(position: 800, remaining: 100))]
        controller.start(b)
        try await waitUntilIdle(controller)
        XCTAssertEqual(controller.results.map(\.seed), ["AAA-AAA-AAB", "AAA-AAA-AAC"])
        XCTAssertEqual(controller.target?.seeds, ["AAA-AAA-AAA", "AAA-AAA-AAB", "AAA-AAA-AAC"])
        XCTAssertEqual(engine.freshCalls, 2)
        engine.filterResult = ["AAA-AAA-AAA", "AAA-AAA-AAB", "AAA-AAA-AAC"]
        controller.start(a)
        try await waitUntilIdle(controller)
        XCTAssertEqual(Array(engine.filteredSeeds.suffix(2)), [["AAA-AAA-AAA", "AAA-AAA-AAB"], ["AAA-AAA-AAC"]])
        XCTAssertEqual(try QueryDocument.encode(engine.filterSources.last!), try QueryDocument.encode(b))
        XCTAssertEqual(controller.results.count, 3)
        controller.clearResults()
        XCTAssertNil(controller.target)
    }

    func testOnlyAnUnchangedQueryResumesItsScan() async throws {
        let engine = FakeEngine()
        let controller = SearchController(engine: engine)
        let query = try wandRequest(count: 1)
        engine.startSessions = [FakeSearchSession(batches: [[result("AAA-AAA-AAA")]], hint: ResumeHint(position: 500, remaining: 100))]
        controller.start(query, workers: 2)
        try await waitUntilIdle(controller)
        engine.filterResult = ["AAA-AAA-AAA"]
        controller.start(query, workers: 3)
        try await waitUntilIdle(controller)
        XCTAssertEqual(engine.resumedCalls.first?.resumeFrom, 500)
        XCTAssertEqual(engine.resumedWorkers, [3])
        controller.start(try wandRequest(count: 2))
        try await waitUntilIdle(controller)
        XCTAssertEqual(engine.freshCalls, 2)
        XCTAssertEqual(engine.filteredSeeds, [["AAA-AAA-AAA"], ["AAA-AAA-AAA"]])
    }

    func testImportAddsToThePoolAndFailedFilteringDoesNotDiscardIt() async throws {
        let engine = FakeEngine()
        let controller = SearchController(engine: engine)
        let query = try wandRequest(count: 1)
        engine.startSessions = [FakeSearchSession(batches: [[result("AAA-AAA-AAA")]], hint: ResumeHint(position: 500, remaining: 100))]
        controller.start(query)
        try await waitUntilIdle(controller)
        let saved = SavedQuery(requirements: query.requirements, maximumDepth: query.maximumDepth)
        controller.loadImported(seeds: ["AAA-AAA-AAB"], query: saved)
        XCTAssertEqual(controller.target?.seeds, ["AAA-AAA-AAA", "AAA-AAA-AAB"])
        engine.filterError = SeedFinderEngineError.invalidArgument
        controller.start(try ringRequest())
        try await waitUntilIdle(controller)
        XCTAssertEqual(controller.state, .failed)
        XCTAssertEqual(controller.target?.seeds, ["AAA-AAA-AAA", "AAA-AAA-AAB"])
        XCTAssertEqual(controller.results.map(\.seed), ["AAA-AAA-AAB"])
    }

    func testCancelledAndFailedScansKeepDeliveredSeeds() async throws {
        for state in [SearchState.cancelled, .failed] {
            let engine = FakeEngine()
            let controller = SearchController(engine: engine)
            engine.startSessions = [FakeSearchSession(batches: [[result("AAA-AAA-AAA")]], finalState: state, hint: ResumeHint(position: 500, remaining: 100))]
            controller.start(try wandRequest(count: 1))
            try await waitUntilIdle(controller)
            XCTAssertEqual(controller.target?.seeds, ["AAA-AAA-AAA"])
        }
    }

    func testDisplayCapsAtResultCapWhileTheFullSetStaysTheRefineBase() async throws {
        let engine = FakeEngine()
        let controller = SearchController(engine: engine)
        let base = try wandRequest(count: 1)
        let seeds = (0..<1_500).map { String(format: "SEED-%04d", $0) }

        engine.startSessions = [FakeSearchSession(
            batches: [seeds.map { result($0) }],
            hint: ResumeHint(position: 500, remaining: 100))]
        controller.start(base)
        try await waitUntilIdle(controller)
        XCTAssertEqual(controller.results.count, SearchController.resultCap)
        XCTAssertTrue(controller.reachedResultCap)
        XCTAssertEqual(controller.target?.seeds.count, 1_500,
                       "the Target Set keeps every delivered seed, not just the displayed rows")

        // An unchanged query refines the full 1,500-seed set — and still
        // resumes the scan for more, display cap notwithstanding.
        engine.filterResult = seeds
        engine.resumedSessions = [FakeSearchSession(
            batches: [], hint: ResumeHint(position: 600, remaining: 0))]
        controller.start(base)
        try await waitUntilIdle(controller)
        XCTAssertEqual(engine.filteredSeeds.last?.count, 1_500)
        XCTAssertEqual(controller.refinedKept, 1_500)
        XCTAssertEqual(controller.refinedOf, 1_500)
        XCTAssertEqual(controller.results.count, SearchController.resultCap)
        XCTAssertEqual(engine.resumedCalls.count, 1,
                       "a continuation with coverage left scans for more even at the display cap")
    }

    func testClearResultsIsIgnoredWhileASearchIsRunning() async throws {
        let engine = FakeEngine()
        let controller = SearchController(engine: engine)

        engine.startSessions = [FakeSearchSession(
            batches: [[result("AAA-AAA-AAA")]], hint: ResumeHint(position: 500, remaining: 100))]
        controller.start(try wandRequest(count: 1))
        try await waitUntilIdle(controller)

        // A refine's filter phase counts as running just like a scan does.
        engine.filterDelay = .seconds(60)
        controller.start(try wandRequest(count: 2))
        XCTAssertTrue(controller.isRunning)
        XCTAssertFalse(controller.canClearResults)
        controller.clearResults()
        XCTAssertEqual(controller.results.map(\.seed), ["AAA-AAA-AAA"],
                       "clearing mid-run must not touch the results")
        XCTAssertEqual(controller.baseRun?.resumeFrom, 500)
        XCTAssertTrue(controller.isRunning)

        controller.cancel()
        try await waitUntilIdle(controller)
        XCTAssertEqual(controller.results.map(\.seed), ["AAA-AAA-AAA"])
        XCTAssertEqual(controller.baseRun?.resumeFrom, 500)
    }

    func testDefaultedWorkerCountAsksTheEngineForEveryCore() async throws {
        let engine = FakeEngine()
        let controller = SearchController(engine: engine)
        engine.startSessions = [FakeSearchSession(
            batches: [[result("AAA-AAA-AAA")]], hint: ResumeHint(position: 0, remaining: 0))]
        controller.start(try wandRequest(count: 1))
        try await waitUntilIdle(controller)
        XCTAssertEqual(engine.startWorkers, [WorkerPersistence.unset])
    }

    func testClearResultsAlsoDiscardsImportedResults() throws {
        let controller = SearchController(engine: FakeEngine())
        let requirement = try ItemRequirement(key: 1, item: nil, upgrade: 3, kind: .wand)
        controller.loadImported(seeds: ["AAA-AAA-AAA"],
                                query: SavedQuery(requirements: [requirement]))
        XCTAssertTrue(controller.canClearResults)
        controller.clearResults()
        XCTAssertTrue(controller.results.isEmpty)
        XCTAssertFalse(controller.isImported)
        XCTAssertEqual(controller.importedDropped, 0)
        XCTAssertNil(controller.exportQuery)
        XCTAssertFalse(controller.canClearResults)
    }
}
