// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsDisplayed
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.*
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w360dp-h800dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class BlanketRequirementsUiTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    init { PackagedCatalog.install() }

    @Test fun blanketBoardEditsTheOriginalQueryIndex() {
        val requirements = listOf(
            ItemRequirement(1, ItemCatalog.findById("wand_frost")!!, 2),
            ItemRequirement(2, null, 3, kind = ItemKind.WAND, blanket = true,
                source = ScoutItemSource.WANDMAKER_REWARD),
        )
        var editedIndex: Int? = null
        compose.setContent {
            SeedSeekerTheme {
                RequirementBoard(requirements, enabled = true, blanket = true,
                    onChange = {}, onEdit = { _, index -> editedIndex = index }, onRemove = {}, onAdd = {})
            }
        }
        compose.onNodeWithText("Any wand").performClick()
        compose.runOnIdle { assertEquals(1, editedIndex) }
        compose.onNodeWithText(requirements[0].title).assertDoesNotExist()
    }

    @Test fun blanketSheetSavesFiltersWithoutOfferingExtraCopies() {
        val blanket = ItemRequirement(2, null, 3, kind = ItemKind.WAND, blanket = true,
            source = ScoutItemSource.WANDMAKER_REWARD)
        var saved: ItemRequirement? = null
        compose.setContent {
            SeedSeekerTheme {
                RequirementSheet(editing = blanket, onDismiss = {}, onSave = { requirement, count, total, _ ->
                    saved = requirement
                    assertEquals(1, count)
                    assertNull(total)
                })
            }
        }
        compose.onNodeWithText("Edit blanket requirement").assertIsDisplayed()
        compose.onNodeWithText("How many").assertDoesNotExist()
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle { assertEquals(blanket, saved) }
    }
}
