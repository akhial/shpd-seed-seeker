// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.Manifest
import android.os.Build
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import android.content.Context
import android.content.Intent
import android.graphics.BitmapFactory
import androidx.activity.compose.PredictiveBackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.Place
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import dev.seedseeker.app.BuildConfig
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.engine.EngineInfo
import dev.seedseeker.app.engine.NativeSeedFinder
import dev.seedseeker.app.engine.ScoutMatches
import dev.seedseeker.app.engine.SearchWorkers
import dev.seedseeker.app.engine.SeedCode
import dev.seedseeker.app.model.BoardItem
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.Challenge
import dev.seedseeker.app.model.DeepLink
import dev.seedseeker.app.model.BuiltInPresets
import dev.seedseeker.app.model.PresetQuery
import dev.seedseeker.app.model.PresetStorage
import dev.seedseeker.app.model.QueryPreset
import dev.seedseeker.app.model.ResultsExport
import dev.seedseeker.app.model.ScoutWorld
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.model.applyEdit
import dev.seedseeker.app.model.boardItems
import dev.seedseeker.app.model.copyDepthOf
import dev.seedseeker.app.model.removeItem
import dev.seedseeker.app.model.removeMember
import dev.seedseeker.app.model.slotCount
import dev.seedseeker.app.model.toPresetQuery
import dev.seedseeker.app.model.validationProblem
import dev.seedseeker.app.model.WorkerPreference
import dev.seedseeker.app.update.UpdateChecker
import dev.seedseeker.app.update.UpdateInfo
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val ATLAS_PATH = "third_party/shattered-pixel-dungeon/items.png"
private const val ITEM_ICONS_PATH = "third_party/shattered-pixel-dungeon/item_icons.png"
private const val SETTINGS_PREFERENCES = "seed_seeker_settings"
private const val CHALLENGES_KEY = "challenges_mask"
private const val COMPACT_CHIPS_KEY = "compact_chips"
private const val UPDATE_LAST_CHECK_KEY = "update_last_check"
private const val UPDATE_SKIPPED_KEY = "update_skipped_version"
private const val UPDATE_CHECK_INTERVAL_MILLIS = 24L * 60 * 60 * 1000

/**
 * Reads the picked file as UTF-8, stopping one byte past the engine's own
 * import cap so an arbitrarily large pick never has to fit in memory. The
 * engine refuses anything above the cap when it decodes those bytes.
 */
private fun readForImport(stream: java.io.InputStream): String {
    val buffer = ByteArray(EngineInfo.resultsFileMaxBytes + 1)
    var total = 0
    while (total < buffer.size) {
        val read = stream.read(buffer, total, buffer.size - total)
        if (read < 0) break
        total += read
    }
    return String(buffer, 0, total, Charsets.UTF_8)
}

private enum class Destination { FINDER, SCOUT, SETTINGS, ABOUT }
private data class ScoutRun(val id: Long, val seed: String, val challenges: Int, val query: SearchRequest?, val trinket: String? = null)

/**
 * Link text received through an incoming intent. A plain class, not a data
 * class: every arrival is a fresh instance so re-tapping the same link still
 * re-applies it.
 */
class SharedLink(val text: String)

