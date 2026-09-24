// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.DemoNativeSeedFinder
import dev.seedseeker.app.engine.NativeSearchSession
import dev.seedseeker.app.engine.NativeSeedFinder
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.ResumeHint
import dev.seedseeker.app.model.SearchBatch
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.model.toPresetQuery
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.*
import org.junit.Test
import kotlin.coroutines.CoroutineContext

@OptIn(ExperimentalCoroutinesApi::class)
class SearchControllerTest {
    init { PackagedCatalog.install() }
    private val request = SearchRequest(listOf(ItemRequirement(1, ItemCatalog.wands.first(), 1)))
    private val a = SeedResult("AAA-AAA-AAA", 1, "mossy_clump")
    private val b = SeedResult("BBB-BBB-BBB", 1)
    private val c = SeedResult("CCC-CCC-CCC", 1)

    @Test fun queryPreparationYieldsToTheUiAndRejectsDuplicateStarts() = runTest {
        val worker = QueuedWorker()
        val fixture = fixture(workerDispatcher = worker)
        var checks = 0
        fixture.engine.onPrepare = { assertTrue(worker.executing); checks++ }
        fixture.controller.start(request, 2)
        fixture.controller.start(request, 3)
        runCurrent()
        assertTrue(fixture.controller.isSearching)
        assertTrue(fixture.controller.isPreparing)
        assertEquals(0, checks)
        assertEquals(0, fixture.serviceStarts)
        worker.runNext()
        runCurrent()
        assertEquals(1, checks)
        assertEquals(1, fixture.serviceStarts)
        assertEquals(2, fixture.controller.snapshot.pending?.workers)
        assertFalse(fixture.controller.isPreparing)
        fixture.scope.cancel()
    }

    @Test fun cancellingDuringPreparationCannotStartALateSearch() = runTest {
        val worker = QueuedWorker()
        val fixture = fixture(workerDispatcher = worker)
        val before = fixture.controller.snapshot
        fixture.controller.start(request, 2)
        runCurrent()
        fixture.controller.stop()
        runCurrent()
        worker.runNext()
        runCurrent()
        assertEquals(before, fixture.controller.snapshot)
        assertNull(fixture.store.saved.pending)
        assertEquals(0, fixture.serviceStarts)
        assertFalse(fixture.controller.isSearching)
        assertFalse(fixture.controller.isPreparing)
        assertTrue(fixture.engine.sessions.isEmpty())
        fixture.scope.cancel()
    }

    @Test fun queryPreparationFailureIsReportedWithoutLosingSavedResults() = runTest {
        val pool = TargetState(request, listOf(a))
        val fixture = fixture(MemoryStore(SearchSnapshot(results = pool.results, target = pool)))
        fixture.engine.onPrepare = { error("query preparation failed") }
        fixture.controller.start(request, 2)
        runCurrent()
        assertEquals(listOf(a), fixture.controller.snapshot.results)
        assertEquals(pool, fixture.controller.snapshot.target)
        assertEquals("query preparation failed", fixture.controller.snapshot.error)
        assertFalse(fixture.controller.isSearching)
        assertFalse(fixture.controller.isPreparing)
        assertEquals(0, fixture.serviceStarts)
        fixture.scope.cancel()
    }

    @Test fun impossibleQueriesKeepSavedResultsAndDoNotStartWorkers() = runTest {
        val fixture = fixture()
        fixture.engine.impossibleReason = "Requires 5 initial trinket offers, but each seed offers only 4."
        val before = fixture.controller.snapshot
        fixture.controller.start(request, 2)
        runCurrent()
        assertEquals(before, fixture.controller.snapshot)
        assertEquals(0, fixture.serviceStarts)
        assertTrue(fixture.engine.sessions.isEmpty())
        assertEquals("Impossible query. ${fixture.engine.impossibleReason}", fixture.controller.notice)
        fixture.scope.cancel()
    }

