// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
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

    @Test fun blanketBoardEditsTheBlanketRow() {
        val requirements = listOf(
            ItemRequirement(1, ItemCatalog.findById("wand_frost")!!, 2),
            ItemRequirement(2, null, 3, kind = ItemKind.WAND, blanket = true,
                source = ScoutItemSource.WANDMAKER_REWARD),
        )
        val board = RequirementEditor.view(requirements)
        var editedKey: Long? = null
        compose.setContent {
            SeedSeekerTheme {
                RequirementBoard(board, enabled = true, blanket = true,
                    onChange = {}, onEdit = { editedKey = it }, onAdd = {})
            }
        }
        compose.onNodeWithText("Any wand").performClick()
        compose.runOnIdle { assertEquals(2L, editedKey) }
        compose.onNodeWithText("Wand of Frost").assertDoesNotExist()
    }

    @Test fun blanketSheetSavesFiltersWithoutOfferingExtraCopies() {
        val rows = listOf(
            ItemRequirement(1, ItemCatalog.findById("wand_frost")!!, 2),
            ItemRequirement(2, null, 3, kind = ItemKind.WAND, blanket = true,
                source = ScoutItemSource.WANDMAKER_REWARD),
        )
        var saved: SheetSave.Saved? = null
        val sheet = RequirementEditor.open(rows, key = 2)
        compose.setContent {
            SeedSeekerTheme {
                RequirementSheet(sheet, rows, onDismiss = {}, onSaved = { saved = it })
            }
        }
        compose.onNodeWithText("Edit blanket requirement").assertIsDisplayed()
        compose.onNodeWithText("Total item count").assertDoesNotExist()
        compose.onNodeWithText("Exclude from Auto resin").assertDoesNotExist()
        compose.onNodeWithText("Save").performClick()
        // Saving what is already there changes nothing, and lands on the blanket.
        compose.runOnIdle {
            assertNull(saved!!.rows)
            assertEquals(2L, saved!!.focus)
        }
    }

    @Test fun aNewBlanketStartsOnTheFirstOrdinaryKind() {
        val rows = listOf(ItemRequirement(1, null, 2, kind = ItemKind.MELEE_WEAPON))
        var saved: SheetSave.Saved? = null
        val sheet = RequirementEditor.open(rows, blanket = true)
        compose.setContent {
            SeedSeekerTheme {
                RequirementSheet(sheet, rows, nextKey = 5, onDismiss = {}, onSaved = { saved = it })
            }
        }
        compose.onNodeWithText("Add blanket requirement").assertIsDisplayed()
        compose.onNodeWithText("Any melee weapon").assertIsSelected()
        compose.onNodeWithText("Arcane Resin").assertDoesNotExist()
        compose.onNodeWithText("Next").performClick()
        compose.onNodeWithText("Add").performClick()
        compose.runOnIdle {
            val blanket = saved!!.rows!!.last()
            assertEquals(5L, blanket.key)
            assertTrue(blanket.blanket)
            assertEquals(ItemKind.MELEE_WEAPON, blanket.kind)
            assertEquals(5L, saved!!.focus)
        }
    }
}
