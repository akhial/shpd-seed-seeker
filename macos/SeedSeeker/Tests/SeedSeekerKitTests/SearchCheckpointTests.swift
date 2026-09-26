import Foundation
import XCTest
@testable import SeedSeekerKit

private actor CheckpointSession: SeedFinderSearchSession {
    private var initial: [SeedResult]
    private var buffered: [[SeedResult]]
    private var stopped: Bool
    private let hint: ResumeHint
    private let scanned: Int64

    init(initial: [SeedResult] = [], buffered: [[SeedResult]] = [], stopped: Bool = false,
         position: Int64 = 300, remaining: Int64 = 100, scanned: Int64 = 3) {
        self.initial = initial; self.buffered = buffered; self.stopped = stopped
        self.hint = ResumeHint(position: position, remaining: remaining); self.scanned = scanned
    }
    func poll(_ maximum: Int) async throws -> [SeedResult] {
        if !initial.isEmpty { defer { initial = [] }; return initial }
        if stopped, !buffered.isEmpty { return buffered.removeFirst() }
        return []
    }
    func status() async throws -> SearchStatus {
        SearchStatus(state: stopped ? .cancelled : .running, scannedSeeds: scanned,
                     totalSeeds: 103, errorCode: 0, matchProbability: 0.01)
    }
    func resumeHint() async throws -> ResumeHint {
        guard stopped, buffered.isEmpty else { throw SeedFinderEngineError.invalidResponse }
        return hint
    }
    func cancel() async { stopped = true }
    func close() async { stopped = true }
}

private actor CheckpointEngine: SeedFinderEngine {
    var fresh: CheckpointSession
    var resumed: CheckpointSession
    private(set) var freshCount = 0
    private(set) var resumedPositions: [Int64] = []
    private(set) var resumedWorkers: [Int] = []
    private let startDelay: Duration?
    private let filterDelay: Duration?

    init(fresh: CheckpointSession, resumed: CheckpointSession = CheckpointSession(stopped: true, remaining: 0, scanned: 0),
         startDelay: Duration? = nil, filterDelay: Duration? = nil) {
        self.fresh = fresh; self.resumed = resumed
        self.startDelay = startDelay
        self.filterDelay = filterDelay
    }
    func startSearch(_ request: SearchRequest, workers: Int) async throws -> any SeedFinderSearchSession {
        freshCount += 1
        if let startDelay { await Task.detached { try? await Task.sleep(for: startDelay) }.value }
        return fresh
    }
    func startResumedSearch(_ request: SearchRequest, resumeFrom: Int64, scanLen: Int64,
                            workers: Int) async throws -> any SeedFinderSearchSession {
        resumedPositions.append(resumeFrom); resumedWorkers.append(workers); return resumed
    }
    func filterSeeds(_ request: SearchRequest, seeds: [String]) async throws -> [String] { seeds }
    func filterRecipes(_ request: SearchRequest, base: SearchRequest, recipes: [SeedResult]) async throws -> [SeedResult] {
        if let filterDelay { await Task.detached { try? await Task.sleep(for: filterDelay) }.value }
        return recipes
    }
    func scoutSeed(_ seed: String, challenges: Int) async throws -> ScoutWorld { throw SeedFinderEngineError.invalidArgument }
}

