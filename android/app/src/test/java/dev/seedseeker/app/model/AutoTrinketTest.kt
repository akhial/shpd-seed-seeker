package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.JniNativeSeedFinder
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

class AutoTrinketTest {
    init { PackagedCatalog.install() }
    private fun query() = SearchRequest(listOf(ItemRequirement(
        key = 1, item = ItemCatalog.findById("runic_blade"), upgrade = 1,
        effect = EffectFilter.named("Grim"), kind = ItemKind.WEAPON, upgradeMatch = UpgradeMatch.EXACT,
    )), maximumDepth = 19, autoApplyTrinket = true)

    @Test fun defaultsAndRecipeDocumentsRetainExplicitNone() {
        assertTrue(PresetQuery(emptyList()).autoApplyTrinket)
        assertFalse(ResultsExport.decodeQuery(JSONObject("""{"requirements":[{"item":"runic_blade"}]}""")).autoApplyTrinket)
        val preset = ResultsExport.decodeQuery(ResultsExport.encodeQuery(query()))
        assertTrue(preset.autoApplyTrinket)
        assertTrue(DeepLink.decode(DeepLink.encodeLink(preset)).autoApplyTrinket)
        val imported = ResultsExport.decode(ResultsExport.encode(preset,
            listOf("SRU-YSU-QHS", "EYY-RUL-LQG"), "test", listOf("parchment_scrap", null)))
        assertEquals(listOf("parchment_scrap", null), imported.trinkets)
    }

    @Test fun nativeRefinementStripsAndRestoresOnlyNeededTrinkets() {
        val engine = JniNativeSeedFinder(); val query = query()
        val matches = engine.filterRecipes(query, query, listOf(
            SeedResult("SRU-YSU-QHS", 1, "parchment_scrap"), SeedResult("EYY-RUL-LQG", 1, "parchment_scrap"),
        ))
        assertEquals(listOf("parchment_scrap", null), matches.map { it.selectedTrinket })
        val refined = query.copy(requirements = query.requirements + ItemRequirement(
            key = 2, item = ItemCatalog.findById("whip"), effect = EffectFilter.named("Venomous"), upgrade = 0, upgradeMatch = UpgradeMatch.ANY,
        ))
        assertEquals("parchment_scrap", engine.filterRecipes(refined, query, listOf(matches[1])).single().selectedTrinket)
    }
}
