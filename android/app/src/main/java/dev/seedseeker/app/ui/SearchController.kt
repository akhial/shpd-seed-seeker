// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.seedseeker.app.engine.NativeSearchSession
import dev.seedseeker.app.engine.EngineInfo
import dev.seedseeker.app.engine.NativeSeedFinder
import dev.seedseeker.app.model.PresetQuery
import dev.seedseeker.app.model.ResumeHint
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.model.toPresetQuery
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Application-owned search state. All commands and observable updates run on the main thread. */
internal class SearchController(
    private val engine: NativeSeedFinder,
    private val store: SearchCheckpointStore,
    private val scope: CoroutineScope,
    private val startService: () -> Unit,
    private val workerDispatcher: CoroutineDispatcher = Dispatchers.Default,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val now: () -> Long = { System.nanoTime() / 1_000_000 },
    private val checkpointMillis: Long = 15_000,
) {
    var snapshot by mutableStateOf(SearchSnapshot())
        private set
    var ready by mutableStateOf(false)
        private set
    var isSearching by mutableStateOf(false)
        private set
    var isPreparing by mutableStateOf(false)
        private set
    var seedsPerSecond by mutableStateOf(0.0)
        private set
    var notice by mutableStateOf<String?>(null)
    var refineProgress by mutableStateOf<RefineProgress?>(null)
        private set
    val refinePhase: RefinePhase?
        get() = if (isSearching && snapshot.pending?.refine != null) RefinePhase.FILTERING else null

    private var job: Job? = null
    private var preparation: Job? = null
    private var stopRequested = false
    private var pauseRequested = false
    private val writes = Mutex()
    private var durable = SearchSnapshot()
    private val loading = scope.launch {
        snapshot = try {
            withContext(ioDispatcher) { store.load() }
        } catch (failure: Exception) {
            SearchSnapshot(error = "The saved search could not be restored: ${failure.message}")
        }
        durable = snapshot
        ready = true
    }

    /** Called only while the activity is visible, so a pending search may start a foreground service. */
    fun onVisible() {
        scope.launch {
            loading.join()
            // Returning while an interruption is still draining must also resume, without
            // opening a second native session before the old one has closed.
            if (pauseRequested) job?.join()
            if (snapshot.pending != null && !isSearching) requestService()
        }
    }

    fun start(request: SearchRequest, workers: Int) {
        if (!ready || isSearching) return
        // JNI planning can rank multiple trinket profiles and inflate tables. Let the
        // screen update immediately, and keep all of that work off the main thread.
        isSearching = true
        isPreparing = true
        stopRequested = false
        pauseRequested = false
        notice = null
        preparation = scope.launch {
            try {
                val reason = withContext(workerDispatcher) { engine.impossibilityReason(request) }
                if (stopRequested || pauseRequested) {
                    isSearching = false
                    return@launch
                }
                if (reason != null) {
                    notice = "Impossible query. $reason"
                    isSearching = false
                    return@launch
                }
                val refine = refineFor(request, snapshot.target, snapshot.lastRun)
                snapshot = snapshot.copy(
                    pending = PendingSearch(request, workers, refine),
                    target = snapshot.target ?: TargetState(request, emptyList()),
                    results = if (refine == null) emptyList() else snapshot.results,
                    query = if (refine == null) request.toPresetQuery() else snapshot.query,
                    status = null, error = null, elapsedSeconds = 0,
                )
                refineProgress = refine?.let { RefineProgress(0, it.keepSeeds.size) }
                requestService()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (failure: Exception) {
                snapshot = snapshot.copy(error = failure.message ?: "The native query could not be prepared.")
                isSearching = false
            } finally {
                isPreparing = false
            }
        }
    }

    private fun requestService() {
        isSearching = true
        try {
            startService()
        } catch (failure: Exception) {
            serviceUnavailable(failure)
        }
    }

    fun serviceUnavailable(failure: Exception) {
        pauseRequested = true
        snapshot = snapshot.copy(error = "Search paused: ${failure.message ?: "background search is unavailable"}. Reopen the app to retry.")
        if (job?.isActive != true) isSearching = false
        scope.launch { saveSafely() }
    }

    /** The service calls this only after startForeground succeeds. Repeated starts attach to one job. */
    fun runPending() {
        pauseRequested = false
        if (job?.isActive == true) return
        job = scope.launch {
            loading.join()
            preparation?.join()
            if (snapshot.pending == null) {
                isSearching = false
                return@launch
            }
            isSearching = true
            seedsPerSecond = 0.0
            try {
                drive()
            } catch (cancelled: CancellationException) {
                // Process/test scope shutdown keeps the last durable checkpoint resumable.
                throw cancelled
            } catch (failure: Exception) {
                snapshot = snapshot.copy(
                    pending = null, lastRun = null,
                    status = snapshot.status?.copy(state = SearchState.FAILED)
                        ?: SearchStatus(SearchState.FAILED, 0, 0, -1),
                    error = failure.message ?: "The native search engine could not start.",
                )
                saveSafely()
            } finally {
                refineProgress = null
                isPreparing = false
                isSearching = false
            }
        }
    }

    /** User cancellation is distinct from interruption: a saved Stop must never auto-resume. */
    fun stop() {
        stopRequested = true
        scope.launch {
            loading.join()
            // Commit the user's intent before waiting for native workers to finish draining.
            saveSafely(durable.copy(pending = null, status = durable.status?.copy(state = SearchState.CANCELLED)))
            if (job?.isActive != true) {
                snapshot = snapshot.copy(pending = null, status = snapshot.status?.copy(state = SearchState.CANCELLED))
                if (preparation?.isActive != true) isSearching = false
                saveSafely()
            }
        }
    }

    /** Service destruction/timeouts retain the pending run; the next visible activity resumes it. */
    fun interrupt() {
        pauseRequested = true
    }

    fun clear() {
        if (!ready || isSearching) return
        snapshot = SearchSnapshot()
        scope.launch { saveSafely() }
    }

    fun clearDisplayedResults() {
        if (!ready || isSearching) return
        // A shared query changes the board/display; only the explicit Clear discards Target.
        snapshot = snapshot.copy(results = emptyList(), query = null, status = null, error = null,
            pending = null, elapsedSeconds = 0)
        scope.launch { saveSafely() }
    }

    fun importResults(query: PresetQuery, results: List<SeedResult>, target: TargetState?) {
        if (!ready || isSearching) return
        snapshot = SearchSnapshot(results = results, query = query, target = target?.let {
            settledTarget(snapshot.target, it.request, it.results)
        } ?: snapshot.target)
        scope.launch { saveSafely() }
    }

    private suspend fun drive() {
        // A kill before the first checkpoint restarts the whole fresh traversal. No partial
        // native cursor is used: it can skip claimed chunks whose workers have not started yet.
        save()
        var pending = checkNotNull(snapshot.pending)
        val refine = pending.refine
        if (refine != null) {
            val kept = mutableListOf<SeedResult>()
            var checked = 0
            val startedAt = now()
            val elapsedBefore = snapshot.elapsedSeconds
            refineProgress = RefineProgress(0, refine.keepSeeds.size)
            for (chunk in refine.keepSeeds.chunked(24)) {
                if (stopRequested || pauseRequested) break
                kept += withContext(workerDispatcher) {
                    chunk.groupBy { refine.sources[it.seed] ?: refine.base ?: pending.request }
                        .flatMap { (source, seeds) -> engine.filterRecipes(pending.request, source, seeds) }
                }
                checked += chunk.size
                refineProgress = RefineProgress(checked, refine.keepSeeds.size)
                snapshot = snapshot.copy(elapsedSeconds = elapsedBefore + (now() - startedAt) / 1000)
            }
            if (!stopRequested && !pauseRequested) {
                pending = pending.copy(
                    refine = null,
                    window = if (refine.freshScan) null else ResumeHint(refine.resumeFrom, refine.remaining),
                    // Fill the visible list; when already full, Search explicitly asks for
                    // another batch so repeated searches still advance the traversal.
                    scanLimit = (EngineInfo.maxResults - kept.size).takeIf { it > 0 } ?: EngineInfo.maxResults,
                )
                snapshot = snapshot.copy(pending = pending, results = kept.toList(), query = pending.request.toPresetQuery())
                refineProgress = null
                notice = "Kept ${kept.size} of ${refine.keepSeeds.size} previous seeds."
                save()
            }
        }

        while (!stopRequested && !pauseRequested) {
            val window = pending.window
            if (window != null && (window.remaining == 0L || pending.scanMatches >= pending.scanLimit)) {
                finish(pending, window, SearchState.COMPLETED)
                return
            }
            val startedAt = now()
            val elapsedBefore = snapshot.elapsedSeconds
            var previousScanned = 0L
            var previousTime = startedAt
            var checkpointing = false
            var session: NativeSearchSession? = null
            try {
                // Assign inside the dispatcher block: coroutine cancellation during dispatch
                // back to main must not orphan a just-created JNI handle.
                isPreparing = true
                withContext(workerDispatcher) {
                    session = pending.window?.let {
                        engine.startResumedSearch(pending.request, it.position, it.remaining, pending.workers)
                    } ?: engine.startSearch(pending.request, pending.workers)
                }
                isPreparing = false
                val opened = checkNotNull(session)
                val seen = snapshot.results.mapTo(mutableSetOf()) { it.seed }
                while (true) {
                    if (stopRequested || pauseRequested || pending.scanMatches >= pending.scanLimit || now() - startedAt >= checkpointMillis) {
                        checkpointing = true
                        withContext(workerDispatcher) { opened.cancel() }
                    }
                    val (batch, status) = withContext(workerDispatcher) {
                        opened.poll(256) to opened.status()
                    }
                    val added = batch.results.filter { seen.add(it.seed) }
                    pending = pending.copy(scanMatches = pending.scanMatches + added.size)
                    val time = now()
                    if (time > previousTime && status.scannedSeeds > previousScanned) {
                        val rate = (status.scannedSeeds - previousScanned) * 1000.0 / (time - previousTime)
                        seedsPerSecond = if (seedsPerSecond == 0.0) rate else seedsPerSecond * 0.7 + rate * 0.3
                    }
                    previousScanned = status.scannedSeeds
                    previousTime = time
                    val total = pending.total.takeIf { it > 0 } ?: status.totalSeeds
                    val scanned = (pending.scanned + status.scannedSeeds).coerceAtMost(total)
                    snapshot = snapshot.copy(
                        results = if (added.isEmpty()) snapshot.results else snapshot.results + added,
                        target = settledTarget(snapshot.target, pending.request, added),
                        status = status.copy(
                            state = if (checkpointing && status.state == SearchState.CANCELLED) SearchState.RUNNING else status.state,
                            scannedSeeds = scanned, totalSeeds = total,
                        ),
                        elapsedSeconds = elapsedBefore + (time - startedAt) / 1000,
                    )
                    if (status.state == SearchState.FAILED) {
                        error(if (status.errorCode == 2_001L) "A native world-generation worker stopped unexpectedly."
                            else "The native search stopped with error ${status.errorCode}.")
                    }
                    if (status.state != SearchState.RUNNING) {
                        // A terminal native status guarantees all workers exited AND all
                        // buffered matches were drained. Only now is resumeHint safe to save.
                        val hint = withContext(workerDispatcher) { opened.resumeHint() }
                        pending = pending.copy(window = hint, scanned = scanned, total = total)
                        snapshot = snapshot.copy(pending = pending)
                        // A native session's cap includes duplicates already kept by the
                        // filter. Keep scanning its remainder until enough unique seeds
                        // arrive. A zero-work terminal session is an impossible query.
                        if (stopRequested || hint.remaining == 0L || pending.scanMatches >= pending.scanLimit ||
                            (!checkpointing && status.scannedSeeds == 0L)) {
                            finish(pending, hint, if (stopRequested) SearchState.CANCELLED else SearchState.COMPLETED)
                            return
                        }
                        save()
                        break
                    }
                    delay(90)
                }
            } finally {
                withContext(NonCancellable + workerDispatcher) { session?.close() }
            }
        }
        if (stopRequested && pending.refine == null && pending.window != null) {
            finish(pending, checkNotNull(pending.window), SearchState.CANCELLED)
            return
        }
        // An interrupted filter keeps its old results/query together and retries the filter.
        snapshot = snapshot.copy(
            pending = if (stopRequested) null else pending,
            status = snapshot.status?.copy(state = SearchState.CANCELLED),
            error = if (pauseRequested && !stopRequested) "Search interrupted. It will continue when you reopen the app." else null,
        )
        save()
    }

    private suspend fun finish(pending: PendingSearch, hint: ResumeHint, state: SearchState) {
        snapshot = snapshot.copy(
            pending = null,
            status = (snapshot.status ?: SearchStatus(state, 0, 0)).copy(state = state),
            lastRun = FinishedRun(pending.request, hint.position, hint.remaining, snapshot.results),
            target = settledTarget(snapshot.target, pending.request, snapshot.results),
            error = null,
        )
        if (snapshot.results.size >= RESULT_CAP) notice = "Result limit reached (1,024 seeds)."
        save()
    }

    private suspend fun save(value: SearchSnapshot = snapshot) {
        writes.withLock {
            val saved = if (stopRequested) value.copy(pending = null) else value
            withContext(ioDispatcher) { store.save(saved) }
            durable = saved
        }
    }

    private suspend fun saveSafely(value: SearchSnapshot = snapshot) {
        try {
            save(value)
        } catch (failure: Exception) {
            snapshot = snapshot.copy(error = "Could not save search progress: ${failure.message}")
        }
    }
}
