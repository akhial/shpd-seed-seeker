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
    val workers: Int,
    val refine: RefineSpec? = null,
    val window: ResumeHint? = null,
    val scanned: Long = 0,
    val total: Long = 0,
    val scanMatches: Int = 0,
    val scanLimit: Int = RESULT_CAP,
)

internal data class SearchSnapshot(
    val results: List<SeedResult> = emptyList(),
    val query: PresetQuery? = null,
    val status: SearchStatus? = null,
    val elapsedSeconds: Long = 0,
    val target: TargetState? = null,
    val lastRun: FinishedRun? = null,
    val pending: PendingSearch? = null,
    val error: String? = null,
)

internal interface SearchCheckpointStore {
    fun load(): SearchSnapshot
    fun save(snapshot: SearchSnapshot)
}

/** Private, uncapped pool and scan storage. */
internal class FileSearchCheckpointStore(file: File, private val engineVersion: String) : SearchCheckpointStore {
    private val file = AtomicFile(file)

    override fun load(): SearchSnapshot {
        val text = try {
            file.openRead().bufferedReader().use { it.readText() }
        } catch (_: java.io.FileNotFoundException) {
            return SearchSnapshot()
        }
        val document = JSONObject(text)
        require(document.getInt("schema") in 1..2) { "Unsupported saved search format." }
        val saved = SearchCheckpointCodec.decode(document.getJSONObject("search"))
        if (document.getString("engine") == engineVersion) return saved
        // Never carry traversal coverage across a game/engine update. Keep the user's finds.
        return saved.copy(
            pending = null, lastRun = null,
            status = null,
            error = "The engine changed. Saved results were restored; start a search to recheck them.",
        )
    }

    override fun save(snapshot: SearchSnapshot) {
        val bytes = JSONObject().put("schema", 2).put("engine", engineVersion)
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
            put("target", JSONObject().put("request", ResultsExport.encodeQuery(it.request)).put("results", results(it.results)).put("sources", sources(it.sources)))
        }
        value.lastRun?.let {
            put("last", run(it.request, it.results, it.resumeFrom, it.remaining))
        }
        value.pending?.let { pending ->
            put("pending", JSONObject().apply {
                put("request", ResultsExport.encodeQuery(pending.request))
                put("workers", pending.workers)
                put("scanned", pending.scanned)
                put("total", pending.total)
                put("scanMatches", pending.scanMatches)
                put("scanLimit", pending.scanLimit)
                pending.window?.let { put("window", window(it.position, it.remaining)) }
                pending.refine?.let { refine ->
                    put("refine", window(refine.resumeFrom, refine.remaining).apply {
                        put("results", results(refine.keepSeeds))
                        put("freshScan", refine.freshScan)
                        put("sources", sources(refine.sources))
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
                readSources(it.optJSONObject("sources")))
        },
        lastRun = value.optJSONObject("last")?.let {
            FinishedRun(request(it.getJSONObject("request")), it.getLong("position"),
                it.getLong("remaining"), readResults(it.getJSONArray("results")))
        },
        pending = value.optJSONObject("pending")?.let {
            PendingSearch(
                request(it.getJSONObject("request")),
                it.getInt("workers").coerceAtLeast(1),
                refine = it.optJSONObject("refine")?.let { refine ->
                    RefineSpec(refine.getLong("position"), refine.getLong("remaining"),
                        readResults(refine.getJSONArray("results")),
                        refine.optJSONObject("base")?.let(::request), refine.optBoolean("freshScan", false), readSources(refine.optJSONObject("sources")))
                },
                window = it.optJSONObject("window")?.let { window ->
                    ResumeHint(window.getLong("position"), window.getLong("remaining"))
                },
                scanned = it.getLong("scanned"), total = it.getLong("total"),
                scanMatches = it.getInt("scanMatches"),
                scanLimit = it.optInt("scanLimit", RESULT_CAP).coerceIn(1, RESULT_CAP),
            )
        },
        error = value.opt("error") as? String,
    ).let { saved ->
        if (!value.has("kind") && value.optJSONObject("pending")?.has("mode") != true) return@let saved
        var pool = saved.target
        value.optJSONObject("last")?.let { last ->
            val source = request(last.optJSONObject("selectionQuery") ?: last.getJSONObject("request"))
            pool = settledTarget(pool, source, saved.lastRun?.results.orEmpty())
        }
        val visibleSource = value.optJSONObject("pending")?.let {
            request(it.optJSONObject("selectionQuery") ?: it.getJSONObject("request"))
        } ?: value.optJSONObject("query")?.let(::request)
        if (visibleSource != null) pool = settledTarget(pool, visibleSource, saved.results)
        val retained = pool
        saved.copy(target = retained, lastRun = null, pending = saved.pending?.let {
            PendingSearch(it.request, it.workers, retained?.let { seeds ->
                RefineSpec(0, 0, seeds.results, seeds.request, freshScan = true, sources = seeds.sources)
            })
        })
    }

    private fun sources(values: Map<String, SearchRequest>) = JSONObject().apply {
        values.forEach { (seed, query) -> put(seed, ResultsExport.encodeQuery(query)) }
    }

    private fun readSources(value: JSONObject?): Map<String, SearchRequest> =
        value?.keys()?.asSequence()?.associateWith { request(value.getJSONObject(it)) }.orEmpty()

    private fun request(value: JSONObject): SearchRequest = ResultsExport.decodeQuery(value).let {
        SearchRequest(it.requirements, it.maximumDepth, it.challenges, it.requireBlacksmith,
            it.excludeBlacksmithRewards, it.wandmakerQuest, it.autoApplyTrinket, it.arcaneResin, it.arcaneResinFilter, it.arcaneResinAuto, it.floorRequirements)
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