    @Test fun checkpointDrainsBeforeSavingAndReattachingNeverDuplicatesTheRunner() = runTest {
        val fixture = fixture()
        fixture.controller.start(request, 2)
        fixture.controller.runPending()
        runCurrent()
        repeat(3) { fixture.controller.runPending(); fixture.controller.onVisible() }
        runCurrent()
        assertEquals(1, fixture.engine.sessions.size)
        assertEquals(1, fixture.serviceStarts)
        advanceTimeBy(15_400)
        runCurrent()
        val saved = fixture.store.saved
        assertEquals(ResumeHint(100, 900), saved.pending?.window)
        assertEquals(listOf(a, b, c), saved.results)
        assertEquals(2, saved.pending?.workers)
        assertTrue(fixture.engine.sessions.first().closed)
        assertEquals(2, fixture.engine.sessions.size)
        assertTrue(fixture.controller.isSearching)
        assertEquals(SearchState.RUNNING, fixture.controller.snapshot.status?.state)
        fixture.scope.cancel()
        runCurrent()
    }

    @Test fun processDeathReplaysOnlyTheSavedWindowAndDeduplicatesOverlappingResults() = runTest {
        val original = fixture()
        original.controller.start(request, 3)
        original.controller.runPending()
        advanceTimeBy(15_400)
        runCurrent()
        original.scope.cancel() // No Activity callback or graceful service shutdown.
        runCurrent()
        val restored = fixture(original.store)
        restored.controller.onVisible()
        runCurrent()
        assertEquals(1, restored.serviceStarts)
        restored.controller.runPending()
        runCurrent()
        assertEquals(listOf(ResumeHint(100, 900)), restored.engine.windows)
        assertEquals(listOf(a, b, c), restored.controller.snapshot.results)
        assertEquals(request, restored.controller.snapshot.pending?.request)
        restored.controller.stop()
        advanceUntilIdle()
        assertEquals(listOf(a, b, c), restored.controller.snapshot.results)
        assertEquals(listOf(a, b, c), restored.controller.snapshot.target?.results)
        assertNull(restored.store.saved.pending)
        assertEquals(SearchState.CANCELLED, restored.controller.snapshot.status?.state)
        restored.scope.cancel()
    }

    @Test fun stopIsPersistedBeforeNativeDrainAndNeverAutoResumes() = runTest {
        val fixture = fixture()
        fixture.controller.start(request, 2)
        fixture.controller.runPending()
        runCurrent()
        fixture.controller.stop()
        runCurrent()
        assertNull(fixture.store.saved.pending)
        assertTrue(fixture.controller.isSearching) // Native drain is still pending.
        advanceUntilIdle()
        assertFalse(fixture.controller.isSearching)
        assertTrue(fixture.engine.sessions.single().closed)
        val restored = fixture(fixture.store)
        restored.controller.onVisible()
        runCurrent()
        assertEquals(0, restored.serviceStarts)
        fixture.scope.cancel()
        restored.scope.cancel()
    }

    @Test fun serviceInterruptionSavesAnExactWindowAndResumesWhenVisible() = runTest {
        val fixture = fixture()
        fixture.controller.start(request, 2)
        fixture.controller.runPending()
        runCurrent()
        fixture.controller.interrupt()
        advanceUntilIdle()
        assertFalse(fixture.controller.isSearching)
        assertEquals(ResumeHint(100, 900), fixture.store.saved.pending?.window)
        assertEquals(listOf(a, b, c), fixture.store.saved.results)
        fixture.controller.onVisible()
        runCurrent()
        fixture.controller.runPending()
        runCurrent()
        assertEquals(2, fixture.engine.sessions.size)
        fixture.scope.cancel()
        runCurrent()
    }

