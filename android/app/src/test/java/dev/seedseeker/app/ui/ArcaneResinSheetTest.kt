package dev.seedseeker.app.ui

import android.graphics.BitmapFactory
import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.*
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.RequirementEditor
import dev.seedseeker.app.model.ResinCondition
import dev.seedseeker.app.model.SavedResin
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.model.SheetSave
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
    init { PackagedCatalog.install() }

    private val rows = listOf(ItemRequirement(1, null, 2, kind = ItemKind.WAND))

    /** The resin sheet on the query's [resin], as the app opens it; the last save's condition goes to [saved]. */
    private fun showResinSheet(resin: ResinCondition?, onRemove: (() -> Unit)? = {}, saved: (ResinCondition) -> Unit) {
        val sheet = RequirementEditor.open(rows, resin = resin, openResin = true)
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent { SeedSeekerTheme {
            CompositionLocalProvider(LocalItemAtlas provides atlas) {
                ArcaneResinSheet(sheet, rows, onDismiss = {}, onRemove = onRemove, onSaved = { stored ->
                    // Saving the query's resin leaves the rows as they were.
                    assertNull(stored.rows)
                    saved((stored.resin as SavedResin.Set).condition)
                })
            }
        } }
    }

    @Test fun editorValidatesWholeAmountsAndPreservesDonorFilters() {
        val filter = ArcaneResinFilter(false, 12, ScoutItemSource.WANDMAKER_REWARD)
        var saved: ResinCondition? = null
        var removed = false
        showResinSheet(ResinCondition(6, auto = false, filter), onRemove = { removed = true }) { saved = it }
        compose.onNodeWithText("Surplus wands provide", substring = true).assertDoesNotExist()
        val minimumBounds = compose.onNodeWithText("Minimum resin").fetchSemanticsNode().boundsInRoot
        val sourceBounds = compose.onNodeWithText("Wand source").fetchSemanticsNode().boundsInRoot
        assertEquals(sourceBounds.left, minimumBounds.left, 1f)
        assertEquals(sourceBounds.right, minimumBounds.right, 1f)
        compose.onNodeWithText("Within first 12 floors").assertIsDisplayed()
        compose.onNodeWithText("Wandmaker reward").assertIsDisplayed()
        compose.captureResinScreenshot("sheet", requireNotNull(ShadowDialog.getLatestDialog().window))
        val invalid = "Enter an amount from 1 to 65535."
        compose.onNodeWithText("Minimum resin").performTextReplacement("")
        compose.onNodeWithText(invalid).assertIsDisplayed()
        compose.onNodeWithText("Minimum resin").performTextReplacement("1.5")
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText(invalid).assertIsDisplayed()
        compose.onNodeWithText("Minimum resin").performTextReplacement("65536")
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText(invalid).assertIsDisplayed()
        compose.captureResinScreenshot("sheet-error", requireNotNull(ShadowDialog.getLatestDialog().window))
        compose.onNodeWithText("Minimum resin").performTextReplacement("3")
        compose.onNodeWithText(invalid).assertDoesNotExist()
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle { assertEquals(ResinCondition(3, auto = false, filter), saved) }
        compose.onNodeWithText("Remove").performClick()
        compose.runOnIdle { assertTrue(removed) }
    }

    @Test fun autoCanBeSavedWithInvalidHiddenAmountAndKeepsFilters() {
        val filter = ArcaneResinFilter(false, 12, ScoutItemSource.WANDMAKER_REWARD)
        var saved: ResinCondition? = null
        showResinSheet(ResinCondition(0, auto = true, filter)) { saved = it }
        compose.onNodeWithText("Auto").assertIsSelected()
        compose.onNodeWithText("Minimum resin").assertDoesNotExist()
        compose.onNodeWithText("Remove").assertIsDisplayed()
        compose.onNodeWithText("Amount").performClick()
        compose.onNodeWithText("Minimum resin").performTextReplacement("1.5")
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText("Auto").performClick()
        compose.onNodeWithText("Save").assertIsEnabled().performClick()
        compose.runOnIdle { assertEquals(ResinCondition(0, auto = true, filter), saved) }
        compose.captureResinScreenshot("sheet-auto", requireNotNull(ShadowDialog.getLatestDialog().window))
        compose.onNodeWithText("Amount").performClick()
        compose.onNodeWithText("Minimum resin").performTextReplacement("3")
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle { assertEquals(ResinCondition(3, auto = false, filter), saved) }
    }

    @Test fun startingWandCanBeEnabledInBothResinModes() {
        var saved: ResinCondition? = null
        // A query without resin adds one: amount 2, uncursed donors.
        showResinSheet(null, onRemove = null) { saved = it }
        compose.onNodeWithText("Remove").assertDoesNotExist()
        compose.onNodeWithContentDescription("Include Mage’s starting wand")
            .performScrollTo().assertIsOff().performClick().assertIsOn()
        compose.onNodeWithText("Add").performScrollTo().performClick()
        compose.runOnIdle { assertEquals(ResinCondition(2, auto = false, ArcaneResinFilter(includeMageWand = true)), saved) }
        compose.onNodeWithText("Auto").performScrollTo().performClick()
        compose.onNodeWithText("Add").performScrollTo().performClick()
        compose.runOnIdle { assertEquals(ResinCondition(0, auto = true, ArcaneResinFilter(includeMageWand = true)), saved) }
        compose.captureResinScreenshot("sheet-mage", requireNotNull(ShadowDialog.getLatestDialog().window))
    }

    @Test fun wandExclusionSavesWithoutChangingOtherFilters() {
        val wand = rows.single()
        var saved: SheetSave.Saved? = null
        val sheet = RequirementEditor.open(rows, key = wand.key)
        compose.setContent { SeedSeekerTheme {
            RequirementSheet(sheet, rows, onDismiss = {}, onSaved = { saved = it })
        } }
        compose.onNodeWithText("Exclude from Auto resin").performScrollTo().performClick()
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle {
            assertEquals(listOf(wand.copy(excludeResin = true)), saved!!.rows)
            assertEquals(wand.key, saved!!.focus)
            assertNull(saved!!.resin)
        }
    }
}
