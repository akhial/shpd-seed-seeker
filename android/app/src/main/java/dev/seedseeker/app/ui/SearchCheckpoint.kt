// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.util.AtomicFile
import dev.seedseeker.app.model.PresetQuery
import dev.seedseeker.app.model.ResultsExport
import dev.seedseeker.app.model.ResumeHint
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.SeedResult
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** A pending filter, or a scan starting at the last fully drained native checkpoint. */
internal data class PendingSearch(
    val request: SearchRequest,
    val mode: StartMode,
    val workers: Int,
    val refine: RefineSpec? = null,
    val window: ResumeHint? = null,
    val scanned: Long = 0,
    val total: Long = 0,
    val scanMatches: Int = 0,
)

internal data class SearchSnapshot(
    val results: List<SeedResult> = emptyList(),
    val query: PresetQuery? = null,
    val status: SearchStatus? = null,
    val elapsedSeconds: Long = 0,
    val target: TargetState? = null,
    val lastRun: FinishedRun? = null,
    val lastKind: StartMode? = null,
    val pending: PendingSearch? = null,
    val error: String? = null,
)

internal interface SearchCheckpointStore {
    fun load(): SearchSnapshot
    fun save(snapshot: SearchSnapshot)
}

/** Private, uncapped session storage: exports intentionally discard coverage and cap results. */
internal class FileSearchCheckpointStore(file: File, private val engineVersion: String) : SearchCheckpointStore {
    private val file = AtomicFile(file)

    override fun load(): SearchSnapshot {
        val text = try {
            file.openRead().bufferedReader().use { it.readText() }
        } catch (_: java.io.FileNotFoundException) {
            return SearchSnapshot()
        }
        val document = JSONObject(text)
        require(document.getInt("schema") == 1) { "Unsupported saved search format." }
        val saved = SearchCheckpointCodec.decode(document.getJSONObject("search"))
        if (document.getString("engine") == engineVersion) return saved
        // Never carry traversal coverage across a game/engine update. Keep the user's finds.
        return saved.copy(
            pending = null, lastRun = null, lastKind = null,
            target = saved.target?.copy(resumeFrom = 0, remaining = 0),
            status = null,
            error = "The engine changed. Saved results were restored; start a search to recheck them.",
        )
    }

    override fun save(snapshot: SearchSnapshot) {
        val bytes = JSONObject().put("schema", 1).put("engine", engineVersion)
            .put("search", SearchCheckpointCodec.encode(snapshot)).toString().toByteArray()
        val stream = file.startWrite()
        try {
            stream.write(bytes)
            file.finishWrite(stream)
        } catch (failure: Throwable) {
            file.failWrite(stream)
            throw failure
        }
    }
}

internal object SearchCheckpointCodec {
    fun encode(value: SearchSnapshot): JSONObject = JSONObject().apply {
        put("results", results(value.results))
        value.query?.let { put("query", ResultsExport.encodeQuery(it)) }
        value.status?.let {
            put("status", JSONObject().put("state", it.state.name).put("scanned", it.scannedSeeds)
                .put("total", it.totalSeeds).put("error", it.errorCode).put("probability", it.matchProbability))
        }
        put("elapsed", value.elapsedSeconds)
        value.target?.let {
            put("target", run(it.request, it.results, it.resumeFrom, it.remaining))
        }
        value.lastRun?.let {
            put("last", run(it.request, it.results, it.resumeFrom, it.remaining))
        }
        value.lastKind?.let { put("kind", it.name) }
        value.pending?.let { pending ->
            put("pending", JSONObject().apply {
                put("request", ResultsExport.encodeQuery(pending.request))
                put("mode", pending.mode.name)
                put("workers", pending.workers)
                put("scanned", pending.scanned)
                put("total", pending.total)
                put("scanMatches", pending.scanMatches)
                pending.window?.let { put("window", window(it.position, it.remaining)) }
                pending.refine?.let { refine ->
                    put("refine", window(refine.resumeFrom, refine.remaining).apply {
                        put("results", results(refine.keepSeeds))
                        refine.base?.let { put("base", ResultsExport.encodeQuery(it)) }
                    })
                }
            })
        }
        value.error?.let { put("error", it) }
    }

    fun decode(value: JSONObject): SearchSnapshot = SearchSnapshot(
        results = readResults(value.getJSONArray("results")),
        query = value.optJSONObject("query")?.let(ResultsExport::decodeQuery),
        status = value.optJSONObject("status")?.let {
            SearchStatus(SearchState.valueOf(it.getString("state")), it.getLong("scanned"),
                it.getLong("total"), it.getLong("error"), it.getDouble("probability"))
        },
        elapsedSeconds = value.getLong("elapsed"),
        target = value.optJSONObject("target")?.let {
            TargetState(request(it.getJSONObject("request")), readResults(it.getJSONArray("results")),
                it.getLong("position"), it.getLong("remaining"))
        },
        lastRun = value.optJSONObject("last")?.let {
            FinishedRun(request(it.getJSONObject("request")), it.getLong("position"),
                it.getLong("remaining"), readResults(it.getJSONArray("results")))
        },
        lastKind = (value.opt("kind") as? String)?.let(StartMode::valueOf),
        pending = value.optJSONObject("pending")?.let {
            PendingSearch(
                request(it.getJSONObject("request")), StartMode.valueOf(it.getString("mode")),
                it.getInt("workers").coerceAtLeast(1),
                refine = it.optJSONObject("refine")?.let { refine ->
                    RefineSpec(refine.getLong("position"), refine.getLong("remaining"),
                        readResults(refine.getJSONArray("results")),
                        refine.optJSONObject("base")?.let(::request))
                },
                window = it.optJSONObject("window")?.let { window ->
                    ResumeHint(window.getLong("position"), window.getLong("remaining"))
                },
                scanned = it.getLong("scanned"), total = it.getLong("total"),
                scanMatches = it.getInt("scanMatches"),
            )
        },
        error = value.opt("error") as? String,
    )

    private fun request(value: JSONObject): SearchRequest = ResultsExport.decodeQuery(value).let {
        SearchRequest(it.requirements, it.maximumDepth, it.challenges, it.requireBlacksmith,
            it.excludeBlacksmithRewards, it.wandmakerQuest, it.autoApplyTrinket, it.arcaneResin, it.arcaneResinFilter)
    }

    private fun window(position: Long, remaining: Long) =
        JSONObject().put("position", position).put("remaining", remaining)

    private fun run(request: SearchRequest, results: List<SeedResult>, position: Long, remaining: Long) =
        window(position, remaining).put("request", ResultsExport.encodeQuery(request))
            .put("results", results(results))

    private fun results(values: List<SeedResult>) = JSONArray().apply {
        values.forEach { put(JSONArray().put(it.seed).put(it.matchedRequirements).put(it.selectedTrinket)) }
    }

    private fun readResults(values: JSONArray) = List(values.length()) { index ->
        val row = values.getJSONArray(index)
        SeedResult(row.getString(0), row.getInt(1), row.opt(2) as? String)
    }
}
