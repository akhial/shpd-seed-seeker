// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SeedResult

/**
 * How many rows the displayed results list holds at most. A run's full collection — filter
 * survivors plus scanned finds — is uncapped and is what feeds the Target Set and any later
 * refine's filter base; only the LazyColumn's rows stop here (an uncapped list is what a
 * several-thousand-row UI hang is made of).
 */
internal const val RESULT_CAP = 1_024

/** Progress through the saved pool; it is separate from newly scanned seeds. */
data class RefineProgress(val checked: Int, val total: Int)

/** The displayed slice of a run's collected results: discovery order, at most [RESULT_CAP] rows. */
internal fun displayedResults(collected: List<SeedResult>): List<SeedResult> =
    if (collected.size <= RESULT_CAP) collected else collected.subList(0, RESULT_CAP)

/**
 * Which half of an in-flight refine run is executing. Only [FILTERING] is "refining" to the user;
 * once the kept seeds have been re-verified the run is an ordinary search over the resumed window.
 */
enum class RefinePhase {
    /** Re-verifying the previous run's seeds against the new query. */
    FILTERING,

    /** Scanning the seeds the base run never reached. */
    SCANNING,
}

/** Resume window and previously shown seeds a refine run starts from. */
internal data class RefineSpec(
    val resumeFrom: Long,
    val remaining: Long,
    val keepSeeds: List<SeedResult>,
    val base: SearchRequest? = null,
    /** Imports and changed world conditions need a fresh scan after filtering. */
    val freshScan: Boolean = false,
    val sources: Map<String, SearchRequest> = emptyMap(),
)

/** An unchanged query can resume this completed or cancelled traversal. */
internal data class FinishedRun(
    val request: SearchRequest,
    val resumeFrom: Long,
    val remaining: Long,
    val results: List<SeedResult>,
)

/** Every loaded or discovered seed, retained until Clear; sources preserve trinket choices. */
internal data class TargetState(
    val request: SearchRequest,
    val results: List<SeedResult>,
    val sources: Map<String, SearchRequest> = emptyMap(),
)

/** Every search checks the entire pool. Only an unchanged query reuses its cursor. */
internal fun refineFor(request: SearchRequest, target: TargetState?, lastRun: FinishedRun?): RefineSpec? {
    if (target == null) return null
    val previous = lastRun?.takeIf { it.request == request }
    return RefineSpec(previous?.resumeFrom ?: 0, previous?.remaining ?: 0,
        target.results, target.request, freshScan = previous == null, sources = target.sources)
}

/** Discoveries accumulate regardless of which query found them. */
internal fun settledTarget(
    target: TargetState?, request: SearchRequest, results: List<SeedResult>,
): TargetState {
    val known = target?.results.orEmpty().mapTo(mutableSetOf()) { it.seed }
    val merged = target?.results.orEmpty() + results.filter { known.add(it.seed) }
    val sources = target?.sources.orEmpty().toMutableMap()
    target?.results.orEmpty().forEach { sources.getOrPut(it.seed) { target!!.request } }
    results.forEach { sources.getOrPut(it.seed) { request } }
    return TargetState(target?.request ?: request, merged, sources = sources)
}
