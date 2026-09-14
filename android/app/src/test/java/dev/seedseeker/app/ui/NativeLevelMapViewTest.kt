// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.os.SystemClock
import android.view.MotionEvent
import androidx.test.core.app.ApplicationProvider
import dev.seedseeker.app.engine.LevelMapRequest
import dev.seedseeker.app.engine.LevelMaps
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35])
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class NativeLevelMapViewTest {
    @Test fun viewportSurvivesLoadingRetryAndTrinketsButFitsNewLocations() = runBlocking {
        val request = LevelMapRequest("AAA-AAA-AAA", 12, 0, null)
        val original = LevelMaps.load(request)
        val changed = request.copy(trinket = "mimic_tooth")
        val replacement = LevelMaps.load(changed)
        val view = NativeLevelMapView(ApplicationProvider.getApplicationContext())
        view.layout(0, 0, 600, 400)
        view.bind(original, request, false, true, true) {}
        zoomAndPanMap(view)
        val transform = listOf(view.zoom, view.panX, view.panY)
        assertTrue(view.zoom > 1f)
        assertTrue(view.panX != 0f || view.panY != 0f)
        view.bind(null, changed, false, true, true) {}
        assertEquals(transform, listOf(view.zoom, view.panX, view.panY))
        // Retry and hidden/revealed scene changes use the same native viewport.
        view.bind(null, changed, true, true, true) {}
        view.bind(replacement, changed, true, true, true) {}
        assertEquals(transform, listOf(view.zoom, view.panX, view.panY))
        view.bind(null, request, true, true, true) {}
        view.bind(original, request, true, true, true) {}
        assertEquals(transform, listOf(view.zoom, view.panX, view.panY))
        for (location in listOf(request.copy(depth = 13), request.copy(branch = 1),
            request.copy(seed = "AAA-AAA-AAB"), request.copy(challenges = 1))) {
            view.bind(null, location, false, true, true) {}
            assertEquals(listOf(1f, 0f, 0f), listOf(view.zoom, view.panX, view.panY))
            view.bind(original, request, false, true, true) {}
            zoomAndPanMap(view)
        }
        view.release()
    }
}

internal fun zoomAndPanMap(view: NativeLevelMapView) {
    val time = SystemClock.uptimeMillis()
    val x = view.width / 2f - 20f
    val y = view.height / 2f - 20f
    for ((delay, action) in listOf(0L to MotionEvent.ACTION_DOWN, 20L to MotionEvent.ACTION_UP,
        100L to MotionEvent.ACTION_DOWN, 120L to MotionEvent.ACTION_UP)) {
        MotionEvent.obtain(time, time + delay, action, x, y, 0).let { event ->
            view.onTouchEvent(event)
            event.recycle()
        }
    }
}