@MainActor
final class SearchCheckpointTests: XCTestCase {
    private func temporaryURL() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).appendingPathComponent("search.json")
    }
    private func query() throws -> SavedQuery {
        SavedQuery(requirements: [try ItemRequirement(key: 1, item: nil, upgrade: 2, kind: .wand)])
    }
    private func results() -> [SeedResult] {
        ["AAA-AAA-AAA", "AAA-AAA-AAB", "AAA-AAA-AAC"].map {
            SeedResult(seed: $0, matchedRequirements: 1, selectedTrinket: "mossy_clump")
        }
    }
    private func waitForScan(_ controller: SearchController) async throws {
        let deadline = ContinuousClock.now + .seconds(5)
        while controller.scannedSeeds == 0 && controller.isRunning && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertGreaterThan(controller.scannedSeeds, 0)
    }

    func testImportsPersistRecipesAndClearDisplayRetainsPool() throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let controller = SearchController(checkpointURL: url)
        controller.loadImported(seeds: results().map(\.seed), query: try query(),
                                trinkets: results().map(\.selectedTrinket))
        let restored = SearchController(checkpointURL: url)
        XCTAssertEqual(restored.results, results())
        XCTAssertEqual(restored.foundCount, 3)
        XCTAssertEqual(restored.target?.seeds, results().map(\.seed))
        XCTAssertTrue(restored.isImported)
        restored.clearDisplayedResults()
        let cleared = SearchController(checkpointURL: url)
        XCTAssertTrue(cleared.results.isEmpty)
        XCTAssertNil(cleared.exportQuery)
        XCTAssertEqual(cleared.target?.seeds.count, 3)
        cleared.clearResults()
        XCTAssertNil(SearchController(checkpointURL: url).target)
    }

    func testInterruptionDrainsAllBatchesBeforeSavingAndResumesSafeCursor() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let entries = results()
        let session = CheckpointSession(initial: [entries[0]], buffered: [[entries[1]], [entries[2]]])
        let engine = CheckpointEngine(fresh: session)
        let controller = SearchController(engine: engine, checkpointURL: url)
        controller.start(try query().searchRequest(), workers: 2)
        try await waitForScan(controller)
        controller.interrupt()
        await controller.waitUntilSettled()
        XCTAssertEqual(controller.results, entries)
        XCTAssertEqual(controller.baseRun?.resumeFrom, 300)
        XCTAssertTrue(controller.hasPendingSearch)
        let restored = SearchController(engine: engine, checkpointURL: url)
        XCTAssertEqual(restored.foundCount, 3)
        XCTAssertTrue(restored.hasPendingSearch)
        restored.resumeInterrupted()
        await restored.waitUntilSettled()
        let positions = await engine.resumedPositions
        let workers = await engine.resumedWorkers
        let freshCount = await engine.freshCount
        XCTAssertEqual(positions, [300])
        XCTAssertEqual(workers, [2])
        XCTAssertEqual(freshCount, 1)
        XCTAssertEqual(restored.target?.seeds, entries.map(\.seed))
        XCTAssertFalse(restored.hasPendingSearch)
    }

    func testStopPersistsIntentBeforeNativeDrain() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let engine = CheckpointEngine(fresh: CheckpointSession(initial: [results()[0]]))
        let controller = SearchController(engine: engine, checkpointURL: url)
        controller.start(try query().searchRequest())
        try await waitForScan(controller)
        controller.cancel()
        XCTAssertFalse(SearchController(engine: engine, checkpointURL: url).hasPendingSearch)
        await controller.waitUntilSettled()
        let restored = SearchController(engine: engine, checkpointURL: url)
        XCTAssertFalse(restored.hasPendingSearch)
        XCTAssertEqual(restored.state, .cancelled)
        XCTAssertEqual(restored.baseRun?.resumeFrom, 300)
    }

    func testPeriodicCheckpointContinuesFromSettledCursor() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let engine = CheckpointEngine(fresh: CheckpointSession(initial: [results()[0]]))
        let controller = SearchController(engine: engine, checkpointURL: url, checkpointInterval: .zero)
        controller.start(try query().searchRequest(), workers: 3)
        await controller.waitUntilSettled()
        let positions = await engine.resumedPositions
        XCTAssertEqual(positions, [300])
        XCTAssertEqual(controller.scannedSeeds, 3)
        XCTAssertEqual(controller.totalSeeds, 103)
        XCTAssertEqual(controller.results.count, 1)
        XCTAssertFalse(controller.hasPendingSearch)
    }

    func testEngineChangeRetainsFindsAndDiscardsTraversal() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let engine = CheckpointEngine(fresh: CheckpointSession(initial: [results()[0]]))
        let controller = SearchController(engine: engine, checkpointURL: url)
        controller.start(try query().searchRequest())
        try await waitForScan(controller)
        controller.interrupt()
        await controller.waitUntilSettled()
        var checkpoint = try XCTUnwrap(SearchCheckpoint.load(from: url))
        checkpoint.engine = "previous engine"
        try checkpoint.save(to: url)
        let restored = SearchController(engine: engine, checkpointURL: url)
        XCTAssertEqual(restored.results.count, 1)
        XCTAssertEqual(restored.target?.seeds.count, 1)
        XCTAssertNil(restored.baseRun)
        XCTAssertFalse(restored.hasPendingSearch)
        XCTAssertEqual(restored.message, "The engine changed. Saved results were restored; start a search to recheck them.")
    }

    func testCancellationDuringNativeSetupClosesWithoutLeavingRunningState() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let engine = CheckpointEngine(fresh: CheckpointSession(), startDelay: .milliseconds(80))
        let controller = SearchController(engine: engine, checkpointURL: url)
        controller.start(try query().searchRequest())
        let deadline = ContinuousClock.now + .seconds(5)
        while await engine.freshCount == 0 && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        controller.cancel()
        await controller.waitUntilSettled()
        XCTAssertFalse(controller.isRunning)
        XCTAssertFalse(controller.isPreparing)
        XCTAssertEqual(controller.state, .cancelled)
        XCTAssertFalse(SearchController(engine: engine, checkpointURL: url).hasPendingSearch)
    }

    func testReturningDuringInterruptionWaitsForDrainThenResumes() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let engine = CheckpointEngine(fresh: CheckpointSession(initial: [results()[0]]))
        let controller = SearchController(engine: engine, checkpointURL: url)
        controller.start(try query().searchRequest())
        try await waitForScan(controller)
        controller.interrupt()
        controller.resumeInterrupted()
        let deadline = ContinuousClock.now + .seconds(5)
        while await engine.resumedPositions.isEmpty && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        await controller.waitUntilSettled()
        let positions = await engine.resumedPositions
        XCTAssertEqual(positions, [300])
        XCTAssertFalse(controller.isRunning)
        XCTAssertFalse(controller.hasPendingSearch)
    }

    func testRepeatedInterruptionDuringRefinementPreservesOriginalResultGoal() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let engine = CheckpointEngine(fresh: CheckpointSession(initial: [results()[0]]), filterDelay: .milliseconds(80))
        let controller = SearchController(engine: engine, checkpointURL: url)
        controller.start(try query().searchRequest())
        try await waitForScan(controller)
        controller.interrupt()
        await controller.waitUntilSettled()
        let restored = SearchController(engine: engine, checkpointURL: url)
        restored.resumeInterrupted()
        let deadline = ContinuousClock.now + .seconds(5)
        while restored.refineProgress == nil && restored.isRunning && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertNotNil(restored.refineProgress)
        restored.interrupt()
        await restored.waitUntilSettled()
        let saved = try XCTUnwrap(SearchCheckpoint.load(from: url))
        XCTAssertEqual(saved.pending?.resultGoal, SearchController.resultCap)
        XCTAssertEqual(saved.baseRun?.resumeFrom, 300)
        XCTAssertEqual(saved.target?.seeds, [results()[0].seed])
    }

    func testPendingQueryRestoresRefinementBoardWithoutReplacingPreviousResultQuery() async throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let engine = CheckpointEngine(fresh: CheckpointSession())
        let controller = SearchController(engine: engine, checkpointURL: url)
        let previous = try query()
        controller.loadImported(seeds: [results()[0].seed], query: previous)
        var pending = previous
        pending.maximumDepth = 19
        pending.requireBlacksmith = true
        pending.excludeBlacksmithRewards = true
        pending.wandmakerQuest = .rotberry
        pending.challenges = 1
        pending.autoApplyTrinket = false
        pending.arcaneResin = 6
        pending.arcaneResinAuto = true
        pending.arcaneResinFilter = .init(uncursed: false, maximumDepth: 7, includeMageWand: true)
        pending.floorRequirements = [.init(depth: 2, feeling: "grass")]
        controller.start(try pending.searchRequest())
        // Restore the durable pre-filter snapshot, where the old matches and
        // their export query correctly coexist with the new pending board.
        let restored = SearchController(engine: engine, checkpointURL: url)
        XCTAssertEqual(restored.pendingQuery, pending)
        XCTAssertEqual(restored.exportQuery, previous)
        XCTAssertNotEqual(restored.pendingQuery, restored.exportQuery)
        controller.cancel()
        await controller.waitUntilSettled()
        XCTAssertNil(SearchController(engine: engine, checkpointURL: url).pendingQuery)
    }
}
