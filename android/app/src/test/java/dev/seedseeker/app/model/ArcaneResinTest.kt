package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.JniNativeSeedFinder
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

class ArcaneResinTest {
    init { PackagedCatalog.install() }

    @Test fun resinOnlyQueriesSurviveEveryPortableAndLocalFormat() {
        val storage = PresetStorage(MemoryPreferences())
        for (amount in listOf(1, 3, 65535)) {
            for (filter in listOf(ArcaneResinFilter(), ArcaneResinFilter(false, 12, ScoutItemSource.WANDMAKER_REWARD))) {
                val query = SearchRequest(emptyList(), arcaneResin = amount, arcaneResinFilter = filter).toPresetQuery()
                assertEquals(query, ResultsExport.decodeQuery(ResultsExport.encodeQuery(query)))
                assertEquals(query, DeepLink.decode(DeepLink.encodeLink(query)))
                assertEquals(query, ResultsExport.decode(ResultsExport.encode(query, listOf("AAA-AAA-AAA"), "test")).query)
                storage.saveCurrentQuery(query)
                assertEquals(query, storage.loadCurrentQuery())
                storage.save(listOf(QueryPreset(id = "resin", name = "Resin", query = query)))
                assertEquals(query, storage.load().single().query)
            }
        }
    }

    @Test fun resinDefaultsValidationAndSlots() {
        assertEquals(ArcaneResinFilter(), ResultsExport.decodeQuery(JSONObject("""{"arcane_resin":6,"requirements":[]}""")).arcaneResinFilter)
        assertEquals(1, SearchRequest(emptyList(), arcaneResin = 6).slotCount)
        assertNull(emptyList<ItemRequirement>().validationProblem(6))
        assertNotNull(emptyList<ItemRequirement>().validationProblem())
        for (amount in listOf(-1, 65536)) assertThrows(IllegalArgumentException::class.java) { SearchRequest(emptyList(), arcaneResin = amount) }
        for (value in listOf("-1", "65536", "1.5", "true", "null", "\"6\"")) {
            assertThrows(IllegalArgumentException::class.java) { ResultsExport.decodeQuery(JSONObject("""{"arcane_resin":$value}""")) }
        }
        for (depth in listOf(0, 25)) assertThrows(IllegalArgumentException::class.java) { ArcaneResinFilter(maximumDepth = depth) }
    }

    @Test fun nativeSearchScoutAndRefinementKeepResin() {
        val engine = JniNativeSeedFinder()
        val query = SearchRequest(emptyList(), arcaneResin = 2)
        assertEquals(listOf("AAA-AAA-AAA"), engine.filterSeeds(query, listOf("AAA-AAA-AAA")))
        val marks = engine.scoutMatches("AAA-AAA-AAA", 0, query)
        assertEquals(1, marks.totalSlots)
        assertEquals(1, marks.matchedSlots)
        assertTrue(marks.items.isNotEmpty())
        val harder = query.copy(arcaneResin = 65535)
        assertTrue(engine.filterSeeds(harder, listOf("AAA-AAA-AAA")).isEmpty())
        assertTrue(engine.queryContinues(harder, query))
        assertFalse(engine.queryContinues(query, harder))
    }
}
