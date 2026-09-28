// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import dev.seedseeker.app.catalog.PackagedCatalog
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
