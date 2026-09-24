// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app

import android.Manifest
import android.app.Service
import android.content.Intent
import android.os.Build
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.engine.DemoNativeSeedFinder
import dev.seedseeker.app.engine.NativeSearchSession
import dev.seedseeker.app.engine.NativeSeedFinder
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.ResumeHint
import dev.seedseeker.app.model.SearchBatch
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.ui.ScoutRobolectricTestRunner
import dev.seedseeker.app.ui.SearchCheckpointStore
import dev.seedseeker.app.ui.SearchController
import dev.seedseeker.app.ui.SearchSnapshot
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.android.controller.ServiceController
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowPowerManager

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [23, 33, 35], application = SearchServiceTest.TestApplication::class)
class SearchServiceTest {
    private lateinit var app: TestApplication
    private var service: ServiceController<SearchService>? = null

    @Before fun prepare() {
        app = RuntimeEnvironment.getApplication() as TestApplication
        Dispatchers.setMain(app.dispatcher)
        app.searchController
        app.scheduler.runCurrent()
    }

    @After fun cleanUp() {
        service?.destroy()
        app.scope.cancel()
        app.scheduler.runCurrent()
        Dispatchers.resetMain()
    }

    @Test fun foregroundNotificationAndWakeLockSurviveTaskRemovalAndStopTogether() {
        val running = start()
        val shadow = shadowOf(running)
        assertNotNull(shadow.lastForegroundNotification)
        assertEquals("Stop", shadow.lastForegroundNotification.actions.single().title)
        val lock = ShadowPowerManager.getLatestWakeLock()
        assertTrue(lock.isHeld)
        running.onTaskRemoved(Intent())
        app.scheduler.runCurrent()
        assertTrue(app.searchController.isSearching)
        assertTrue(lock.isHeld)

        running.onStartCommand(Intent().setAction(SearchService.ACTION_STOP), 0, 2)
        app.scheduler.advanceTimeBy(2_000)
        app.scheduler.runCurrent()
        assertFalse(lock.isHeld)
        assertFalse(app.searchController.isSearching)
        assertTrue(shadow.isStoppedBySelf)
        assertNull(app.saved.pending)
    }

    @Test fun destroyingTheServiceReleasesPowerAndLeavesRecoverableProgress() {
        start()
        val lock = ShadowPowerManager.getLatestWakeLock()
        service!!.destroy()
        service = null
        assertFalse(lock.isHeld)
        app.scheduler.advanceUntilIdle()
        assertEquals(ResumeHint(100, 900), app.saved.pending?.window)
        assertFalse(app.searchController.isSearching)
    }

    @Test fun notificationPermissionDenialStillAllowsForegroundSearch() {
        if (Build.VERSION.SDK_INT >= 33) shadowOf(app).denyPermissions(Manifest.permission.POST_NOTIFICATIONS)
        val running = start()
        assertTrue(app.searchController.isSearching)
        assertNotNull(shadowOf(running).lastForegroundNotification)
        assertTrue(ShadowPowerManager.getLatestWakeLock().isHeld)
    }

    @Test fun stickyRestartWithNoPendingSearchStopsWithoutHoldingPower() {
        val running = Robolectric.buildService(SearchService::class.java).create().also { service = it }.get()
        assertEquals(Service.START_STICKY, running.onStartCommand(null, 0, 1))
        app.scheduler.runCurrent()
        assertFalse(app.searchController.isSearching)
        assertTrue(shadowOf(running).isStoppedBySelf)
        assertFalse(ShadowPowerManager.getLatestWakeLock()?.isHeld ?: false)
    }

    @Test fun foregroundPromotionFailureStopsServiceAndPreservesRecovery() {
        app.searchController.start(SearchRequest(listOf(ItemRequirement(1, ItemCatalog.wands.first(), 1))), 2)
        // Foreground launch follows the asynchronous query check.
        app.scheduler.runCurrent()
        val running = Robolectric.buildService(SearchService::class.java).create().also { service = it }.get()
        shadowOf(running).setThrowInStartForeground(SecurityException("foreground denied"))
        assertEquals(Service.START_NOT_STICKY, running.onStartCommand(Intent(), 0, 1))
        app.scheduler.runCurrent()
        assertFalse(app.searchController.isSearching)
        assertNotNull(app.saved.pending)
        assertFalse(ShadowPowerManager.getLatestWakeLock()?.isHeld ?: false)
        assertTrue(shadowOf(running).isStoppedBySelf)
    }

    @Test @Config(sdk = [35]) fun timeoutReleasesPowerImmediatelyAndSavesForRestore() {
        val running = start()
        val lock = ShadowPowerManager.getLatestWakeLock()
        running.onTimeout(1, android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        assertFalse(lock.isHeld)
        assertTrue(shadowOf(running).isStoppedBySelf)
        app.scheduler.advanceUntilIdle()
        assertNotNull(app.saved.pending)
        assertFalse(app.searchController.isSearching)
    }

    private fun start(): SearchService {
        app.searchController.start(SearchRequest(listOf(ItemRequirement(1, ItemCatalog.wands.first(), 1))), 2)
        // Foreground launch follows the asynchronous query check.
        app.scheduler.runCurrent()
        val running = Robolectric.buildService(SearchService::class.java).create().also { service = it }.get()
        assertEquals(Service.START_STICKY, running.onStartCommand(Intent(), 0, 1))
        app.scheduler.runCurrent()
        return running
    }

    class TestApplication : SeedSeekerApplication() {
        val scheduler = TestCoroutineScheduler()
        val dispatcher = StandardTestDispatcher(scheduler)
        val scope = CoroutineScope(SupervisorJob() + dispatcher)
        internal var saved = SearchSnapshot()
        internal override val searchController by lazy {
            val engine = object : NativeSeedFinder by DemoNativeSeedFinder() {
                override fun startSearch(request: SearchRequest, workers: Int): NativeSearchSession = object : NativeSearchSession {
                    var stopped = false
                    override fun poll(maxResults: Int) = SearchBatch(emptyList())
                    override fun status() = SearchStatus(if (stopped) SearchState.CANCELLED else SearchState.RUNNING, 100, 1_000)
                    override fun resumeHint() = ResumeHint(100, 900)
                    override fun cancel() { stopped = true }
                    override fun close() { stopped = true }
                }
            }
            SearchController(engine, object : SearchCheckpointStore {
                override fun load() = saved
                override fun save(snapshot: SearchSnapshot) { saved = snapshot }
            }, scope, {}, dispatcher, dispatcher, now = { scheduler.currentTime })
        }
    }
}
