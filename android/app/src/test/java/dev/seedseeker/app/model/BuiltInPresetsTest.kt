// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.PackagedCatalog
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class BuiltInPresetsTest {
    init { PackagedCatalog.install() }

    /**
     * Each preset as the shared JSON query document writes it, the form the
     * other platforms carry the same presets in.
     */
    private val documents = mapOf(
        "disintegrate" to """
            {"max_depth":19,"auto_apply_trinket":true,"requirements":[
              {"item":"wand_disintegration","kind":"wand","upgrade":{"at_least":3}},
              {"item":"wand_disintegration","kind":"wand"},
              {"item":"wand_disintegration","kind":"wand"},
              {"item":"eye_of_newt","kind":"trinket","trinket_transmutations":1},
              {"item":"ring_energy","kind":"ring","upgrade":{"at_least":2}}]}
        """,
        "guerilla-assassin" to """
            {"auto_apply_trinket":true,"requirements":[
              {"item":"assassins_blade","kind":"weapon","effect":"Blooming","max_depth":7,"upgrade":3},
              {"kind":"armor","effect":"Camouflage"},
              {"item":"ring_arcana","kind":"ring","upgrade":{"at_least":2}}]}
        """,
        "ring-of-wealth" to """
            {
             "floor_requirements":[{"depth":17,"feeling":"dark","any_rooms":["garden","secret_garden"]}],
             "requirements":[
              {"item":"ring_wealth","kind":"ring","upgrade":4},
              {"item":"dried_rose","kind":"artifact","max_depth":9},
              {"kind":"armor","max_depth":4,"tier":{"at_most":4},"upgrade":3},
              {"kind":"weapon","max_depth":9,"tier":{"at_most":4},"upgrade":3},
              {"item":"dimensional_sundial","kind":"trinket","trinket_transmutations":1}]}
        """,
        "necromancer" to """
            {"max_depth":14,"wandmaker_quest":"corpse_dust","auto_apply_trinket":true,"requirements":[
              {"item":"wand_corruption","kind":"wand","upgrade":3},
              {"kind":"weapon","tier":{"exact":5},"upgrade":3},
              {"item":"plate_armor","kind":"armor","upgrade":3}]}
        """,
        "blood-berserker" to """
            {"auto_apply_trinket":true,"requirements":[
              {"kind":"weapon","tier":{"exact":5},"upgrade":3,"effect":"Vampiric"},
              {"effect":"Thorns","item":"plate_armor","kind":"armor","upgrade":3},
              {"item":"ring_arcana","kind":"ring","upgrade":4},
              {"item":"chalice_of_blood","kind":"artifact"}]}
        """,
    )

    @Test
    fun presetsComeInTheirMenuOrder() {
        assertEquals(
            listOf("disintegrate", "guerilla-assassin", "ring-of-wealth", "necromancer", "blood-berserker"),
            BuiltInPresets.all.map { it.id },
        )
        assertEquals(
            listOf("DISINTEGRATE", "Guerilla Assassin", "Ring of Wealth", "Necromancer", "Blood Berserker"),
            BuiltInPresets.all.map { it.name },
        )
        assertTrue(BuiltInPresets.all.all { it.isBuiltIn })
        assertEquals(
            listOf(true, true, false, true, true),
            BuiltInPresets.all.map { it.query.autoApplyTrinket },
        )
    }

    /**
     * The engine validates each document strictly and the app's reader maps
     * it, so a preset that matches its import is one the engine accepts.
     */
    @Test
    fun everyPresetIsWhatImportingItsDocumentGives() {
        for (preset in BuiltInPresets.all) {
            val document = """{"format":"seed-seeker-results","format_version":1,"query":${documents.getValue(preset.id)},"results":[]}"""
            assertEquals(preset.name, preset.query, ResultsExport.decode(document).query)
        }
    }

    @Test
    fun everyPresetIsRunnable() {
        for (preset in BuiltInPresets.all) {
            val query = preset.query
            SearchRequest(
                requirements = query.requirements,
                maximumDepth = query.maximumDepth,
                wandmakerQuest = query.wandmakerQuest,
                autoApplyTrinket = query.autoApplyTrinket,
                floorRequirements = query.floorRequirements,
            )
            assertEquals(preset.name, emptyList<RequirementProblem>(), RequirementEditor.view(query.requirements).problems)
        }
    }

    @Test
    fun disintegratePresetStacksThreeDisintegrationWands() {
        val query = BuiltInPresets.disintegrate.query

        assertEquals(19, query.maximumDepth)
        assertEquals(
            listOf("wand_disintegration", "wand_disintegration", "wand_disintegration", "eye_of_newt", "ring_energy"),
            query.requirements.map { it.item?.id },
        )
        assertEquals(
            listOf(UpgradeMatch.AT_LEAST, UpgradeMatch.ANY, UpgradeMatch.ANY, UpgradeMatch.ANY, UpgradeMatch.AT_LEAST),
            query.requirements.map { it.upgradeMatch },
        )
        assertEquals(listOf(3, 0, 0, 0, 2), query.requirements.map { it.upgrade })
        assertEquals(1, query.requirements[3].trinketTransmutations)
    }

    @Test
    fun guerillaAssassinPresetWantsAnEarlyBloomingBlade() {
        val (blade, armor, ring) = BuiltInPresets.guerillaAssassin.query.requirements

        assertEquals("assassins_blade", blade.item?.id)
        assertEquals(EffectFilter.named("Blooming"), blade.effect)
        assertEquals(7, blade.maximumDepth)
        assertEquals(UpgradeMatch.EXACT to 3, blade.upgradeMatch to blade.upgrade)
        assertEquals(null, armor.item)
        assertEquals(EffectFilter.named("Camouflage"), armor.effect)
        assertEquals(UpgradeMatch.ANY, armor.upgradeMatch)
        assertEquals("ring_arcana", ring.item?.id)
        assertEquals(UpgradeMatch.AT_LEAST to 2, ring.upgradeMatch to ring.upgrade)
    }

    @Test
    fun ringOfWealthPresetAsksForAFarmingFloor() {
        val query = BuiltInPresets.ringOfWealth.query
        val requirements = query.requirements

        assertEquals(24, query.maximumDepth)
        assertEquals(listOf(17), query.floorRequirements.map { it.depth })
        assertTrue(query.floorRequirements.single().isFarming)
        // The same floor the toggle writes, so it shows as toggled on.
        assertEquals(query, query.copy(floorRequirements = emptyList()).toggleFarmingFloor(17))
        assertEquals(
            listOf("ring_wealth", "dried_rose", null, null, "dimensional_sundial"),
            requirements.map { it.item?.id },
        )
        assertEquals(listOf(null, 9, 4, 9, null), requirements.map { it.maximumDepth })
        assertEquals(listOf(TierMatch.ANY, TierMatch.ANY, TierMatch.AT_MOST, TierMatch.AT_MOST, TierMatch.ANY), requirements.map { it.tierMatch })
        assertEquals(listOf(4, 0, 3, 3, 0), requirements.map { it.upgrade })
        assertEquals(1, requirements[4].trinketTransmutations)
    }

    @Test
    fun necromancerPresetDemandsTheCorpseDustQuest() {
        val query = BuiltInPresets.necromancer.query

        assertEquals(14, query.maximumDepth)
        assertEquals(WandmakerQuest.CORPSE_DUST, query.wandmakerQuest)
        assertEquals(listOf("wand_corruption", null, "plate_armor"), query.requirements.map { it.item?.id })
        assertEquals(TierMatch.EXACT to 5, query.requirements[1].let { it.tierMatch to it.tier })
        assertTrue(query.requirements.all { it.upgradeMatch == UpgradeMatch.EXACT && it.upgrade == 3 })
    }

    @Test
    fun bloodBerserkerPresetWantsAVampiricTierFiveWeaponAndThornsPlate() {
        val (weapon, armor, ring, chalice) = BuiltInPresets.bloodBerserker.query.requirements

        assertEquals(EffectFilter.named("Vampiric"), weapon.effect)
        assertEquals(TierMatch.EXACT to 5, weapon.tierMatch to weapon.tier)
        assertEquals(3, weapon.upgrade)
        assertEquals("plate_armor", armor.item?.id)
        assertEquals(EffectFilter.named("Thorns"), armor.effect)
        assertEquals(3, armor.upgrade)
        assertEquals("ring_arcana", ring.item?.id)
        assertEquals(4, ring.upgrade)
        assertEquals("chalice_of_blood", chalice.item?.id)
        assertEquals(ItemKind.ARTIFACT, chalice.kind)
    }
}
