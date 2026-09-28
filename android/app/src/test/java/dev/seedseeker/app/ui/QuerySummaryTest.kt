// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.LevelSum
import dev.seedseeker.app.model.RequirementEditor
import dev.seedseeker.app.model.ResinCondition
import dev.seedseeker.app.model.UpgradeMatch
import dev.seedseeker.app.model.WandmakerQuest
import org.junit.Assert.assertEquals
import org.junit.Test

class QuerySummaryTest {
    init { PackagedCatalog.install() }

    @Test
    fun scoutMatchTextCountsRequirements() {
        assertEquals("1 of 2 requirements", scoutMatchText(1, 2))
        assertEquals("1 of 1 requirement", scoutMatchText(1, 1))
    }

    /** The collapsed board reads the requirement editor's names, badges and resin tag. */
    @Test
    fun requirementsSummaryNamesEachSlotAndTheResin() {
        val might = ItemCatalog.findById("ring_might")!!
        val energy = ItemCatalog.findById("ring_energy")!!
        val rows = listOf(
            ItemRequirement(1, might, 2),
            ItemRequirement(2, might, 0, upgradeMatch = UpgradeMatch.ANY),
            ItemRequirement(3, ItemCatalog.findById("wand_frost")!!, 2, alternativeGroup = 1),
            ItemRequirement(4, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY, alternativeGroup = 1),
            ItemRequirement(5, energy, 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 3)),
            ItemRequirement(6, energy, 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 3)),
            ItemRequirement(7, null, 0, kind = ItemKind.MELEE_WEAPON, upgradeMatch = UpgradeMatch.ANY, blanket = true),
        )
        val slots = "Ring of Might ×2 · Wand of Frost or Any wand · Ring of Energy ≤2 · Blanket: Any melee"
        assertEquals(slots, requirementsSummaryText(RequirementEditor.view(rows)))
        assertEquals(
            "$slots · ≥6 Arcane Resin",
            requirementsSummaryText(RequirementEditor.view(rows, ResinCondition.of(6, auto = false, ArcaneResinFilter()))),
        )
        assertEquals(
            "Auto Arcane Resin",
            requirementsSummaryText(
                RequirementEditor.view(emptyList(), ResinCondition.of(0, auto = true, ArcaneResinFilter(includeMageWand = true))),
            ),
        )
        assertEquals("", requirementsSummaryText(RequirementEditor.view(emptyList())))
    }

    @Test
    fun scopeSummaryListsOnlyActiveConstraints() {
        assertEquals(
            "≤ floor 24",
            scopeSummaryText(24, requireBlacksmith = false, excludeBlacksmithRewards = false, challenges = 0),
        )
        assertEquals(
            "≤ floor 12 · smith · no smith rewards · 2 challenges",
            scopeSummaryText(12, requireBlacksmith = true, excludeBlacksmithRewards = true, challenges = 0b101),
        )
        assertEquals(
            "≤ floor 1 · 1 challenge",
            scopeSummaryText(1, requireBlacksmith = false, excludeBlacksmithRewards = false, challenges = 16),
        )
        assertEquals(
            "≤ floor 9 · Corpse Dust",
            scopeSummaryText(
                9,
                requireBlacksmith = false,
                excludeBlacksmithRewards = false,
                wandmakerQuest = WandmakerQuest.CORPSE_DUST,
                challenges = 0,
            ),
        )
    }
}
