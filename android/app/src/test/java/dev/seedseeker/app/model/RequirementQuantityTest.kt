// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class RequirementQuantityTest {
    private val wand = ItemCatalog.wands.first { it.id == "wand_frost" }

    @Test
    fun coalescesIdenticalCriteriaPreservingFirstKeyThenSortsByFloor() {
        val requirements = listOf(
            ItemRequirement(key = 10, item = wand, upgrade = 2, maximumDepth = 9),
            ItemRequirement(key = 20, item = wand, upgrade = 3),
            ItemRequirement(key = 30, item = wand, upgrade = 2, maximumDepth = 4, quantity = 2),
            ItemRequirement(key = 40, item = wand, upgrade = 2, maximumDepth = 9, quantity = 2),
            ItemRequirement(key = 50, item = wand, upgrade = 2, maximumDepth = 4),
        )

        val displayed = requirements.coalescedAndSorted()

        assertEquals(listOf(20L, 30L, 10L), displayed.map(ItemRequirement::key))
        assertEquals(listOf(1, 3, 3), displayed.map(ItemRequirement::quantity))
        assertEquals(listOf(null, 4, 9), displayed.map(ItemRequirement::maximumDepth))
        assertEquals("3× Wand of Frost", displayed[1].displayTitle)
    }

    @Test
    fun validatesPerRowAndTotalQuantity() {
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(key = 1, item = wand, upgrade = 2, quantity = 0)
        }
        assertThrows(IllegalArgumentException::class.java) {
            SearchRequest(
                requirements = listOf(
                    ItemRequirement(key = 1, item = wand, upgrade = 2, quantity = 33),
                    ItemRequirement(key = 2, item = wand, upgrade = 3, quantity = 32),
                ),
            )
        }
    }

    @Test
    fun quantityDefaultsToOneForExistingCallers() {
        assertEquals(1, ItemRequirement(key = 1, item = wand, upgrade = 2).quantity)
    }
}
