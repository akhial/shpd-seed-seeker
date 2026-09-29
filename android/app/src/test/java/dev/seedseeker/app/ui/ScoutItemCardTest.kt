// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.Rect
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import kotlin.math.ceil
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import java.io.File
import androidx.compose.ui.graphics.asImageBitmap
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.model.RingGems
import dev.seedseeker.app.model.ScoutAccessibility
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w600dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ScoutItemCardTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Test fun sourceSharesWrappedChipRowWhenThereAreNoStatusBadges() {
        val width = mutableStateOf(360.dp)
        val fontScale = mutableStateOf(1f)
        val matched = mutableStateOf(true)
        val accessibility = mutableStateOf<ScoutAccessibility>(ScoutAccessibility.Independent)
        val item = ScoutItem(
            item = requireNotNull(ItemCatalog.findById("wand_prismatic_light")),
            depth = 4, upgrade = 2, effect = null, cursed = false,
            source = ScoutItemSource.MIMIC,
            accessibility = ScoutAccessibility.Independent,
        )
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas, LocalDensity provides Density(compose.density.density, fontScale.value)) {
                    ScoutItemCard(item.copy(accessibility = accessibility.value), RingGems.CATALOG,
                        matches = matched.value, modifier = Modifier.width(width.value).testTag("item-card"))
                }
            }
        }
        compose.onNodeWithTag("scout-item-match-choices").assertExists()
        for (scale in listOf(1f, 1.5f, 2f)) {
            for (cardWidth in listOf(360.dp, 320.dp)) {
                for (hasChoice in listOf(false, true)) {
                    for (isMatch in listOf(true, false)) {
                        compose.runOnIdle {
                            width.value = cardWidth
                            fontScale.value = scale
                            accessibility.value = if (hasChoice) ScoutAccessibility.Choice(0, 0) else ScoutAccessibility.Independent
                            matched.value = isMatch
                        }
                        val title = compose.onNodeWithText(item.item.name).fetchSemanticsNode().boundsInRoot
                        val source = compose.onNodeWithText("Mimic").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                        assertTrue("Source stays below the title", source.top >= title.bottom)
                        if (isMatch) compose.onNodeWithText("match").assertIsDisplayed()
                        if (hasChoice) compose.onNodeWithText("A").assertIsDisplayed()
                        val trailing = compose.onAllNodesWithTag("scout-item-match-choices")
                            .fetchSemanticsNodes().singleOrNull()?.boundsInRoot
                        if (trailing != null && (isMatch || hasChoice)) {
                            val card = compose.onNodeWithTag("item-card").fetchSemanticsNode().boundsInRoot
                            assertEquals("Source shares the wrapped chip row at $cardWidth / $scale",
                                trailing.center.y, source.center.y, 1f)
                            assertTrue("Source does not overlap the chips", source.right <= trailing.left)
                            assertEquals("Chips stay at the right padding",
                                card.right - with(compose.density) { 14.dp.toPx() }, trailing.right, 1f)
                        } else {
                            assertTrue("No empty badge row above the source",
                                source.top - title.bottom <= with(compose.density) { 4.dp.toPx() })
                        }
                    }
                }
            }
        }
        compose.runOnIdle {
            width.value = 360.dp
            fontScale.value = 1f
            accessibility.value = ScoutAccessibility.Independent
            matched.value = true
        }
        screenshot("item-source-with-match")
    }

    @Test fun matchLabelStaysVisibleWhenCardWidthChanges() {
        val width = mutableStateOf(360.dp)
        val item = ScoutItem(
            item = requireNotNull(ItemCatalog.findById("wand_fireblast")),
            depth = 1, upgrade = 2, effect = null, cursed = true,
            source = ScoutItemSource.HEAP,
            accessibility = ScoutAccessibility.Choice(group = 0, option = 0),
        )
        compose.setContent {
            SeedSeekerTheme {
                ScoutItemCard(item, RingGems.CATALOG, matches = true, modifier = Modifier.width(width.value).testTag("item-card"))
            }
        }
        compose.onNodeWithText("match").assertIsDisplayed()
        val layouts = mutableListOf<TextLayoutResult>()
        compose.onNodeWithText(item.item.name).performSemanticsAction(SemanticsActions.GetTextLayoutResult) { it(layouts) }
        assertEquals(1, layouts.single().lineCount)
        compose.runOnIdle { width.value = 560.dp }
        compose.onNodeWithText("match").assertIsDisplayed()
        compose.runOnIdle { width.value = 360.dp }
        compose.onNodeWithText("match").assertIsDisplayed()
    }

    @Test fun resinDonorMatchesWearTheResinChip() {
        val donor = mutableStateOf(true)
        val item = ScoutItem(
            item = requireNotNull(ItemCatalog.findById("wand_frost")),
            depth = 3, upgrade = 0, effect = null, cursed = false,
            source = ScoutItemSource.HEAP,
            accessibility = ScoutAccessibility.Independent,
        )
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas) {
                    ScoutItemCard(item, RingGems.CATALOG, matches = true, resinDonor = donor.value,
                        modifier = Modifier.width(360.dp).testTag("item-card"))
                }
            }
        }
        // The donor chip keeps the "match" label but is announced as a resin donor,
        // and its sprite adds no second "Arcane Resin" announcement.
        compose.onNodeWithText("match").assertIsDisplayed()
        compose.onNodeWithContentDescription("Arcane Resin donor match").assertIsDisplayed()
        compose.onAllNodesWithContentDescription("Arcane Resin", useUnmergedTree = true).assertCountEquals(0)
        screenshot("item-resin-donor-match")
        compose.runOnIdle { donor.value = false }
        compose.onNodeWithText("match").assertIsDisplayed()
        compose.onAllNodesWithContentDescription("Arcane Resin donor match").assertCountEquals(0)
    }

    @Test fun upgradeTagIsSetLikeTheBoardsAndTagsKeepTheTitleRowHeight() {
        val fontScale = mutableStateOf(1f)
        val tagged = mutableStateOf(true)
        val item = ScoutItem(
            item = requireNotNull(ItemCatalog.findById("plate_armor")),
            depth = 1, upgrade = 3, effect = "Viscosity", cursed = true,
            source = ScoutItemSource.TOMB,
            accessibility = ScoutAccessibility.Independent,
        )
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalDensity provides Density(compose.density.density, fontScale.value)) {
                    ScoutItemCard(
                        if (tagged.value) item else item.copy(upgrade = 0, cursed = false), RingGems.CATALOG,
                        matches = true, modifier = Modifier.width(560.dp).testTag("item-card"),
                    )
                }
            }
        }
        // The upgrade wears the requirement board's upgrade tag: monospace, semibold.
        val layouts = mutableListOf<TextLayoutResult>()
        compose.onNodeWithText("+3").performSemanticsAction(SemanticsActions.GetTextLayoutResult) { it(layouts) }
        val style = layouts.single().layoutInput.style
        assertEquals(FontFamily.Monospace, style.fontFamily)
        assertEquals(FontWeight.SemiBold, style.fontWeight)
        for (scale in listOf(1f, 1.5f, 2f)) {
            val heights = listOf(true, false).map { withTags ->
                compose.runOnIdle {
                    fontScale.value = scale
                    tagged.value = withTags
                }
                compose.onNodeWithTag("item-card").fetchSemanticsNode().boundsInRoot.height
            }
            assertEquals("Tags must not grow the card at $scale", heights[1], heights[0], 0.5f)
        }
    }

    @Test fun longNamesStayCompleteWithBadgesAndLargeFonts() {
        val width = mutableStateOf(360.dp)
        val fontScale = mutableStateOf(1f)
        val matched = mutableStateOf(true)
        val item = ScoutItem(
            item = requireNotNull(ItemCatalog.findById("wand_disintegration")),
            depth = 1, upgrade = 1, effect = null, cursed = true, secret = true,
            source = ScoutItemSource.HEAP,
            accessibility = ScoutAccessibility.Choice(group = 0, option = 0),
        )
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas, LocalDensity provides Density(compose.density.density, fontScale.value)) {
                    ScoutItemCard(item, RingGems.CATALOG, matches = matched.value, modifier = Modifier.width(width.value).testTag("item-card"))
                }
            }
        }
        for (scale in listOf(1f, 1.5f, 2f)) {
            for (cardWidth in listOf(360.dp, 320.dp, 280.dp)) {
                for (isMatch in listOf(true, false)) {
                    compose.runOnIdle {
                        width.value = cardWidth
                        fontScale.value = scale
                        matched.value = isMatch
                    }
                    val title = compose.onNodeWithText(item.item.name)
                    val layouts = mutableListOf<TextLayoutResult>()
                    title.performSemanticsAction(SemanticsActions.GetTextLayoutResult) { it(layouts) }
                    val layout = layouts.single()
                    assertEquals(1, layout.lineCount)
                    assertFalse("Title overflow at $cardWidth / $scale", layout.hasVisualOverflow)
                    assertFalse(layout.isLineEllipsized(0))
                    assertEquals(item.item.name.length, layout.getLineEnd(0))
                    val titleBounds = title.fetchSemanticsNode().boundsInRoot
                    val upgrade = compose.onNodeWithText("+1").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                    assertTrue("Upgrade must follow the title", upgrade.left >= titleBounds.right)
                    assertTrue("Upgrade must remain on the title row", upgrade.top < titleBounds.bottom && upgrade.bottom > titleBounds.top)
                    for (label in listOf("cursed", "secret")) {
                        val badge = compose.onNodeWithText(label).assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                        assertTrue("$label must be below the title", badge.top >= titleBounds.bottom)
                    }
                    if (isMatch) compose.onNodeWithText("match").assertIsDisplayed()
                    val trailing = compose.onNodeWithTag("scout-item-match-choices").fetchSemanticsNode().boundsInRoot
                    val card = compose.onNodeWithTag("item-card").fetchSemanticsNode().boundsInRoot
                    assertEquals("Match and choice chips stay at the right padding", card.right - with(compose.density) { 14.dp.toPx() }, trailing.right, 1f)
                }
            }
        }
        compose.runOnIdle {
            width.value = 360.dp
            fontScale.value = 1.5f
            matched.value = true
        }
        screenshot("item-title-fit")
    }

    private fun screenshot(name: String) {
        compose.waitForIdle()
        System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
        val window = compose.activity.window
        val bounds = compose.onNodeWithTag("item-card").fetchSemanticsNode().boundsInRoot
        val rect = Rect(bounds.left.toInt(), bounds.top.toInt(), ceil(bounds.right).toInt(), ceil(bounds.bottom).toInt())
        val bitmap = Bitmap.createBitmap(rect.width(), rect.height(), Bitmap.Config.ARGB_8888)
        var copyResult: Int? = null
        compose.runOnIdle {
            PixelCopy.request(window, rect, bitmap, { copyResult = it }, Handler(Looper.getMainLooper()))
        }
        compose.waitUntil { copyResult != null }
        assertEquals(PixelCopy.SUCCESS, copyResult)
        val output = File("build/outputs/scout-ui/$name.png")
        output.parentFile?.mkdirs()
        output.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

}
