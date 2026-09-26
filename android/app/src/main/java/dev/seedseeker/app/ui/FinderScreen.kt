// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.ClipData
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ContainedLoadingIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Snackbar
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.rotate
import androidx.compose.animation.core.Animatable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.Measurable
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.zIndex
import kotlin.math.roundToInt
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.toClipEntry
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.graphics.shapes.RoundedPolygon
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.BoardItem
import dev.seedseeker.app.model.FloorRequirement
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.QueryPreset
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.model.WandmakerQuest
import dev.seedseeker.app.model.boardCount
import dev.seedseeker.app.model.boardItems
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun FinderScreen(
    requirements: List<ItemRequirement>,
    maximumDepth: Int,
    autoApplyTrinket: Boolean,
    floorRequirements: List<FloorRequirement>,
    arcaneResin: Int,
    arcaneResinAuto: Boolean = false,
    arcaneResinFilter: ArcaneResinFilter,
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
    isPreparing: Boolean = false,
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
    onImportClipboard: () -> Unit,
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
    // The two pages are one accordion: turning the page morphs the space from
    // one to the other on a spring rather than swapping them outright.
    // The fraction is read only while measuring and drawing, so a page turn
    // re-lays out the screen each frame without recomposing the board.
    val pageAnimation = remember { Animatable(if (showResults) 1f else 0f) }
    LaunchedEffect(showResults) { pageAnimation.animateTo(if (showResults) 1f else 0f, LayoutFractionSpring) }
    val resultsFraction = { pageAnimation.value.coerceIn(0f, 1f) }
    // Whether each page is on screen at all; these flip only at the ends of a turn.
    val queryShown by remember { derivedStateOf { pageAnimation.value < 1f } }
    val resultsShown by remember { derivedStateOf { pageAnimation.value > 0f } }
    val hasResin = arcaneResinAuto || arcaneResin > 0
    // New rows spring in once; rows already seen stay put when the page turns
    // or the user comes back from another tab.
    val entrances = LocalEntranceMemory.current
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        snackbarHost = {
            SnackbarHost(snackbarHostState) { data ->
                Snackbar(data, shape = MaterialTheme.shapes.large, modifier = Modifier.springEntrance(rise = 24f))
            }
        },
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        BrandMark(Modifier.size(32.dp))
                        Spacer(Modifier.width(10.dp))
                        Text("Seed Seeker", fontWeight = FontWeight.ExtraBold, maxLines = 1, softWrap = false)
                    }
                },
                actions = {
                    val presetInteraction = remember { MutableInteractionSource() }
                    FilledTonalButton(
                        onClick = { showPresets = true },
                        enabled = !isSearching,
                        shapes = ButtonDefaults.shapes(),
                        interactionSource = presetInteraction,
                        contentPadding = PaddingValues(start = 10.dp, end = 14.dp),
                        modifier = Modifier.height(36.dp).pressScale(presetInteraction),
                    ) {
                        Icon(Icons.Filled.Star, contentDescription = null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(6.dp))
                        Text("Presets")
                    }
                    IconButton(onClick = onSettings, shapes = IconButtonDefaults.shapes()) {
                        Icon(Icons.Filled.Settings, contentDescription = "Settings")
                    }
                    Box {
                        IconButton(onClick = { showOverflowMenu = true }, shapes = IconButtonDefaults.shapes()) {
                            Icon(Icons.Filled.MoreVert, contentDescription = "More options")
                        }
                        DropdownMenu(
                            expanded = showOverflowMenu,
                            onDismissRequest = { showOverflowMenu = false },
                            shape = MaterialTheme.shapes.large,
                        ) {
                            DropdownMenuItem(
                                text = { Text("About & licenses") },
                                leadingIcon = { Icon(Icons.Filled.Info, contentDescription = null) },
                                onClick = {
                                    showOverflowMenu = false
                                    onAbout()
                                },
                            )
                            HorizontalDivider(Modifier.padding(vertical = 4.dp))
                            DropdownMenuItem(
                                text = { Text("Share search…") },
                                enabled = requirements.isNotEmpty() || floorRequirements.isNotEmpty() || hasResin,
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
                                text = { Text("Import from clipboard") },
                                enabled = !isSearching,
                                onClick = {
                                    showOverflowMenu = false
                                    onImportClipboard()
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
                    isPreparing = isPreparing,
                    foundCount = foundCount,
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
            PageAccordion(
                fraction = resultsFraction,
                modifier = Modifier
                    .fillMaxHeight()
                    .fillMaxWidth()
                    .widthIn(max = 680.dp),
            ) {
                val boardCount = requirements.filterNot { it.blanket }.boardCount() + if (hasResin) 1 else 0
                PageHeader(
                    title = "Requirements",
                    count = boardCount,
                    polygon = MaterialShapes.Clover4Leaf,
                    accent = MaterialTheme.colorScheme.primaryContainer,
                    onAccent = MaterialTheme.colorScheme.onPrimaryContainer,
                    summary = listOf(
                        requirementsSummaryText(requirements),
                        if (hasResin) (if (arcaneResinAuto) "Auto Arcane Resin" else "≥$arcaneResin Arcane Resin") else "",
                    ).filter { it.isNotEmpty() }.joinToString(" · "),
                    open = !showResults,
                    openFraction = { 1f - resultsFraction() },
                    openDescription = "Show requirements",
                    onOpen = { showResults = false },
                )
                if (queryShown) {
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
                        modifier = Modifier
                            .layoutId(QueryPageId)
                            .clipToBounds()
                            .graphicsLayer { alpha = 1f - resultsFraction() },
                    )
                }
                HorizontalDivider(
                    modifier = Modifier.padding(horizontal = 16.dp),
                    color = MaterialTheme.colorScheme.outlineVariant,
                )
                ResultsHeader(
                    resultCount = foundCount,
                    status = status,
                    isSearching = isSearching,
                    refinePhase = refinePhase,
                    open = showResults,
                    openFraction = resultsFraction,
                    onOpen = { showResults = true },
                )
                AnimatedVisibility(
                    visible = error != null,
                    enter = expandVertically() + fadeIn(),
                    exit = shrinkVertically() + fadeOut(),
                ) {
                    NoticeBanner(
                        text = error.orEmpty(),
                        container = MaterialTheme.colorScheme.errorContainer,
                        content = MaterialTheme.colorScheme.onErrorContainer,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                    )
                }
                if (resultsShown) {
                    LazyColumn(
                        modifier = Modifier
                            .layoutId(ResultsPageId)
                            .fillMaxWidth()
                            .clipToBounds()
                            .graphicsLayer { alpha = resultsFraction() },
                        contentPadding = PaddingValues(start = 16.dp, top = 4.dp, end = 16.dp, bottom = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        importNotice?.let { notice ->
                            item(key = "import-notice") {
                                NoticeBanner(
                                    text = notice,
                                    container = MaterialTheme.colorScheme.secondaryContainer,
                                    content = MaterialTheme.colorScheme.onSecondaryContainer,
                                    modifier = Modifier.animateItem().springEntrance(),
                                )
                            }
                        }
                        if (results.isEmpty()) {
                            item(key = "empty") {
                                ResultsEmptyState(
                                    isSearching = isSearching,
                                    impossible = status?.isImpossibleQuery == true,
                                    completed = status?.state == SearchState.COMPLETED,
                                    modifier = Modifier.animateItem(),
                                )
                            }
                        } else {
                            itemsIndexed(results, key = { _, result -> result.seed }) { index, result ->
                                val fresh = remember(result.seed) { entrances.firstTime("result:${result.seed}") }
                                ResultRow(
                                    result = result,
                                    rank = index + 1,
                                    onScout = { onScoutSeed(result.seed) },
                                    modifier = Modifier
                                        .animateItem()
                                        .springEntrance(enabled = fresh, delayMillis = if (fresh) (index % 12) * 28 else 0),
                                )
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

private const val QueryPageId = "query-page"
private const val ResultsPageId = "results-page"

/**
 * A column whose two pages share the height left over by everything else:
 * the requirements page gets `1 - fraction` of it and the results page
 * `fraction`, so a page turn morphs one into the other. A page on its own
 * takes its share of the whole space; the rest of the children stack at
 * their natural heights, in order.
 */
@Composable
private fun PageAccordion(fraction: () -> Float, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val loose = constraints.copy(minHeight = 0)
        val isPage = { m: Measurable -> m.layoutId == QueryPageId || m.layoutId == ResultsPageId }
        val fixed = measurables.filterNot(isPage).associateWith { it.measure(loose) }
        val remaining = (constraints.maxHeight - fixed.values.sumOf { it.height }).coerceAtLeast(0)
        val f = fraction()
        val hasQuery = measurables.any { it.layoutId == QueryPageId }
        val queryHeight = if (hasQuery) (remaining * (1f - f)).roundToInt().coerceIn(0, remaining) else 0
        val resultsHeight = if (hasQuery) remaining - queryHeight else (remaining * f).roundToInt().coerceIn(0, remaining)
        val placeables = measurables.map { m ->
            val height = if (m.layoutId == QueryPageId) queryHeight else resultsHeight
            fixed[m] ?: m.measure(loose.copy(minHeight = height, maxHeight = height))
        }
        layout(constraints.maxWidth, constraints.maxHeight) {
            var y = 0
            placeables.forEach {
                it.placeRelative(0, y)
                y += it.height
            }
        }
    }
}

/**
 * The title line of the requirements page. Its count sits in a little
 * clover that shouts when the count changes. Only the closed page's header
 * is a control — it opens that page, in place of the other — and only it
 * carries a chevron and a one-line summary of what it hides.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PageHeader(
    title: String,
    count: Int,
    polygon: RoundedPolygon,
    accent: Color,
    onAccent: Color,
    summary: String?,
    open: Boolean,
    openFraction: () -> Float,
    openDescription: String,
    onOpen: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = !open, onClick = onOpen)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ShapeBackdrop(polygon, accent, Modifier.size(30.dp).popOnChange(count)) {
            Text(
                "$count",
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.Bold,
                color = onAccent,
            )
        }
        Spacer(Modifier.width(10.dp))
        Text(title, style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.width(8.dp))
        if (!summary.isNullOrEmpty() && !open) {
            Text(
                summary,
                modifier = Modifier.weight(1f).graphicsLayer { alpha = 1f - openFraction() },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        } else {
            Spacer(Modifier.weight(1f))
        }
        HeaderChevron(open = open, openFraction = openFraction, pointsDown = true, description = openDescription)
    }
}

/**
 * The chevron on a page header: it turns as the page opens and fades away
 * once the page is the one on show, since an open page has nothing to open.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun HeaderChevron(open: Boolean, openFraction: () -> Float, pointsDown: Boolean, description: String) {
    if (open) return
    Surface(
        shape = CircleShape,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.size(28.dp).graphicsLayer { alpha = 1f - openFraction() },
    ) {
        Icon(
            Icons.Filled.KeyboardArrowDown,
            contentDescription = if (open) null else description,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier
                .padding(4.dp)
                .graphicsLayer { rotationZ = (if (pointsDown) 0f else 180f) + 180f * openFraction() },
        )
    }
}

/**
 * The results page's title: a live light and status while a search runs,
 * and a count in a seal that shouts every time the engine lands a match.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ResultsHeader(
    resultCount: Int,
    status: SearchStatus?,
    isSearching: Boolean,
    refinePhase: RefinePhase?,
    open: Boolean,
    openFraction: () -> Float,
    onOpen: () -> Unit,
) {
    val description = resultsHeaderText(resultCount, status?.state, isSearching, refinePhase)
    val stateLabel = when {
        isSearching && refinePhase == RefinePhase.FILTERING -> "refining"
        isSearching && refinePhase == RefinePhase.SCANNING -> "searching"
        isSearching -> "live"
        status?.state == SearchState.COMPLETED -> "complete"
        status?.state == SearchState.CANCELLED -> "stopped"
        else -> null
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            // Above the list below, so its match bursts are never hidden under rows.
            .zIndex(1f)
            .clickable(enabled = !open, onClick = onOpen)
            .semantics { contentDescription = description }
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ShapeBackdrop(
            polygon = SeekerShapes.Seed,
            color = if (resultCount > 0) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.surfaceContainerHighest,
            modifier = Modifier
                .size(30.dp)
                .popOnChange(resultCount, peak = 1.35f)
                // Bursts at each milestone, and once more when a search completes with finds.
                .celebrate(
                    milestoneOf(resultCount) * 2 + if (status?.state == SearchState.COMPLETED && resultCount > 0) 1 else 0,
                    CelebrationColors, count = 12, reach = 38f,
                ),
        ) {
            Text(
                compactCount(resultCount.toLong()),
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.Bold,
                color = if (resultCount > 0) MaterialTheme.colorScheme.onTertiary else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
        }
        Spacer(Modifier.width(10.dp))
        Text("Results", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.width(10.dp))
        AnimatedContent(
            targetState = stateLabel,
            transitionSpec = {
                (slideInVertically { it } + fadeIn()).togetherWith(slideOutVertically { -it } + fadeOut())
            },
            label = "results-state",
        ) { label ->
            if (label != null) {
                Surface(
                    shape = CircleShape,
                    color = if (isSearching) MaterialTheme.colorScheme.primaryContainer
                    else MaterialTheme.colorScheme.surfaceContainerHigh,
                ) {
                    Row(
                        Modifier.padding(start = 8.dp, end = 10.dp, top = 3.dp, bottom = 3.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        if (isSearching) {
                            PulsingDot(MaterialTheme.colorScheme.primary, Modifier.size(10.dp))
                            Spacer(Modifier.width(6.dp))
                        }
                        Text(
                            label,
                            style = MaterialTheme.typography.labelMedium,
                            color = if (isSearching) MaterialTheme.colorScheme.onPrimaryContainer
                            else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
        Spacer(Modifier.weight(1f))
        HeaderChevron(open = open, openFraction = openFraction, pointsDown = false, description = "Show results")
    }
}

/** The celebration milestones a result count crosses: the first find, then ever rarer round numbers. */
internal fun milestoneOf(count: Int): Int =
    listOf(1, 5, 10, 25, 50, 100, 250, 500, 1_000).count { count >= it }

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
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun QueryPage(
    requirements: List<ItemRequirement>,
    maximumDepth: Int,
    autoApplyTrinket: Boolean,
    floorRequirements: List<FloorRequirement>,
    arcaneResin: Int,
    arcaneResinAuto: Boolean = false,
    arcaneResinFilter: ArcaneResinFilter,
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
            modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
        )
        AnimatedVisibility(
            visible = validationMessage != null && (requirements.isNotEmpty() || floorRequirements.isNotEmpty() || (arcaneResinAuto || arcaneResin > 0)),
            enter = expandVertically(LayoutSizeSpring) + fadeIn(),
            exit = shrinkVertically(LayoutSizeSpring) + fadeOut(),
        ) {
            // Keep the last message on screen while the banner folds away.
            var shown by remember { mutableStateOf(validationMessage.orEmpty()) }
            if (validationMessage != null) shown = validationMessage
            NoticeBanner(
                text = shown,
                container = MaterialTheme.colorScheme.errorContainer,
                content = MaterialTheme.colorScheme.onErrorContainer,
                icon = true,
                modifier = Modifier.padding(top = 10.dp).shakeOnChange(shown),
            )
        }
        Spacer(Modifier.height(10.dp))
        var blanketsExpanded by remember { mutableStateOf(false) }
        var showBlanketHelp by remember { mutableStateOf(false) }
        val blanketCount = requirements.filter { it.blanket }.boardCount()
        Surface(
            shape = MaterialTheme.shapes.extraLarge,
            color = MaterialTheme.colorScheme.surfaceContainerLow,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Row(
                        Modifier
                            .weight(1f)
                            .clip(MaterialTheme.shapes.extraLarge)
                            .clickable { blanketsExpanded = !blanketsExpanded }
                            .semantics {
                                contentDescription = if (blanketsExpanded) "Collapse blankets" else "Expand blankets"
                            }
                            .padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        ShapeBackdrop(
                            MaterialShapes.Puffy,
                            if (blanketCount > 0) MaterialTheme.colorScheme.secondaryContainer
                            else MaterialTheme.colorScheme.surfaceContainerHighest,
                            Modifier.size(26.dp).popOnChange(blanketCount),
                        ) {
                            Text(
                                "$blanketCount",
                                style = MaterialTheme.typography.labelMedium,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSecondaryContainer,
                            )
                        }
                        Spacer(Modifier.width(10.dp))
                        Text(
                            "Blanket Requirements",
                            style = MaterialTheme.typography.titleSmall,
                            modifier = Modifier.weight(1f),
                        )
                        val turn by animateFloatAsState(
                            if (blanketsExpanded) 180f else 0f,
                            MaterialTheme.motionScheme.fastSpatialSpec(),
                            label = "blanket-chevron",
                        )
                        Icon(
                            Icons.Filled.KeyboardArrowDown,
                            contentDescription = null,
                            modifier = Modifier.rotate(turn),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    IconButton(onClick = { showBlanketHelp = true }, shapes = IconButtonDefaults.shapes()) {
                        Icon(
                            Icons.Filled.Info,
                            contentDescription = "About blanket requirements",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                AnimatedVisibility(
                    visible = blanketsExpanded,
                    enter = expandVertically(LayoutSizeSpring) + fadeIn(),
                    exit = shrinkVertically(LayoutSizeSpring) + fadeOut(),
                ) {
                    RequirementBoard(
                        requirements = requirements, blanket = true, enabled = !isSearching,
                        compact = compactChips, onChange = onRequirementsChange,
                        onEdit = onEdit, onRemove = onRemove, onAdd = { onAdd(true) },
                        arcaneResin = 0, arcaneResinFilter = ArcaneResinFilter(),
                        onEditResin = onEditResin, onRemoveResin = onRemoveResin,
                        modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 14.dp),
                    )
                }
            }
        }
        if (showBlanketHelp) {
            AlertDialog(
                onDismissRequest = { showBlanketHelp = false },
                icon = {
                    ShapeBackdrop(MaterialShapes.Puffy, MaterialTheme.colorScheme.secondaryContainer, Modifier.size(48.dp)) {
                        Icon(Icons.Filled.Info, contentDescription = null, tint = MaterialTheme.colorScheme.onSecondaryContainer)
                    }
                },
                title = { Text("Blanket Requirements") },
                text = { Text("Each blanket must match at least one item fulfilling your ordinary requirements or contributing Arcane Resin. " +
                    "It does not ask for an additional item. All filters in one blanket apply to the same item; " +
                    "separate blankets can match the same or different chosen items.\n\n" +
                    "For example, require Lightning, Disintegration, and Frost at +2 or higher, then add an " +
                    "Any wand blanket at exactly +3 from the Wandmaker.") },
                confirmButton = { TextButton(onClick = { showBlanketHelp = false }) { Text("Got it") } },
            )
        }
        Spacer(Modifier.height(12.dp))
        SearchSettingsLink(
            summary = listOfNotNull(
                scopeSummaryText(maximumDepth, requireBlacksmith, excludeBlacksmithRewards, wandmakerQuest, challenges),
                "AutoTrinket off".takeUnless { autoApplyTrinket },
                floorRequirements.takeIf { it.isNotEmpty() }?.joinToString(", ", prefix = "Required floors: ") { "${it.depth}" },
            ).joinToString(" · "),
            onClick = onSearchSettings,
        )
        // Let the settings card scroll clear of the fixed Results header for easier tapping.
        Spacer(Modifier.height(64.dp))
    }
}

/** A rounded, tinted message line: import notices, search errors and validation problems. */
@Composable
private fun NoticeBanner(
    text: String,
    container: Color,
    content: Color,
    icon: Boolean = false,
    modifier: Modifier = Modifier,
) {
    Surface(shape = MaterialTheme.shapes.large, color = container, modifier = modifier.fillMaxWidth()) {
        Row(Modifier.padding(horizontal = 14.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            if (icon) {
                Icon(Icons.Filled.Warning, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(10.dp))
            }
            Text(text, style = MaterialTheme.typography.bodySmall, color = content)
        }
    }
}

/**
 * Nothing to list yet. Idle, a slowly morphing seal invites a search; while a
 * search runs the seal turns into a busier, faster shape-shifter; a query no
 * seed can ever satisfy gets a spiky warning instead.
 */
@Composable
private fun ResultsEmptyState(isSearching: Boolean, impossible: Boolean, completed: Boolean, modifier: Modifier = Modifier) {
    val message = when {
        isSearching -> "0 matches yet."
        impossible -> "Impossible query. No seed can satisfy this combination of requirements."
        completed -> "0 matches."
        else -> "No results — run a search."
    }
    val hint = when {
        isSearching -> "Combing the dungeon, floor by floor. Matches appear here the moment they surface."
        impossible -> "Loosen a requirement, or raise the floor limit, and try again."
        completed -> "Every seed was checked. Try relaxing an upgrade or effect."
        else -> "Tap Search and every match will land here, ready to scout."
    }
    Column(
        modifier.fillMaxWidth().padding(top = 28.dp, bottom = 16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        AnimatedContent(
            targetState = Triple(isSearching, impossible, completed),
            transitionSpec = {
                (scaleIn(spring(0.55f, 300f), initialScale = 0.6f) + fadeIn())
                    .togetherWith(scaleOut(targetScale = 0.6f) + fadeOut())
                    .using(SizeTransform(clip = false))
            },
            label = "empty-illustration",
        ) { (searching, isImpossible, _) ->
            when {
                isImpossible -> ShapeBackdrop(
                    MaterialShapes.SoftBoom,
                    MaterialTheme.colorScheme.errorContainer,
                    Modifier.size(96.dp),
                ) {
                    Icon(
                        Icons.Filled.Warning,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onErrorContainer,
                        modifier = Modifier.size(36.dp),
                    )
                }
                searching -> MorphingBackdrop(
                    shapes = SeekerShapes.Busy,
                    color = MaterialTheme.colorScheme.primaryContainer,
                    modifier = Modifier.size(104.dp),
                    stepMillis = 650,
                    turning = true,
                ) {
                    Icon(
                        Icons.Filled.Search,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.size(38.dp),
                    )
                }
                else -> MorphingBackdrop(
                    shapes = SeekerShapes.Idle,
                    color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    modifier = Modifier.size(104.dp),
                    stepMillis = 2400,
                ) {
                    Icon(
                        Icons.Filled.Search,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(38.dp),
                    )
                }
            }
        }
        Spacer(Modifier.height(18.dp))
        Text(
            message,
            style = MaterialTheme.typography.titleMedium,
            textAlign = TextAlign.Center,
            color = if (impossible) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            hint,
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 24.dp),
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ResultRow(result: SeedResult, rank: Int, onScout: () -> Unit, modifier: Modifier = Modifier) {
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) {
        if (copied) {
            delay(1600)
            copied = false
        }
    }
    val interaction = remember { MutableInteractionSource() }
    Surface(
        onClick = onScout,
        shape = MaterialTheme.shapes.extraLarge,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        interactionSource = interaction,
        modifier = modifier.fillMaxWidth().pressScale(interaction, pressed = 0.97f),
    ) {
        Row(
            modifier = Modifier.padding(start = 10.dp, top = 6.dp, end = 4.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "$rank",
                modifier = Modifier.widthIn(min = 28.dp),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                maxLines = 1,
            )
            Spacer(Modifier.width(8.dp))
            Row(
                Modifier.weight(1f),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    result.seed,
                    fontFamily = FontFamily.Monospace,
                    fontWeight = FontWeight.Bold,
                    fontSize = 19.sp,
                    letterSpacing = 1.sp,
                    color = MaterialTheme.colorScheme.tertiary,
                    maxLines = 1,
                )
                result.selectedTrinket?.let(ItemCatalog::findById)?.let {
                    Surface(shape = CircleShape, color = MaterialTheme.colorScheme.surfaceContainerHighest) {
                        ItemSprite(it, modifier = Modifier.padding(4.dp).size(18.dp))
                    }
                }
            }
            TextButton(
                onClick = {
                    scope.launch {
                        clipboard.setClipEntry(ClipData.newPlainText("Seed", result.seed).toClipEntry())
                    }
                    copied = true
                },
                shapes = ButtonDefaults.shapes(),
                contentPadding = PaddingValues(horizontal = 10.dp),
            ) {
                AnimatedContent(
                    targetState = copied,
                    transitionSpec = {
                        (scaleIn(spring(0.5f, 500f), initialScale = 0.5f) + fadeIn())
                            .togetherWith(scaleOut(targetScale = 0.5f) + fadeOut())
                            .using(SizeTransform(clip = false))
                    },
                    label = "copy-state",
                ) { done ->
                    if (done) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(16.dp))
                            Spacer(Modifier.width(4.dp))
                            Text("Copied")
                        }
                    } else {
                        Text("Copy")
                    }
                }
            }
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/**
 * The finder's hero control. At rest it is a big, bouncy Search button; the
 * moment a search starts it morphs into a live panel — a shape-shifting
 * loading seal, a wavy progress line and the numbers that matter, with the
 * match count shouting and throwing sparkles as seeds turn up.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SearchActionBar(
    canSearch: Boolean,
    status: SearchStatus?,
    seedsPerSecond: Double,
    elapsedSeconds: Long,
    refineProgress: RefineProgress?,
    isSearching: Boolean,
    isPreparing: Boolean = false,
    foundCount: Int,
    onSearch: () -> Unit,
    onCancel: () -> Unit,
) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
    ) {
        AnimatedContent(
            targetState = isSearching,
            transitionSpec = {
                (fadeIn(tween(220, delayMillis = 60)) + scaleIn(spring(0.7f, 380f), initialScale = 0.85f))
                    .togetherWith(fadeOut(tween(100)) + scaleOut(targetScale = 0.9f))
                    .using(SizeTransform(clip = false) { _, _ -> LayoutSizeSpring })
            },
            contentAlignment = Alignment.Center,
            label = "search-bar",
        ) { searching ->
            if (searching) {
                SearchingPanel(
                    status = status,
                    seedsPerSecond = seedsPerSecond,
                    elapsedSeconds = elapsedSeconds,
                    refineProgress = refineProgress,
                    isPreparing = isPreparing,
                    foundCount = foundCount,
                    onCancel = onCancel,
                )
            } else {
                val interaction = remember { MutableInteractionSource() }
                Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
                    Button(
                        onClick = onSearch,
                        enabled = canSearch,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(60.dp)
                            .pressScale(interaction, pressed = 0.95f)
                            // The query just became runnable: the button says so.
                            .popOnChange(canSearch, peak = 1.05f),
                        shapes = ButtonDefaults.shapes(),
                        interactionSource = interaction,
                    ) {
                        ShapeBackdrop(
                            SeekerShapes.Seed,
                            if (canSearch) MaterialTheme.colorScheme.onPrimary.copy(alpha = 0.16f) else Color.Transparent,
                            Modifier.size(34.dp),
                        ) {
                            Icon(Icons.Filled.Search, contentDescription = null, modifier = Modifier.size(20.dp))
                        }
                        Spacer(Modifier.width(10.dp))
                        Text("Search", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SearchingPanel(
    status: SearchStatus?,
    seedsPerSecond: Double,
    elapsedSeconds: Long,
    refineProgress: RefineProgress?,
    isPreparing: Boolean,
    foundCount: Int,
    onCancel: () -> Unit,
) {
    val haptics = LocalHapticFeedback.current
    // The first match of a run is worth a tap on the wrist — but only when it
    // lands while watching, not when coming back to a run that already has some.
    var awaitingFirst by remember { mutableStateOf(foundCount == 0) }
    LaunchedEffect(foundCount > 0) {
        if (foundCount > 0 && awaitingFirst) {
            awaitingFirst = false
            haptics.performHapticFeedback(HapticFeedbackType.Confirm)
        }
    }
    Column(Modifier.fillMaxWidth().padding(bottom = 12.dp)) {
        if (refineProgress != null && refineProgress.total > 0) {
            LinearWavyProgressIndicator(
                progress = { refineProgress.checked.toFloat() / refineProgress.total },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 10.dp),
            )
        } else {
            LinearWavyProgressIndicator(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 10.dp),
            )
        }
        Row(
            Modifier.padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ContainedLoadingIndicator(
                modifier = Modifier.size(48.dp),
                containerColor = MaterialTheme.colorScheme.primaryContainer,
                indicatorColor = MaterialTheme.colorScheme.primary,
            )
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    when {
                        isPreparing -> "Preparing search…"
                        refineProgress != null -> "Checking saved seeds"
                        else -> "Searching the dungeon"
                    },
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    if (refineProgress != null) "${refineProgress.checked} of ${refineProgress.total} checked"
                    else searchEstimateText(status, seedsPerSecond),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(Modifier.width(8.dp))
            val stopInteraction = remember { MutableInteractionSource() }
            FilledTonalButton(
                onClick = onCancel,
                shapes = ButtonDefaults.shapes(),
                interactionSource = stopInteraction,
                modifier = Modifier.pressScale(stopInteraction),
                contentPadding = PaddingValues(start = 12.dp, end = 16.dp),
            ) {
                // A drawn stop square: the one glyph the core icon set lacks.
                Box(
                    Modifier
                        .size(12.dp)
                        .clip(RoundedCornerShape(3.dp))
                        .background(MaterialTheme.colorScheme.onSecondaryContainer),
                )
                Spacer(Modifier.width(8.dp))
                Text("Cancel")
            }
        }
        Spacer(Modifier.height(10.dp))
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            StatTile(
                value = if (isPreparing || refineProgress != null) "—" else formatSeedRate(seedsPerSecond),
                label = "seeds/s",
            )
            StatTile(value = formatElapsedTime(elapsedSeconds), label = "elapsed")
            StatTile(
                value = if (refineProgress != null) "${refineProgress.checked}" else compactCount(status?.scannedSeeds ?: 0L),
                label = if (refineProgress != null) "checked" else "scanned",
            )
            StatTile(
                value = compactCount(foundCount.toLong()),
                label = "found",
                highlighted = foundCount > 0,
                modifier = Modifier.popOnChange(foundCount, peak = 1.15f),
            )
        }
    }
}

/** One number in the live search panel, big and tabular, with its unit below. */
@Composable
private fun RowScope.StatTile(value: String, label: String, modifier: Modifier = Modifier, highlighted: Boolean = false) {
    Surface(
        shape = MaterialTheme.shapes.large,
        color = if (highlighted) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = modifier.weight(1f),
    ) {
        Column(
            Modifier.padding(vertical = 6.dp, horizontal = 4.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                value,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                fontFamily = FontFamily.Monospace,
                color = if (highlighted) MaterialTheme.colorScheme.onTertiary else MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
            )
            Text(
                label,
                style = MaterialTheme.typography.labelSmall,
                color = if (highlighted) MaterialTheme.colorScheme.onTertiary.copy(alpha = 0.8f)
                else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
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
        icon = {
            ShapeBackdrop(MaterialShapes.Sunny, MaterialTheme.colorScheme.tertiaryContainer, Modifier.size(52.dp)) {
                Icon(Icons.Filled.Star, contentDescription = null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
            }
        },
        title = { Text("Presets") },
        text = {
            Column(
                Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                presets.forEachIndexed { index, preset ->
                    PresetRow(
                        preset = preset,
                        onApply = {
                            onApplyPreset(preset)
                            onDismiss()
                        },
                        onDelete = { onDeletePreset(preset) },
                        modifier = Modifier.springEntrance(delayMillis = index * 35),
                    )
                }
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = presetName,
                    onValueChange = { presetName = it },
                    label = { Text("New preset name") },
                    singleLine = true,
                    shape = MaterialTheme.shapes.large,
                    modifier = Modifier.fillMaxWidth(),
                )
                val saveInteraction = remember { MutableInteractionSource() }
                Button(
                    onClick = {
                        onSavePreset(presetName)
                        presetName = ""
                    },
                    enabled = presetName.isNotBlank(),
                    shapes = ButtonDefaults.shapes(),
                    interactionSource = saveInteraction,
                    modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).pressScale(saveInteraction),
                ) { Text("Save current query") }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Done") }
        },
    )
}

/** A preset: a peek at the items it asks for, its name, and — for your own — a delete. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PresetRow(preset: QueryPreset, onApply: () -> Unit, onDelete: () -> Unit, modifier: Modifier = Modifier) {
    val interaction = remember { MutableInteractionSource() }
    Surface(
        onClick = onApply,
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainerHighest,
        interactionSource = interaction,
        modifier = modifier.fillMaxWidth().pressScale(interaction, pressed = 0.96f),
    ) {
        Row(Modifier.padding(start = 8.dp, top = 6.dp, bottom = 6.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            // The first few requirements, fanned like a hand of cards.
            Box(Modifier.width(56.dp).height(36.dp)) {
                preset.query.requirements.filterNot { it.blanket }.take(3).forEachIndexed { index, requirement ->
                    SpriteTile(
                        item = requirement.item,
                        wildcardKind = requirement.kind,
                        tileSize = 30,
                        modifier = Modifier
                            .padding(start = (index * 12).dp, top = 3.dp)
                            .rotate((index - 1) * 8f),
                    )
                }
            }
            Spacer(Modifier.width(8.dp))
            Text(
                preset.name,
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.weight(1f),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (!preset.isBuiltIn) {
                FilledTonalIconButton(
                    onClick = onDelete,
                    shapes = IconButtonDefaults.shapes(),
                    modifier = Modifier.size(36.dp),
                ) {
                    Icon(Icons.Filled.Delete, contentDescription = "Delete ${preset.name}", modifier = Modifier.size(18.dp))
                }
            }
        }
    }
}
