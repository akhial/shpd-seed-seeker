// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.BitmapFactory
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.TextButton
import dev.seedseeker.app.model.DailyRunDate
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import dev.seedseeker.app.model.FloorFeeling
import kotlin.math.roundToInt
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.rememberScrollableState
import androidx.compose.foundation.gestures.scrollable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.DateRange
import androidx.compose.material.icons.outlined.Place
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.layout
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.sp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.CatalogItem
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.engine.ScoutMatches
import dev.seedseeker.app.engine.SeedCode
import dev.seedseeker.app.engine.isMapDepthSupported
import dev.seedseeker.app.model.RingGems
import dev.seedseeker.app.model.ScoutAccessibility
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutQuest
import dev.seedseeker.app.model.ScoutWorld
import dev.seedseeker.app.ui.theme.SpdCurse
import dev.seedseeker.app.ui.theme.SpdDanger
import dev.seedseeker.app.ui.theme.SpdGreen
import dev.seedseeker.app.ui.theme.SpdSecret
import dev.seedseeker.app.ui.theme.SpdTeal
import dev.seedseeker.app.ui.theme.SpdUpgrade
import kotlin.math.abs

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ScoutScreen(
    seedInput: String,
    result: ScoutWorld?,
    isScouting: Boolean,
    error: String?,
    matches: ScoutMatches?,
    resultSeeds: List<String>,
    scoutedSeed: String?,
    onScoutSeed: (String) -> Unit,
    onSeedChange: (String) -> Unit,
    onScout: () -> Unit,
    onSelectTrinket: (String) -> Unit,
    onSettings: () -> Unit,
    onAbout: () -> Unit,
    bottomBar: @Composable () -> Unit,
    mapChallenges: Int = 0,
) {
    val listState = rememberLazyListState()
    val collapseWindow = with(LocalDensity.current) { 96.dp.toPx() }
    val headerScroll = remember(collapseWindow) { ScoutHeaderScrollState(collapseWindow) }
    val hasResult by rememberUpdatedState(result != null)
    val scrollConnection = remember(headerScroll) {
        object : NestedScrollConnection {
            override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset =
                if (hasResult && available.y < 0f) Offset(0f, headerScroll.consume(available.y)) else Offset.Zero

            override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset =
                if (hasResult && available.y > 0f) Offset(0f, headerScroll.consume(available.y)) else Offset.Zero
        }
    }
    // Gestures starting on the form/summary also scroll the same floor list.
    val headerDragState = rememberScrollableState { delta -> -listState.dispatchRawDelta(-delta) }
    LaunchedEffect(error, result == null) {
        if (error != null || result == null) headerScroll.expand()
    }
    val floors = remember(result) { scoutFloors(result) }
    val mapFloors = remember(floors) { floors.keys.filter(::isMapDepthSupported) }
    val matchedChoices = remember(result, matches) { matchedScoutChoices(result?.items.orEmpty(), matches?.items.orEmpty()) }
    var openMapDepth by remember(result?.seed, mapChallenges) { mutableStateOf<Int?>(null) }
    val offerFloorIndex = floors.values.indexOfFirst { rows -> rows.any { it.value.item.kind == ItemKind.TRINKET } }
    val offerDepth = floors.keys.elementAtOrNull(offerFloorIndex)
    val offerBodyIndex = if (offerFloorIndex >= 0) 1 + 2 * offerFloorIndex else -1
    var offerTop by remember(result?.seed) { mutableStateOf(0f) }
    var offerHeight by remember(result?.seed) { mutableStateOf(0f) }
    var offerMapHeight by remember(result?.seed) { mutableStateOf(0f) }
    var floorHeaderHeight by remember { mutableStateOf(0) }
    val reveal by remember(offerBodyIndex, offerTop, offerHeight, floorHeaderHeight, openMapDepth, offerDepth, offerMapHeight) { derivedStateOf {
        val row = listState.layoutInfo.visibleItemsInfo.find { it.index == offerBodyIndex }
        when {
            offerBodyIndex < 0 || offerHeight <= 0 -> 0f
            row != null -> ((floorHeaderHeight - row.offset - offerTop - if (openMapDepth == offerDepth) offerMapHeight else 0f) / offerHeight).coerceIn(0f, 1f)
            listState.firstVisibleItemIndex > offerBodyIndex -> 1f
            else -> 0f
        }
    } }
    val seedIsReady = SeedCode.isScoutable(seedInput)
    // Position within the search results, when the scouted seed came from one.
    val resultIndex = ScoutResultNavigation.position(resultSeeds, scoutedSeed)
    val stepToResult: (Int) -> Unit = { delta ->
        ScoutResultNavigation.step(resultSeeds, scoutedSeed, delta)?.let(onScoutSeed)
    }
    // The gesture coroutine must survive recomposition: search matches stream
    // in every ~90 ms and restarting pointerInput on them would cancel any
    // swipe in progress.
    val currentStepToResult by rememberUpdatedState(stepToResult)
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("Scout") },
                actions = {
                    IconButton(onClick = onSettings) {
                        Icon(Icons.Filled.Settings, contentDescription = "Settings")
                    }
                    IconButton(onClick = onAbout) {
                        Icon(Icons.Filled.Info, contentDescription = "About and licenses")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
        bottomBar = bottomBar,
    ) { scaffoldPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(scaffoldPadding)
                // Horizontal swipes step through the search results; vertical
                // drags stay with the list's own scrolling.
                .pointerInput(Unit) {
                    var dragTotal = 0f
                    val threshold = 64.dp.toPx()
                    detectHorizontalDragGestures(
                        onDragStart = { dragTotal = 0f },
                        onDragCancel = { dragTotal = 0f },
                        onDragEnd = {
                            if (abs(dragTotal) >= threshold) {
                                currentStepToResult(if (dragTotal < 0f) 1 else -1)
                            }
                        },
                    ) { _, dragAmount -> dragTotal += dragAmount }
                },
            contentAlignment = Alignment.TopCenter,
        ) {
            Column(
                Modifier.fillMaxHeight().widthIn(max = 680.dp).fillMaxWidth().padding(horizontal = 16.dp)
                    .clipToBounds()
                    .nestedScroll(scrollConnection)
                    .scrollable(headerDragState, Orientation.Vertical, enabled = result != null),
            ) {
                Box(
                    Modifier.clipToBounds().layout { measurable, constraints ->
                        // Measure the full form even when only its departing bottom edge is visible.
                        val placeable = measurable.measure(constraints.copy(minHeight = 0, maxHeight = Constraints.Infinity))
                        val offset = headerScroll.inputOffset.roundToInt()
                        layout(placeable.width, (placeable.height - offset).coerceAtLeast(0)) {
                            placeable.placeRelative(0, -offset)
                        }
                    }.onSizeChanged { headerScroll.updateMeasurements(input = it.height.toFloat()) },
                ) {
                    Column(Modifier.padding(bottom = 12.dp)) {
                        SeedInputCard(seedInput, seedIsReady, isScouting, error, onSeedChange, onScout, onScoutSeed)
                    }
                }
                result?.let { world ->
                    ScoutSummaryCard(
                        world = world,
                        matches = matches,
                        progress = headerScroll.progress,
                        onCollapseDistanceChanged = { headerScroll.updateMeasurements(summary = it) },
                    )
                }
                if (resultIndex != null || result?.trinketOrder?.isNotEmpty() == true) {
                    ResultNavigationBar(
                        index = resultIndex, total = resultSeeds.size, onStep = stepToResult,
                        offers = result?.trinketOrder?.take(4).orEmpty(), selectedTrinket = result?.selectedTrinket,
                        reveal = reveal, enabled = !isScouting, onSelect = onSelectTrinket,
                        modifier = Modifier.testTag("scout-navigation").padding(top = 6.dp),
                    )
                }
                LazyColumn(state = listState, modifier = Modifier.weight(1f).fillMaxWidth().testTag("scout-floors"),
                    contentPadding = PaddingValues(top = 4.dp, bottom = 24.dp)) {
                if (result == null && !isScouting) {
                    item {
                        Card(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(top = 20.dp),
                            shape = MaterialTheme.shapes.large,
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
                            ),
                        ) {
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(24.dp),
                                horizontalAlignment = Alignment.CenterHorizontally,
                            ) {
                                Icon(
                                    Icons.Outlined.Place,
                                    contentDescription = null,
                                    modifier = Modifier.size(44.dp),
                                    tint = MaterialTheme.colorScheme.primary,
                                )
                                Spacer(Modifier.height(14.dp))
                                Text(
                                    "Enter a seed or tap a search result to list its items through floor 24.",
                                    textAlign = TextAlign.Center,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }

                result?.let { world ->
                    if (world.artifactDecks.isNotEmpty()) item(key = "artifact-deck") { ArtifactDeckRow(world, matches) }
                    val questsByDepth = world.quests.associateBy(ScoutQuest::depth)
                    floors
                        .forEach { (depth, floorItems) ->
                            stickyHeader(key = "floor-$depth") {
                                FloorHeading(
                                    depth = depth,
                                    feeling = world.floorFeelings[depth],
                                    itemCount = floorItems.size,
                                    questLabel = questsByDepth[depth]?.variant?.label,
                                    farming = world.isFarmingFloor(depth),
                                    mapExpanded = openMapDepth == depth,
                                    onMapToggle = if (isMapDepthSupported(depth)) ({ openMapDepth = if (openMapDepth == depth) null else depth }) else null,
                                    modifier = Modifier.background(MaterialTheme.colorScheme.background).onSizeChanged { floorHeaderHeight = it.height }.padding(vertical = 4.dp),
                                )
                            }
                            item(key = "floor-body-$depth") {
                                Column {
                                    if (openMapDepth == depth && isMapDepthSupported(depth)) {
                                        Box(Modifier.onSizeChanged { if (depth == offerDepth) offerMapHeight = it.height.toFloat() }) {
                                            LevelMapView(world, depth, mapFloors, mapChallenges, isScouting, onSelectTrinket)
                                        }
                                    }
                                    if (floorItems.isEmpty()) {
                                        Text("No notable items on this floor.", Modifier.padding(vertical = 8.dp),
                                            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                    val trinkets = floorItems.filter { it.value.item.kind == ItemKind.TRINKET }
                                    if (trinkets.isNotEmpty()) {
                                        TrinketCatalystCard(trinkets, world.trinketOrder, matches, world.selectedTrinket, !isScouting, onSelectTrinket,
                                            onOffersLayout = { top, height -> offerTop = top; offerHeight = height })
                                    }
                                    floorItems.filter { it.value.item.kind != ItemKind.TRINKET }.forEach { indexedItem ->
                                        ScoutItemCard(scoutItem = indexedItem.value, ringGems = world.ringGems,
                                            matches = matches?.items?.contains(indexedItem.index) == true,
                                            dimmed = isAlternateScoutChoice(indexedItem.value.accessibility, indexedItem.index in matches?.items.orEmpty(), matchedChoices),
                                            modifier = Modifier.padding(bottom = 8.dp))
                                    }
                                }
                            }
                        }
                }
            }
        }
    }
}

}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SeedInputCard(
    seedInput: String,
    seedIsReady: Boolean,
    isScouting: Boolean,
    error: String?,
    onSeedChange: (String) -> Unit,
    onScout: () -> Unit,
    onScoutSeed: (String) -> Unit,
) {
    val daily = seedInput.firstOrNull()?.let { it in '0'..'9' } == true
    val focusManager = LocalFocusManager.current
    var entryFocused by remember { mutableStateOf(false) }
    var showDatePicker by remember { mutableStateOf(false) }
    val scout = {
        focusManager.clearFocus()
        onScout()
    }
    if (showDatePicker) {
        val picker = rememberDatePickerState(
            initialSelectedDateMillis = DailyRunDate.parse(seedInput) ?: System.currentTimeMillis(),
            yearRange = 1970..9999,
        )
        DatePickerDialog(
            onDismissRequest = { showDatePicker = false },
            confirmButton = {
                TextButton(enabled = picker.selectedDateMillis != null, onClick = {
                    picker.selectedDateMillis?.let { onSeedChange(DailyRunDate.format(it)) }
                    showDatePicker = false
                }) { Text("Use date") }
            },
            dismissButton = { TextButton(onClick = { showDatePicker = false }) { Text("Cancel") } },
        ) { DatePicker(state = picker, title = { Text("Daily run date (UTC)", Modifier.padding(24.dp)) }) }
    }
    var fieldValue by remember {
        mutableStateOf(
            TextFieldValue(seedInput, selection = TextRange(seedInput.length)),
        )
    }
    LaunchedEffect(seedInput) {
        if (seedInput != fieldValue.text) {
            fieldValue = TextFieldValue(seedInput, selection = TextRange(seedInput.length))
        }
    }

    Card(
        modifier = Modifier.fillMaxWidth().testTag("scout-input"),
        shape = MaterialTheme.shapes.extraLarge,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(start = 18.dp, top = 10.dp, end = 18.dp, bottom = 18.dp)) {
            BoxWithConstraints(Modifier.fillMaxWidth()) {
                val compact = maxWidth < 300.dp
                Row(
                    modifier = Modifier.onFocusChanged { entryFocused = it.hasFocus }.focusGroup(),
                    verticalAlignment = Alignment.Bottom,
                ) {
                    OutlinedTextField(
                        value = fieldValue,
                        onValueChange = {
                            val formattedValue = formatSeedFieldValue(it)
                            fieldValue = formattedValue
                            onSeedChange(formattedValue.text)
                        },
                        enabled = !isScouting,
                        modifier = Modifier.weight(1f).heightIn(min = 64.dp).testTag("scout-run-field"),
                        label = { Text("Seed / date", maxLines = 1) },
                        placeholder = { Text("Seed / YYYY-MM-DD", maxLines = 1) },
                        singleLine = true,
                        shape = MaterialTheme.shapes.medium,
                        textStyle = MaterialTheme.typography.titleLarge.copy(
                            fontFamily = FontFamily.Monospace,
                            fontSize = if (compact) 16.sp else 20.sp,
                            letterSpacing = 0.6.sp,
                        ),
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.Characters,
                            keyboardType = if (daily) KeyboardType.Number else KeyboardType.Ascii,
                            imeAction = ImeAction.Search,
                        ),
                        keyboardActions = KeyboardActions(
                            onSearch = { if (seedIsReady && !isScouting) scout() },
                        ),
                    )
                    // Keep the actions available when focus moves into them or the picker.
                    AnimatedVisibility(
                        visible = !isScouting && (entryFocused || showDatePicker),
                        enter = expandHorizontally(tween(160), expandFrom = Alignment.End) + fadeIn(tween(120)),
                        exit = shrinkHorizontally(tween(140), shrinkTowards = Alignment.End) + fadeOut(tween(100)),
                    ) {
                        Row(Modifier.padding(start = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            IconButton(
                                onClick = { showDatePicker = true }, enabled = !isScouting,
                                modifier = Modifier.width(48.dp).height(56.dp).testTag("scout-date-picker"),
                            ) { Icon(Icons.Outlined.DateRange, contentDescription = "Choose daily run date") }
                            TextButton(
                                onClick = {
                                    focusManager.clearFocus()
                                    onScoutSeed(DailyRunDate.today())
                                }, enabled = !isScouting,
                                modifier = Modifier.widthIn(min = 48.dp).height(56.dp).testTag("scout-today"),
                                contentPadding = PaddingValues(horizontal = 4.dp),
                            ) { Text("Today", maxLines = 1) }
                        }
                    }
                }
            }
            Spacer(Modifier.height(12.dp))
            Button(
                onClick = scout,
                enabled = seedIsReady && !isScouting,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(52.dp),
                shapes = ButtonDefaults.shapes(),
            ) {
                if (isScouting) {
                    LoadingIndicator(modifier = Modifier.size(28.dp))
                    Spacer(Modifier.width(10.dp))
                    Text("Generating world…")
                } else {
                    Text(if (daily) "Scout daily run" else "Scout seed")
                }
            }
            error?.let {
                Spacer(Modifier.height(10.dp))
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}

/**
 * Position of the scouted seed within the search results, with previous/next
 * affordances; horizontal swipes anywhere on the screen do the same.
 */
@Composable
private fun ResultNavigationBar(
    index: Int?, total: Int, onStep: (Int) -> Unit,
    offers: List<CatalogItem>, selectedTrinket: String?, reveal: Float,
    enabled: Boolean, onSelect: (String) -> Unit, modifier: Modifier = Modifier,
) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        if (index != null) {
            IconButton(onClick = { onStep(-1) }, enabled = index > 0, modifier = Modifier.size(36.dp)) {
                Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, "Previous result")
            }
            Text("${index + 1} of $total", style = MaterialTheme.typography.labelMedium)
            IconButton(onClick = { onStep(1) }, enabled = index < total - 1, modifier = Modifier.size(36.dp)) {
                Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, "Next result")
            }
        } else Text("Trinkets", style = MaterialTheme.typography.labelMedium)
        Box(Modifier.weight(1f).height(44.dp).clipToBounds(), contentAlignment = Alignment.CenterEnd) {
            if (index != null) Text("swipe to browse", modifier = Modifier.graphicsLayer { alpha = 1f - reveal },
                style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (reveal > 0) TrinketShortcuts(
                offers = offers, selectedTrinket = selectedTrinket, enabled = enabled, onSelect = onSelect,
                modifier = Modifier.graphicsLayer { alpha = reveal; translationY = (1f - reveal) * size.height },
            )
        }
    }
}

/** Keeps the logical cursor position when seed or date grouping inserts or removes hyphens. */
internal fun formatSeedFieldValue(input: TextFieldValue): TextFieldValue {
    val formatted = SeedCode.formatInput(input.text)
    if (formatted == input.text) return input

    fun remapOffset(offset: Int): Int = SeedCode
        .formatInput(input.text.take(offset))
        .length
        .coerceAtMost(formatted.length)

    return TextFieldValue(
        text = formatted,
        selection = TextRange(
            remapOffset(input.selection.start),
            remapOffset(input.selection.end),
        ),
    )
}

@Composable
internal fun FloorFeelingSprite(feeling: FloorFeeling) {
    val context = LocalContext.current
    val atlas = remember(context) {
        context.assets.open("third_party/shattered-pixel-dungeon/dungeon-icons.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
    }
    Canvas(Modifier.size(width = 15.dp, height = 16.dp).semantics {
        contentDescription = "${feeling.label} floor"
    }) {
        drawImage(
            image = atlas,
            srcOffset = IntOffset(16 * feeling.ordinal, 64),
            srcSize = IntSize(15, 16),
            dstSize = IntSize(size.width.roundToInt(), size.height.roundToInt()),
            filterQuality = FilterQuality.None,
        )
    }
}

/** One row of a scouted world, drawn with the gems [ringGems] says that run holds. */
@Composable
internal fun ScoutItemCard(
    scoutItem: ScoutItem,
    ringGems: RingGems,
    matches: Boolean,
    dimmed: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val hasStatusBadges = scoutItem.cursed || scoutItem.secret
    val accessibilityLabel = when (scoutItem.accessibility) {
        ScoutAccessibility.Independent -> null
        is ScoutAccessibility.Choice -> null
        is ScoutAccessibility.Scenarios ->
            "Route group ${scoutGroupLetter(scoutItem.accessibility.group)} · access changes with room choices"
    }

    Card(
        modifier = modifier.fillMaxWidth().alpha(if (dimmed) 0.45f else 1f),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(
            containerColor = if (matches) {
                MaterialTheme.colorScheme.surfaceContainerHighest
            } else {
                MaterialTheme.colorScheme.surfaceContainerLow
            },
        ),
    ) {
        BoxWithConstraints(Modifier.padding(14.dp)) {
            val measurer = rememberTextMeasurer()
            val typography = MaterialTheme.typography
            val density = LocalDensity.current
            fun textWidth(text: String, style: androidx.compose.ui.text.TextStyle) =
                measurer.measure(text, style, softWrap = false).size.width
            val stackedBadges = with(density) {
                val titleWidth = textWidth(scoutItem.item.name, typography.titleMedium)
                val upgradeWidth = if (scoutItem.displayedUpgrade != 0) {
                    textWidth("+${scoutItem.displayedUpgrade}", typography.labelMedium.copy(fontFamily = FontFamily.Monospace)) +
                        22.dp.roundToPx()
                } else 0
                val curseWidth = if (scoutItem.cursed) textWidth("cursed", typography.labelSmall) + 20.dp.roundToPx() else 0
                val secretWidth = if (scoutItem.secret) textWidth("secret", typography.labelSmall) + 20.dp.roundToPx() else 0
                val matchWidth = textWidth("match", typography.labelSmall) + 28.dp.roundToPx()
                val choiceWidth = (scoutItem.accessibility as? ScoutAccessibility.Choice)?.let {
                    textWidth(scoutGroupLetter(it.group).toString(), typography.labelSmall) + 28.dp.roundToPx()
                } ?: 0
                // Reserve the sprite, gaps, title badges, and the wider trailing chip.
                val titleAndBadges = titleWidth + upgradeWidth + curseWidth + secretWidth + 64.dp.roundToPx()
                val trailingWidth = maxOf(if (matches) matchWidth else 0, choiceWidth)
                titleAndBadges + trailingWidth > constraints.maxWidth
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Bare sprite on the row background, like the web's scout rows: the
                // pulsing masked tint is the only modifier cue, no tile, no halo.
                ItemSprite(
                    item = scoutItem.item,
                    spriteIndex = ringGems.spriteIndexFor(scoutItem.item),
                    glows = listOfNotNull(ItemGlows.forItem(kind = scoutItem.item.kind, effect = scoutItem.effect, cursed = scoutItem.cursed)),
                    modifier = Modifier.size(40.dp),
                )
                Spacer(Modifier.width(14.dp))
                Column(Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        ScoutItemTitle(scoutItem.item.name, Modifier.weight(1f, fill = false))
                        if (scoutItem.displayedUpgrade != 0) {
                            Spacer(Modifier.width(8.dp))
                            ScoutItemUpgrade(scoutItem.displayedUpgrade)
                        }
                        if (!stackedBadges) {
                            if (hasStatusBadges) {
                                Row(Modifier.padding(start = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    ScoutItemBadges(scoutItem)
                                }
                            }
                        }
                    }
                    if (stackedBadges) {
                        Layout(
                            modifier = Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 2.dp),
                            content = {
                                if (hasStatusBadges) {
                                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                        ScoutItemBadges(scoutItem)
                                    }
                                } else {
                                    // Reuse the empty badge space so the source and chips share a line.
                                    ScoutItemDetails(scoutItem)
                                }
                                FlowRow(
                                    modifier = Modifier.testTag("scout-item-match-choices"),
                                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                                    verticalArrangement = Arrangement.spacedBy(4.dp),
                                ) {
                                    if (matches) ScoutItemMatchChip()
                                    (scoutItem.accessibility as? ScoutAccessibility.Choice)?.let { ChoiceGroupChip(it) }
                                }
                            },
                        ) { measurables, constraints ->
                            val loose = constraints.copy(minWidth = 0, minHeight = 0)
                            val leading = measurables[0].measure(loose)
                            val trailing = measurables[1].measure(loose)
                            val separateRows = leading.width > 0 && trailing.width > 0 &&
                                leading.width + 8.dp.roundToPx() + trailing.width > constraints.maxWidth
                            val height = if (separateRows) leading.height + 4.dp.roundToPx() + trailing.height
                                else maxOf(leading.height, trailing.height)
                            layout(constraints.maxWidth, height) {
                                leading.placeRelative(0, if (separateRows) 0 else (height - leading.height) / 2)
                                trailing.placeRelative(constraints.maxWidth - trailing.width,
                                    if (separateRows) leading.height + 4.dp.roundToPx() else (height - trailing.height) / 2)
                            }
                        }
                    }
                    if (!stackedBadges || hasStatusBadges) {
                        ScoutItemDetails(scoutItem)
                    }
                    accessibilityLabel?.let {
                        Spacer(Modifier.height(2.dp))
                        Text(
                            it,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                if (!stackedBadges) {
                    Spacer(Modifier.width(10.dp))
                    Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        if (matches) ScoutItemMatchChip()
                        (scoutItem.accessibility as? ScoutAccessibility.Choice)?.let { ChoiceGroupChip(it) }
                    }
                }
            }
        }
    }
}

@Composable
private fun ScoutItemDetails(scoutItem: ScoutItem) {
    val effectIsCurse = scoutItem.effect != null &&
        ItemCatalog.cursesFor(scoutItem.item.kind).contains(scoutItem.effect)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        scoutItem.effect?.let { effect ->
            Text(
                effect,
                style = MaterialTheme.typography.bodySmall,
                color = if (effectIsCurse) SpdDanger else SpdTeal,
                modifier = Modifier.alignByBaseline(),
            )
        }
        Text(
            scoutItem.source.label,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.alignByBaseline(),
        )
    }
}


/** Preserve the complete name even at narrow widths or enlarged system fonts. */
@Composable
private fun ScoutItemTitle(name: String, modifier: Modifier = Modifier) {
    val style = MaterialTheme.typography.titleMedium.copy(color = LocalContentColor.current)
    BasicText(
        name,
        modifier = modifier,
        style = style,
        maxLines = 1,
        autoSize = TextAutoSize.StepBased(minFontSize = 1.sp, maxFontSize = style.fontSize, stepSize = 0.25.sp),
    )
}

@Composable
private fun ScoutItemUpgrade(upgrade: Int) {
    Surface(
        shape = MaterialTheme.shapes.extraSmall,
        color = SpdUpgrade.copy(alpha = 0.12f),
    ) {
        Text(
            "+$upgrade",
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 2.dp),
            style = MaterialTheme.typography.labelMedium,
            fontFamily = FontFamily.Monospace,
            color = SpdUpgrade,
        )
    }
}

@Composable
private fun ScoutItemBadges(scoutItem: ScoutItem) {
    if (scoutItem.cursed) {
        Surface(
            shape = MaterialTheme.shapes.extraSmall,
            color = SpdDanger.copy(alpha = 0.14f),
        ) {
            Text(
                "cursed",
                modifier = Modifier.padding(horizontal = 6.dp, vertical = 1.dp),
                style = MaterialTheme.typography.labelSmall,
                color = SpdCurse,
            )
        }
    }
    if (scoutItem.secret) {
        Surface(
            shape = MaterialTheme.shapes.extraSmall,
            color = SpdSecret.copy(alpha = 0.14f),
        ) {
            Text(
                "secret",
                modifier = Modifier.padding(horizontal = 6.dp, vertical = 1.dp),
                style = MaterialTheme.typography.labelSmall,
                color = SpdSecret,
            )
        }
    }
}

@Composable
private fun ScoutItemMatchChip() {
    Surface(
        shape = MaterialTheme.shapes.extraSmall,
        color = SpdGreen.copy(alpha = 0.1f),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 6.dp, vertical = 1.dp),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.Filled.Check, contentDescription = null,
                modifier = Modifier.size(12.dp), tint = SpdGreen)
            Spacer(Modifier.width(4.dp))
            Text("match", style = MaterialTheme.typography.labelSmall, color = SpdGreen)
        }
    }
}