@Composable
internal fun SeedFinderApp(
    engine: NativeSeedFinder,
    controller: SearchController,
    fakeLatestVersion: String? = null,
    sharedLink: SharedLink? = null,
) {
    val context = LocalContext.current
    val atlas = remember(context) {
        runCatching {
            context.assets.open(ATLAS_PATH).use(BitmapFactory::decodeStream)
                ?.asImageBitmap()
        }.getOrNull()
    }
    val itemIcons = remember(context) {
        runCatching {
            context.assets.open(ITEM_ICONS_PATH).use(BitmapFactory::decodeStream)?.asImageBitmap()
        }.getOrNull()
    }
    val scope = rememberCoroutineScope()
    val preferences = remember(context) {
        context.getSharedPreferences(SETTINGS_PREFERENCES, Context.MODE_PRIVATE)
    }
    val presetStorage = remember(preferences) { PresetStorage(preferences) }
    val initialQuery = remember(presetStorage) {
        presetStorage.loadCurrentQuery() ?: PresetQuery(
            requirements = listOf(
                ItemRequirement(1, ItemCatalog.wands.first { it.id == "wand_fireblast" }, 3),
            ),
            challenges = preferences.getInt(CHALLENGES_KEY, 0)
                .takeIf { it in 0..Challenge.ALL_MASK } ?: 0,
        )
    }
    val workerPreference = remember(preferences) {
        WorkerPreference(preferences, SearchWorkers.ceiling)
    }

    var destination by remember { mutableStateOf(Destination.FINDER) }
    var aboutReturnDestination by remember { mutableStateOf(Destination.FINDER) }
    var settingsReturnDestination by remember { mutableStateOf(Destination.FINDER) }
    var requirements by remember { mutableStateOf(initialQuery.requirements) }
    var nextRequirementKey by remember {
        mutableLongStateOf((initialQuery.requirements.maxOfOrNull { it.key } ?: 0L) + 1L)
    }
    var addingBlanket by remember { mutableStateOf(false) }
    var userPresets by remember { mutableStateOf(presetStorage.load()) }
    var arcaneResinAuto by remember { mutableStateOf(initialQuery.arcaneResinAuto) }
    var arcaneResin by remember { mutableStateOf(initialQuery.arcaneResin) }
    var arcaneResinFilter by remember { mutableStateOf(initialQuery.arcaneResinFilter) }
    var showResinSheet by remember { mutableStateOf(false) }
    var autoApplyTrinket by remember { mutableStateOf(initialQuery.autoApplyTrinket) }
    var maximumDepth by remember { mutableStateOf(initialQuery.maximumDepth) }
    var requireBlacksmith by remember { mutableStateOf(initialQuery.requireBlacksmith) }
    var excludeBlacksmithRewards by remember { mutableStateOf(initialQuery.excludeBlacksmithRewards) }
    var wandmakerQuest by remember { mutableStateOf(initialQuery.wandmakerQuest) }
    var challenges by remember { mutableStateOf(initialQuery.challenges) }

    val currentQuery = PresetQuery(
        requirements = requirements,
        maximumDepth = maximumDepth,
        requireBlacksmith = requireBlacksmith,
        excludeBlacksmithRewards = excludeBlacksmithRewards,
        wandmakerQuest = wandmakerQuest,
        challenges = challenges,
        autoApplyTrinket = autoApplyTrinket,
        arcaneResin = arcaneResin,
        arcaneResinFilter = arcaneResinFilter,
        arcaneResinAuto = arcaneResinAuto,
    )
    // Save edits as they happen, including drafts that haven't been searched yet.
    LaunchedEffect(presetStorage, currentQuery) {
        presetStorage.saveCurrentQuery(currentQuery)
    }
    var compactChips by remember { mutableStateOf(preferences.getBoolean(COMPACT_CHIPS_KEY, false)) }
    // Device-local, so unlike the query state above nothing an import, a
    // preset or a share link carries ever writes it.
    var workerCount by remember { mutableStateOf(workerPreference.load()) }
    // The board anchor the editor is open on, plus the stack shape it showed;
    // null means the sheet is building a new chip.
    var editingIndex by remember { mutableStateOf<Int?>(null) }
    var editingCount by remember { mutableStateOf(1) }
    var editingTotal by remember { mutableStateOf<Int?>(null) }
    var editingCopyDepth by remember { mutableStateOf<Int?>(null) }
    var showRequirementSheet by remember { mutableStateOf(false) }
    val search = controller.snapshot
    val results = displayedResults(search.results)
    val foundCount = search.results.size
    val searchStatus = search.status
    val searchSeedsPerSecond = controller.seedsPerSecond
    val searchElapsedSeconds = search.elapsedSeconds
    val target = search.target
    val lastFinishedRun = search.lastRun
    val refinePhase = controller.refinePhase
    val isSearching = controller.isSearching || !controller.ready
    val searchError = search.error
    val snackbarHostState = remember { SnackbarHostState() }
    var scoutInput by remember { mutableStateOf("") }
    var scoutResult by remember { mutableStateOf<ScoutWorld?>(null) }
    var scoutRun by remember { mutableStateOf<ScoutRun?>(null) }
    var completedScoutRun by remember { mutableStateOf<ScoutRun?>(null) }
    var nextScoutRunId by remember { mutableLongStateOf(1L) }
    var isScouting by remember { mutableStateOf(false) }
    var scoutError by remember { mutableStateOf<String?>(null) }
    var availableUpdate by remember { mutableStateOf<UpdateInfo?>(null) }
    // Survives the activity recreation a document picker can trigger.
    var pendingExport by rememberSaveable { mutableStateOf<String?>(null) }
    var transferError by remember { mutableStateOf<String?>(null) }
    var linkError by remember { mutableStateOf<String?>(null) }
    var importNotice by remember { mutableStateOf<String?>(null) }
    val searchedQuery = search.query

    val notificationPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }

    // Restore the board once per activity after disk loading, including a pending refine's query.
    LaunchedEffect(controller.ready) {
        if (!controller.ready) return@LaunchedEffect
        val query = controller.snapshot.pending?.request?.toPresetQuery() ?: controller.snapshot.query
        if (query != null) {
            requirements = query.requirements.map { it.copy(key = nextRequirementKey++) }
            autoApplyTrinket = query.autoApplyTrinket
            arcaneResin = query.arcaneResin
            arcaneResinAuto = query.arcaneResinAuto
            arcaneResinFilter = query.arcaneResinFilter
            maximumDepth = query.maximumDepth
            requireBlacksmith = query.requireBlacksmith
            excludeBlacksmithRewards = query.excludeBlacksmithRewards
            wandmakerQuest = query.wandmakerQuest
            challenges = query.challenges
        }
    }
    LaunchedEffect(controller.notice) {
        val message = controller.notice ?: return@LaunchedEffect
        snackbarHostState.showSnackbar(message)
        controller.notice = null
    }

    val exportLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/json"),
    ) { uri ->
        val contents = pendingExport
        pendingExport = null
        if (uri != null && contents != null) {
            scope.launch {
                withContext(Dispatchers.IO) {
                    runCatching {
                        context.contentResolver.openOutputStream(uri, "wt")
                            ?.use { it.write(contents.toByteArray()) }
                            ?: error("Could not open the selected file.")
                    }
                }.onFailure { failure ->
                    transferError = "Export failed: ${failure.message}"
                }
            }
        }
    }
    val importLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val outcome = withContext(Dispatchers.IO) {
                runCatching {
                    val text = context.contentResolver.openInputStream(uri)?.use { stream ->
                        readForImport(stream)
                    } ?: error("Could not read the selected file.")
                    ResultsExport.decode(text)
                }
            }
            outcome.onSuccess { imported ->
                // A search may have started while the picker was open or the
                // file was being read.
                if (controller.isSearching || !controller.ready) {
                    transferError = "Stop the search before importing results."
                    return@onSuccess
                }
                requirements = imported.query.requirements.map { it.copy(key = nextRequirementKey++) }
                autoApplyTrinket = imported.query.autoApplyTrinket
                arcaneResin = imported.query.arcaneResin
                arcaneResinAuto = imported.query.arcaneResinAuto
                arcaneResinFilter = imported.query.arcaneResinFilter
                maximumDepth = imported.query.maximumDepth
                requireBlacksmith = imported.query.requireBlacksmith
                excludeBlacksmithRewards = imported.query.excludeBlacksmithRewards
                wandmakerQuest = imported.query.wandmakerQuest
                challenges = imported.query.challenges
                preferences.edit().putInt(CHALLENGES_KEY, challenges).apply()
                // The engine already deduplicated and capped the list and
                // reported what that removed.
                val kept = imported.seeds
                val dropped = imported.dropped
                val importedResults = kept.mapIndexed { index, seed -> SeedResult(seed, (imported.query.requirements.slotCount() + if (imported.query.arcaneResinAuto || imported.query.arcaneResin > 0) 1 else 0), imported.trinkets.getOrNull(index)) }
                controller.importResults(
                    imported.query, importedResults,
                    runCatching {
                        TargetState(
                            request = SearchRequest(
                                requirements = imported.query.requirements,
                                autoApplyTrinket = imported.query.autoApplyTrinket,
                                arcaneResin = imported.query.arcaneResin,
                                arcaneResinFilter = imported.query.arcaneResinFilter,
                                arcaneResinAuto = imported.query.arcaneResinAuto,
                                maximumDepth = imported.query.maximumDepth,
                                challenges = imported.query.challenges,
                                requireBlacksmith = imported.query.requireBlacksmith,
                                excludeBlacksmithRewards = imported.query.excludeBlacksmithRewards,
                                wandmakerQuest = imported.query.wandmakerQuest,
                            ),
                            results = importedResults,
                            resumeFrom = 0,
                            remaining = 0,
                        )
                    }.getOrNull(),
                )
                importNotice = buildString {
                    append("Imported ${kept.size} seed${if (kept.size == 1) "" else "s"} from file")
                    if (dropped > 0) {
                        append(" · $dropped duplicate or over-limit entr${if (dropped == 1) "y" else "ies"} dropped")
                    }
                    val fileVersion = imported.shpdVersion
                    if (fileVersion != null && fileVersion != EngineInfo.shpdVersion) {
                        append(
                            " · made for Shattered Pixel Dungeon v$fileVersion; this app targets " +
                                "v${EngineInfo.shpdVersion}, so seeds may generate differently",
                        )
                    }
                }
            }.onFailure { failure ->
                transferError = failure.message ?: "The results file could not be imported."
            }
        }
    }

    LaunchedEffect(sharedLink, controller.ready) {
        if (!controller.ready) return@LaunchedEffect
        val text = sharedLink?.text ?: return@LaunchedEffect
        // App Links deliver every URL on the host; only ones that carry a
        // share code touch the query.
        val code = DeepLink.extractCode(text) ?: return@LaunchedEffect
        if (isSearching) {
            linkError = "Stop the search before opening a shared search."
            return@LaunchedEffect
        }
        runCatching { DeepLink.decode(code) }.onSuccess { query ->
            requirements = query.requirements.map { it.copy(key = nextRequirementKey++) }
            autoApplyTrinket = query.autoApplyTrinket
            arcaneResin = query.arcaneResin
            arcaneResinAuto = query.arcaneResinAuto
            arcaneResinFilter = query.arcaneResinFilter
            maximumDepth = query.maximumDepth
            requireBlacksmith = query.requireBlacksmith
            excludeBlacksmithRewards = query.excludeBlacksmithRewards
            wandmakerQuest = query.wandmakerQuest
            challenges = query.challenges
            preferences.edit().putInt(CHALLENGES_KEY, challenges).apply()
            controller.clearDisplayedResults()
            importNotice = "Loaded shared search"
            destination = Destination.FINDER
        }.onFailure { failure ->
            linkError = failure.message ?: "This shared search link could not be read."
        }
    }

    LaunchedEffect(Unit) {
        val now = System.currentTimeMillis()
        val lastCheck = preferences.getLong(UPDATE_LAST_CHECK_KEY, 0L)
        if (fakeLatestVersion == null && now - lastCheck < UPDATE_CHECK_INTERVAL_MILLIS) {
            return@LaunchedEffect
        }
        preferences.edit().putLong(UPDATE_LAST_CHECK_KEY, now).apply()
        val update = withContext(Dispatchers.IO) {
            UpdateChecker.check(BuildConfig.VERSION_NAME, fakeLatestVersion)
        }
        if (update != null && update.version != preferences.getString(UPDATE_SKIPPED_KEY, null)) {
            availableUpdate = update
        }
    }

    PredictiveBackHandler(enabled = destination != Destination.FINDER) { progress ->
        progress.collect { }
        destination = when (destination) {
            Destination.ABOUT -> aboutReturnDestination
            Destination.SETTINGS -> settingsReturnDestination
            else -> Destination.FINDER
        }
    }

    LaunchedEffect(scoutRun?.id) {
        val currentRun = scoutRun ?: return@LaunchedEffect
        isScouting = true
        scoutError = null
        // Keep the current manifest mounted while switching trinkets so its
        // stable floor keys retain the user's scroll position.
        if (scoutResult?.seed != currentRun.seed) scoutResult = null
        try {
            val world = withContext(Dispatchers.Default) {
                engine.scoutSelectedSeed(currentRun.seed, currentRun.challenges, currentRun.query, currentRun.trinket)
            }
            completedScoutRun = currentRun
            scoutResult = world
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (failure: Throwable) {
            scoutError = failure.message ?: "The native scout could not generate this seed."
        } finally {
            isScouting = false
        }
    }

    // Why the query cannot run yet — no requirements, an unattainable combined
    // upgrade total, … — shown in the header instead of silently disabling Search.
    val validationMessage = requirements.validationProblem(arcaneResin, arcaneResinAuto)
    // Null while the query is not runnable.
    val currentRequest = runCatching {
        SearchRequest(
            requirements = requirements,
            autoApplyTrinket = autoApplyTrinket,
            arcaneResin = arcaneResin,
            arcaneResinFilter = arcaneResinFilter,
            arcaneResinAuto = arcaneResinAuto,
            maximumDepth = maximumDepth,
            challenges = challenges,
            requireBlacksmith = requireBlacksmith,
            excludeBlacksmithRewards = excludeBlacksmithRewards,
            wandmakerQuest = wandmakerQuest,
        )
    }.getOrNull()
    fun scoutSeed(seed: String) {
        val formatted = SeedCode.formatInput(seed)
        scoutInput = formatted
        scoutError = null
        destination = Destination.SCOUT
        if (SeedCode.isCanonical(formatted)) {
            val saved = results.find { it.seed == formatted }
            val query = if (saved != null) searchedQuery?.let {
                SearchRequest(it.requirements, it.maximumDepth, it.challenges, it.requireBlacksmith,
                    it.excludeBlacksmithRewards, it.wandmakerQuest, it.autoApplyTrinket, it.arcaneResin, it.arcaneResinFilter, it.arcaneResinAuto)
            } ?: currentRequest else currentRequest
            scoutRun = ScoutRun(nextScoutRunId++, formatted, query?.challenges ?: challenges, query,
                if (saved != null) saved.selectedTrinket ?: "none" else null)
        }
    }

    // Anything a Clear would actually erase: listed seeds, the Target and refine base,
    // or the status/notice lines the results area still shows.
    val canClearResults = results.isNotEmpty() || target != null || lastFinishedRun != null ||
        searchStatus != null || searchError != null || importNotice != null

    val navBar: @Composable () -> Unit = {
        SeedSeekerNavBar(
            current = destination,
            onSelect = { destination = it },
        )
    }

    val resultSeeds = remember(results) { results.map { it.seed } }
    // Anchor for result navigation: the in-flight request's seed while
    // scouting, otherwise the seed of the rendered manifest. Editing the
    // seed field does not move the anchor until a scout actually runs.
    val scoutedSeed = if (isScouting) scoutRun?.seed else scoutResult?.seed

    // Which items explain the saved query of this completed scout. The engine
    // scouts that same world again and marks it (its `scout_matches`), so the
    // app never re-derives the selection; null means there is nothing to mark
    // — no runnable query, or an engine that did not produce this world.
    val scoutMatches by produceState<ScoutMatches?>(null, scoutResult, completedScoutRun) {
        value = null
        val world = scoutResult
        val completedRun = completedScoutRun
        val request = completedRun?.query ?: currentRequest
        value = if (world == null || request == null || completedRun == null) {
            null
        } else {
            withContext(Dispatchers.Default) {
                runCatching {
                    engine.scoutSelectedMatches(world.seed, completedRun.challenges, request, completedRun.query, completedRun.trinket)
                }.getOrNull()
            }
        }
    }

    CompositionLocalProvider(
        LocalItemAtlas provides atlas,
        LocalItemIconAtlas provides itemIcons,
        // One clock drives every enchantment/curse pulse in the app.
        LocalGlowPulse provides rememberGlowPulse(),
    ) {
        when (destination) {
            Destination.FINDER -> FinderScreen(
                requirements = requirements,
                autoApplyTrinket = autoApplyTrinket,
                arcaneResin = arcaneResin,
                arcaneResinFilter = arcaneResinFilter,
                arcaneResinAuto = arcaneResinAuto,
                maximumDepth = maximumDepth,
                requireBlacksmith = requireBlacksmith,
                excludeBlacksmithRewards = excludeBlacksmithRewards,
                wandmakerQuest = wandmakerQuest,
                challenges = challenges,
                workerCount = workerCount,
                workerCeiling = workerPreference.ceiling,
                presets = BuiltInPresets.all + userPresets,
                compactChips = compactChips,
                results = results,
                foundCount = foundCount,
                status = searchStatus,
                seedsPerSecond = searchSeedsPerSecond,
                elapsedSeconds = searchElapsedSeconds,
                isSearching = isSearching,
                refinePhase = refinePhase,
                error = searchError,
                snackbarHostState = snackbarHostState,
                onAbout = {
                    aboutReturnDestination = Destination.FINDER
                    destination = Destination.ABOUT
                },
                onSettings = {
                    settingsReturnDestination = Destination.FINDER
                    destination = Destination.SETTINGS
                },
                onApplyPreset = { preset ->
                    requirements = preset.query.requirements.map { it.copy(key = nextRequirementKey++) }
                    autoApplyTrinket = preset.query.autoApplyTrinket
                    arcaneResin = preset.query.arcaneResin
                    arcaneResinAuto = preset.query.arcaneResinAuto
                    arcaneResinFilter = preset.query.arcaneResinFilter
                    maximumDepth = preset.query.maximumDepth
                    requireBlacksmith = preset.query.requireBlacksmith
                    excludeBlacksmithRewards = preset.query.excludeBlacksmithRewards
                    wandmakerQuest = preset.query.wandmakerQuest
                    challenges = preset.query.challenges
                    preferences.edit().putInt(CHALLENGES_KEY, challenges).apply()
                },
                onSavePreset = { name ->
                    val cleanName = name.trim()
                    if (cleanName.isNotEmpty()) {
                        val query = PresetQuery(
                            requirements = requirements,
                            autoApplyTrinket = autoApplyTrinket,
                            arcaneResin = arcaneResin,
                            arcaneResinFilter = arcaneResinFilter,
                            arcaneResinAuto = arcaneResinAuto,
                            maximumDepth = maximumDepth,
                            requireBlacksmith = requireBlacksmith,
                            excludeBlacksmithRewards = excludeBlacksmithRewards,
                            wandmakerQuest = wandmakerQuest,
                            challenges = challenges,
                        )
                        val existing = userPresets.indexOfFirst { it.name.equals(cleanName, ignoreCase = true) }
                        userPresets = if (existing >= 0) {
                            userPresets.toMutableList().also { it[existing] = it[existing].copy(query = query) }
                        } else {
                            userPresets + QueryPreset(name = cleanName, query = query)
                        }
                        presetStorage.save(userPresets)
                    }
                },
                onDeletePreset = { preset ->
                    userPresets = userPresets.filterNot { it.id == preset.id }
                    presetStorage.save(userPresets)
                },
                onEditResin = { showResinSheet = true },
                onRemoveResin = { arcaneResin = 0; arcaneResinAuto = false; arcaneResinFilter = dev.seedseeker.app.model.ArcaneResinFilter() },
                onAdd = { blanket ->
                    addingBlanket = blanket
                    editingIndex = null
                    editingCount = 1
                    editingTotal = null
                    editingCopyDepth = null
                    showRequirementSheet = true
                },
                // The tapped chip is what the editor opens on, but the stack it
                // shows belongs to the whole board item behind it.
                onEdit = { item, index ->
                    editingIndex = index
                    editingCount = item.stackCount
                    editingTotal = item.total
                    editingCopyDepth = requirements.copyDepthOf(item)
                    showRequirementSheet = true
                },
                onRequirementsChange = { requirements = it },
                onRemove = { item -> requirements = requirements.removeItem(item) },
                onMaximumDepthChange = { maximumDepth = it },
                onAutoApplyTrinketChange = { autoApplyTrinket = it },
                onRequireBlacksmithChange = { requireBlacksmith = it },
                onExcludeBlacksmithRewardsChange = { excludeBlacksmithRewards = it },
                onWandmakerQuestChange = { wandmakerQuest = it },
                onWorkerCountChange = { workerCount = workerPreference.save(it) },
                validationMessage = validationMessage,
                onSearch = {
                    if (currentRequest != null) {
                        importNotice = null
                        controller.start(currentRequest, workerCount)
                        if (Build.VERSION.SDK_INT >= 33 &&
                            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED &&
                            !preferences.getBoolean("notification_permission_requested", false)
                        ) {
                            preferences.edit().putBoolean("notification_permission_requested", true).apply()
                            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
                        }
                    }
                },
                onCancel = controller::stop,
                canExportResults = searchedQuery != null && results.isNotEmpty(),
                canClearResults = canClearResults,
                importNotice = importNotice,
                onClearResults = {
                    controller.clear()
                    importNotice = null
                },
                onExportResults = {
                    // Export the query snapshot that produced the results,
                    // never the live editor state.
                    val query = searchedQuery
                    if (query == null || results.isEmpty()) {
                        transferError = "Run a search first — there are no results to export yet."
                    } else {
                        runCatching {
                            ResultsExport.encode(query, results.map { it.seed }, BuildConfig.VERSION_NAME, results.map { it.selectedTrinket })
                        }.onSuccess { contents ->
                            pendingExport = contents
                            exportLauncher.launch(ResultsExport.SUGGESTED_FILE_NAME)
                        }.onFailure { failure ->
                            transferError = "Export failed: ${failure.message}"
                        }
                    }
                },
                onImportResults = {
                    importLauncher.launch(
                        arrayOf("application/json", "text/plain", "application/octet-stream"),
                    )
                },
                onShareQuery = {
                    runCatching {
                        DeepLink.encodeLink(
                            PresetQuery(
                                requirements = requirements,
                                autoApplyTrinket = autoApplyTrinket,
                                arcaneResin = arcaneResin,
                                arcaneResinFilter = arcaneResinFilter,
                                arcaneResinAuto = arcaneResinAuto,
                                maximumDepth = maximumDepth,
                                requireBlacksmith = requireBlacksmith,
                                excludeBlacksmithRewards = excludeBlacksmithRewards,
                                wandmakerQuest = wandmakerQuest,
                                challenges = challenges,
                            ),
                        )
                    }.onSuccess { link ->
                        val send = Intent(Intent.ACTION_SEND)
                            .setType("text/plain")
                            .putExtra(Intent.EXTRA_TEXT, link)
                        context.startActivity(Intent.createChooser(send, "Share search"))
                    }.onFailure { failure ->
                        linkError = failure.message ?: "This search could not be shared."
                    }
                },
                onScoutSeed = ::scoutSeed,
                bottomBar = navBar,
            )

            Destination.SCOUT -> ScoutScreen(
                seedInput = scoutInput,
                result = scoutResult,
                mapChallenges = completedScoutRun?.challenges ?: 0,
                isScouting = isScouting,
                error = scoutError,
                matches = scoutMatches,
                resultSeeds = resultSeeds,
                scoutedSeed = scoutedSeed,
                onScoutSeed = ::scoutSeed,
                onSeedChange = {
                    val formatted = SeedCode.formatInput(it)
                    scoutInput = formatted
                    if (formatted != scoutResult?.seed) scoutResult = null
                    scoutError = null
                },
                onSelectTrinket = { trinket ->
                    // Keep the completed scout profile, including an empty query's
                    // captured challenges, even after editing the finder or a failed retry.
                    val previous = completedScoutRun
                    if (!isScouting && previous != null && scoutResult != null) {
                        scoutRun = previous.copy(id = nextScoutRunId++, trinket = trinket)
                    }
                },
                onScout = {
                    if (SeedCode.isCanonical(scoutInput)) {
                        scoutSeed(scoutInput)
                    }
                },
                onSettings = {
                    settingsReturnDestination = Destination.SCOUT
                    destination = Destination.SETTINGS
                },
                onAbout = {
                    aboutReturnDestination = Destination.SCOUT
                    destination = Destination.ABOUT
                },
                bottomBar = navBar,
            )

            Destination.SETTINGS -> SettingsScreen(
                compactChips = compactChips,
                onCompactChipsChange = { checked ->
                    compactChips = checked
                    preferences.edit().putBoolean(COMPACT_CHIPS_KEY, checked).apply()
                },
                challenges = challenges,
                challengesEnabled = !isSearching && !isScouting,
                onChallengeChange = { challenge, checked ->
                    val updatedChallenges = if (checked) {
                        challenges or challenge.bit
                    } else {
                        challenges and challenge.bit.inv()
                    }
                    challenges = updatedChallenges
                    scoutResult = null
                    preferences.edit().putInt(CHALLENGES_KEY, updatedChallenges).apply()
                },
                onBack = { destination = settingsReturnDestination },
            )

            Destination.ABOUT -> AboutScreen(onBack = { destination = aboutReturnDestination })
        }

        if (showResinSheet) {
            ArcaneResinSheet(arcaneResin, arcaneResinFilter, auto = arcaneResinAuto,
                onDismiss = { showResinSheet = false },
                onSave = { amount, filter, auto -> arcaneResin = amount; arcaneResinAuto = auto; arcaneResinFilter = filter; showResinSheet = false },
                onRemove = { arcaneResin = 0; arcaneResinAuto = false; arcaneResinFilter = dev.seedseeker.app.model.ArcaneResinFilter(); showResinSheet = false })
        }
        if (showRequirementSheet) {
            RequirementSheet(
                onAddResin = if (editingIndex == null && !addingBlanket) ({ showRequirementSheet = false; showResinSheet = true }) else null,
                editing = editingIndex?.let(requirements::get),
                blanket = editingIndex?.let { requirements[it].blanket } ?: addingBlanket,
                initialKind = if (addingBlanket) requirements.firstOrNull { !it.blanket }?.kind ?: ItemKind.WEAPON else ItemKind.WEAPON,
                editingCount = editingCount,
                editingTotal = editingTotal,
                editingCopyDepth = editingCopyDepth,
                onDismiss = { showRequirementSheet = false },
                onSave = { saved, count, total, copyDepth ->
                    requirements = requirements.applyEdit(editingIndex, saved, count, total, copyDepth)
                    showRequirementSheet = false
                },
                // As from the board's drop zone: a lone chip goes with its
                // copies, a member leaves the cluster and its stack behind.
                onRemove = editingIndex?.let { index ->
                    {
                        val item = requirements.boardItems().first { index in it.members }
                        requirements = if (item.cluster != null) {
                            requirements.removeMember(index)
                        } else {
                            requirements.removeItem(item)
                        }
                        showRequirementSheet = false
                    }
                },
            )
        }

        transferError?.let { message ->
            AlertDialog(
                onDismissRequest = { transferError = null },
                title = { Text("Results file") },
                text = { Text(message) },
                confirmButton = {
                    TextButton(onClick = { transferError = null }) { Text("OK") }
                },
            )
        }

        linkError?.let { message ->
            AlertDialog(
                onDismissRequest = { linkError = null },
                title = { Text("Shared search") },
                text = { Text(message) },
                confirmButton = {
                    TextButton(onClick = { linkError = null }) { Text("OK") }
                },
            )
        }

        availableUpdate?.let { update ->
            val uriHandler = LocalUriHandler.current
            AlertDialog(
                onDismissRequest = { availableUpdate = null },
                title = { Text("Update available") },
                text = {
                    Text(
                        "Seed Seeker ${update.version} is available on GitHub. " +
                            "You have ${BuildConfig.VERSION_NAME}.",
                    )
                },
                confirmButton = {
                    TextButton(
                        onClick = {
                            availableUpdate = null
                            runCatching { uriHandler.openUri(update.url) }
                        },
                    ) { Text("Download") }
                },
                dismissButton = {
                    TextButton(
                        onClick = {
                            preferences.edit().putString(UPDATE_SKIPPED_KEY, update.version).apply()
                            availableUpdate = null
                        },
                    ) { Text("Skip") }
                    TextButton(onClick = { availableUpdate = null }) { Text("Not now") }
                },
            )
        }
    }
}

