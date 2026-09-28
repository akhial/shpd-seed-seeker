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

    @Test fun fixedArtifactsExcludeOnlyRewardsReservedBySearchMatches() {
        val engine = dev.seedseeker.app.engine.JniNativeSeedFinder()
        val world = engine.scoutSeed("AAA-AAA-AAA")
        val natural = setOf("unstable_spellbook", "sandals_of_nature", "alchemists_toolkit", "skeleton_key")
        assertEquals(natural, availableScoutArtifacts(world.items, emptySet()))
        val query = dev.seedseeker.app.model.SearchRequest(listOf(
            dev.seedseeker.app.model.ItemRequirement(key = 1, item = ItemCatalog.findById("ring_haste"), upgrade = 0,
                upgradeMatch = dev.seedseeker.app.model.UpgradeMatch.ANY, source = ScoutItemSource.IMP_REWARD),
            dev.seedseeker.app.model.ItemRequirement(key = 2, item = ItemCatalog.findById("wand_prismatic_light"), upgrade = 0,
                upgradeMatch = dev.seedseeker.app.model.UpgradeMatch.ANY, source = ScoutItemSource.CRYSTAL_CHEST),
        ), autoApplyTrinket = false)
        val marks = engine.scoutMatches(world.seed, 0, query)
        assertEquals(2, marks.matchedSlots)
        assertEquals(setOf("unstable_spellbook", "skeleton_key"), availableScoutArtifacts(world.items, marks.items))
        val own = query.copy(requirements = listOf(dev.seedseeker.app.model.ItemRequirement(key = 3,
            item = ItemCatalog.findById("sandals_of_nature"), upgrade = 0,
            upgradeMatch = dev.seedseeker.app.model.UpgradeMatch.ANY, source = ScoutItemSource.IMP_REWARD)))
        assertEquals(natural, availableScoutArtifacts(world.items, engine.scoutMatches(world.seed, 0, own).items))
    }

    @Test fun onlyConflictingUnmatchedOptionsAreDimmed() {
        val chosen = mapOf(2 to 1)
        assertTrue(isAlternateScoutChoice(ScoutAccessibility.Choice(2, 0), false, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Choice(2, 1), false, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Choice(3, 0), false, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Choice(2, 0), true, chosen))
        assertFalse(isAlternateScoutChoice(ScoutAccessibility.Independent, false, chosen))
    }

    @Test fun excludedSmithRewardsAreDimmedLikeAlternateChoices() {
        val engine = dev.seedseeker.app.engine.JniNativeSeedFinder()
        val world = engine.scoutSeed("AAA-AAA-AAA")
        val smith = world.items.withIndex().filter { it.value.source == ScoutItemSource.BLACKSMITH_REWARD }.map { it.index }.toSet()
        assertTrue("the seed's Blacksmith offers rewards", smith.isNotEmpty())
        val query = dev.seedseeker.app.model.SearchRequest(listOf(
            dev.seedseeker.app.model.ItemRequirement(key = 1, item = ItemCatalog.findById("ring_haste"), upgrade = 0,
                upgradeMatch = dev.seedseeker.app.model.UpgradeMatch.ANY),
        ), excludeBlacksmithRewards = true)
        val marks = engine.scoutMatches(world.seed, 0, query)
        assertEquals(smith, marks.excluded)
        assertTrue(marks.items.none { it in smith })
        val choices = matchedScoutChoices(world.items, marks.items)
        world.items.forEachIndexed { index, item ->
            val alternate = isAlternateScoutChoice(item.accessibility, index in marks.items, choices)
            assertEquals(index in smith || alternate, isDimmedScoutItem(index, item.accessibility, marks, choices))
        }
        // With the option off the same rewards are ordinary rows again.
        val allowed = engine.scoutMatches(world.seed, 0, query.copy(excludeBlacksmithRewards = false))
        assertTrue(allowed.excluded.isEmpty())
        val allowedChoices = matchedScoutChoices(world.items, allowed.items)
        assertTrue(smith.none { isDimmedScoutItem(it, world.items[it].accessibility, allowed, allowedChoices) })
        // Before the engine's marks arrive nothing is dimmed.
        assertFalse(isDimmedScoutItem(smith.first(), world.items[smith.first()].accessibility, null, emptyMap()))
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
