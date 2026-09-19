package dev.seedseeker.app.ui

import androidx.activity.ComponentActivity
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

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w600dp-h915dp-xhdpi")
class ArcaneResinSheetTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Test fun editorValidatesWholeAmountsAndPreservesDonorFilters() {
        val filter = ArcaneResinFilter(false, 12, ScoutItemSource.WANDMAKER_REWARD)
        var saved: Pair<Int, ArcaneResinFilter>? = null
        var removed = false
        compose.setContent { SeedSeekerTheme {
            ArcaneResinSheet(6, filter, onDismiss = {}, onSave = { amount, selected -> saved = amount to selected }, onRemove = { removed = true })
        } }
        compose.onNodeWithText("Minimum resin").performTextReplacement("1.5")
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText("Minimum resin").performTextReplacement("65536")
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText("Minimum resin").performTextReplacement("3")
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle { assertEquals(3 to filter, saved) }
        compose.onNodeWithText("Remove").performClick()
        compose.runOnIdle { assertTrue(removed) }
    }
}
