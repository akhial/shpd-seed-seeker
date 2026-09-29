// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import org.junit.Assert.*
import org.junit.Test

class ArtifactRequirementsTest {
    init { PackagedCatalog.install() }

    @Test fun scoutArtifactsShowRoundedGameLevels() {
        for (item in ItemCatalog.artifacts) {
            val expected = when (item.id) {
                "sandals_of_nature" -> 7
                "ethereal_chains", "timekeepers_hourglass" -> 6
                else -> 5
            }
            val scout = ScoutItem(item, 19, 5, null, false,
                source = ScoutItemSource.IMP_REWARD, accessibility = ScoutAccessibility.Independent)
            assertEquals(expected, scout.displayedUpgrade)
            assertEquals(5, scout.upgrade)
            assertEquals(0, scout.copy(upgrade = 0).displayedUpgrade)
        }
    }


    private fun PresetQuery.normalized() = copy(requirements = requirements.map { it.copy(key = 0) })

    private fun artifact(key: Long = 1) = ItemRequirement(
        key = key, item = ItemCatalog.artifacts.first(), upgrade = 0,
        upgradeMatch = UpgradeMatch.ANY, maximumDepth = 19,
    )

    @Test fun artifactsRequireNamesAndCannotStack() {
        assertEquals(7, ItemKind.ARTIFACT.ordinal)
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(1, null, 0, kind = ItemKind.ARTIFACT, upgradeMatch = UpgradeMatch.ANY)
        }
        assertThrows(IllegalArgumentException::class.java) { artifact().copy(identityGroup = 1) }
        assertThrows(IllegalArgumentException::class.java) { artifact().copy(levelSum = LevelSum(1, 2)) }
        val requirements = listOf(artifact())
        assertEquals(1, RequirementEditor.view(requirements).items.single().chips.single().stack.count)
        assertNull(RequirementEditor.board(requirements, listOf(BoardEdit.SetCount(1, 2))).rows)
        assertEquals(2, RequirementEditor.view(listOf(artifact(), artifact(2))).items.size)
    }

    @Test fun vaultUpgradeAndFloorLimitsSurviveDocumentsAndShareLinks() {
        val requirement = artifact().copy(
            upgrade = 5, upgradeMatch = UpgradeMatch.EXACT,
            source = ScoutItemSource.IMP_REWARD, requireUncursed = true,
        )
        assertEquals(
            listOf("exactly +5", "uncursed", "Imp reward", "floors 1–19"),
            RequirementEditor.view(listOf(requirement)).items.single().chips.single().details,
        )
        assertThrows(IllegalArgumentException::class.java) { requirement.copy(upgrade = 6) }
        val query = PresetQuery(requirements = listOf(requirement)).normalized()
        assertEquals(query, ResultsExport.decodeQuery(ResultsExport.encodeQuery(query)).normalized())
        assertEquals(query, DeepLink.decode(DeepLink.encodeLink(query)).normalized())
        val file = ResultsExport.encode(query, listOf("AAA-AAA-AAA"), "test")
        assertEquals(query, ResultsExport.decode(file).query.normalized())
    }

    @Test fun artifactAlternativesKeepOneSlotAndEachFloorLimit() {
        val requirements = RequirementEditor.board(
            listOf(artifact(), artifact(2).copy(item = ItemCatalog.artifacts[1], maximumDepth = 9)),
            listOf(BoardEdit.Join(source = 1, target = 2)),
        ).rows!!
        assertEquals(1, requirements.slotCount())
        // The dragged source moves after its target; each keeps its floor limit.
        assertEquals(listOf(9, 19), requirements.map { it.maximumDepth })
        assertTrue(RequirementEditor.view(requirements).problems.isEmpty())
        assertNull(RequirementEditor.board(requirements, listOf(BoardEdit.SetCount(2, 2))).rows)
        val query = PresetQuery(requirements = requirements).normalized()
        assertEquals(query, DeepLink.decode(DeepLink.encodeLink(query)).normalized())
    }
}
