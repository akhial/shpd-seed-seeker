// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.engine.ScoutMatches
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.model.CatalogItem
import dev.seedseeker.app.model.RingGems
import dev.seedseeker.app.model.ScoutAccessibility
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.model.ScoutWorld
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w412dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ScoutScreenScrollTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    private val offers by lazy {
        listOf("cracked_spyglass", "dimensional_sundial", "mimic_tooth", "trap_mechanism")
            .map { requireNotNull(ItemCatalog.findById(it)) }
    }
    private val world by lazy {
        ScoutWorld(
            seed = "EQI-HLQ-RTU",
            items = offers.map { item(it, 1) } + (1..19).map {
                item(requireNotNull(ItemCatalog.findById("fishing_spear")), it)
            },
            quests = emptyList(),
            ringGems = RingGems.CATALOG,
            trinketOrder = offers,
        )
    }

    private fun item(catalog: CatalogItem, depth: Int) = ScoutItem(
        item = catalog, depth = depth, upgrade = 0, effect = null, cursed = false,
        source = ScoutItemSource.HEAP, accessibility = ScoutAccessibility.Independent,
    )

    private fun show(
        matches: ScoutMatches? = ScoutMatches(emptySet(), 5, 5),
        fontScale: Float = 1f,
        onStep: (String) -> Unit = {},
        onSelect: (String) -> Unit = {},
    ) {
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(
                    LocalItemAtlas provides atlas,
                    LocalDensity provides Density(compose.density.density, fontScale),
                ) {
                    ScoutScreen(
                        seedInput = world.seed, result = world, isScouting = false, error = null,
                        matches = matches, resultSeeds = listOf(world.seed, "ABC-DEF-GHI"), scoutedSeed = world.seed,
                        onScoutSeed = onStep, onSeedChange = {}, onScout = {}, onSelectTrinket = onSelect,
                        onSettings = {}, onAbout = {}, bottomBar = { Box(Modifier.fillMaxWidth().height(80.dp)) },
                    )
                }
            }
        }
    }

    private fun bounds(tag: String) = compose.onNodeWithTag(tag).fetchSemanticsNode().boundsInRoot

    /** Slow drags with a held release avoid a fling obscuring intermediate assertions. */
    private fun drag(distanceDp: Float, fromHeader: Boolean = false) {
        val distance = with(compose.density) { distanceDp.dp.toPx() }
        val start = if (fromHeader) bounds("scout-summary").center else null
        compose.onRoot().performTouchInput {
            down(start ?: Offset(centerX, if (distanceDp > 0) height * 0.85f else height * 0.3f))
            moveBy(Offset(0f, -distance), delayMillis = 1000)
            advanceEventTime(200)
            up()
        }
        compose.waitForIdle()
    }

    private fun screenshot(name: String) {
        // PixelCopy directly avoids Compose's VSYNC wait, which has no render thread on Robolectric.
        System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
        val window = compose.activity.window
        val bitmap = Bitmap.createBitmap(window.decorView.width, window.decorView.height, Bitmap.Config.ARGB_8888)
        var copyResult: Int? = null
        compose.runOnIdle {
            PixelCopy.request(window, bitmap, { copyResult = it }, Handler(Looper.getMainLooper()))
        }
        compose.waitUntil { copyResult != null }
        assertEquals(PixelCopy.SUCCESS, copyResult)
        val output = File("build/outputs/scout-ui/$name.png")
        output.parentFile?.mkdirs()
        output.outputStream().use {
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)
        }
    }

    @Test fun formScrollsAwayAndSummaryCollapsesAboveNavigationThenReverses() {
        show()
        val expanded = bounds("scout-summary")
        assertTrue(bounds("scout-navigation").top >= expanded.bottom)
        screenshot("expanded")
        drag(155f)
        val middle = bounds("scout-summary")
        assertTrue(middle.top < expanded.top)
        assertTrue(middle.height < expanded.height)
        screenshot("collapsing")
        drag(250f)
        compose.onNodeWithTag("scout-input").assertIsNotDisplayed()
        val compact = bounds("scout-summary")
        val badge = bounds("scout-requirements")
        assertTrue(compact.height < middle.height)
        assertEquals(badge.width, badge.height, 1f)
        assertEquals(bounds("scout-seed").center.y, badge.center.y, 1f)
        assertTrue(bounds("scout-navigation").top >= compact.bottom)
        compose.onNodeWithText("Copy").assertIsDisplayed().performClick()
        screenshot("compact")
        repeat(4) { drag(-300f) }
        compose.onNodeWithTag("scout-input").assertIsDisplayed()
        assertEquals(expanded.height, bounds("scout-summary").height, 1f)
        assertEquals(expanded.top, bounds("scout-summary").top, 1f)
    }

    @Test fun compactNavigationAndTrinketActionsStayAboveTheFloorItems() {
        var steppedTo: String? = null
        var selected: String? = null
        show(onStep = { steppedTo = it }, onSelect = { selected = it })
        drag(500f)
        val pinned = bounds("scout-summary")
        drag(240f)
        assertEquals(pinned.top, bounds("scout-summary").top, 1f)
        assertEquals(pinned.height, bounds("scout-summary").height, 1f)
        compose.onNode(hasContentDescription("Mimic Tooth") and hasAnyAncestor(hasTestTag("scout-navigation")))
            .assertIsDisplayed().performClick()
        compose.runOnIdle { assertEquals("mimic_tooth", selected) }
        compose.onNodeWithContentDescription("Next result").performClick()
        compose.runOnIdle { assertEquals("ABC-DEF-GHI", steppedTo) }
        steppedTo = null
        compose.onRoot().performTouchInput { swipeLeft() }
        compose.runOnIdle { assertEquals("ABC-DEF-GHI", steppedTo) }
        screenshot("trinket-controls")
    }

    @Test fun draggingTheSummaryAlsoScrollsAndPartialMatchesKeepTheirAccessibleCount() {
        show(matches = ScoutMatches(emptySet(), 2, 5))
        drag(170f, fromHeader = true)
        drag(100f, fromHeader = true)
        val badge = bounds("scout-requirements")
        assertEquals(badge.width, badge.height, 1f)
        compose.onNodeWithTag("scout-requirements").assertContentDescriptionEquals("2 of 5 requirements")
        screenshot("partial-match")
    }

    @Test @Config(qualifiers = "w320dp-h740dp-xhdpi")
    fun narrowPhoneKeepsCompactSeedBadgeAndCopySeparate() {
        show()
        drag(330f)
        val seed = bounds("scout-seed")
        val badge = bounds("scout-requirements")
        val copy = compose.onNodeWithText("Copy").fetchSemanticsNode().boundsInRoot
        assertTrue(seed.right <= badge.left)
        assertTrue(badge.right <= copy.left)
        screenshot("compact-320dp")
    }

    @Test fun standaloneScoutCollapsesWithoutARequirementsBadge() {
        show(matches = null)
        drag(330f)
        compose.onNodeWithTag("scout-input").assertIsNotDisplayed()
        compose.onNodeWithTag("scout-requirements").assertDoesNotExist()
        compose.onNodeWithTag("scout-seed").assertIsDisplayed()
        compose.onNodeWithText("Copy").assertIsDisplayed()
    }

    @Test fun largerTextStillKeepsTheCompactSummaryOnOneLine() {
        show(fontScale = 1.5f)
        drag(400f)
        val seed = bounds("scout-seed")
        val badge = bounds("scout-requirements")
        assertEquals(seed.center.y, badge.center.y, 1f)
        assertTrue(seed.right <= badge.left)
        compose.onNodeWithText("Copy").assertIsDisplayed()
        screenshot("compact-large-text")
    }

    @Test fun adjacentScrollFractionsScaleTheSeedWithoutChangingItsGlyphLayout() {
        val progress = mutableFloatStateOf(0.45f)
        compose.setContent {
            SeedSeekerTheme {
                ScoutSummaryCard(world, ScoutMatches(emptySet(), 5, 5), progress.floatValue, {})
            }
        }
        fun seedLayout(): TextLayoutResult {
            val results = mutableListOf<TextLayoutResult>()
            compose.onNodeWithTag("scout-seed").performSemanticsAction(SemanticsActions.GetTextLayoutResult) {
                it(results)
            }
            return results.single()
        }
        val beforeLayout = seedLayout()
        val before = bounds("scout-seed")
        compose.runOnIdle { progress.floatValue = 0.451f }
        val afterLayout = seedLayout()
        val after = bounds("scout-seed")
        assertEquals(24.sp, beforeLayout.layoutInput.style.fontSize)
        assertEquals(beforeLayout.layoutInput.style.fontSize, afterLayout.layoutInput.style.fontSize)
        assertEquals(beforeLayout.size, afterLayout.size)
        assertTrue("The visible seed width should change by a fraction of a pixel", before.width - after.width in 0.001f..0.2f)
        assertEquals(before.left, after.left, 0.001f)
    }
}