    @Test fun interruptedFilterKeepsOldQueryAndResultsAndRetriesAllSeeds() = runTest {
        val base = request.copy(maximumDepth = 12)
        val seeds = List(1_100) { SeedResult("seed-$it", 1, "mossy_clump") }
        val target = TargetState(base, seeds)
        val store = MemoryStore(SearchSnapshot(results = seeds, query = base.toPresetQuery(), target = target,
            pending = PendingSearch(request, 2, RefineSpec(50, 0, seeds, base))))
        val fixture = fixture(store)
        fixture.engine.onFilter = { fixture.controller.interrupt() }
        fixture.controller.runPending()
        advanceUntilIdle()
        assertEquals(base.toPresetQuery(), store.saved.query)
        assertEquals(seeds, store.saved.results)
        assertNotNull(store.saved.pending?.refine)
        fixture.engine.onFilter = {}
        fixture.controller.runPending()
        advanceUntilIdle()
        assertEquals(seeds, fixture.controller.snapshot.results)
        assertEquals(target.results, fixture.controller.snapshot.target?.results)
        assertEquals(request.toPresetQuery(), fixture.controller.snapshot.query)
        assertNull(store.saved.pending)
        assertEquals(0, fixture.engine.sessions.size)
        fixture.scope.cancel()
    }

    @Test fun checkpointWriteFailureStopsTheWorkersAndReportsTheFailure() = runTest {
        val fixture = fixture()
        fixture.controller.start(request, 2)
        fixture.controller.runPending()
        runCurrent()
        fixture.store.failWrites = true
        advanceUntilIdle()
        assertFalse(fixture.controller.isSearching)
        assertTrue(fixture.engine.sessions.single().closed)
        assertTrue(fixture.controller.snapshot.error!!.contains("disk full"))
        fixture.scope.cancel()
    }


    @Test fun searchingImportedResultsFiltersThenStartsAFreshScan() = runTest {
        val target = TargetState(request, listOf(a, b))
        val fixture = fixture(MemoryStore(SearchSnapshot(results = target.results, query = request.toPresetQuery(), target = target)))
        fixture.engine.filter = { it.filter { result -> result == a } }
        fixture.engine.completedBatches += listOf(a, c) to ResumeHint(1_000, 0)
        fixture.controller.start(request, 2)
        fixture.controller.runPending()
        advanceUntilIdle()
        assertEquals(listOf(a, b), fixture.engine.filtered)
        assertEquals(listOf(a, c), fixture.controller.snapshot.results)
        assertEquals(1, fixture.engine.sessions.size)
        assertTrue(fixture.engine.windows.isEmpty())
        assertEquals(listOf(request), fixture.engine.filterSources)
        assertEquals(listOf(a, b, c), fixture.controller.snapshot.target?.results)
        fixture.scope.cancel()
    }

    @Test fun fullImportedPoolsFinishVerificationAndReportEveryCheckedSeed() = runTest {
        for (count in listOf(11, 1023, 1024, 1025)) {
            val seeds = List(count) { SeedResult("loaded-$it", 1) }
            val pool = TargetState(request, seeds)
            val fixture = fixture(MemoryStore(SearchSnapshot(results = seeds,
                query = request.toPresetQuery(), target = pool)))
            val progress = mutableListOf<RefineProgress?>()
            fixture.engine.onFilter = { progress += fixture.controller.refineProgress }
            fixture.engine.filter = { emptyList() }
            fixture.engine.completedBatches += listOf(c) to ResumeHint(1000, 0)
            fixture.controller.start(request, 2)
            fixture.controller.runPending()
            advanceUntilIdle()
            assertEquals((0 until count step 24).map { RefineProgress(it, count) }, progress)
            assertEquals(seeds, fixture.engine.filtered)
            assertEquals(listOf(c), fixture.controller.snapshot.results)
            assertEquals(seeds + c, fixture.controller.snapshot.target!!.results)
            assertEquals(1, fixture.engine.sessions.size)
            assertFalse(fixture.controller.isSearching)
            assertNull(fixture.controller.refineProgress)
            fixture.scope.cancel()
        }
    }

