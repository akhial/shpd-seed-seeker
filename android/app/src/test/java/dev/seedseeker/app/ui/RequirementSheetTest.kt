// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.BoardEdit
import dev.seedseeker.app.model.EditorSheet
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.LevelSum
import dev.seedseeker.app.model.RequirementEditor
import dev.seedseeker.app.model.SheetSave
import dev.seedseeker.app.model.UpgradeMatch
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The requirement sheet drawn from the shared editor's form, driven through
 * its controls: every change and save goes through the real engine.
 */
@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w412dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class RequirementSheetTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    init { PackagedCatalog.install() }

    private val frost = ItemRequirement(1, ItemCatalog.findById("wand_frost")!!, 2)

    private fun show(
        sheet: EditorSheet,
        rows: List<ItemRequirement>,
        nextKey: Long? = null,
        onPickResin: (() -> Unit)? = null,
        onSaved: (SheetSave.Saved) -> Unit = {},
    ) = compose.setContent {
        SeedSeekerTheme {
            RequirementSheet(sheet, rows, nextKey, onDismiss = {}, onSaved = onSaved, onPickResin = onPickResin)
        }
    }

    @Test fun aNewRingStackCountsItsLevelsTogether() {
        val rows = listOf(frost)
        var saved: SheetSave.Saved? = null
        show(RequirementEditor.open(rows, offerResin = true), rows, nextKey = 7) { saved = it }
        compose.onNodeWithText("Add requirement").assertIsDisplayed()
        compose.onNodeWithText("Any weapon").assertIsSelected()
        compose.onNodeWithText("Ring").performClick()
        compose.onNodeWithText("Any ring").assertIsSelected()
        compose.onNodeWithText("Ring of Accuracy").performClick()
        compose.onNodeWithText("Next").performClick()
        compose.onNodeWithText("Upgrade").assertIsDisplayed()
        compose.onNodeWithText("Total item count").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("+").performScrollTo().performClick()
        compose.onNodeWithText("×2").assertIsDisplayed()
        // The combined level's help explains its switch, so it shows before the switch is on.
        compose.onNodeWithText("Each item counts its upgrade plus one, and spare items may go unused.")
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Count levels together").performScrollTo().performClick()
        compose.onNodeWithText("≥ 2 across up to 2").performScrollTo().assertIsDisplayed()
        // The combined level speaks for the rings' upgrades.
        compose.onNodeWithText("Upgrade").assertDoesNotExist()
        compose.onNodeWithText("Add").performClick()
        compose.runOnIdle {
            val rings = saved!!.rows!!.drop(1)
            assertEquals(listOf(7L, 8L), rings.map { it.key })
            assertEquals(listOf(LevelSum(1, 2), LevelSum(1, 2)), rings.map { it.levelSum })
            assertEquals(7L, saved!!.focus)
        }
    }

    @Test fun anArmorSheetShowsItsGlyphsAndTheirHelpOnlyWhenPickingThem() {
        val armor = ItemRequirement(1, null, 0, kind = ItemKind.ARMOR, upgradeMatch = UpgradeMatch.ANY)
        show(RequirementEditor.open(listOf(armor), key = 1), listOf(armor))
        compose.onNodeWithText("Glyph").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Enchantment").assertDoesNotExist()
        val help = "Tick the effects the item may carry; none ticked means any."
        compose.onNodeWithText(help).assertDoesNotExist()
        compose.onNodeWithText("Specific…").performScrollTo().performClick()
        compose.onNodeWithText(help).performScrollTo().assertIsDisplayed()
        // The tier slider shows only under a mode that takes a value.
        compose.onNodeWithText("Maximum tier").assertDoesNotExist()
        compose.onNodeWithText("At most").performScrollTo().performClick()
        compose.onNodeWithText("Maximum tier").performScrollTo().assertIsDisplayed()
    }

    @Test fun aTrinketSheetShowsEachHelpTextWithWhatItExplains() {
        val skull = ItemRequirement(1, ItemCatalog.findById("rat_skull")!!, 0, upgradeMatch = UpgradeMatch.ANY)
        show(RequirementEditor.open(listOf(skull), key = 1), listOf(skull))
        compose.onNodeWithText("Applies after the first brewing opportunity", substring = true).performScrollTo().assertIsDisplayed()
        // The transmutation limit's help describes the limit, so it shows while the limit is on.
        val limit = "Matches an initial offer or any of the next"
        compose.onNodeWithText(limit, substring = true).assertDoesNotExist()
        compose.onNodeWithText("Allow transmutations").performScrollTo().performClick()
        compose.onNodeWithText(limit, substring = true).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("At most 1").assertIsDisplayed()
    }

    @Test fun arcaneResinIsHandedToItsOwnSheet() {
        var picked = false
        show(RequirementEditor.open(listOf(frost), offerResin = true), listOf(frost), onPickResin = { picked = true })
        compose.onNodeWithText("Arcane Resin").assertDoesNotExist()
        compose.onNodeWithText("Wand").performClick()
        compose.onNodeWithText("Arcane Resin").performClick()
        compose.runOnIdle { assertTrue(picked) }
    }

    @Test fun anImportedTierOneItemKeepsItsTile() {
        // Starting gear is never offered, but a row that already names it shows it.
        val starter = ItemCatalog.weapons.first { it.tier == 1 }
        val rows = listOf(ItemRequirement(1, starter, 1))
        var saved: SheetSave.Saved? = null
        show(RequirementEditor.open(rows, key = 1), rows) { saved = it }
        compose.onNodeWithText("Back").performClick()
        compose.onNodeWithText(starter.name).assertIsSelected()
        compose.onNodeWithText("Next").performClick()
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle { assertNull(saved!!.rows) }
    }

    @Test fun aStackedMemberCannotLeaveItsStacksCategory() {
        val spear = ItemRequirement(0, ItemCatalog.findById("spear")!!, 2)
        val stacked = RequirementEditor.board(
            emptyList(), listOf(BoardEdit.Save(null, spear, count = 2, total = null, copyDepth = null)), nextKey = 1,
        ).rows!! + ItemRequirement(3, ItemCatalog.findById("mace")!!, 2)
        val cluster = RequirementEditor.board(stacked, listOf(BoardEdit.Join(source = 3, target = 1))).rows!!
        var saved: SheetSave.Saved? = null
        show(RequirementEditor.open(cluster, key = 3), cluster) { saved = it }
        compose.onNodeWithText("Edit alternative").assertIsDisplayed()
        compose.onNodeWithText("Total item count").assertDoesNotExist()
        compose.onNodeWithText("Back").performClick()
        compose.onNodeWithText("Trinket").performClick()
        compose.onNodeWithText("Next").performClick()
        compose.onNodeWithText("Copies can only be grouped with the same item type.").assertIsDisplayed()
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.runOnIdle { assertNull(saved) }
    }
}
