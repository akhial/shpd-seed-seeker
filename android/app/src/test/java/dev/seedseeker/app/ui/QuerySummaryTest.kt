// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.model.TierMatch
import dev.seedseeker.app.model.UpgradeMatch
import org.junit.Assert.assertEquals
import org.junit.Test

class QuerySummaryTest {
    @Test
    fun chipLabelCondensesAnExactItemRequirement() {
        val requirement = ItemRequirement(
            key = 1,
            item = ItemCatalog.weapons.first { it.id == "sword" },
            upgrade = 2,
            modifier = "Lucky",
            maximumDepth = 12,
        )

        assertEquals("Sword +2 · Lucky · ≤12", requirementChipLabel(requirement))
    }

    @Test
    fun chipLabelCondensesAWildcardRequirement() {
        val requirement = ItemRequirement(
            key = 2,
            item = null,
            kind = ItemKind.ARMOR,
            upgrade = 1,
            upgradeMatch = UpgradeMatch.AT_LEAST,
            tierMatch = TierMatch.AT_LEAST,
            tier = 3,
            requireUncursed = true,
            source = ScoutItemSource.GHOST_REWARD,
            identityGroup = 2,
        )

        assertEquals(
            "T3+ armor ≥+1 · uncursed · Ghost reward · grp B",
            requirementChipLabel(requirement),
        )
    }

    @Test
    fun scopeSummaryListsOnlyActiveConstraints() {
        assertEquals(
            "≤ floor 24",
            scopeSummaryText(24, requireBlacksmith = false, excludeBlacksmithRewards = false, fastMode = false, challenges = 0),
        )
        assertEquals(
            "≤ floor 12 · smith · no smith rewards · fast · 2 challenges",
            scopeSummaryText(12, requireBlacksmith = true, excludeBlacksmithRewards = true, fastMode = true, challenges = 0b101),
        )
        assertEquals(
            "≤ floor 1 · 1 challenge",
            scopeSummaryText(1, requireBlacksmith = false, excludeBlacksmithRewards = false, fastMode = false, challenges = 16),
        )
    }
}
