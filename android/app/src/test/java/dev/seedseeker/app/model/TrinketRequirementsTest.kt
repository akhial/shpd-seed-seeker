// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import org.junit.Assert.*
import org.junit.Test

class TrinketRequirementsTest {
    init { PackagedCatalog.install() }

    @Test fun namedTrinketsJoinAnOrGroupAndEncode() {
        val requirements = ItemCatalog.trinkets.take(2).mapIndexed { index, item ->
            ItemRequirement(key = index.toLong(), item = item, upgrade = 0, upgradeMatch = UpgradeMatch.ANY)
        }.joinAlternatives(0, 1)
        assertEquals(1, requirements.slotCount())
        assertEquals("Trinket", requirements.first().description)
        // Joining moves the dragged source after the target, just like other categories.
        assertEquals(listOf("Parchment Scrap", "Rat Skull"), requirements.map { it.title })
        assertNull(requirements.validationProblem())
        val document = ResultsExport.encodeQuery(SearchRequest(requirements))
        assertTrue(document.toString().contains("any_of"))
        assertTrue(document.toString().contains("rat_skull"))
        assertFalse(document.toString().contains("upgrade"))
    }

    @Test fun selectedTrinketSurvivesCanonicalQueryRoundTrip() {
        val selected = ItemRequirement(
            key = 0, item = ItemCatalog.trinkets.first(), upgrade = 0,
            upgradeMatch = UpgradeMatch.ANY, selectTrinket = true,
        )
        val document = ResultsExport.encodeQuery(SearchRequest(listOf(selected)))
        assertTrue(document.getJSONArray("requirements").getJSONObject(0).getBoolean("select_trinket"))
        assertTrue(ResultsExport.decodeQuery(document).requirements.single().selectTrinket)
        val preset = ResultsExport.decodeQuery(document)
        assertTrue(DeepLink.decode(DeepLink.encodeLink(preset)).requirements.single().selectTrinket)
        assertTrue(ResultsExport.decode(ResultsExport.encode(preset, emptyList(), "test")).query.requirements.single().selectTrinket)
        assertEquals("Trinket", selected.description)
        val plain = ResultsExport.encodeQuery(SearchRequest(listOf(selected.copy(selectTrinket = false))))
        assertFalse(plain.toString().contains("select_trinket"))
        assertFalse(ResultsExport.decodeQuery(plain).requirements.single().selectTrinket)
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(key = 0, item = ItemCatalog.weapons.first(), upgrade = 1, selectTrinket = true)
        }
    }

    @Test fun transmutationLimitSurvivesEveryQueryFormatAndScoutsTheDeck() {
        val requirement = ItemRequirement(key = 1, item = ItemCatalog.trinkets.first { it.id == "rat_skull" },
            upgrade = 0, upgradeMatch = UpgradeMatch.ANY, trinketTransmutations = 11)
        val query = SearchRequest(listOf(requirement), autoApplyTrinket = true)
        assertEquals(query.toPresetQuery(), ResultsExport.decodeQuery(ResultsExport.encodeQuery(query)))
        assertEquals(query.toPresetQuery(), DeepLink.decode(DeepLink.encodeLink(query.toPresetQuery())))
        assertEquals(query.toPresetQuery(), ResultsExport.decode(ResultsExport.encode(query.toPresetQuery(), emptyList(), "test")).query)
        val storage = PresetStorage(MemoryPreferences())
        storage.saveCurrentQuery(query.toPresetQuery())
        assertEquals(query.toPresetQuery(), storage.loadCurrentQuery())
        val marks = dev.seedseeker.app.engine.JniNativeSeedFinder().scoutMatches("AAA-AAA-AAA", 0, query)
        assertEquals(1, marks.matchedSlots)
        assertTrue(marks.items.isEmpty())
        assertEquals(setOf(10), marks.transmutedTrinkets)
        for (count in listOf(-1, 14)) assertThrows(IllegalArgumentException::class.java) {
            requirement.copy(trinketTransmutations = count)
        }
        assertThrows(IllegalArgumentException::class.java) { requirement.copy(selectTrinket = true) }
    }

    @Test fun artifactLimitSurvivesEveryQueryFormatAndScoutsTheDeck() {
        val requirement = ItemRequirement(key = 1, item = ItemCatalog.artifacts.first { it.id == "ethereal_chains" },
            upgrade = 0, upgradeMatch = UpgradeMatch.ANY, artifactTransmutations = 4)
        val query = SearchRequest(listOf(requirement), maximumDepth = 19, autoApplyTrinket = false)
        assertEquals(query.toPresetQuery(), ResultsExport.decodeQuery(ResultsExport.encodeQuery(query)))
        assertEquals(query.toPresetQuery(), DeepLink.decode(DeepLink.encodeLink(query.toPresetQuery())))
        assertEquals(query.toPresetQuery(), ResultsExport.decode(ResultsExport.encode(query.toPresetQuery(), emptyList(), "test")).query)
        val storage = PresetStorage(MemoryPreferences())
        storage.saveCurrentQuery(query.toPresetQuery())
        assertEquals(query.toPresetQuery(), storage.loadCurrentQuery())
        val marks = dev.seedseeker.app.engine.JniNativeSeedFinder().scoutMatches("AAA-AAA-AAA", 0, query)
        assertEquals(1, marks.matchedSlots)
        assertEquals(1, marks.items.size)
        assertEquals(setOf(19 to 3), marks.transmutedArtifacts)
        val world = dev.seedseeker.app.engine.JniNativeSeedFinder().scoutSeed("AAA-AAA-AAA")
        assertEquals(11, world.artifactDecks.getValue(9).size)
        assertEquals("ethereal_chains", world.artifactDecks.getValue(19)[3].id)
        for (count in listOf(-1, 11)) assertThrows(IllegalArgumentException::class.java) {
            requirement.copy(artifactTransmutations = count)
        }
        assertThrows(IllegalArgumentException::class.java) { requirement.copy(selectTrinket = true) }
    }

    @Test fun wildcardTrinketIsRejected() {
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(key = 0, item = null, kind = ItemKind.TRINKET, upgrade = 0, upgradeMatch = UpgradeMatch.ANY)
        }
    }
}
