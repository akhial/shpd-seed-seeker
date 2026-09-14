// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
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

    @Test fun matchCollapsesToPreserveTitleAndExpandsWhenSpaceReturns() {
        val width = mutableStateOf(360.dp)
        val item = ScoutItem(
            item = requireNotNull(ItemCatalog.findById("wand_fireblast")),
            depth = 1, upgrade = 2, effect = null, cursed = true,
            source = ScoutItemSource.HEAP,
            accessibility = ScoutAccessibility.Choice(group = 0, option = 0),
        )
        compose.setContent {
            SeedSeekerTheme {
                ScoutItemCard(item, RingGems.CATALOG, matches = true, modifier = Modifier.width(width.value))
            }
        }
        compose.onNodeWithText("match").assertDoesNotExist()
        compose.onNodeWithContentDescription("match").assertIsDisplayed()
        val layouts = mutableListOf<TextLayoutResult>()
        compose.onNodeWithText(item.item.name).performSemanticsAction(SemanticsActions.GetTextLayoutResult) { it(layouts) }
        assertEquals(1, layouts.single().lineCount)
        compose.runOnIdle { width.value = 560.dp }
        compose.onNodeWithText("match").assertIsDisplayed()
        compose.runOnIdle { width.value = 360.dp }
        compose.onNodeWithText("match").assertDoesNotExist()
        compose.onNodeWithContentDescription("match").assertIsDisplayed()
    }
}
