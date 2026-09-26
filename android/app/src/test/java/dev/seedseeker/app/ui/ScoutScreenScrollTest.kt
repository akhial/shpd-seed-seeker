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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextReplacement
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
import org.robolectric.shadows.ShadowDialog

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
                if (it == 1) item(requireNotNull(ItemCatalog.findById("force_cube")), it).copy(
                    upgrade = 3, effect = "Chilling", source = ScoutItemSource.IMP_REWARD,
                    accessibility = ScoutAccessibility.Choice(group = 6, option = 0),
                ) else item(requireNotNull(ItemCatalog.findById("fishing_spear")), it)
            },
            quests = emptyList(),
            ringGems = RingGems.CATALOG,
            trinketOrder = offers,
            itemMappings = dev.seedseeker.app.engine.JniNativeSeedFinder().scoutSeed("EQI-HLQ-RTU").itemMappings,
        )
    }

    private fun item(catalog: CatalogItem, depth: Int) = ScoutItem(
        item = catalog, depth = depth, upgrade = 0, effect = null, cursed = false,
        source = ScoutItemSource.HEAP, accessibility = ScoutAccessibility.Independent,
    )

    private fun show(
        matches: ScoutMatches? = ScoutMatches(setOf(4), 5, 5),
        fontScale: Float = 1f,
        initialSeed: String = world.seed,
        onStep: (String) -> Unit = {},
        onSelect: (String) -> Unit = {},
        artifactDeck: List<CatalogItem> = emptyList(),
        offerDepth: Int = 1,
    ) {
        val shown = world.copy(
            artifactDecks = if (artifactDeck.isEmpty()) emptyMap() else mapOf(1 to artifactDeck),
            items = world.items.map { if (it.item in offers) it.copy(depth = offerDepth) else it },
        )
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        val iconAtlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/item_icons.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            val input = remember { mutableStateOf(initialSeed) }
            SeedSeekerTheme {
                CompositionLocalProvider(
                    LocalItemAtlas provides atlas,
                    LocalItemIconAtlas provides iconAtlas,
                    LocalDensity provides Density(compose.density.density, fontScale),
                ) {
                    ScoutScreen(
                        seedInput = input.value, result = shown, isScouting = false, error = null,
                        matches = matches, resultSeeds = listOf(world.seed, "ABC-DEF-GHI"), scoutedSeed = world.seed,
                        onScoutSeed = onStep, onSeedChange = { input.value = it }, onScout = { onStep(input.value) }, onSelectTrinket = onSelect,
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

    private fun screenshot(name: String, window: android.view.Window = compose.activity.window) {
        // PixelCopy directly avoids Compose's VSYNC wait, which has no render thread on Robolectric.
        System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
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

    @Test fun partialEightLetterSeedStaysEditable() {
        show(initialSeed = "ABC-DEF-GH")
        compose.onNodeWithText("Scout seed").assertIsDisplayed()
        compose.onNodeWithText("Scout daily run").assertDoesNotExist()
    }

    @Test fun seedAndDateEntryKeepTheSameFieldAndFormHeight() {
        show(initialSeed = "")
        screenshot("input-empty")
        compose.onNodeWithTag("scout-date-picker").assertDoesNotExist()
        compose.onNodeWithTag("scout-today").assertDoesNotExist()
        val restingField = bounds("scout-run-field")
        val form = bounds("scout-input")
        compose.onNodeWithTag("scout-run-field").performClick()
        val field = bounds("scout-run-field")
        assertTrue(field.width < restingField.width)
        assertEquals(restingField.height, field.height, 1f)
        screenshot("input-editing")
        for (input in listOf("2", "202609", "20260925", "", "ABCDEFGH", "ABCDEFGHI")) {
            compose.onNodeWithTag("scout-run-field").performTextReplacement(input)
            compose.waitForIdle()
            assertEquals(field, bounds("scout-run-field"))
            assertEquals(form, bounds("scout-input"))
            val calendar = bounds("scout-date-picker")
            val today = bounds("scout-today")
            assertTrue(field.right <= calendar.left)
            assertTrue(calendar.right <= today.left)
            assertEquals(field.bottom, calendar.bottom, 1f)
            assertEquals(field.bottom, today.bottom, 1f)
            compose.onNodeWithContentDescription("Choose daily run date").assertIsDisplayed()
            compose.onNodeWithText("Today").assertIsDisplayed()
        }
    }

    @Test fun dailyPickerAndTodayUseTheExistingScoutActions() {
        var scouted: String? = null
        show(initialSeed = "2026-09-25", onStep = { scouted = it })
        compose.onNodeWithTag("scout-run-field").performClick()
        compose.onNodeWithContentDescription("Choose daily run date").assertIsDisplayed().performClick()
        compose.onNodeWithText("Daily run date (UTC)").assertIsDisplayed()
        compose.onNodeWithText("Use date").performClick()
        compose.onNodeWithText("Scout daily run").performClick()
        compose.runOnIdle { assertEquals("2026-09-25", scouted) }
        compose.onNodeWithTag("scout-date-picker").assertDoesNotExist()
        compose.onNodeWithTag("scout-today").assertDoesNotExist()
        screenshot("daily-run")
        compose.onNodeWithTag("scout-run-field").performClick()
        compose.onNodeWithContentDescription("Choose daily run date").performClick()
        compose.onNodeWithText("Cancel").performClick()
        compose.onNodeWithText("Today").performClick()
        compose.runOnIdle { assertEquals(dev.seedseeker.app.model.DailyRunDate.today(), scouted) }
        compose.onNodeWithTag("scout-date-picker").assertDoesNotExist()
        compose.onNodeWithTag("scout-today").assertDoesNotExist()
        compose.onNodeWithTag("scout-run-field").performTextReplacement("ABCDEFGHI")
        compose.onNodeWithText("Scout seed").assertIsDisplayed()
    }

    @Test fun seedInformationRemainsAccessibleAfterCollapsingAndScrollsThroughEveryCategory() {
        show()
        compose.onNodeWithContentDescription("Seed information").assertIsDisplayed()
        drag(405f)
        compose.onNodeWithContentDescription("Seed information").performClick()
        compose.onNodeWithText("Potions").assertIsDisplayed()
        screenshot("seed-mapping-grid", requireNotNull(ShadowDialog.getLatestDialog().window))
        val mappings = requireNotNull(world.itemMappings)
        for ((category, entries) in listOf("potions" to mappings.potions, "scrolls" to mappings.scrolls, "rings" to mappings.rings)) {
            compose.onNodeWithTag("mapping-$category-0").performScrollTo()
            val cells = entries.indices.map { bounds("mapping-$category-$it") }
            assertTrue(cells.take(6).all { it.top == cells[0].top })
            assertTrue(cells.drop(6).all { it.top == cells[6].top })
            assertTrue(cells[6].top > cells[0].bottom)
            assertEquals(cells[0].left, cells[6].left, 1f)
            for (entry in entries) {
                compose.onNodeWithContentDescription(mappingLabel(entry)).performScrollTo().assertIsDisplayed()
                compose.onNodeWithText(entry.name).assertDoesNotExist()
            }
            val label = mappingLabel(entries.first())
            compose.onNodeWithContentDescription(label).performScrollTo().performClick()
            compose.onNodeWithTag("mapping-detail").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText(label).assertIsDisplayed()
            compose.onNodeWithContentDescription(label).performScrollTo().performClick()
            compose.onNodeWithTag("mapping-detail").assertDoesNotExist()
        }
        compose.onNodeWithText("Close").performClick()
        compose.onNodeWithTag("seed-mappings").assertDoesNotExist()
        compose.onNodeWithContentDescription("Seed information").assertIsDisplayed()
    }

    @Test fun formScrollsAwayAndSummaryCollapsesAboveNavigationThenReverses() {
        show()
        val expanded = bounds("scout-summary")
        assertTrue(bounds("scout-navigation").top >= expanded.bottom)
        screenshot("expanded")
        // Keep the intermediate collapse assertion independent of form controls.
        drag(bounds("scout-input").height / compose.density.density - 30f)
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

    /** The artifact deck adds a row above the floors; the shortcuts must still track the trinket offers. */
    @Test fun trinketShortcutsAppearOnceTheOffersScrollAwayEvenWithAnArtifactDeck() {
        val deck = listOf("chalice_of_blood", "horn_of_plenty", "master_thieves_armband")
            .mapNotNull { ItemCatalog.findById(it) }
        // As on a real seed: the offers lie a few floors down, below the deck.
        show(artifactDeck = deck, offerDepth = 3)
        val shortcut = hasContentDescription("Mimic Tooth") and hasAnyAncestor(hasTestTag("scout-navigation"))
        compose.onNode(shortcut).assertDoesNotExist()
        drag(500f)
        // Creep up until the offer tiles have slid under the pinned floor header:
        // from then on, the shortcuts must stand in for them.
        repeat(20) {
            val header = compose.onNodeWithText("FLOOR 3").fetchSemanticsNode().boundsInRoot
            val offer = compose.onNodeWithText("Cracked Spyglass").fetchSemanticsNode().boundsInRoot
            if (offer.bottom > header.bottom) drag(30f)
        }
        compose.onNodeWithText("FLOOR 3").assertIsDisplayed()
        compose.onNode(shortcut).assertIsDisplayed()
    }

    @Test fun draggingTheSummaryAlsoScrollsAndPartialMatchesKeepTheirAccessibleCount() {
        show(matches = ScoutMatches(emptySet(), 2, 5))
        drag(bounds("scout-input").height / compose.density.density, fromHeader = true)
        drag(100f, fromHeader = true)
        val badge = bounds("scout-requirements")
        assertEquals(badge.width, badge.height, 1f)
        compose.onNodeWithTag("scout-requirements").assertContentDescriptionEquals("2 of 5 requirements")
        screenshot("partial-match")
    }

    @Test @Config(qualifiers = "w320dp-h740dp-xhdpi")
    fun narrowPhoneKeepsCompactSeedBadgeAndCopySeparate() {
        show()
        screenshot("input-320dp")
        compose.onNodeWithTag("scout-run-field").performClick()
        assertTrue(bounds("scout-run-field").right <= bounds("scout-date-picker").left)
        assertTrue(bounds("scout-date-picker").right <= bounds("scout-today").left)
        screenshot("input-editing-320dp")
        compose.onNodeWithText("Scout seed").performClick()
        compose.onNodeWithTag("scout-date-picker").assertDoesNotExist()
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
