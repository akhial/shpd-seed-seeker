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
        arcaneResinFilter = dev.seedseeker.app.model.ArcaneResinFilter(false, 12, dev.seedseeker.app.model.ScoutItemSource.WANDMAKER_REWARD))
    private val results = List(1_200) { SeedResult("seed-$it", 1, if (it % 2 == 0) "mossy_clump" else null) }

    @Test fun roundTripKeepsUncappedTargetDetachedHistoryAndRefineRecipes() {
        val targetRequest = request.copy(maximumDepth = 12)
        val saved = SearchSnapshot(
            results = results, query = request.toPresetQuery(),
            status = SearchStatus(SearchState.RUNNING, 4_000, 50_000, matchProbability = 0.003),
            elapsedSeconds = 27,
            target = TargetState(targetRequest, results.reversed(), 55, 90_000),
            lastRun = FinishedRun(request, 123, 50_000, results),
            lastKind = StartMode.DETACHED,
            pending = PendingSearch(request, StartMode.CONTINUE_DETACHED, 3,
                RefineSpec(123, 50_000, results, request)),
        )
        val restored = SearchCheckpointCodec.decode(JSONObject(SearchCheckpointCodec.encode(saved).toString()))
        assertEquals(saved, restored)
        assertEquals(1_200, restored.target?.results?.size)
    }

    @Test fun atomicFileRetainsThePreviousCheckpointAfterAnInterruptedWrite() {
        val path = temporary.root.resolve("search.json")
        val saved = SearchSnapshot(results = results, query = request.toPresetQuery(),
            pending = PendingSearch(request, StartMode.ANCHOR, 2, window = ResumeHint(100, 900), scanMatches = 12))
        FileSearchCheckpointStore(path, "engine-1").save(saved)
        AtomicFile(path).startWrite().use { it.write("{incomplete checkpoint".toByteArray()) }
        assertEquals(saved, FileSearchCheckpointStore(path, "engine-1").load())
    }

    @Test fun changedEngineRestoresResultsWithoutResumingOldCoverage() {
        val path = temporary.root.resolve("search.json")
        FileSearchCheckpointStore(path, "old-engine").save(SearchSnapshot(
            results = results, query = request.toPresetQuery(),
            target = TargetState(request, results, 123, 90_000),
            pending = PendingSearch(request, StartMode.ANCHOR, 2),
        ))
        val restored = FileSearchCheckpointStore(path, "new-engine").load()
        assertEquals(results, restored.results)
        assertEquals(0L, restored.target?.remaining)
        assertNull(restored.pending)
        assertNotNull(restored.error)
    }

    @Test fun corruptCheckpointIsReportedInsteadOfStartingAnUnrelatedSearch() {
        val path = temporary.newFile("search.json")
        path.writeText("{broken")
        assertThrows(Exception::class.java) { FileSearchCheckpointStore(path, "engine").load() }
    }
}
