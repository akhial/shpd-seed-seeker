package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.JniNativeSeedFinder
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

class FloorRequirementsTest {
    init { PackagedCatalog.install() }

    @Test fun scoutFarmingLabelsUseGeneratedRoomsAndFeelings() {
        val actual = JniNativeSeedFinder().scoutSeed("DJG-HMA-ULY")
        assertEquals(20, actual.floorRooms.size)
        assertTrue(actual.isFarmingFloor(17))
        for (depth in listOf(7, 17, 22, 8)) {
            for (rooms in listOf(setOf("garden"), setOf("secret_garden"), setOf("garden", "secret_garden"),
                setOf("quest_rot_garden"), emptySet())) {
                for (feeling in listOf(FloorFeeling.DARK, FloorFeeling.GRASS)) {
                    val world = actual.copy(floorFeelings = mapOf(depth to feeling), floorRooms = mapOf(depth to rooms))
                    assertEquals(depth != 8 && feeling == FloorFeeling.DARK && rooms.any { it != "quest_rot_garden" },
                        world.isFarmingFloor(depth))
                    assertFalse(world.copy(floorRooms = emptyMap()).isFarmingFloor(depth))
                }
            }
        }
    }

    @Test fun farmingFloorsAreIndependentAndOnlyRaiseTheLimitWhenSelected() {
        var query = PresetQuery(emptyList(), maximumDepth = 4)
        for (depth in listOf(22, 7, 17)) query = query.toggleFarmingFloor(depth)
        assertEquals(22, query.maximumDepth)
        assertEquals(FARMING_FLOORS, query.floorRequirements.map { it.depth })
        assertTrue(query.floorRequirements.all { it.isFarming })
        assertTrue(query.requirements.isEmpty())
        query = query.toggleFarmingFloor(17)
        assertEquals(listOf(7, 22), query.floorRequirements.map { it.depth })
        assertEquals(22, query.maximumDepth)
        assertNotNull(query.floorRequirements.floorValidationProblem(16))
        assertThrows(IllegalArgumentException::class.java) {
            SearchRequest(emptyList(), maximumDepth = 16, floorRequirements = query.floorRequirements)
        }
    }

    @Test fun floorsSurviveDocumentsLinksResultsDraftsAndPresets() {
        val storage = PresetStorage(MemoryPreferences())
        var query = PresetQuery(emptyList(), autoApplyTrinket = false)
        for (depth in FARMING_FLOORS) query = query.toggleFarmingFloor(depth)
        query = query.copy(floorRequirements = query.floorRequirements + FloorRequirement(9,
            FloorFeeling.SECRETS, listOf("secret_library"), listOf("garden", "secret_garden")))
        val request = SearchRequest(emptyList(), floorRequirements = query.floorRequirements)
        assertEquals(4, request.slotCount)
        assertEquals(query, request.toPresetQuery())
        assertEquals(query, ResultsExport.decodeQuery(ResultsExport.encodeQuery(query)))
        assertEquals(query, DeepLink.decode(DeepLink.encodeLink(query)))
        assertEquals(query, ResultsExport.decode(ResultsExport.encode(query, listOf("AAA-AAA-AAA"), "test")).query)
        storage.saveCurrentQuery(query)
        assertEquals(query, storage.loadCurrentQuery())
        storage.save(listOf(QueryPreset(id = "farm", name = "Farm", query = query)))
        assertEquals(query, storage.load().single().query)
        assertEquals(emptyList<FloorRequirement>(), ResultsExport.decodeQuery(JSONObject("""{"requirements":[]}""")).floorRequirements)
    }

    @Test fun nativeFilteringAndScoutingEnforceFloorOnlyConditions() {
        val engine = JniNativeSeedFinder()
        val world = engine.scoutSeed("AAA-AAA-AAA", 0)
        val feeling = world.floorFeelings.getValue(7)
        val query = SearchRequest(emptyList(), floorRequirements = listOf(FloorRequirement(7, feeling)))
        assertEquals(listOf(world.seed), engine.filterSeeds(query, listOf(world.seed)))
        assertEquals(1, engine.scoutMatches(world.seed, 0, query).matchedSlots)
        val rejected = query.copy(floorRequirements = listOf(FloorRequirement(7,
            if (feeling == FloorFeeling.DARK) FloorFeeling.WATER else FloorFeeling.DARK)))
        assertTrue(engine.filterSeeds(rejected, listOf(world.seed)).isEmpty())
        assertEquals(0, engine.scoutMatches(world.seed, 0, rejected).matchedSlots)
        val farm = PresetQuery(emptyList()).toggleFarmingFloor(7)
        val request = SearchRequest(emptyList(), floorRequirements = farm.floorRequirements)
        val estimate = engine.startResumedSearch(request, 0, 0, 1).use { it.status().matchProbability }
        assertTrue(estimate > 0.0 && estimate < 1.0)
    }

    @Test fun floorLinksAndCalibratedEstimatesComposeWithResinPlanning() {
        val engine = JniNativeSeedFinder()
        fun probability(request: SearchRequest) = engine.startResumedSearch(request, 0, 0, 1).use { it.status().matchProbability }
        for ((auto, code) in listOf(false to "sAAAIACfAAA", true to "sAAAIBCfAAA")) {
            val legacy = DeepLink.decode(code)
            assertEquals(listOf(FloorRequirement(7, FloorFeeling.DARK)), legacy.floorRequirements)
            assertEquals(auto, legacy.arcaneResinAuto)
            val query = ResultsExport.decodeQuery(JSONObject("""{"arcane_resin":${if (auto) "\"auto\"" else "4"},"arcane_resin_filter":{"include_mage_wand":true},"requirements":[{"item":"wand_frost","exclude_resin":true},{"item":"wand_frost"}],"floor_requirements":[{"depth":7,"feeling":"dark","any_rooms":["garden","secret_garden"]}]}"""))
            assertEquals(query, DeepLink.decode(DeepLink.encodeLink(query)))
            assertEquals(query, ResultsExport.decode(ResultsExport.encode(query, emptyList(), "test")).query)
            val request = SearchRequest(query.requirements, arcaneResin = query.arcaneResin, arcaneResinAuto = query.arcaneResinAuto, arcaneResinFilter = query.arcaneResinFilter, floorRequirements = query.floorRequirements)
            val floorOnly = SearchRequest(emptyList(), floorRequirements = query.floorRequirements)
            val expected = probability(request.copy(floorRequirements = emptyList())) * probability(floorOnly)
            assertTrue(expected > 0.0)
            assertEquals(expected, probability(request), 1e-12)
        }
    }
}