    @Test fun refinementResumesAndFillsTheLimitWithUniqueMatchesAcrossNativeSessions() = runTest {
        val seeds = List(RESULT_CAP - 2) { SeedResult("loaded-$it", 1) }
        val target = TargetState(request, seeds)
        val fixture = fixture(MemoryStore(SearchSnapshot(results = seeds, query = request.toPresetQuery(), target = target,
            lastRun = FinishedRun(request, 50, 950, seeds))))
        // A native accept cap can include seeds already kept by the filter. It must not
        // terminate the whole search while the list still has room for unique matches.
        fixture.engine.completedBatches += listOf(seeds.first(), b) to ResumeHint(100, 900)
        fixture.engine.completedBatches += listOf(b, c) to ResumeHint(200, 800)
        fixture.controller.start(request, 2)
        fixture.controller.runPending()
        advanceUntilIdle()
        assertEquals(listOf(ResumeHint(50, 950), ResumeHint(100, 900)), fixture.engine.windows)
        assertEquals(seeds + listOf(b, c), fixture.controller.snapshot.results)
        assertEquals(RESULT_CAP, fixture.controller.snapshot.results.size)
        assertEquals(800L, fixture.controller.snapshot.lastRun?.remaining)
        assertTrue(fixture.engine.sessions.all { it.closed })
        fixture.scope.cancel()
    }



    @Test fun failedForegroundStartPreservesPendingSearchForVisibleRetry() = runTest {
        val fixture = fixture()
        fixture.failService = true
        fixture.controller.start(request, 2)
        runCurrent()
        assertFalse(fixture.controller.isSearching)
        assertNotNull(fixture.store.saved.pending)
        assertEquals(0, fixture.engine.sessions.size)
        fixture.failService = false
        fixture.controller.onVisible()
        runCurrent()
        assertEquals(2, fixture.serviceStarts)
        fixture.scope.cancel()
    }

    @Test fun clearRemovesResultsCoverageAndPendingRecovery() = runTest {
        val fixture = fixture()
        fixture.controller.start(request, 2)
        fixture.controller.runPending()
        runCurrent()
        fixture.controller.stop()
        advanceUntilIdle()
        fixture.controller.clear()
        runCurrent()
        assertEquals(SearchSnapshot(), fixture.store.saved)
        fixture.scope.cancel()
    }

    @Test fun differentQueriesAndImportsKeepTheWholePoolAndOriginalSources() = runTest {
        val other = SearchRequest(listOf(ItemRequirement(2, ItemCatalog.rings.first(), 1)))
        val fixture = fixture()
        fixture.controller.importResults(request.toPresetQuery(), listOf(a, b), TargetState(request, listOf(a, b)))
        fixture.engine.filter = { it.filter { seed -> seed == b } }
        fixture.engine.completedBatches += listOf(c) to ResumeHint(1000, 0)
        fixture.controller.start(other, 1)
        fixture.controller.runPending()
        advanceUntilIdle()
        assertEquals(listOf(b, c), fixture.controller.snapshot.results)
        assertEquals(listOf(a, b, c), fixture.controller.snapshot.target!!.results)
        assertEquals(request, fixture.controller.snapshot.target!!.sources[a.seed])
        assertEquals(other, fixture.controller.snapshot.target!!.sources[c.seed])

        fixture.engine.filtered.clear()
        fixture.engine.filterSources.clear()
        fixture.engine.filter = { it }
        fixture.engine.completedBatches += emptyList<SeedResult>() to ResumeHint(1000, 0)
        fixture.controller.start(request, 1)
        fixture.controller.runPending()
        advanceUntilIdle()
        assertEquals(listOf(a, b, c), fixture.engine.filtered)
        assertEquals(listOf(request, other), fixture.engine.filterSources)
        assertEquals(listOf(a, b, c), fixture.controller.snapshot.results)
        fixture.controller.importResults(other.toPresetQuery(), listOf(c), TargetState(other, listOf(c)))
        assertEquals(listOf(a, b, c), fixture.controller.snapshot.target!!.results)
        fixture.controller.clear()
        assertNull(fixture.controller.snapshot.target)
        fixture.scope.cancel()
    }

