// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.JniNativeSeedFinder
import org.junit.Assert.*
import org.junit.Test

class BlanketRequirementsTest {
    init { PackagedCatalog.install() }

    private fun wand(key: Long, blanket: Boolean = false) = ItemRequirement(
        key, ItemCatalog.findById("wand_frost")!!, 0,
        upgradeMatch = UpgradeMatch.ANY, blanket = blanket,
    )

    @Test fun blanketsSurviveDocumentsLinksResultsAndSavedDrafts() {
        val ordinary = listOf("wand_lightning", "wand_disintegration", "wand_frost").mapIndexed { i, id ->
            ItemRequirement(i + 1L, ItemCatalog.findById(id)!!, 2, upgradeMatch = UpgradeMatch.AT_LEAST)
        }
        val blanket = ItemRequirement(4, null, 3, kind = ItemKind.WAND,
            source = ScoutItemSource.WANDMAKER_REWARD, blanket = true)
        val query = PresetQuery(requirements = ordinary + blanket)
        val document = ResultsExport.encodeQuery(query)
        assertTrue(document.getJSONArray("requirements").getJSONObject(3).getBoolean("blanket"))
        val storage = PresetStorage(MemoryPreferences())
        storage.saveCurrentQuery(query)
        storage.save(listOf(QueryPreset("blankets", "Three wands", query)))
        val restored = listOf(
            ResultsExport.decodeQuery(document),
            DeepLink.decode(DeepLink.encodeLink(query)),
            ResultsExport.decode(ResultsExport.encode(query, emptyList(), "test")).query,
            storage.loadCurrentQuery()!!, storage.load().single().query,
        )
        restored.forEach {
            assertEquals(4, it.requirements.size)
            assertEquals(listOf(false, false, false, true), it.requirements.map { r -> r.blanket })
            assertEquals(ScoutItemSource.WANDMAKER_REWARD, it.requirements.last().source)
            assertEquals(3, it.requirements.last().upgrade)
            assertTrue(RequirementEditor.view(it.requirements).problems.isEmpty())
        }
    }

    @Test fun blanketsNeverBecomeCopiesOrCrossSectionAlternatives() {
        val requirements = listOf(wand(1), wand(2, true), wand(3, true))
        fun edit(rows: List<ItemRequirement>, edit: BoardEdit) = RequirementEditor.board(rows, listOf(edit)).rows
        val board = RequirementEditor.view(requirements)
        assertEquals(listOf(1, 2), listOf(board.ordinaryCount, board.blanketCount))
        assertNull(edit(requirements, BoardEdit.SetCount(2, 3)))
        assertNull(edit(requirements, BoardEdit.Join(source = 1, target = 2)))
        val grouped = edit(requirements, BoardEdit.Join(source = 2, target = 3))!!
        assertEquals(1, RequirementEditor.view(grouped).blanketCount)
        assertTrue(grouped.all { it.identityGroup == null })
        assertTrue(RequirementEditor.view(grouped).problems.isEmpty())
        val decoded = DeepLink.decode(DeepLink.encodeLink(PresetQuery(requirements = grouped))).requirements
        assertEquals(2, decoded.filter { it.blanket }.size)
        assertEquals(2, decoded.slotCount())
        assertEquals(2, RequirementEditor.view(edit(grouped, BoardEdit.Detach(2))!!).blanketCount)
        val saved = edit(requirements, BoardEdit.Save(2, wand(2, true).copy(upgrade = 3, upgradeMatch = UpgradeMatch.EXACT), 1, null, null))!!
        assertEquals(2, saved.count { it.blanket })
    }

    @Test fun invalidBlanketsAreRejectedBeforeSearching() {
        fun problem(rows: List<ItemRequirement>) = RequirementEditor.view(rows).problems.firstOrNull()?.message
        assertEquals("Add at least one ordinary requirement.", problem(listOf(wand(1, true))))
        assertEquals(
            "An either/or group cannot mix ordinary and blanket requirements.",
            problem(listOf(wand(1).copy(alternativeGroup = 1), wand(2, true).copy(alternativeGroup = 1))),
        )
        assertThrows(IllegalArgumentException::class.java) { wand(1, true).copy(identityGroup = 1) }
        assertThrows(IllegalArgumentException::class.java) { wand(1, true).copy(levelSum = LevelSum(1, 3)) }
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(1, ItemCatalog.trinkets.first(), 0, upgradeMatch = UpgradeMatch.ANY,
                blanket = true, selectTrinket = true)
        }
    }

    @Test fun conflictingBlanketStopsBeforeScanningAndReportsImpossible() {
        val requirements = listOf("wand_lightning", "wand_disintegration").mapIndexed { i, id ->
            ItemRequirement(i + 1L, ItemCatalog.findById(id)!!, 2, upgradeMatch = UpgradeMatch.EXACT)
        } + ItemRequirement(3, null, 3, kind = ItemKind.WAND, blanket = true)
        JniNativeSeedFinder().startResumedSearch(
            SearchRequest(requirements = requirements, maximumDepth = 24), 42, 1_000, 1,
        ).use { session ->
            val deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(5)
            var status = session.status()
            while (status.state == SearchState.RUNNING && System.nanoTime() < deadline) {
                Thread.sleep(10)
                status = session.status()
            }
            assertTrue(status.isImpossibleQuery)
            assertEquals(0L, status.scannedSeeds)
            assertTrue(session.poll().results.isEmpty())
            assertEquals(ResumeHint(42, 1_000), session.resumeHint())
        }
        // A filter-only refinement has no scan range; an empty result is not a proof.
        assertFalse(SearchStatus(SearchState.COMPLETED, 0, 0).isImpossibleQuery)
        assertFalse(SearchStatus(SearchState.CANCELLED, 0, 1_000).isImpossibleQuery)
        assertFalse(SearchStatus(SearchState.COMPLETED, 1_000, 1_000).isImpossibleQuery)
    }
}
