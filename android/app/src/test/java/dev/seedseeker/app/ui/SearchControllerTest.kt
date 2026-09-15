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

@OptIn(ExperimentalCoroutinesApi::class)
class SearchControllerTest {
    init { PackagedCatalog.install() }
    private val request = SearchRequest(listOf(ItemRequirement(1, ItemCatalog.wands.first(), 1)))
    private val a = SeedResult("AAA-AAA-AAA", 1, "mossy_clump")
    private val b = SeedResult("BBB-BBB-BBB", 1)
    private val c = SeedResult("CCC-CCC-CCC", 1)

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
        val target = TargetState(base, seeds, 50, 0)
        val store = MemoryStore(SearchSnapshot(results = seeds, query = base.toPresetQuery(), target = target,
            pending = PendingSearch(request, StartMode.TARGET_FILTER, 2, RefineSpec(50, 0, seeds, base))))
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
        assertEquals(target, fixture.controller.snapshot.target)
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

    private fun TestScope.fixture(store: MemoryStore = MemoryStore()): Fixture {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val scope = CoroutineScope(SupervisorJob() + dispatcher)
        val fixture = Fixture(store, scope, FakeEngine())
        fixture.controller = SearchController(fixture.engine, store, scope,
            startService = {
                fixture.serviceStarts++
                if (fixture.failService) error("foreground launch denied")
            }, workerDispatcher = dispatcher, ioDispatcher = dispatcher, now = { testScheduler.currentTime })
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
        val sessions = mutableListOf<FakeSession>()
        val windows = mutableListOf<ResumeHint>()
        var onFilter: () -> Unit = {}
        override fun startSearch(request: SearchRequest, workers: Int): NativeSearchSession = open()
        override fun startResumedSearch(request: SearchRequest, resumeFrom: Long, scanLen: Long, workers: Int): NativeSearchSession {
            windows += ResumeHint(resumeFrom, scanLen)
            return open()
        }
        override fun filterRecipes(request: SearchRequest, base: SearchRequest, recipes: List<SeedResult>): List<SeedResult> {
            onFilter()
            return recipes
        }
        private fun open(): NativeSearchSession {
            assertTrue("Previous handle must close before resuming", sessions.all { it.closed })
            return FakeSession().also { sessions += it }
        }
    }

    private inner class FakeSession : NativeSearchSession {
        var closed = false
        var cancelled = false
        private var emittedInitial = false
        private var drained = 0
        override fun poll(maxResults: Int): SearchBatch {
            check(!closed)
            return SearchBatch(when {
                !emittedInitial -> listOf(a).also { emittedInitial = true }
                cancelled && drained < 2 -> listOf(listOf(b, c)[drained++])
                else -> emptyList()
            })
        }
        override fun status() = SearchStatus(
            if (cancelled && drained == 2) SearchState.CANCELLED else SearchState.RUNNING, 100, 1_000,
        )
        override fun resumeHint(): ResumeHint {
            check(cancelled && drained == 2) { "Unsafe cursor read before draining" }
            return ResumeHint(100, 900)
        }
        override fun cancel() { cancelled = true }
        override fun close() { closed = true }
    }
}