    private fun TestScope.fixture(store: MemoryStore = MemoryStore(), workerDispatcher: CoroutineDispatcher? = null): Fixture {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val scope = CoroutineScope(SupervisorJob() + dispatcher)
        val fixture = Fixture(store, scope, FakeEngine())
        fixture.controller = SearchController(fixture.engine, store, scope,
            startService = {
                fixture.serviceStarts++
                if (fixture.failService) error("foreground launch denied")
            }, workerDispatcher = workerDispatcher ?: dispatcher, ioDispatcher = dispatcher, now = { testScheduler.currentTime })
        runCurrent()
        return fixture
    }

    private class MemoryStore(var saved: SearchSnapshot = SearchSnapshot()) : SearchCheckpointStore {
        var failWrites = false
        override fun load() = saved
        override fun save(snapshot: SearchSnapshot) {
            if (failWrites) error("disk full")
            saved = snapshot
        }
    }

    private class Fixture(val store: MemoryStore, val scope: CoroutineScope, val engine: FakeEngine) {
        lateinit var controller: SearchController
        var serviceStarts = 0
        var failService = false
    }

    private inner class FakeEngine : NativeSeedFinder by DemoNativeSeedFinder() {
        var impossibleReason: String? = null
        var onPrepare: () -> Unit = {}
        override fun impossibilityReason(request: SearchRequest): String? {
            onPrepare()
            return impossibleReason
        }
        val sessions = mutableListOf<FakeSession>()
        val windows = mutableListOf<ResumeHint>()
        val filterSources = mutableListOf<SearchRequest>()
        val filtered = mutableListOf<SeedResult>()
        val completedBatches = ArrayDeque<Pair<List<SeedResult>, ResumeHint>>()
        var filter: (List<SeedResult>) -> List<SeedResult> = { it }
        var onFilter: () -> Unit = {}
        override fun startSearch(request: SearchRequest, workers: Int): NativeSearchSession = open()
        override fun startResumedSearch(request: SearchRequest, resumeFrom: Long, scanLen: Long, workers: Int): NativeSearchSession {
            windows += ResumeHint(resumeFrom, scanLen)
            return open()
        }
        override fun filterRecipes(request: SearchRequest, base: SearchRequest, recipes: List<SeedResult>): List<SeedResult> {
            onFilter()
            filterSources += base
            filtered += recipes
            return filter(recipes)
        }
        private fun open(): NativeSearchSession {
            assertTrue("Previous handle must close before resuming", sessions.all { it.closed })
            return FakeSession(completedBatches.removeFirstOrNull()).also { sessions += it }
        }
    }

    private inner class FakeSession(private val completed: Pair<List<SeedResult>, ResumeHint>? = null) : NativeSearchSession {
        var closed = false
        var cancelled = false
        private var emittedInitial = false
        private var drained = 0
        override fun poll(maxResults: Int): SearchBatch {
            check(!closed)
            if (completed != null) return SearchBatch(completed.first)
            return SearchBatch(when {
                !emittedInitial -> listOf(a).also { emittedInitial = true }
                cancelled && drained < 2 -> listOf(listOf(b, c)[drained++])
                else -> emptyList()
            })
        }
        override fun status() = SearchStatus(
            if (completed != null) SearchState.COMPLETED else if (cancelled && drained == 2) SearchState.CANCELLED else SearchState.RUNNING, 100, 1_000,
        )
        override fun resumeHint(): ResumeHint {
            if (completed != null) return completed.second
            check(cancelled && drained == 2) { "Unsafe cursor read before draining" }
            return ResumeHint(100, 900)
        }
        override fun cancel() { cancelled = true }
        override fun close() { closed = true }
    }

    private class QueuedWorker : CoroutineDispatcher() {
        private val tasks = ArrayDeque<Runnable>()
        var executing = false
            private set
        override fun dispatch(context: CoroutineContext, block: Runnable) { tasks += block }
        fun runNext() {
            executing = true
            try { tasks.removeFirst().run() } finally { executing = false }
        }
    }
}
