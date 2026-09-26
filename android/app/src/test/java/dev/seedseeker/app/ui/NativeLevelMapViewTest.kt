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
    @Test fun inspectionUsesOriginalTextHonorsSecretsAndClearsWhenMapChanges() = runBlocking {
        val request = LevelMapRequest("AAA-AAA-AAA", 1, 0, null)
        val bundle = LevelMaps.load(request)
        val map = bundle.map
        val tip = map.itemTooltips.first { !it.hidden }
        assertTrue(tip.items.first().description.isNotEmpty())
        val bounds = requireNotNull(tip.bounds)
        assertTrue(map.itemTooltips.flatMap { it.items }.any { it.icon?.size == 4 })
        val spriteX = tip.cell % map.width * 16 + bounds[0] + bounds[2] / 2f
        val spriteY = tip.cell / map.width * 16 + bounds[1]
        assertEquals(tip, map.itemAt(spriteX, spriteY + .5f, false))
        assertNotEquals(tip, map.itemAt(spriteX, spriteY - .5f, false))
        val x = (tip.cell % map.width + .5f) * 16
        val y = (tip.cell / map.width + .5f) * 16
        assertEquals(tip, map.itemAt(x, y, false))
        assertNull(map.itemAt(-1f, y, true))
        val hidden = tip.copy(hidden = true)
        val concealed = map.copy(itemTooltips = listOf(hidden))
        assertNull(concealed.itemAt(x, y, false))
        assertEquals(hidden, concealed.itemAt(x, y, true))
        val view = NativeLevelMapView(ApplicationProvider.getApplicationContext())
        view.layout(0, 0, map.width * 16, map.height * 16)
        view.bind(bundle, request, false, true, true) {}
        view.inspectItem(x, y)
        assertEquals(1, view.childCount)
        view.bind(null, request.copy(trinket = "mimic_tooth"), false, true, true) {}
        assertEquals(0, view.childCount)
        view.release()
    }

    @Test fun inspectionDecodesGeneratedUpgradesEnchantmentsAndCurses() = runBlocking {
        val bundle = LevelMaps.load(LevelMapRequest("AAA-AAA-AAA", 7, 0, null))
        val items = bundle.map.itemTooltips.flatMap { it.items }
        val enchanted = items.first { it.name == "Assassin's blade" }
        assertEquals(1, enchanted.upgrade)
        assertEquals("Vorpal", enchanted.enchantment)
        assertFalse(enchanted.cursed)
        assertNull(enchanted.curse)
        assertArrayEquals(intArrayOf(170, 102, 102), enchanted.glow!!.color)
        assertEquals(1000, enchanted.glow.periodMs)
        val cursed = items.first { it.curse == "Wondrous" }
        assertEquals(1, cursed.upgrade)
        assertTrue(cursed.cursed)
        assertNull(cursed.enchantment)
        assertArrayEquals(intArrayOf(0, 0, 0), cursed.glow!!.color)
        assertTrue(items.any { it.upgrade == null && it.glow == null })
    }

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
