package dev.seedseeker.app.ui

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SeedResult
import org.junit.Assert.*
import org.junit.Test

class RefinePlanTest {
    init { PackagedCatalog.install() }
    private val a = SearchRequest(listOf(ItemRequirement(1, ItemCatalog.wands.first(), 1)))
    private val b = SearchRequest(listOf(ItemRequirement(2, ItemCatalog.rings.first(), 1)))
    private val seeds = listOf(SeedResult("AAA-AAA-AAA", 1), SeedResult("BBB-BBB-BBB", 1))

    @Test fun everyQueryChecksTheEntirePool() {
        val pool = TargetState(a, seeds)
        for (query in listOf(a, b, a.copy(maximumDepth = 12))) {
            val refine = refineFor(query, pool, null)!!
            assertEquals(seeds, refine.keepSeeds)
            assertTrue(refine.freshScan)
        }
    }

    @Test fun onlyAnUnchangedQueryResumesItsCursor() {
        val pool = TargetState(a, seeds)
        val previous = FinishedRun(a, 50, 950, seeds.take(1))
        assertEquals(50L, refineFor(a, pool, previous)!!.resumeFrom)
        assertFalse(refineFor(a, pool, previous)!!.freshScan)
        assertTrue(refineFor(b, pool, previous)!!.freshScan)
        assertNull(refineFor(a, null, null))
    }

    @Test fun discoveriesAccumulateAndKeepTheirOriginalRecipeAndSource() {
        val first = settledTarget(null, a, seeds)
        val extra = SeedResult("CCC-CCC-CCC", 1)
        val next = settledTarget(first, b, listOf(seeds[0].copy(selectedTrinket = "mossy_clump"), extra))
        assertEquals(seeds + extra, next.results)
        assertEquals(a, next.sources[seeds[0].seed])
        assertEquals(b, next.sources[extra.seed])
        assertEquals(next.results, settledTarget(next, a, emptyList()).results)
    }
}
