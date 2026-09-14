// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.Rect
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.onNodeWithTag
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
                }
            }
        }
        compose.runOnIdle {
            width.value = 360.dp
            fontScale.value = 1.5f
            matched.value = true
        }
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
        val output = File("build/outputs/scout-ui/item-title-fit.png")
        output.parentFile?.mkdirs()
        output.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

}
