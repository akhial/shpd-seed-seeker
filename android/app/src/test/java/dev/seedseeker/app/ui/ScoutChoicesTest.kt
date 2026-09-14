// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.RingGems
import dev.seedseeker.app.model.ScoutAccessibility
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.model.ScoutWorld
import org.junit.Assert.*
import org.junit.Test

class ScoutChoicesTest {
    init { PackagedCatalog.install() }

    @Test fun onlyConflictingUnmatchedOptionsAreDimmed() {
        val chosen = mapOf(2 to 1)
        assertTrue(isAlternateScoutChoice(ScoutAccessibility.Choice(2, 0), false, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Choice(2, 1), false, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Choice(3, 0), false, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Choice(2, 0), true, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Independent, false, chosen))
    }

    @Test fun floorsIncludeEmptyMapsAndSupportedBossesWithinTheScoutedPrefix() {
        val item = ScoutItem(item = requireNotNull(ItemCatalog.findById("fishing_spear")), depth = 16,
            upgrade = 0, effect = null, cursed = false,
            source = ScoutItemSource.HEAP, accessibility = ScoutAccessibility.Independent)
        val world = ScoutWorld("AAA-AAA-AAA", listOf(item), emptyList(), RingGems.CATALOG)
        val floors = scoutFloors(world)
        assertEquals((1..16).filter { it != 10 }, floors.keys.toList())
        assertTrue(floors.getValue(5).isEmpty())
        assertTrue(floors.getValue(15).isEmpty())
        assertEquals(0, floors.getValue(16).single().index)
    }
}
