// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.util.AtomicFile
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.ResumeHint
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.model.toPresetQuery
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35])
class SearchCheckpointTest {
    @get:Rule val temporary = TemporaryFolder()
    init { PackagedCatalog.install() }
    private val request = SearchRequest(listOf(ItemRequirement(1, ItemCatalog.wands.first(), 1)), autoApplyTrinket = true, arcaneResin = 6,
        floorRequirements = listOf(dev.seedseeker.app.model.FloorRequirement(7, dev.seedseeker.app.model.FloorFeeling.DARK,
            anyRooms = listOf("garden", "secret_garden"))),
        arcaneResinFilter = dev.seedseeker.app.model.ArcaneResinFilter(false, 12, dev.seedseeker.app.model.ScoutItemSource.WANDMAKER_REWARD))
    private val results = List(1_200) { SeedResult("seed-$it", 1, if (it % 2 == 0) "mossy_clump" else null) }

    @Test fun roundTripKeepsUncappedPoolSourcesAndRefineRecipes() {
        for (automatic in listOf(false, true)) {
            val request = request.copy(arcaneResin = if (automatic) 0 else 6, arcaneResinAuto = automatic)
            val targetRequest = request.copy(maximumDepth = 12)
            val saved = SearchSnapshot(
                results = results, query = request.toPresetQuery(),
                status = SearchStatus(SearchState.RUNNING, 4_000, 50_000, matchProbability = 0.003),
                elapsedSeconds = 27,
                target = TargetState(targetRequest, results.reversed(), sources = mapOf(results.first().seed to request)),
                lastRun = FinishedRun(request, 123, 50_000, results),
                pending = PendingSearch(request, 3,
                    RefineSpec(123, 50_000, results, targetRequest, sources = mapOf(results.first().seed to request))),
            )
            val restored = SearchCheckpointCodec.decode(JSONObject(SearchCheckpointCodec.encode(saved).toString()))
            assertEquals(saved, restored)
            assertEquals(1_200, restored.target?.results?.size)
        }
    }

    @Test fun atomicFileRetainsThePreviousCheckpointAfterAnInterruptedWrite() {
        val path = temporary.root.resolve("search.json")
        val saved = SearchSnapshot(results = results, query = request.toPresetQuery(),
            pending = PendingSearch(request, 2, window = ResumeHint(100, 900), scanMatches = 12))
        FileSearchCheckpointStore(path, "engine-1").save(saved)
        AtomicFile(path).startWrite().use { it.write("{incomplete checkpoint".toByteArray()) }
        assertEquals(saved, FileSearchCheckpointStore(path, "engine-1").load())
    }

    @Test fun changedEngineRestoresResultsWithoutResumingOldCoverage() {
        val path = temporary.root.resolve("search.json")
        FileSearchCheckpointStore(path, "old-engine").save(SearchSnapshot(
            results = results, query = request.toPresetQuery(),
            target = TargetState(request, results),
            pending = PendingSearch(request, 2),
        ))
        val restored = FileSearchCheckpointStore(path, "new-engine").load()
        assertEquals(results, restored.results)
        assertEquals(results, restored.target?.results)
        assertNull(restored.lastRun)
        assertNull(restored.pending)
        assertNotNull(restored.error)
    }

    @Test fun corruptCheckpointIsReportedInsteadOfStartingAnUnrelatedSearch() {
        val path = temporary.newFile("search.json")
        path.writeText("{broken")
        assertThrows(Exception::class.java) { FileSearchCheckpointStore(path, "engine").load() }
    }

    @Test fun freshScanSurvivesRecovery() {
        val saved = SearchSnapshot(
            results = results, query = request.toPresetQuery(),
            target = TargetState(request, results),
            pending = PendingSearch(request, 2,
                RefineSpec(0, 0, results, request, freshScan = true), scanLimit = 7),
        )
        assertEquals(saved, SearchCheckpointCodec.decode(SearchCheckpointCodec.encode(saved)))
        // Older checkpoints did not distinguish an import from an exhausted scan.
        val legacy = SearchCheckpointCodec.encode(saved)
        legacy.getJSONObject("pending").remove("scanLimit")
        legacy.getJSONObject("pending").getJSONObject("refine").remove("freshScan")
        val restored = SearchCheckpointCodec.decode(legacy)
        assertFalse(restored.pending!!.refine!!.freshScan)
        assertEquals(RESULT_CAP, restored.pending.scanLimit)
    }
    @Test fun legacyRoutingCheckpointKeepsBothCollectionsAndRestartsCoverage() {
        val other = request.copy(maximumDepth = 12)
        val saved = SearchSnapshot(
            results = results.takeLast(1), query = other.toPresetQuery(),
            target = TargetState(request, results.take(1)),
            lastRun = FinishedRun(other, 100, 900, results.takeLast(1)),
            pending = PendingSearch(other, 2, window = ResumeHint(100, 900)),
        )
        val legacy = SearchCheckpointCodec.encode(saved).put("kind", "DETACHED")
        legacy.getJSONObject("pending").put("mode", "CONTINUE_DETACHED")
        val restored = SearchCheckpointCodec.decode(legacy)
        assertEquals(results.take(1) + results.takeLast(1), restored.target!!.results)
        assertEquals(request, restored.target.sources[results.first().seed])
        assertEquals(other, restored.target.sources[results.last().seed])
        assertNull(restored.lastRun)
        assertNull(restored.pending!!.window)
        assertTrue(restored.pending.refine!!.freshScan)
        assertEquals(restored.target.results, restored.pending.refine.keepSeeds)
    }

}
