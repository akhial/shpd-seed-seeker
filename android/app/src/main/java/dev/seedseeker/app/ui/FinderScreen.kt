// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.ClipData
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.alpha
import dev.seedseeker.app.catalog.ItemCatalog
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.toClipEntry
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.QueryPreset
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.FloorRequirement
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.model.WandmakerQuest
import dev.seedseeker.app.model.BoardItem
import dev.seedseeker.app.model.boardCount
import dev.seedseeker.app.model.boardItems
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FinderScreen(
    requirements: List<ItemRequirement>,
    maximumDepth: Int,
    autoApplyTrinket: Boolean,
    floorRequirements: List<FloorRequirement>,
    arcaneResin: Int,
    arcaneResinAuto: Boolean = false,
    arcaneResinFilter: dev.seedseeker.app.model.ArcaneResinFilter,
    requireBlacksmith: Boolean,
    excludeBlacksmithRewards: Boolean,
    wandmakerQuest: WandmakerQuest?,
    challenges: Int,
    presets: List<QueryPreset>,
    /** Draw the board's chips at their smaller size. */
    compactChips: Boolean,
    results: List<SeedResult>,
    /** The run's full collection size; `results` lists at most the display cap. */
    foundCount: Int,
    status: SearchStatus?,
    seedsPerSecond: Double,
    elapsedSeconds: Long,
    isSearching: Boolean,
    refinePhase: RefinePhase?,
    refineProgress: RefineProgress?,
    error: String?,
    snackbarHostState: SnackbarHostState,
    onAbout: () -> Unit,
    onSettings: () -> Unit,
    onSearchSettings: () -> Unit,
    onApplyPreset: (QueryPreset) -> Unit,
    onSavePreset: (String) -> Unit,
    onDeletePreset: (QueryPreset) -> Unit,
    onEditResin: () -> Unit,
    onRemoveResin: () -> Unit,
    onAdd: (Boolean) -> Unit,
    onEdit: (BoardItem, Int) -> Unit,
    onRequirementsChange: (List<ItemRequirement>) -> Unit,
    onRemove: (BoardItem) -> Unit,
    /** Why the query cannot run yet, shown in the header; null when it is runnable. */
    validationMessage: String?,
    onSearch: () -> Unit,
    onCancel: () -> Unit,
    canExportResults: Boolean,
    canClearResults: Boolean,
    importNotice: String?,
    onExportResults: () -> Unit,
    onImportResults: () -> Unit,
    onClearResults: () -> Unit,
    onShareQuery: () -> Unit,
    onScoutSeed: (String) -> Unit,
    bottomBar: @Composable () -> Unit,
) {
    var showPresets by remember { mutableStateOf(false) }
    var showOverflowMenu by remember { mutableStateOf(false) }
    // One page at a time: what is asked for, or what was found. Nothing here is
    // chosen by hand first — starting a search turns to the results it will
    // fill, a new query (edited, from a preset, or from a shared link) turns
    // back to the board it lands on, and the collapsed header at the top or
    // bottom edge turns the page the other way. Effects run in this order, so a
    // file import, which brings a query *and* its seeds, settles on the seeds.
    var showResults by remember { mutableStateOf(results.isNotEmpty()) }
    // A run's own findings never turn the page: the start of the run turned it
    // once, and a reader who turns it back to the board stays there while seeds
    // land, since the bar at the bottom counts them anyway. Only seeds arriving
    // from outside a run — an import — turn the page by themselves. The flag
    // outlives the run by a frame, so the last batch of a finishing search,
    // which may land in the same frame as the run ending, is covered too.
    var runOwnsResults by remember { mutableStateOf(false) }
    LaunchedEffect(requirements, arcaneResin, arcaneResinAuto, arcaneResinFilter, floorRequirements) { showResults = false }
    LaunchedEffect(results) { if (results.isNotEmpty() && !runOwnsResults) showResults = true }
    LaunchedEffect(isSearching) {
        if (isSearching) {
            showResults = true
            runOwnsResults = true
        } else {
            runOwnsResults = false
        }
    }
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            TopAppBar(
                title = { Text("Seed Seeker") },
                actions = {
                    TextButton(onClick = { showPresets = true }, enabled = !isSearching) {
                        Text("Presets")
                    }
                    IconButton(onClick = onSettings) {
                        Icon(Icons.Filled.Settings, contentDescription = "Settings")
                    }
                    IconButton(onClick = onAbout) {
                        Icon(Icons.Filled.Info, contentDescription = "About and licenses")
                    }
                    Box {
                        IconButton(onClick = { showOverflowMenu = true }) {
                            Icon(Icons.Filled.MoreVert, contentDescription = "More options")
                        }
                        DropdownMenu(
                            expanded = showOverflowMenu,
                            onDismissRequest = { showOverflowMenu = false },
                        ) {
                            DropdownMenuItem(
                                text = { Text("Share search…") },
                                enabled = requirements.isNotEmpty() || floorRequirements.isNotEmpty() || (arcaneResinAuto || arcaneResin > 0),
                                onClick = {
                                    showOverflowMenu = false
                                    onShareQuery()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text("Import results…") },
                                enabled = !isSearching,
                                onClick = {
                                    showOverflowMenu = false
                                    onImportResults()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text("Export results…") },
                                enabled = !isSearching && canExportResults,
                                onClick = {
                                    showOverflowMenu = false
                                    onExportResults()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text("Clear results") },
                                enabled = !isSearching && canClearResults,
                                onClick = {
                                    showOverflowMenu = false
                                    showResults = false
                                    onClearResults()
                                },
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
        bottomBar = {
            Column {
                SearchActionBar(
                    canSearch = validationMessage == null,
                    status = status,
                    seedsPerSecond = seedsPerSecond,
                    elapsedSeconds = elapsedSeconds,
                    refineProgress = refineProgress,
                    isSearching = isSearching,
                    onSearch = {
                        showResults = true
                        onSearch()
                    },
                    onCancel = onCancel,
                )
                bottomBar()
            }
        },
    ) { scaffoldPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(scaffoldPadding),
            contentAlignment = Alignment.TopCenter,
        ) {
            Column(
                modifier = Modifier
                    .fillMaxHeight()
                    .fillMaxWidth()
                    .widthIn(max = 680.dp),
            ) {
                PageHeader(
                    title = "Requirements (${requirements.filterNot { it.blanket }.boardCount() + if (arcaneResinAuto || arcaneResin > 0) 1 else 0})",
                    summary = listOf(requirementsSummaryText(requirements), if (arcaneResinAuto || arcaneResin > 0) (if (arcaneResinAuto) "Auto Arcane Resin" else "≥$arcaneResin Arcane Resin") else "").filter { it.isNotEmpty() }.joinToString(" · "),
                    open = !showResults,
                    openDescription = "Show requirements",
                    onOpen = { showResults = false },
                )
                if (!showResults) {
                    QueryPage(
                        floorRequirements = floorRequirements,
                        requirements = requirements,
                        maximumDepth = maximumDepth,
                        autoApplyTrinket = autoApplyTrinket,
                        arcaneResin = arcaneResin,
                        arcaneResinFilter = arcaneResinFilter,
                        arcaneResinAuto = arcaneResinAuto,
                        requireBlacksmith = requireBlacksmith,
                        excludeBlacksmithRewards = excludeBlacksmithRewards,
                        wandmakerQuest = wandmakerQuest,
                        challenges = challenges,
                        isSearching = isSearching,
                        validationMessage = validationMessage,
                        compactChips = compactChips,
                        onEditResin = onEditResin,
                        onRemoveResin = onRemoveResin,
                        onAdd = onAdd,
                        onEdit = onEdit,
                        onRequirementsChange = onRequirementsChange,
                        onRemove = onRemove,
                        onSearchSettings = onSearchSettings,
                        // Takes every line down to the closed page's header,
                        // which waits at the bottom edge above the search bar
                        // that fills it; a query taller than that scrolls.
                        modifier = Modifier.weight(1f),
                    )
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                PageHeader(
                    title = resultsHeaderText(
                        resultCount = foundCount,
                        state = status?.state,
                        isSearching = isSearching,
                        refinePhase = refinePhase,
                    ),
                    summary = null,
                    open = showResults,
                    openDescription = "Show results",
                    onOpen = { showResults = true },
                )
                if (error != null) {
                    Text(
                        error,
                        modifier = Modifier.padding(horizontal = 16.dp),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                if (showResults) {
                    LazyColumn(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth(),
                        contentPadding = PaddingValues(start = 16.dp, top = 4.dp, end = 16.dp, bottom = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        importNotice?.let { notice ->
                            item {
                                Text(
                                    notice,
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(vertical = 4.dp),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                        if (results.isEmpty()) {
                            item {
                                Text(
                                    when {
                                        isSearching -> "0 matches yet."
                                        status?.isImpossibleQuery == true ->
                                            "Impossible query. No seed can satisfy this combination of requirements."
                                        status?.state == SearchState.COMPLETED -> "0 matches."
                                        else -> "No results — run a search."
                                    },
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(vertical = 12.dp),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        } else {
                            items(results, key = { it.seed }) { result ->
                                ResultRow(result = result, onScout = { onScoutSeed(result.seed) })
                            }
                        }
                    }
                }
            }
        }
    }

    if (showPresets) {
        PresetsDialog(
            presets = presets,
            onApplyPreset = onApplyPreset,
            onSavePreset = onSavePreset,
            onDeletePreset = onDeletePreset,
            onDismiss = { showPresets = false },
        )
    }
}

/**
 * The title line of one of the finder's two pages. Only the closed page's
 * header is a control — it opens that page, in place of the other — and only
 * it carries a chevron and, when it has one, a one-line summary of what it
 * hides.
 */
@Composable
private fun PageHeader(
    title: String,
    summary: String?,
    open: Boolean,
    openDescription: String,
    onOpen: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = !open, onClick = onOpen)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.titleSmall)
        Spacer(Modifier.width(8.dp))
        if (!open && !summary.isNullOrEmpty()) {
            Text(
                summary,
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        } else {
            Spacer(Modifier.weight(1f))
        }
        if (!open) {
            Icon(
                Icons.Filled.KeyboardArrowDown,
                contentDescription = openDescription,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** What the board asks for, in a line: each slot by name, its alternatives joined by "or". */
private fun requirementsSummaryText(requirements: List<ItemRequirement>): String =
    requirements.boardItems().joinToString(" · ") { item ->
        buildString {
            if (requirements[item.anchor].blanket) append("Blanket: ")
            append(item.members.joinToString(" or ") { chipTitle(requirements[it]) })
            if (item.stackCount > 1) append(" ×${item.stackCount}")
        }
    }

/** The requirement board and a summary linking to the full search settings. */
@Composable
private fun QueryPage(
    requirements: List<ItemRequirement>,
    maximumDepth: Int,
    autoApplyTrinket: Boolean,
    floorRequirements: List<FloorRequirement>,
    arcaneResin: Int,
    arcaneResinAuto: Boolean = false,
    arcaneResinFilter: dev.seedseeker.app.model.ArcaneResinFilter,
    requireBlacksmith: Boolean,
    excludeBlacksmithRewards: Boolean,
    wandmakerQuest: WandmakerQuest?,
    challenges: Int,
    isSearching: Boolean,
    validationMessage: String?,
    compactChips: Boolean,
    onEditResin: () -> Unit,
    onRemoveResin: () -> Unit,
    onAdd: (Boolean) -> Unit,
    onEdit: (BoardItem, Int) -> Unit,
    onRequirementsChange: (List<ItemRequirement>) -> Unit,
    onRemove: (BoardItem) -> Unit,
    onSearchSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .padding(horizontal = 16.dp)
            .verticalScroll(rememberScrollState()),
    ) {
        RequirementBoard(
            requirements = requirements,
            enabled = !isSearching,
            compact = compactChips,
            onChange = onRequirementsChange,
            onEdit = onEdit,
            onRemove = onRemove,
            onAdd = { onAdd(false) },
            arcaneResin = arcaneResin,
            arcaneResinFilter = arcaneResinFilter,
            arcaneResinAuto = arcaneResinAuto,
            onEditResin = onEditResin,
            onRemoveResin = onRemoveResin,

            modifier = Modifier.fillMaxWidth(),
        )
        var blanketsExpanded by remember { mutableStateOf(false) }
        var showBlanketHelp by remember { mutableStateOf(false) }
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { blanketsExpanded = !blanketsExpanded }, modifier = Modifier.weight(1f)) {
                Text("Blanket Requirements (${requirements.filter { it.blanket }.boardCount()})",
                    modifier = Modifier.weight(1f))
                Icon(if (blanketsExpanded) Icons.Filled.KeyboardArrowUp else Icons.Filled.KeyboardArrowDown,
                    contentDescription = if (blanketsExpanded) "Collapse blankets" else "Expand blankets")
            }
            IconButton(onClick = { showBlanketHelp = true }) {
                Icon(Icons.Filled.Info, contentDescription = "About blanket requirements")
            }
        }
        if (blanketsExpanded) {
            RequirementBoard(
                requirements = requirements, blanket = true, enabled = !isSearching,
                compact = compactChips, onChange = onRequirementsChange,
                onEdit = onEdit, onRemove = onRemove, onAdd = { onAdd(true) },
                arcaneResin = 0, arcaneResinFilter = ArcaneResinFilter(),
                onEditResin = onEditResin, onRemoveResin = onRemoveResin,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        if (showBlanketHelp) {
            AlertDialog(
                onDismissRequest = { showBlanketHelp = false },
                title = { Text("Blanket Requirements") },
                text = { Text("Each blanket must match at least one item fulfilling your ordinary requirements or contributing Arcane Resin. " +
                    "It does not ask for an additional item. All filters in one blanket apply to the same item; " +
                    "separate blankets can match the same or different chosen items.\n\n" +
                    "For example, require Lightning, Disintegration, and Frost at +2 or higher, then add an " +
                    "Any wand blanket at exactly +3 from the Wandmaker.") },
                confirmButton = { TextButton(onClick = { showBlanketHelp = false }) { Text("Got it") } },
            )
        }
        if (validationMessage != null && (requirements.isNotEmpty() || floorRequirements.isNotEmpty() || (arcaneResinAuto || arcaneResin > 0))) {
            Text(
                validationMessage,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
        Spacer(Modifier.height(4.dp))
        SearchSettingsLink(
            summary = listOfNotNull(
                scopeSummaryText(maximumDepth, requireBlacksmith, excludeBlacksmithRewards, wandmakerQuest, challenges),
                "AutoTrinket off".takeUnless { autoApplyTrinket },
                floorRequirements.takeIf { it.isNotEmpty() }?.joinToString(", ", prefix = "Required floors: ") { "${it.depth}" },
            ).joinToString(" · "),
            onClick = onSearchSettings,
        )
        Spacer(Modifier.height(6.dp))
    }
}

@Composable
private fun ResultRow(result: SeedResult, onScout: () -> Unit) {
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    Surface(
        onClick = onScout,
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier.padding(start = 14.dp, top = 2.dp, end = 2.dp, bottom = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                result.seed,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                letterSpacing = 1.sp,
                color = MaterialTheme.colorScheme.tertiary,

            )
                result.selectedTrinket?.let(ItemCatalog::findById)?.let { ItemSprite(it, modifier = Modifier.size(16.dp).alpha(0.6f)) }
            }
            TextButton(
                onClick = {
                    scope.launch {
                        clipboard.setClipEntry(ClipData.newPlainText("Seed", result.seed).toClipEntry())
                    }
                },
            ) {
                Text("Copy")
            }
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SearchActionBar(
    canSearch: Boolean,
    status: SearchStatus?,
    seedsPerSecond: Double,
    elapsedSeconds: Long,
    refineProgress: RefineProgress?,
    isSearching: Boolean,
    onSearch: () -> Unit,
    onCancel: () -> Unit,
) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainer) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 10.dp),
        ) {
            if (isSearching) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            if (refineProgress != null) "Checking saved seeds · ${formatElapsedTime(elapsedSeconds)}"
                            else "${formatSeedRate(seedsPerSecond)} seeds/s · " +
                                "${formatElapsedTime(elapsedSeconds)} · " +
                                "${compactCount(status?.scannedSeeds ?: 0L)} scanned",
                            style = MaterialTheme.typography.labelLarge,
                        )
                        Text(
                            if (refineProgress != null) "${refineProgress.checked} of ${refineProgress.total} checked"
                            else searchEstimateText(status, seedsPerSecond),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Spacer(Modifier.width(10.dp))
                    OutlinedButton(onClick = onCancel, shapes = ButtonDefaults.shapes()) {
                        Text("Cancel")
                    }
                }
            } else {
                Button(
                    onClick = onSearch,
                    enabled = canSearch,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(48.dp),
                    shapes = ButtonDefaults.shapes(),
                ) {
                    Text("Search", style = MaterialTheme.typography.titleMedium)
                }
            }
        }
    }
}

@Composable
private fun PresetsDialog(
    presets: List<QueryPreset>,
    onApplyPreset: (QueryPreset) -> Unit,
    onSavePreset: (String) -> Unit,
    onDeletePreset: (QueryPreset) -> Unit,
    onDismiss: () -> Unit,
) {
    var presetName by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Presets") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                presets.forEach { preset ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        TextButton(
                            onClick = {
                                onApplyPreset(preset)
                                onDismiss()
                            },
                            modifier = Modifier.weight(1f),
                            contentPadding = PaddingValues(horizontal = 4.dp),
                        ) {
                            Text(
                                preset.name,
                                modifier = Modifier.fillMaxWidth(),
                                textAlign = TextAlign.Start,
                            )
                        }
                        if (!preset.isBuiltIn) {
                            TextButton(onClick = { onDeletePreset(preset) }) { Text("Delete") }
                        }
                    }
                }
                OutlinedTextField(
                    value = presetName,
                    onValueChange = { presetName = it },
                    label = { Text("New preset name") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(
                    onClick = {
                        onSavePreset(presetName)
                        presetName = ""
                    },
                    enabled = presetName.isNotBlank(),
                    modifier = Modifier.fillMaxWidth(),
                ) { Text("Save current query") }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Done") }
        },
    )
}
