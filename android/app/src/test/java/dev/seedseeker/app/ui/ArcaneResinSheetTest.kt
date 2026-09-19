package dev.seedseeker.app.ui

import android.graphics.BitmapFactory
import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.*
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowDialog

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w412dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ArcaneResinSheetTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Test fun editorValidatesWholeAmountsAndPreservesDonorFilters() {
        val filter = ArcaneResinFilter(false, 12, ScoutItemSource.WANDMAKER_REWARD)
        var saved: Pair<Int, ArcaneResinFilter>? = null
        var removed = false
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent { SeedSeekerTheme {
            CompositionLocalProvider(LocalItemAtlas provides atlas) {
                ArcaneResinSheet(6, filter, onDismiss = {}, onSave = { amount, selected -> saved = amount to selected }, onRemove = { removed = true })
            }
        } }
        compose.onNodeWithText("Surplus wands provide", substring = true).assertDoesNotExist()
        val minimumBounds = compose.onNodeWithText("Minimum resin").fetchSemanticsNode().boundsInRoot
        val sourceBounds = compose.onNodeWithText("Wand source").fetchSemanticsNode().boundsInRoot
        assertEquals(sourceBounds.left, minimumBounds.left, 1f)
        assertEquals(sourceBounds.right, minimumBounds.right, 1f)
        compose.captureResinScreenshot("sheet", requireNotNull(ShadowDialog.getLatestDialog().window))
        compose.onNodeWithText("Minimum resin").performTextReplacement("")
        compose.onNodeWithText("Enter a whole number.").assertIsDisplayed()
        compose.onNodeWithText("Minimum resin").performTextReplacement("1.5")
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText("Enter a whole number.").assertIsDisplayed()
        compose.onNodeWithText("Minimum resin").performTextReplacement("65536")
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText("Enter a whole number.").assertIsDisplayed()
        compose.captureResinScreenshot("sheet-error", requireNotNull(ShadowDialog.getLatestDialog().window))
        compose.onNodeWithText("Minimum resin").performTextReplacement("3")
        compose.onNodeWithText("Enter a whole number.").assertDoesNotExist()
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle { assertEquals(3 to filter, saved) }
        compose.onNodeWithText("Remove").performClick()
        compose.runOnIdle { assertTrue(removed) }
    }
}
