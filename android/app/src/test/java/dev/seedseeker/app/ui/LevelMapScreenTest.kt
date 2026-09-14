// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.BitmapFactory
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.JniNativeSeedFinder
import dev.seedseeker.app.engine.LevelMapRequest
import dev.seedseeker.app.engine.LevelMaps
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowDialog
import java.io.File

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w412dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class LevelMapScreenTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Test fun floorMapsStartClosedAndOnlyTheSelectedDisclosureStaysOpen() {
        PackagedCatalog.install()
        val original = JniNativeSeedFinder().scoutSelectedSeed("AAA-AAA-AAA", 0, null, "none")
        val world = original.copy(items = original.items.filter { it.depth in 1..2 }.groupBy { it.depth }.values.map { it.first() },
            floorFeelings = original.floorFeelings.filterKeys { it <= 2 }, quests = emptyList(), trinketOrder = emptyList())
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas) {
                    ScoutScreen(seedInput = world.seed, result = world, isScouting = false, error = null, matches = null,
                        resultSeeds = emptyList(), scoutedSeed = world.seed,
                        onScoutSeed = {}, onSeedChange = {}, onScout = {}, onSelectTrinket = {}, onSettings = {}, onAbout = {}, bottomBar = {})
                }
            }
        }
        compose.onNodeWithText("Expand map").assertDoesNotExist()
        compose.onNodeWithText("FLOOR 1").performClick()
        compose.onNodeWithText("Expand map").assertIsDisplayed()
        compose.onNodeWithText("FLOOR 1").performClick()
        compose.onNodeWithText("Expand map").assertDoesNotExist()
        compose.onNodeWithTag("scout-floors").performScrollToNode(hasText("FLOOR 2"))
        compose.onNodeWithText("FLOOR 2").performClick()
        assertEquals(1, compose.onAllNodesWithText("Expand map").fetchSemanticsNodes().size)
        compose.onNodeWithTag("scout-floors").performScrollToNode(hasText("FLOOR 1"))
        compose.onNodeWithText("FLOOR 1").performClick()
        assertEquals(1, compose.onAllNodesWithText("Expand map").fetchSemanticsNodes().size)
    }

    @Test
    @Config(qualifiers = "w320dp-h915dp-xhdpi")
    fun expandedFloorSurvivesTrinketChangesAndCloseReturnsToInlineFloor() {
        PackagedCatalog.install()
        val initial = JniNativeSeedFinder().scoutSelectedSeed("MKG-FUN-IHX", 0, null, "none")
        val world = mutableStateOf(initial)
        val offer = initial.trinketOrder.first()
        runBlocking {
            LevelMaps.load(LevelMapRequest(initial.seed, 18, 0, null))
            LevelMaps.load(LevelMapRequest(initial.seed, 19, 0, null))
            LevelMaps.load(LevelMapRequest(initial.seed, 19, 0, offer.id))
        }
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas) {
                    Box(Modifier.fillMaxWidth().height(350.dp)) {
                        LevelMapView(world.value, 18, listOf(18, 19), 0, false) { trinket ->
                            world.value = world.value.copy(selectedTrinket = trinket.takeUnless { it == "none" })
                        }
                    }
                }
            }
        }
        compose.onNodeWithText("Expand map").performClick()
        compose.onNodeWithContentDescription("Next floor").performClick()
        compose.onNodeWithText("FLOOR 19").assertIsDisplayed()
        initial.trinketOrder.take(4).forEach { compose.onNodeWithContentDescription(it.name).assertIsDisplayed() }
        compose.onNodeWithContentDescription(offer.name).performClick()
        compose.onNodeWithText("FLOOR 19").assertIsDisplayed()
        compose.runOnIdle { assertEquals(offer.id, world.value.selectedTrinket) }
        compose.onNodeWithContentDescription(offer.name).assertIsSelected()
        compose.onNodeWithText("+3").assertDoesNotExist()
        compose.onNodeWithText("Fit").assertDoesNotExist()
        compose.onNodeWithContentDescription("Zoom in").assertDoesNotExist()
        compose.onNodeWithContentDescription("Zoom out").assertDoesNotExist()
        compose.onNodeWithContentDescription("Next floor").assertIsNotEnabled()
        val heading = compose.onNodeWithText("FLOOR 19").fetchSemanticsNode().boundsInRoot
        val previous = compose.onNodeWithContentDescription("Previous floor").fetchSemanticsNode().boundsInRoot
        val next = compose.onNodeWithContentDescription("Next floor").fetchSemanticsNode().boundsInRoot
        assertTrue(previous.top > heading.bottom)
        assertEquals(previous.center.y, next.center.y, 1f)
        compose.waitUntil(timeoutMillis = 10_000) {
            compose.onAllNodesWithText("Charting floor 19…").fetchSemanticsNodes().isEmpty()
        }
        screenshot("expanded-320dp")
        compose.onNodeWithContentDescription("Close map").performClick()
        compose.onNodeWithText("Expand map").performClick()
        compose.onNodeWithText("FLOOR 18").assertIsDisplayed()
    }

    private fun screenshot(name: String) {
        System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
        val window = requireNotNull(ShadowDialog.getLatestDialog().window)
        val bitmap = Bitmap.createBitmap(window.decorView.width, window.decorView.height, Bitmap.Config.ARGB_8888)
        var result: Int? = null
        compose.runOnIdle { PixelCopy.request(window, bitmap, { result = it }, Handler(Looper.getMainLooper())) }
        compose.waitUntil { result != null }
        assertEquals(PixelCopy.SUCCESS, result)
        assertTrue("The native map must not paint over the Compose header", (0 until minOf(200, bitmap.height)).any { y ->
            (0 until bitmap.width).any { x -> bitmap.getPixel(x, y) != android.graphics.Color.BLACK }
        })
        val file = File("build/outputs/level-maps/$name.png")
        file.parentFile?.mkdirs()
        file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }
}