/** The catalyst keeps its placement; its deck retains the engine's order. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
private fun TrinketCatalystCard(
    choices: List<IndexedValue<ScoutItem>>,
    deck: List<CatalogItem>,
    matches: ScoutMatches?,
    selectedTrinket: String?,
    enabled: Boolean,
    onSelect: (String) -> Unit,
    onOffersLayout: (Float, Float) -> Unit = { _, _ -> },
) {
    var cardCoordinates by remember { mutableStateOf<LayoutCoordinates?>(null) }
    val catalyst = CatalogItem("trinket_catalyst", "Magical catalyst", ItemKind.TRINKET, 70)
    val placement = choices.first().value
    val ordered = deck.take(4).ifEmpty { choices.map { it.value.item } }
    Card(
        modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp).onGloballyPositioned { cardCoordinates = it },
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
    ) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ItemSprite(catalyst, modifier = Modifier.size(36.dp))
                Spacer(Modifier.width(10.dp))
                Column(Modifier.weight(1f)) {
                    Text(catalyst.name, style = MaterialTheme.typography.titleMedium)
                    Text(placement.source.label, style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                    if (placement.secret) Text("Secret room", style = MaterialTheme.typography.labelSmall, color = SpdSecret)
                    when (val access = placement.accessibility) {
                        ScoutAccessibility.Independent -> Unit
                        is ScoutAccessibility.Choice -> Unit
                        is ScoutAccessibility.Scenarios -> Text("Route group ${scoutGroupLetter(access.group)} · access changes with room choices", style = MaterialTheme.typography.labelSmall)
                    }
                }
                (placement.accessibility as? ScoutAccessibility.Choice)?.let { ChoiceGroupChip(it) }
            }
            Row(Modifier.onGloballyPositioned { row ->
                cardCoordinates?.takeIf { it.isAttached }?.let { card -> onOffersLayout(row.positionInWindow().y - card.positionInWindow().y, row.size.height.toFloat()) }
            }, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ordered.forEach { trinket ->
                    val applied = selectedTrinket == trinket.id
                    val matched = choices.any { it.value.item.id == trinket.id && matches?.items?.contains(it.index) == true }
                    Surface(
                        selected = applied,
                        onClick = { onSelect(if (applied) "none" else trinket.id) },
                        enabled = enabled,
                        border = if (applied) androidx.compose.foundation.BorderStroke(2.dp, SpdGreen) else null,
                        modifier = Modifier.weight(1f).aspectRatio(1f).semantics {
                            contentDescription = trinket.name + if (matched) ", matches requirement" else ""
                        },
                        shape = MaterialTheme.shapes.small,
                        color = if (matched) SpdGreen.copy(alpha = 0.14f) else MaterialTheme.colorScheme.surfaceContainerHigh,
                    ) {
                        BoxWithConstraints {
                            val iconSize = minOf(48.dp, maxWidth * 0.58f)
                            Column(Modifier.fillMaxSize().padding(2.dp), horizontalAlignment = Alignment.CenterHorizontally,
                                verticalArrangement = Arrangement.SpaceEvenly) {
                                if (applied) Text("Applied +3", style = MaterialTheme.typography.labelSmall, color = SpdGreen)
                                ItemSprite(trinket, modifier = Modifier.size(if (applied) iconSize * 0.8f else iconSize))
                                FittedTrinketName(trinket.name)
                            }
                        }
                    }
                }
            }
            if (deck.size > 4) {
                Text("Transmutation order · 1–13", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    deck.drop(4).forEachIndexed { index, trinket ->
                        val matched = matches?.transmutedTrinkets?.contains(index) == true
                        BoxWithConstraints(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                            Surface(modifier = Modifier.size(minOf(maxWidth, 24.dp)).semantics {
                                contentDescription = "Transmutation #${index + 1}: ${trinket.name}" + if (matched) ", matches requirement" else ""
                            }, shape = RoundedCornerShape(4.dp),
                                color = if (matched) SpdGreen.copy(alpha = 0.14f) else Color.Transparent,
                                border = if (matched) androidx.compose.foundation.BorderStroke(1.dp, SpdGreen) else null) {
                                ItemSprite(trinket, modifier = Modifier.fillMaxSize().padding(2.dp))
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun FittedTrinketName(name: String) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        // Restart fitting when the card width changes (rotation or split-screen).
        // Only width keys the state, so shrinking text cannot restart its own fit.
        var fontSize by remember(name, maxWidth) { mutableStateOf(11f) }
        Text(name, modifier = Modifier.fillMaxWidth(), fontSize = fontSize.sp, maxLines = 1,
            softWrap = false, textAlign = TextAlign.Center,
            onTextLayout = { result ->
                if (result.didOverflowWidth && fontSize > 1f) fontSize = (fontSize * 0.9f).coerceAtLeast(1f)
            })
    }
}


@Composable
private fun ArtifactDeckRow(world: ScoutWorld, matches: ScoutMatches?) {
    val order = world.artifactDecks[0].orEmpty()
    val naturalArtifacts = availableScoutArtifacts(world.items, matches?.items.orEmpty())
    if (order.isEmpty()) return
    val targets = matches?.transmutedArtifacts.orEmpty().mapNotNull { (depth, index) ->
        world.artifactDecks.entries.lastOrNull { it.key <= depth }?.value?.getOrNull(index)?.id
    }.toSet()
    Card(Modifier.fillMaxWidth().padding(vertical = 4.dp), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow)) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            order.forEach { artifact ->
                val matched = artifact.id in targets
                val natural = artifact.id in naturalArtifacts
                BoxWithConstraints(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                    val tileWidth = minOf(maxWidth, 36.dp)
                    Surface(shape = RoundedCornerShape(6.dp), color = if (matched) SpdGreen.copy(alpha = 0.14f) else Color.Transparent,
                        border = if (matched) androidx.compose.foundation.BorderStroke(1.dp, SpdGreen) else null,
                        modifier = Modifier.width(tileWidth).alpha(if (natural) 0.3f else 1f).semantics { contentDescription = artifact.name + (if (natural) ", available in dungeon" else "") + (if (matched) ", matches requirement" else "") }) {
                        Box(Modifier.padding(2.dp), contentAlignment = Alignment.Center) {
                            ItemSprite(artifact, modifier = Modifier.size((tileWidth - 4.dp).coerceAtLeast(1.dp)))
                        }
                    }
                }
            }
        }
    }
}
