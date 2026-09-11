// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.ClipData
import android.graphics.BitmapFactory
import androidx.compose.foundation.Canvas
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import dev.seedseeker.app.model.FloorFeeling
import kotlin.math.roundToInt
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.Place
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.toClipEntry
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.CatalogItem
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.engine.ScoutMatches
import dev.seedseeker.app.engine.SeedCode
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
import kotlinx.coroutines.launch

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
) {
    val listState = rememberLazyListState()
    val floors = remember(result) { result?.items?.withIndex()?.groupBy { it.value.depth }?.toSortedMap().orEmpty() }
    val offerFloorIndex = floors.values.indexOfFirst { rows -> rows.any { it.value.item.kind == ItemKind.TRINKET } }
    val offerBodyIndex = if (offerFloorIndex >= 0) 2 + 2 * offerFloorIndex else -1
    var offerTop by remember { mutableStateOf(0f) }
    var offerHeight by remember { mutableStateOf(0f) }
    var floorHeaderHeight by remember { mutableStateOf(0) }
    val reveal by remember(offerBodyIndex, offerTop, offerHeight, floorHeaderHeight) { derivedStateOf {
        val row = listState.layoutInfo.visibleItemsInfo.find { it.index == offerBodyIndex }
        when {
            offerBodyIndex < 0 || offerHeight <= 0 -> 0f
            row != null -> ((floorHeaderHeight - row.offset - offerTop) / offerHeight).coerceIn(0f, 1f)
            listState.firstVisibleItemIndex > offerBodyIndex -> 1f
            else -> 0f
        }
    } }
    val seedIsReady = SeedCode.isCanonical(seedInput)
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
            Column(Modifier.fillMaxHeight().fillMaxWidth().widthIn(max = 680.dp).padding(horizontal = 16.dp)) {
                SeedInputCard(seedInput, seedIsReady, isScouting, error, onSeedChange, onScout)
                if (resultIndex != null || result?.trinketOrder?.isNotEmpty() == true) {
                    ResultNavigationBar(
                        index = resultIndex, total = resultSeeds.size, onStep = stepToResult,
                        offers = result?.trinketOrder?.take(4).orEmpty(), selectedTrinket = result?.selectedTrinket,
                        reveal = reveal, enabled = !isScouting, onSelect = onSelectTrinket,
                        modifier = Modifier.padding(top = 6.dp),
                    )
                }
                LazyColumn(state = listState, modifier = Modifier.weight(1f).fillMaxWidth(),
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
                    item {
                        ScoutSummaryCard(
                            world = world,
                            matches = matches,
                            modifier = Modifier.padding(top = 22.dp),
                        )
                    }

                    val questsByDepth = world.quests.associateBy(ScoutQuest::depth)
                    floors
                        .forEach { (depth, floorItems) ->
                            stickyHeader(key = "floor-$depth") {
                                FloorHeading(
                                    depth = depth,
                                    feeling = world.floorFeelings[depth],
                                    itemCount = floorItems.size,
                                    questLabel = questsByDepth[depth]?.variant?.label,
                                    modifier = Modifier.background(MaterialTheme.colorScheme.background).onSizeChanged { floorHeaderHeight = it.height }.padding(top = 12.dp, bottom = 10.dp),
                                )
                            }
                            item(key = "floor-body-$depth") {
                                Column {
                                    val trinkets = floorItems.filter { it.value.item.kind == ItemKind.TRINKET }
                                    if (trinkets.isNotEmpty()) {
                                        TrinketCatalystCard(trinkets, world.trinketOrder, matches, world.selectedTrinket, !isScouting, onSelectTrinket,
                                            onOffersLayout = { top, height -> offerTop = top; offerHeight = height })
                                    }
                                    floorItems.filter { it.value.item.kind != ItemKind.TRINKET }.forEach { indexedItem ->
                                        ScoutItemCard(scoutItem = indexedItem.value, ringGems = world.ringGems,
                                            matches = matches?.items?.contains(indexedItem.index) == true,
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

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SeedInputCard(
    seedInput: String,
    seedIsReady: Boolean,
    isScouting: Boolean,
    error: String?,
    onSeedChange: (String) -> Unit,
    onScout: () -> Unit,
) {
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
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.extraLarge,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(18.dp)) {
            OutlinedTextField(
                value = fieldValue,
                onValueChange = {
                    val formattedValue = formatSeedFieldValue(it)
                    fieldValue = formattedValue
                    onSeedChange(formattedValue.text)
                },
                enabled = !isScouting,
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Seed") },
                placeholder = { Text("ABC-DEF-GHI") },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                textStyle = MaterialTheme.typography.titleLarge.copy(
                    fontFamily = FontFamily.Monospace,
                    letterSpacing = 1.2.sp,
                ),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Characters,
                    keyboardType = KeyboardType.Ascii,
                    imeAction = ImeAction.Search,
                ),
                keyboardActions = KeyboardActions(
                    onSearch = { if (seedIsReady && !isScouting) onScout() },
                ),
            )
            Spacer(Modifier.height(12.dp))
            Button(
                onClick = onScout,
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
                    Text("Scout seed")
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
            if (reveal > 0) Row(Modifier.graphicsLayer { alpha = reveal; translationY = (1f - reveal) * size.height },
                horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                offers.forEach { offer ->
                    val applied = selectedTrinket == offer.id
                    Surface(selected = applied, onClick = { onSelect(if (applied) "none" else offer.id) }, enabled = enabled,
                        modifier = Modifier.size(36.dp).semantics { contentDescription = offer.name },
                        shape = MaterialTheme.shapes.small, border = BorderStroke(if (applied) 2.dp else 1.dp, if (applied) SpdGreen else MaterialTheme.colorScheme.outlineVariant),
                        color = if (applied) SpdGreen.copy(alpha = 0.14f) else MaterialTheme.colorScheme.surfaceContainerLow) {
                        Box(contentAlignment = Alignment.Center) { ItemSprite(offer, modifier = Modifier.size(22.dp)) }
                    }
                }
            }
        }
    }
}

/** Keeps the logical cursor position when canonical grouping inserts or removes hyphens. */
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
private fun ScoutSummaryCard(
    world: ScoutWorld,
    matches: ScoutMatches?,
    modifier: Modifier = Modifier,
) {
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    val floors = world.items.map(ScoutItem::depth).distinct().size
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
    ) {
        Column(Modifier.padding(18.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    world.seed,
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.headlineSmall,
                    fontFamily = FontFamily.Monospace,
                    color = MaterialTheme.colorScheme.tertiary,
                )
                TextButton(
                    onClick = {
                        scope.launch {
                            clipboard.setClipEntry(ClipData.newPlainText("Seed", world.seed).toClipEntry())
                        }
                    },
                ) {
                    Text("Copy")
                }
            }
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                StatusPill("${world.items.size} items")
                StatusPill("$floors floors")
                if (matches != null) {
                    val matchCount = matches.matchedSlots
                    StatusPill(
                        text = scoutMatchText(matches.matchedSlots, matches.totalSlots),
                        container = if (matchCount > 0) {
                            MaterialTheme.colorScheme.primaryContainer
                        } else {
                            MaterialTheme.colorScheme.surfaceContainerHighest
                        },
                        content = if (matchCount > 0) {
                            MaterialTheme.colorScheme.onPrimaryContainer
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun FloorHeading(
    depth: Int,
    itemCount: Int,
    feeling: FloorFeeling? = null,
    modifier: Modifier = Modifier,
    questLabel: String? = null,
) {
    val region = floorRegionColor(depth)
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        // Region-coloured bar, as on the web's floor headers.
        Box(
            Modifier
                .size(width = 3.dp, height = 14.dp)
                .background(region, RoundedCornerShape(2.dp)),
        )
        Spacer(Modifier.width(8.dp))
        Text(
            "FLOOR $depth",
            style = MaterialTheme.typography.labelLarge,
            letterSpacing = 1.1.sp,
            color = MaterialTheme.colorScheme.onSurface,
        )
        if (feeling != null && feeling != FloorFeeling.NONE) {
            Spacer(Modifier.width(6.dp))
            FloorFeelingSprite(feeling)
        }
        Spacer(Modifier.width(8.dp))
        Text(
            floorRegion(depth),
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.labelMedium,
            color = region,
        )
        questLabel?.let {
            Surface(
                shape = MaterialTheme.shapes.extraSmall,
                color = region.copy(alpha = 0.12f),
            ) {
                Text(
                    it,
                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 1.dp),
                    style = MaterialTheme.typography.labelSmall,
                    color = region,
                )
            }
            Spacer(Modifier.width(8.dp))
        }
        Text(
            if (itemCount == 1) "1 item" else "$itemCount items",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun FloorFeelingSprite(feeling: FloorFeeling) {
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
private fun ScoutItemCard(
    scoutItem: ScoutItem,
    ringGems: RingGems,
    matches: Boolean,
    modifier: Modifier = Modifier,
) {
    val effectIsCurse = scoutItem.effect != null &&
        ItemCatalog.cursesFor(scoutItem.item.kind).contains(scoutItem.effect)
    val accessibilityLabel = when (scoutItem.accessibility) {
        ScoutAccessibility.Independent -> null
        is ScoutAccessibility.Choice ->
            "Choice group ${scoutItem.accessibility.group + 1} · option ${scoutItem.accessibility.option + 1}"
        is ScoutAccessibility.Scenarios ->
            "Route group ${scoutItem.accessibility.group + 1} · access changes with room choices"
    }

    Card(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(
            containerColor = if (matches) {
                MaterialTheme.colorScheme.surfaceContainerHighest
            } else {
                MaterialTheme.colorScheme.surfaceContainerLow
            },
        ),
    ) {
        Row(
            modifier = Modifier.padding(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Bare sprite on the row background, like the web's scout rows: the
            // pulsing masked tint is the only modifier cue, no tile, no halo.
            ItemSprite(
                item = scoutItem.item,
                spriteIndex = ringGems.spriteIndexFor(scoutItem.item),
                glows = listOfNotNull(ItemGlows.forItem(effect = scoutItem.effect, cursed = scoutItem.cursed)),
                modifier = Modifier.size(40.dp),
            )
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        scoutItem.item.name,
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    if (scoutItem.cursed) {
                        Spacer(Modifier.width(8.dp))
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
                        Spacer(Modifier.width(8.dp))
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
                scoutItem.effect?.let { effect ->
                    Text(
                        effect,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (effectIsCurse) SpdDanger else SpdTeal,
                    )
                }
                Text(
                    scoutItem.source.label,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                accessibilityLabel?.let {
                    Spacer(Modifier.height(2.dp))
                    Text(
                        it,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Spacer(Modifier.width(10.dp))
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (scoutItem.displayedUpgrade != 0) {
                    Surface(
                        shape = MaterialTheme.shapes.extraSmall,
                        color = SpdUpgrade.copy(alpha = 0.12f),
                    ) {
                        Text(
                            "+${scoutItem.displayedUpgrade}",
                            modifier = Modifier.padding(horizontal = 7.dp, vertical = 2.dp),
                            style = MaterialTheme.typography.labelMedium,
                            fontFamily = FontFamily.Monospace,
                            color = SpdUpgrade,
                        )
                    }
                }
                if (matches) {
                    Surface(
                        shape = MaterialTheme.shapes.large,
                        color = SpdGreen.copy(alpha = 0.1f),
                        border = BorderStroke(1.dp, SpdGreen.copy(alpha = 0.35f)),
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 9.dp, vertical = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(
                                Icons.Filled.Check,
                                contentDescription = null,
                                modifier = Modifier.size(13.dp),
                                tint = SpdGreen,
                            )
                            Spacer(Modifier.width(4.dp))
                            Text(
                                "match",
                                style = MaterialTheme.typography.labelSmall,
                                color = SpdGreen,
                            )
                        }
                    }
                }
            }
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
                Column {
                    Text(catalyst.name, style = MaterialTheme.typography.titleMedium)
                    Text(placement.source.label, style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                    if (placement.secret) Text("Secret room", style = MaterialTheme.typography.labelSmall, color = SpdSecret)
                    when (val access = placement.accessibility) {
                        ScoutAccessibility.Independent -> Unit
                        is ScoutAccessibility.Choice -> Text("Choice group ${access.group + 1} · option ${access.option + 1}", style = MaterialTheme.typography.labelSmall)
                        is ScoutAccessibility.Scenarios -> Text("Route group ${access.group + 1} · access changes with room choices", style = MaterialTheme.typography.labelSmall)
                    }
                }
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
                Text("Remaining deck order", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    deck.drop(4).forEach { trinket ->
                        BoxWithConstraints(Modifier.weight(1f).height(24.dp), contentAlignment = Alignment.Center) {
                            ItemSprite(trinket, modifier = Modifier.size(minOf(maxWidth, 24.dp)))
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