@Composable
private fun SeedSeekerNavBar(
    current: Destination,
    onSelect: (Destination) -> Unit,
) {
    NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        NavigationBarItem(
            selected = current == Destination.FINDER,
            onClick = { onSelect(Destination.FINDER) },
            icon = {
                Icon(
                    if (current == Destination.FINDER) Icons.Filled.Search else Icons.Outlined.Search,
                    contentDescription = null,
                )
            },
            label = { Text("Finder") },
            colors = NavigationBarItemDefaults.colors(
                selectedIconColor = MaterialTheme.colorScheme.onPrimaryContainer,
                selectedTextColor = MaterialTheme.colorScheme.onSurface,
                indicatorColor = MaterialTheme.colorScheme.primaryContainer,
            ),
        )
        NavigationBarItem(
            selected = current == Destination.SCOUT,
            onClick = { onSelect(Destination.SCOUT) },
            icon = {
                Icon(
                    if (current == Destination.SCOUT) Icons.Filled.Place else Icons.Outlined.Place,
                    contentDescription = null,
                )
            },
            label = { Text("Scout") },
            colors = NavigationBarItemDefaults.colors(
                selectedIconColor = MaterialTheme.colorScheme.onPrimaryContainer,
                selectedTextColor = MaterialTheme.colorScheme.onSurface,
                indicatorColor = MaterialTheme.colorScheme.primaryContainer,
            ),
        )
    }
}
