// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.BitmapFactory
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.BuildConfig
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.engine.NativeSearchSession
import dev.seedseeker.app.engine.NativeSeedFinder
import dev.seedseeker.app.engine.SeedCode
import dev.seedseeker.app.model.CatalogItem
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.SearchRequest
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.ScoutAccessibility
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.model.ScoutWorld
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.model.UpgradeMatch
import dev.seedseeker.app.ui.theme.Amber
import dev.seedseeker.app.ui.theme.DeepMoss
import dev.seedseeker.app.ui.theme.Ink
import dev.seedseeker.app.ui.theme.Mint
import dev.seedseeker.app.ui.theme.Muted
import dev.seedseeker.app.ui.theme.RaisedMoss
import java.util.Locale
import kotlin.math.roundToInt
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val ATLAS_PATH = "third_party/shattered-pixel-dungeon/items.png"
private const val LICENSE_PATH = "third_party/shattered-pixel-dungeon/LICENSE.txt"
private val LocalItemAtlas = staticCompositionLocalOf<ImageBitmap?> { null }

private enum class Destination { FINDER, SCOUT, ABOUT }
private data class SearchRun(val id: Long, val request: SearchRequest)
private data class ScoutRun(val id: Long, val seed: String)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SeedFinderApp(engine: NativeSeedFinder) {
    val context = LocalContext.current
    val atlas = remember(context) {
        runCatching {
            context.assets.open(ATLAS_PATH).use(BitmapFactory::decodeStream)?.asImageBitmap()
        }.getOrNull()
    }
    val scope = rememberCoroutineScope()

    var destination by remember { mutableStateOf(Destination.FINDER) }
    var aboutReturnDestination by remember { mutableStateOf(Destination.FINDER) }
    var requirements by remember {
        mutableStateOf(
            listOf(
                ItemRequirement(1, ItemCatalog.weapons.first { it.id == "sword" }, 2, "Lucky"),
                ItemRequirement(2, ItemCatalog.armor.first { it.id == "plate_armor" }, 1, "Brimstone"),
            ),
        )
    }
    var nextRequirementKey by remember { mutableLongStateOf(3L) }
    var maximumDepth by remember { mutableStateOf(24) }
    var requireBlacksmith by remember { mutableStateOf(false) }
    var editingRequirement by remember { mutableStateOf<ItemRequirement?>(null) }
    var showRequirementSheet by remember { mutableStateOf(false) }
    var results by remember { mutableStateOf(emptyList<SeedResult>()) }
    var searchStatus by remember { mutableStateOf<SearchStatus?>(null) }
    var activeSession by remember { mutableStateOf<NativeSearchSession?>(null) }
    var run by remember { mutableStateOf<SearchRun?>(null) }
    var nextRunId by remember { mutableLongStateOf(1L) }
    var isSearching by remember { mutableStateOf(false) }
    var searchError by remember { mutableStateOf<String?>(null) }
    var scoutInput by remember { mutableStateOf("") }
    var scoutResult by remember { mutableStateOf<ScoutWorld?>(null) }
    var scoutRun by remember { mutableStateOf<ScoutRun?>(null) }
    var nextScoutRunId by remember { mutableLongStateOf(1L) }
    var isScouting by remember { mutableStateOf(false) }
    var scoutError by remember { mutableStateOf<String?>(null) }

    PredictiveBackHandler(enabled = destination != Destination.FINDER) { progress ->
        progress.collect { }
        destination = if (destination == Destination.ABOUT) {
            aboutReturnDestination
        } else {
            Destination.FINDER
        }
    }

    LaunchedEffect(run?.id) {
        val currentRun = run ?: return@LaunchedEffect
        isSearching = true
        searchError = null
        results = emptyList()
        searchStatus = null

        var session: NativeSearchSession? = null
        try {
            val openedSession = withContext(Dispatchers.Default) {
                engine.startSearch(currentRun.request)
            }
            session = openedSession
            activeSession = openedSession

            while (true) {
                val (batch, status) = withContext(Dispatchers.Default) {
                    openedSession.poll(24) to openedSession.status()
                }
                if (batch.results.isNotEmpty()) {
                    results = results + batch.results
                }
                searchStatus = status
                if (status.state == SearchState.FAILED) {
                    searchError = when (status.errorCode) {
                        2_001L -> "A native world-generation worker stopped unexpectedly."
                        else -> "The native search stopped with error ${status.errorCode}."
                    }
                }
                if (status.state != SearchState.RUNNING) break
                delay(90)
            }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (failure: Throwable) {
            searchError = failure.message ?: "The native search engine could not start."
            searchStatus = SearchStatus(SearchState.FAILED, 0, 0, -1)
        } finally {
            activeSession = null
            isSearching = false
            session?.let {
                withContext(NonCancellable + Dispatchers.Default) { it.close() }
            }
        }
    }

    LaunchedEffect(scoutRun?.id) {
        val currentRun = scoutRun ?: return@LaunchedEffect
        isScouting = true
        scoutError = null
        scoutResult = null
        try {
            scoutResult = withContext(Dispatchers.Default) {
                engine.scoutSeed(currentRun.seed)
            }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (failure: Throwable) {
            scoutError = failure.message ?: "The native scout could not generate this seed."
        } finally {
            isScouting = false
        }
    }

    CompositionLocalProvider(LocalItemAtlas provides atlas) {
        when (destination) {
            Destination.FINDER -> FinderScreen(
                requirements = requirements,
                maximumDepth = maximumDepth,
                requireBlacksmith = requireBlacksmith,
                results = results,
                status = searchStatus,
                isSearching = isSearching,
                error = searchError,
                onAbout = {
                    aboutReturnDestination = Destination.FINDER
                    destination = Destination.ABOUT
                },
                onScout = { destination = Destination.SCOUT },
                onAdd = {
                    editingRequirement = null
                    showRequirementSheet = true
                },
                onEdit = {
                    editingRequirement = it
                    showRequirementSheet = true
                },
                onRemove = { requirement ->
                    requirements = requirements.filterNot { it.key == requirement.key }
                },
                onMaximumDepthChange = { maximumDepth = it },
                onRequireBlacksmithChange = { requireBlacksmith = it },
                onSearch = {
                    if (requirements.isNotEmpty()) {
                        run = SearchRun(
                            nextRunId++,
                            SearchRequest(requirements, maximumDepth, requireBlacksmith),
                        )
                    }
                },
                onCancel = {
                    val session = activeSession
                    if (session != null) {
                        scope.launch(Dispatchers.Default) { session.cancel() }
                    }
                },
            )

            Destination.SCOUT -> ScoutScreen(
                seedInput = scoutInput,
                result = scoutResult,
                isScouting = isScouting,
                error = scoutError,
                onSeedChange = {
                    val formatted = SeedCode.formatInput(it)
                    scoutInput = formatted
                    if (formatted != scoutResult?.seed) scoutResult = null
                    scoutError = null
                },
                onScout = {
                    if (SeedCode.isCanonical(scoutInput)) {
                        scoutRun = ScoutRun(nextScoutRunId++, scoutInput)
                    }
                },
                onBack = { destination = Destination.FINDER },
                onAbout = {
                    aboutReturnDestination = Destination.SCOUT
                    destination = Destination.ABOUT
                },
            )

            Destination.ABOUT -> AboutScreen(onBack = { destination = aboutReturnDestination })
        }

        if (showRequirementSheet) {
            RequirementSheet(
                editing = editingRequirement,
                onDismiss = { showRequirementSheet = false },
                onSave = { item, kind, upgradeMatch, upgrade, modifier, source, identityGroup ->
                    val existing = editingRequirement
                    if (existing == null) {
                        requirements = requirements + ItemRequirement(
                            key = nextRequirementKey++,
                            item = item,
                            upgrade = upgrade,
                            modifier = modifier,
                            kind = kind,
                            upgradeMatch = upgradeMatch,
                            source = source,
                            identityGroup = identityGroup,
                        )
                    } else {
                        requirements = requirements.map {
                            if (it.key == existing.key) {
                                existing.copy(
                                    item = item,
                                    upgrade = upgrade,
                                    modifier = modifier,
                                    kind = kind,
                                    upgradeMatch = upgradeMatch,
                                    source = source,
                                    identityGroup = identityGroup,
                                )
                            } else {
                                it
                            }
                        }
                    }
                    showRequirementSheet = false
                },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FinderScreen(
    requirements: List<ItemRequirement>,
    maximumDepth: Int,
    requireBlacksmith: Boolean,
    results: List<SeedResult>,
    status: SearchStatus?,
    isSearching: Boolean,
    error: String?,
    onAbout: () -> Unit,
    onScout: () -> Unit,
    onAdd: () -> Unit,
    onEdit: (ItemRequirement) -> Unit,
    onRemove: (ItemRequirement) -> Unit,
    onMaximumDepthChange: (Int) -> Unit,
    onRequireBlacksmithChange: (Boolean) -> Unit,
    onSearch: () -> Unit,
    onCancel: () -> Unit,
) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Ink.copy(alpha = 0.96f)),
                title = {
                    Column {
                        Text("Seed Seeker", style = MaterialTheme.typography.titleLarge)
                        Text(
                            "Shattered Pixel Dungeon • unofficial",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                actions = {
                    TextButton(onClick = onScout, enabled = !isSearching) { Text("Scout") }
                    TextButton(onClick = onAbout) { Text("About") }
                },
            )
        },
    ) { scaffoldPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(scaffoldPadding),
            contentAlignment = Alignment.TopCenter,
        ) {
            LazyColumn(
                modifier = Modifier
                    .fillMaxHeight()
                    .fillMaxWidth()
                    .widthIn(max = 760.dp)
                    .navigationBarsPadding(),
                contentPadding = PaddingValues(start = 20.dp, top = 16.dp, end = 20.dp, bottom = 40.dp),
            ) {
                item { HeroCard() }

                item {
                    SectionHeading(
                        eyebrow = "WORLD MUST CONTAIN",
                        title = "Search requirements",
                        supporting = "Every card is joined with AND. Add as many items as the world must contain.",
                        modifier = Modifier.padding(top = 28.dp, bottom = 14.dp),
                    )
                }

                if (requirements.isEmpty()) {
                    item { EmptyRequirementsCard() }
                } else {
                    requirements.forEachIndexed { index, requirement ->
                        item(key = requirement.key) {
                            if (index > 0) AndConnector()
                            RequirementCard(
                                requirement = requirement,
                                enabled = !isSearching,
                                onEdit = { onEdit(requirement) },
                                onRemove = { onRemove(requirement) },
                            )
                        }
                    }
                }

                item {
                    OutlinedButton(
                        onClick = onAdd,
                        enabled = !isSearching,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 12.dp),
                        contentPadding = PaddingValues(vertical = 14.dp),
                    ) {
                        Text("＋  Add another item")
                    }
                }

                item {
                    SearchScopeCard(
                        maximumDepth = maximumDepth,
                        requireBlacksmith = requireBlacksmith,
                        enabled = !isSearching,
                        onMaximumDepthChange = onMaximumDepthChange,
                        onRequireBlacksmithChange = onRequireBlacksmithChange,
                        modifier = Modifier.padding(top = 20.dp),
                    )
                }

                item {
                    SearchControls(
                        requirementCount = requirements.size,
                        status = status,
                        isSearching = isSearching,
                        error = error,
                        onSearch = onSearch,
                        onCancel = onCancel,
                        modifier = Modifier.padding(top = 20.dp),
                    )
                }

                item {
                    ResultHeading(
                        count = results.size,
                        isSearching = isSearching,
                        status = status,
                        modifier = Modifier.padding(top = 32.dp, bottom = 12.dp),
                    )
                }

                if (results.isEmpty()) {
                    item {
                        EmptyResultsCard(isSearching = isSearching, status = status)
                    }
                } else {
                    items(results, key = { it.seed }) { result ->
                        ResultCard(
                            result = result,
                            modifier = Modifier.padding(bottom = 10.dp),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun SearchScopeCard(
    maximumDepth: Int,
    requireBlacksmith: Boolean,
    enabled: Boolean,
    onMaximumDepthChange: (Int) -> Unit,
    onRequireBlacksmithChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    OutlinedCard(modifier = modifier.fillMaxWidth(), shape = RoundedCornerShape(20.dp)) {
        Column(Modifier.padding(18.dp)) {
            Text("Search scope", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(5.dp))
            Text(
                "Every required item and facility must be available within the selected floor.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("First $maximumDepth floors", modifier = Modifier.weight(1f))
                Text("Floor $maximumDepth", color = MaterialTheme.colorScheme.primary)
            }
            Slider(
                value = maximumDepth.toFloat(),
                onValueChange = { onMaximumDepthChange(it.roundToInt()) },
                valueRange = 1f..24f,
                steps = 22,
                enabled = enabled,
            )
            FilterChip(
                selected = requireBlacksmith,
                onClick = { onRequireBlacksmithChange(!requireBlacksmith) },
                enabled = enabled,
                label = { Text("Require an accessible blacksmith") },
            )
        }
    }
}

@Composable
private fun HeroCard() {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(28.dp),
        color = Color.Transparent,
    ) {
        Box(
            modifier = Modifier
                .background(
                    Brush.linearGradient(
                        colors = listOf(Color(0xFF173B32), Color(0xFF252C24), Color(0xFF3B2D16)),
                        start = Offset.Zero,
                        end = Offset(1100f, 700f),
                    ),
                )
                .padding(24.dp),
        ) {
            Column {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CompassMark(Modifier.size(42.dp))
                    Spacer(Modifier.width(14.dp))
                    Surface(
                        shape = CircleShape,
                        color = Color.White.copy(alpha = 0.08f),
                    ) {
                        Text(
                            "NATIVE-READY SEARCH",
                            modifier = Modifier.padding(horizontal = 12.dp, vertical = 7.dp),
                            style = MaterialTheme.typography.labelSmall,
                            letterSpacing = 1.2.sp,
                            color = Mint,
                        )
                    }
                }
                Spacer(Modifier.height(22.dp))
                Text(
                    "Describe the loot.\nFind the world.",
                    style = MaterialTheme.typography.headlineLarge,
                    color = Color(0xFFF5F4E9),
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    "Combine upgraded weapons, armor, wands, rings, enchantments, and glyphs into one exact search.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = Color(0xFFD5DED5),
                )
            }
        }
    }
}

@Composable
private fun CompassMark(modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val stroke = size.minDimension * 0.07f
        drawCircle(Mint, style = Stroke(stroke))
        drawLine(
            color = Amber,
            start = Offset(size.width * 0.32f, size.height * 0.72f),
            end = Offset(size.width * 0.67f, size.height * 0.28f),
            strokeWidth = stroke * 1.25f,
            cap = StrokeCap.Round,
        )
        drawCircle(Color.White, radius = stroke * 0.8f, center = center)
    }
}

@Composable
private fun SectionHeading(
    eyebrow: String,
    title: String,
    supporting: String,
    modifier: Modifier = Modifier,
) {
    Column(modifier) {
        Text(
            eyebrow,
            style = MaterialTheme.typography.labelSmall,
            letterSpacing = 1.4.sp,
            color = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.height(5.dp))
        Text(title, style = MaterialTheme.typography.headlineSmall)
        Spacer(Modifier.height(5.dp))
        Text(
            supporting,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun EmptyRequirementsCard() {
    OutlinedCard(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("No requirements yet", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(6.dp))
            Text(
                "Choose a category or item, upgrade predicate, source, and optional same-item group.",
                textAlign = TextAlign.Center,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun RequirementCard(
    requirement: ItemRequirement,
    enabled: Boolean,
    onEdit: () -> Unit,
    onRemove: () -> Unit,
) {
    OutlinedCard(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.outlinedCardColors(containerColor = RaisedMoss),
        border = CardDefaults.outlinedCardBorder(enabled = true),
        shape = RoundedCornerShape(20.dp),
    ) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Surface(
                    modifier = Modifier.size(64.dp),
                    shape = RoundedCornerShape(17.dp),
                    color = MaterialTheme.colorScheme.background.copy(alpha = 0.55f),
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        val item = requirement.item
                        if (item == null) {
                            Text("?", style = MaterialTheme.typography.headlineMedium, color = Mint)
                        } else {
                            ItemSprite(
                                item = item,
                                modifierName = requirement.modifier,
                                modifier = Modifier.size(48.dp),
                            )
                        }
                    }
                }
                Spacer(Modifier.width(15.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        requirement.item?.name ?: "Any ${requirement.kind.label.lowercase(Locale.ROOT)}",
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Spacer(Modifier.height(5.dp))
                    Text(
                        requirement.description,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    requirement.item?.tier?.let {
                        Text(
                            "Tier $it ${requirement.kind.name.lowercase(Locale.ROOT)}",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                Surface(
                    shape = CircleShape,
                    color = MaterialTheme.colorScheme.primaryContainer,
                ) {
                    Text(
                        when (requirement.upgradeMatch) {
                            UpgradeMatch.ANY -> "Any"
                            UpgradeMatch.EXACT -> "= +${requirement.upgrade}"
                            UpgradeMatch.AT_LEAST -> "≥ +${requirement.upgrade}"
                        },
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 9.dp),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                TextButton(onClick = onEdit, enabled = enabled) { Text("Edit") }
                TextButton(onClick = onRemove, enabled = enabled) {
                    Text("Remove", color = if (enabled) MaterialTheme.colorScheme.error else Muted)
                }
            }
        }
    }
}

@Composable
private fun AndConnector() {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(48.dp),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .width(1.dp)
                .fillMaxHeight()
                .background(MaterialTheme.colorScheme.outlineVariant),
        )
        Surface(
            shape = CircleShape,
            color = MaterialTheme.colorScheme.background,
            border = CardDefaults.outlinedCardBorder(enabled = true),
        ) {
            Text(
                "AND",
                modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                style = MaterialTheme.typography.labelSmall,
                letterSpacing = 1.sp,
                color = MaterialTheme.colorScheme.secondary,
            )
        }
    }
}

@Composable
private fun SearchControls(
    requirementCount: Int,
    status: SearchStatus?,
    isSearching: Boolean,
    error: String?,
    onSearch: () -> Unit,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = DeepMoss),
        shape = RoundedCornerShape(22.dp),
    ) {
        Column(Modifier.padding(18.dp)) {
            if (isSearching) {
                val fraction = if (status != null && status.totalSeeds > 0) {
                    (status.scannedSeeds.toDouble() / status.totalSeeds.toDouble())
                        .coerceIn(0.0, 1.0)
                        .toFloat()
                } else {
                    0f
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Searching seeds in order…", style = MaterialTheme.typography.titleMedium)
                        Text(
                            "${compactCount(status?.scannedSeeds ?: 0)} seeds checked",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Text(
                        "${(fraction * 100).toInt()}%",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
                Spacer(Modifier.height(14.dp))
                LinearProgressIndicator(
                    progress = { fraction },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(7.dp)
                        .clip(CircleShape),
                    color = MaterialTheme.colorScheme.primary,
                    trackColor = MaterialTheme.colorScheme.surfaceVariant,
                )
                Spacer(Modifier.height(14.dp))
                OutlinedButton(onClick = onCancel, modifier = Modifier.fillMaxWidth()) {
                    Text("Cancel search")
                }
            } else {
                Text(
                    if (requirementCount == 1) "Ready to match 1 requirement" else "Ready to match $requirementCount requirements",
                    style = MaterialTheme.typography.titleMedium,
                )
                Spacer(Modifier.height(5.dp))
                Text(
                    "Requirements use AND; each result must satisfy all of them.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                error?.let {
                    Spacer(Modifier.height(10.dp))
                    Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
                Spacer(Modifier.height(14.dp))
                Button(
                    onClick = onSearch,
                    enabled = requirementCount > 0,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(52.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
                ) {
                    Text("Search seeds")
                }
                if (status?.state == SearchState.CANCELLED) {
                    Spacer(Modifier.height(9.dp))
                    Text(
                        "Search cancelled. Results found so far are kept below.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun ResultHeading(
    count: Int,
    isSearching: Boolean,
    status: SearchStatus?,
    modifier: Modifier = Modifier,
) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
        Column(Modifier.weight(1f)) {
            Text(
                "MATCHING WORLDS",
                style = MaterialTheme.typography.labelSmall,
                letterSpacing = 1.4.sp,
                color = MaterialTheme.colorScheme.primary,
            )
            Text("Results", style = MaterialTheme.typography.headlineSmall)
        }
        val label = when {
            isSearching -> "LIVE • $count"
            status?.state == SearchState.COMPLETED -> "$count FOUND"
            else -> count.toString()
        }
        Surface(shape = CircleShape, color = MaterialTheme.colorScheme.surfaceVariant) {
            Text(
                label,
                modifier = Modifier.padding(horizontal = 11.dp, vertical = 7.dp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun EmptyResultsCard(isSearching: Boolean, status: SearchStatus?) {
    OutlinedCard(modifier = Modifier.fillMaxWidth()) {
        Text(
            when {
                isSearching -> "Matches will appear here while the search continues."
                status?.state == SearchState.COMPLETED -> "No worlds matched every requirement. Try widening the search."
                else -> "Run a search to reveal seeds in XXX-XXX-XXX form."
            },
            modifier = Modifier.padding(22.dp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun ResultCard(result: SeedResult, modifier: Modifier = Modifier) {
    val clipboard = LocalClipboardManager.current
    Card(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = RaisedMoss),
        shape = RoundedCornerShape(18.dp),
    ) {
        Row(
            modifier = Modifier.padding(start = 18.dp, top = 12.dp, end = 8.dp, bottom = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    result.seed,
                    fontFamily = FontFamily.Monospace,
                    fontWeight = FontWeight.Bold,
                    fontSize = 21.sp,
                    letterSpacing = 1.1.sp,
                    color = MaterialTheme.colorScheme.secondary,
                )
                Text(
                    "All ${result.matchedRequirements} requirements matched",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TextButton(onClick = { clipboard.setText(AnnotatedString(result.seed)) }) {
                Text("Copy")
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ScoutScreen(
    seedInput: String,
    result: ScoutWorld?,
    isScouting: Boolean,
    error: String?,
    onSeedChange: (String) -> Unit,
    onScout: () -> Unit,
    onBack: () -> Unit,
    onAbout: () -> Unit,
) {
    val seedIsReady = SeedCode.isCanonical(seedInput)
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text("Scout seed", style = MaterialTheme.typography.titleLarge)
                        Text(
                            "Inspect one generated world",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                navigationIcon = {
                    TextButton(onClick = onBack) { Text("Back") }
                },
                actions = {
                    TextButton(onClick = onAbout) { Text("About") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Ink.copy(alpha = 0.96f)),
            )
        },
    ) { scaffoldPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(scaffoldPadding),
            contentAlignment = Alignment.TopCenter,
        ) {
            LazyColumn(
                modifier = Modifier
                    .fillMaxHeight()
                    .fillMaxWidth()
                    .widthIn(max = 760.dp)
                    .navigationBarsPadding(),
                contentPadding = PaddingValues(start = 20.dp, top = 18.dp, end = 20.dp, bottom = 40.dp),
            ) {
                item {
                    SectionHeading(
                        eyebrow = "ONE WORLD • FLOORS 1–24",
                        title = "See what a seed contains",
                        supporting = "Enter a seed to list its generated weapons, armor, wands, and rings, including upgrade rolls, modifiers, curses, and sources.",
                    )
                }

                item {
                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 18.dp),
                        colors = CardDefaults.cardColors(containerColor = DeepMoss),
                        shape = RoundedCornerShape(22.dp),
                    ) {
                        Column(Modifier.padding(18.dp)) {
                            OutlinedTextField(
                                value = seedInput,
                                onValueChange = onSeedChange,
                                enabled = !isScouting,
                                modifier = Modifier.fillMaxWidth(),
                                label = { Text("Seed") },
                                placeholder = { Text("ABC-DEF-GHI") },
                                supportingText = {
                                    Text(
                                        if (seedIsReady) {
                                            "Ready to scout this world"
                                        } else {
                                            "Type or paste nine letters; hyphens are added automatically."
                                        },
                                    )
                                },
                                singleLine = true,
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
                            ) {
                                Text(if (isScouting) "Generating world…" else "Scout seed")
                            }
                            if (isScouting) {
                                Spacer(Modifier.height(12.dp))
                                LinearProgressIndicator(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(7.dp)
                                        .clip(CircleShape),
                                )
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

                if (result == null && !isScouting) {
                    item {
                        OutlinedCard(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(top = 22.dp),
                        ) {
                            Text(
                                "Scout a seed to reveal all searchable equipment generated through floor 24.",
                                modifier = Modifier.padding(22.dp),
                                textAlign = TextAlign.Center,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }

                result?.let { world ->
                    item {
                        ScoutSummaryCard(
                            world = world,
                            modifier = Modifier.padding(top = 26.dp, bottom = 8.dp),
                        )
                    }

                    world.items
                        .groupBy(ScoutItem::depth)
                        .toSortedMap()
                        .forEach { (depth, floorItems) ->
                            item(key = "floor-$depth") {
                                ScoutFloorHeading(
                                    depth = depth,
                                    itemCount = floorItems.size,
                                    modifier = Modifier.padding(top = 18.dp, bottom = 10.dp),
                                )
                            }
                            floorItems.forEachIndexed { index, scoutItem ->
                                item(key = "scout-$depth-$index-${scoutItem.item.id}") {
                                    ScoutItemCard(
                                        scoutItem = scoutItem,
                                        modifier = Modifier.padding(bottom = 10.dp),
                                    )
                                }
                            }
                        }
                }
            }
        }
    }
}

@Composable
private fun ScoutSummaryCard(world: ScoutWorld, modifier: Modifier = Modifier) {
    val weaponCount = world.items.count { it.item.kind == ItemKind.WEAPON }
    val armorCount = world.items.count { it.item.kind == ItemKind.ARMOR }
    val wandCount = world.items.count { it.item.kind == ItemKind.WAND }
    val ringCount = world.items.count { it.item.kind == ItemKind.RING }
    Card(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = RaisedMoss),
        shape = RoundedCornerShape(22.dp),
    ) {
        Column(Modifier.padding(18.dp)) {
            Text(
                "CONTENTS OF",
                style = MaterialTheme.typography.labelSmall,
                letterSpacing = 1.4.sp,
                color = MaterialTheme.colorScheme.primary,
            )
            Text(
                world.seed,
                style = MaterialTheme.typography.headlineSmall,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.secondary,
            )
            Spacer(Modifier.height(7.dp))
            Text(
                "${world.items.size} items • $weaponCount weapons • $armorCount armor • $wandCount wands • $ringCount rings",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(6.dp))
            Text(
                "Reward alternatives are shown individually and marked when a choice or route affects access.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun ScoutFloorHeading(depth: Int, itemCount: Int, modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            "FLOOR $depth",
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.labelLarge,
            letterSpacing = 1.1.sp,
            color = MaterialTheme.colorScheme.primary,
        )
        Text(
            if (itemCount == 1) "1 item" else "$itemCount items",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun ScoutItemCard(scoutItem: ScoutItem, modifier: Modifier = Modifier) {
    val effectLabel = scoutItem.effect ?: when (scoutItem.item.kind) {
        ItemKind.WEAPON -> "No enchantment"
        ItemKind.ARMOR -> "No glyph"
        ItemKind.WAND, ItemKind.RING -> "No modifier"
    }
    val accessibilityLabel = when (scoutItem.accessibility) {
        ScoutAccessibility.Independent -> null
        is ScoutAccessibility.Choice -> {
            "Choice group ${scoutItem.accessibility.group + 1} • option ${scoutItem.accessibility.option + 1}"
        }
        is ScoutAccessibility.Scenarios -> {
            "Route group ${scoutItem.accessibility.group + 1} • access changes with room choices"
        }
    }

    OutlinedCard(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.outlinedCardColors(containerColor = RaisedMoss),
        shape = RoundedCornerShape(20.dp),
    ) {
        Row(
            modifier = Modifier.padding(15.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Surface(
                modifier = Modifier.size(62.dp),
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.background.copy(alpha = 0.55f),
            ) {
                Box(contentAlignment = Alignment.Center) {
                    ItemSprite(
                        item = scoutItem.item,
                        modifierName = scoutItem.effect,
                        modifier = Modifier.size(46.dp),
                    )
                }
            }
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    scoutItem.item.name,
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    effectLabel,
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (scoutItem.effect == null) {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    } else {
                        MaterialTheme.colorScheme.secondary
                    },
                )
                Text(
                    scoutItem.source.label,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (scoutItem.cursed) {
                    Text(
                        "Cursed",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                accessibilityLabel?.let {
                    Spacer(Modifier.height(3.dp))
                    Text(
                        it,
                        style = MaterialTheme.typography.labelSmall,
                        color = Amber,
                    )
                }
            }
            Spacer(Modifier.width(8.dp))
            Surface(
                shape = CircleShape,
                color = MaterialTheme.colorScheme.primaryContainer,
            ) {
                Text(
                    "+${scoutItem.upgrade}",
                    modifier = Modifier.padding(horizontal = 11.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RequirementSheet(
    editing: ItemRequirement?,
    onDismiss: () -> Unit,
    onSave: (CatalogItem?, ItemKind, UpgradeMatch, Int, String?, ScoutItemSource?, Int?) -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val identity = editing?.key ?: -1L
    var kind by remember(identity) { mutableStateOf(editing?.kind ?: ItemKind.WEAPON) }
    var selectedItem by remember(identity) {
        mutableStateOf<CatalogItem?>(
            if (editing == null) ItemCatalog.forKind(kind).first() else editing.item,
        )
    }
    var upgradeMatch by remember(identity) { mutableStateOf(editing?.upgradeMatch ?: UpgradeMatch.EXACT) }
    var upgrade by remember(identity) { mutableStateOf(editing?.upgrade ?: 1) }
    var modifierName by remember(identity) { mutableStateOf(editing?.modifier) }
    var modifierMenuExpanded by remember(identity) { mutableStateOf(false) }
    var source by remember(identity) { mutableStateOf(editing?.source) }
    var sourceMenuExpanded by remember(identity) { mutableStateOf(false) }
    var identityGroup by remember(identity) { mutableStateOf(editing?.identityGroup) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = DeepMoss,
        dragHandle = {
            Surface(
                modifier = Modifier
                    .padding(top = 12.dp, bottom = 8.dp)
                    .size(width = 42.dp, height = 4.dp),
                shape = CircleShape,
                color = MaterialTheme.colorScheme.outline,
            ) { }
        },
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .padding(bottom = 16.dp),
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(
                        if (editing == null) "Add requirement" else "Edit requirement",
                        style = MaterialTheme.typography.headlineSmall,
                    )
                    Text(
                        "Combine general item, upgrade, source, and identity constraints.",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(onClick = onDismiss) { Text("Close") }
            }

            TabRow(
                selectedTabIndex = kind.ordinal,
                containerColor = Color.Transparent,
                divider = { HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant) },
            ) {
                ItemKind.entries.forEach { tabKind ->
                    Tab(
                        selected = kind == tabKind,
                        onClick = {
                            kind = tabKind
                            selectedItem = ItemCatalog.forKind(tabKind).first()
                            upgrade = when (upgradeMatch) {
                                UpgradeMatch.ANY -> 0
                                UpgradeMatch.EXACT -> upgrade.coerceIn(1, tabKind.maximumSearchUpgrade)
                                UpgradeMatch.AT_LEAST -> upgrade.coerceIn(0, tabKind.maximumSearchUpgrade)
                            }
                            modifierName = null
                        },
                        text = { Text(tabKind.label) },
                    )
                }
            }

            Row(
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                FilterChip(
                    selected = selectedItem == null,
                    onClick = { selectedItem = null },
                    label = { Text("Any ${kind.label.lowercase(Locale.ROOT)}") },
                )
                Spacer(Modifier.width(10.dp))
                Text(
                    "Use a same-item group to link wildcard requirements.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f),
                )
            }

            LazyVerticalGrid(
                columns = GridCells.Adaptive(92.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = 180.dp, max = 300.dp),
                contentPadding = PaddingValues(14.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(ItemCatalog.forKind(kind), key = { it.id }) { item ->
                    ItemTile(
                        item = item,
                        selected = selectedItem?.id == item.id,
                        onClick = { selectedItem = item },
                    )
                }
            }

            Column(
                modifier = Modifier
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 20.dp),
            ) {
                Text("Upgrade", style = MaterialTheme.typography.titleMedium)
                Spacer(Modifier.height(9.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(9.dp)) {
                    UpgradeMatch.entries.forEach { match ->
                        FilterChip(
                            selected = upgradeMatch == match,
                            onClick = {
                                upgradeMatch = match
                                upgrade = when (match) {
                                    UpgradeMatch.ANY -> 0
                                    UpgradeMatch.EXACT -> upgrade.coerceIn(1, kind.maximumSearchUpgrade)
                                    UpgradeMatch.AT_LEAST -> upgrade.coerceIn(0, kind.maximumSearchUpgrade)
                                }
                            },
                            label = { Text(match.label) },
                        )
                    }
                }
                if (upgradeMatch != UpgradeMatch.ANY) {
                    Spacer(Modifier.height(8.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(9.dp)) {
                        val values = if (upgradeMatch == UpgradeMatch.EXACT) {
                            1..kind.maximumSearchUpgrade
                        } else {
                            0..kind.maximumSearchUpgrade
                        }
                        values.forEach { value ->
                            FilterChip(
                                selected = upgrade == value,
                                onClick = { upgrade = value },
                                label = { Text("+$value") },
                            )
                        }
                    }
                }

                val modifierLabel = kind.modifierLabel
                if (modifierLabel != null) {
                    Spacer(Modifier.height(16.dp))
                    Text(modifierLabel, style = MaterialTheme.typography.titleMedium)
                    Spacer(Modifier.height(8.dp))
                    ExposedDropdownMenuBox(
                        expanded = modifierMenuExpanded,
                        onExpandedChange = { modifierMenuExpanded = it },
                    ) {
                        OutlinedTextField(
                            value = modifierName ?: "Any / none required",
                            onValueChange = { },
                            readOnly = true,
                            singleLine = true,
                            label = { Text("$modifierLabel requirement") },
                            trailingIcon = {
                                ExposedDropdownMenuDefaults.TrailingIcon(expanded = modifierMenuExpanded)
                            },
                            modifier = Modifier
                                .menuAnchor(MenuAnchorType.PrimaryNotEditable, enabled = true)
                                .fillMaxWidth(),
                        )
                        ExposedDropdownMenu(
                            expanded = modifierMenuExpanded,
                            onDismissRequest = { modifierMenuExpanded = false },
                        ) {
                            DropdownMenuItem(
                                text = { Text("Any / none required") },
                                onClick = {
                                    modifierName = null
                                    modifierMenuExpanded = false
                                },
                            )
                            Text(
                                if (kind == ItemKind.WEAPON) "ENCHANTMENTS" else "GLYPHS",
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                                style = MaterialTheme.typography.labelSmall,
                                letterSpacing = 1.sp,
                                color = MaterialTheme.colorScheme.primary,
                            )
                            val regularModifiers = if (kind == ItemKind.WEAPON) {
                                ItemCatalog.enchantments
                            } else {
                                ItemCatalog.glyphs
                            }
                            regularModifiers.forEach { option ->
                                DropdownMenuItem(
                                    text = { Text(option) },
                                    onClick = {
                                        modifierName = option
                                        modifierMenuExpanded = false
                                    },
                                )
                            }
                            HorizontalDivider(
                                modifier = Modifier.padding(vertical = 5.dp),
                                color = MaterialTheme.colorScheme.outlineVariant,
                            )
                            Text(
                                "CURSES",
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                                style = MaterialTheme.typography.labelSmall,
                                letterSpacing = 1.sp,
                                color = MaterialTheme.colorScheme.error,
                            )
                            ItemCatalog.cursesFor(kind).forEach { option ->
                                DropdownMenuItem(
                                    text = { Text(option) },
                                    onClick = {
                                        modifierName = option
                                        modifierMenuExpanded = false
                                    },
                                )
                            }
                        }
                    }
                }

                Spacer(Modifier.height(16.dp))
                Text("Source", style = MaterialTheme.typography.titleMedium)
                Spacer(Modifier.height(8.dp))
                ExposedDropdownMenuBox(
                    expanded = sourceMenuExpanded,
                    onExpandedChange = { sourceMenuExpanded = it },
                ) {
                    OutlinedTextField(
                        value = source?.label ?: "Any source",
                        onValueChange = { },
                        readOnly = true,
                        singleLine = true,
                        label = { Text("Where it must come from") },
                        trailingIcon = {
                            ExposedDropdownMenuDefaults.TrailingIcon(expanded = sourceMenuExpanded)
                        },
                        modifier = Modifier
                            .menuAnchor(MenuAnchorType.PrimaryNotEditable, enabled = true)
                            .fillMaxWidth(),
                    )
                    ExposedDropdownMenu(
                        expanded = sourceMenuExpanded,
                        onDismissRequest = { sourceMenuExpanded = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text("Any source") },
                            onClick = {
                                source = null
                                sourceMenuExpanded = false
                            },
                        )
                        ScoutItemSource.entries.forEach { option ->
                            DropdownMenuItem(
                                text = { Text(option.label) },
                                onClick = {
                                    source = option
                                    sourceMenuExpanded = false
                                },
                            )
                        }
                    }
                }

                Spacer(Modifier.height(16.dp))
                Text("Same-item group", style = MaterialTheme.typography.titleMedium)
                Text(
                    "Requirements sharing a letter must resolve to the exact same item type, using distinct copies.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = identityGroup == null,
                        onClick = { identityGroup = null },
                        label = { Text("None") },
                    )
                    (1..4).forEach { group ->
                        FilterChip(
                            selected = identityGroup == group,
                            onClick = { identityGroup = group },
                            label = { Text(('A'.code + group - 1).toChar().toString()) },
                        )
                    }
                }

                Spacer(Modifier.height(18.dp))
                SelectedRequirementPreview(
                    item = selectedItem,
                    kind = kind,
                    upgradeMatch = upgradeMatch,
                    upgrade = upgrade,
                    modifierName = modifierName,
                    source = source,
                    identityGroup = identityGroup,
                )
                Spacer(Modifier.height(14.dp))
                Button(
                    onClick = {
                        onSave(
                            selectedItem,
                            kind,
                            upgradeMatch,
                            upgrade,
                            modifierName,
                            source,
                            identityGroup,
                        )
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(52.dp),
                ) {
                    Text(if (editing == null) "Add to search" else "Save changes")
                }
            }
        }
    }
}

@Composable
private fun ItemTile(item: CatalogItem, selected: Boolean, onClick: () -> Unit) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .height(106.dp)
            .selectable(selected = selected, onClick = onClick),
        shape = RoundedCornerShape(15.dp),
        color = if (selected) MaterialTheme.colorScheme.primaryContainer else RaisedMoss,
        border = if (selected) {
            androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.primary)
        } else {
            androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
        },
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 9.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            ItemSprite(item, modifier = Modifier.size(42.dp))
            Spacer(Modifier.height(5.dp))
            Text(
                item.name,
                style = MaterialTheme.typography.labelSmall,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            item.tier?.let {
                Text(
                    "Tier $it",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun SelectedRequirementPreview(
    item: CatalogItem?,
    kind: ItemKind,
    upgradeMatch: UpgradeMatch,
    upgrade: Int,
    modifierName: String?,
    source: ScoutItemSource?,
    identityGroup: Int?,
) {
    Surface(
        shape = RoundedCornerShape(18.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (item == null) {
                Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) {
                    Text("?", style = MaterialTheme.typography.headlineMedium, color = Mint)
                }
            } else {
                ItemSprite(item, modifierName, Modifier.size(48.dp))
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    item?.name ?: "Any ${kind.label.lowercase(Locale.ROOT)}",
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    buildString {
                        append(
                            when (upgradeMatch) {
                                UpgradeMatch.ANY -> "Any upgrade"
                                UpgradeMatch.EXACT -> "+$upgrade exactly"
                                UpgradeMatch.AT_LEAST -> "+$upgrade or higher"
                            },
                        )
                        modifierName?.let { append(" • $it") }
                        source?.let { append(" • ${it.label}") }
                        identityGroup?.let {
                            append(" • group ${('A'.code + it - 1).toChar()}")
                        }
                    },
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AboutScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    var showLicense by remember { mutableStateOf(false) }
    val licenseText = remember(context) {
        runCatching {
            context.assets.open(LICENSE_PATH).bufferedReader().use { it.readText() }
        }.getOrElse { "License text could not be loaded: ${it.message}" }
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("About & licenses") },
                navigationIcon = {
                    TextButton(onClick = onBack) { Text("Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Ink.copy(alpha = 0.96f)),
            )
        },
    ) { scaffoldPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(scaffoldPadding),
            contentAlignment = Alignment.TopCenter,
        ) {
            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .widthIn(max = 760.dp)
                    .navigationBarsPadding(),
                contentPadding = PaddingValues(20.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                item {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Surface(shape = CircleShape, color = MaterialTheme.colorScheme.primaryContainer) {
                            Box(Modifier.size(68.dp), contentAlignment = Alignment.Center) {
                                CompassMark(Modifier.size(40.dp))
                            }
                        }
                        Spacer(Modifier.width(16.dp))
                        Column {
                            Text("Seed Seeker", style = MaterialTheme.typography.headlineSmall)
                            Text(
                                "Independent • unofficial • open source",
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }

                item {
                    AboutSection("Exact offline engine") {
                        Text(
                            "The native Rust engine recreates the pinned v3.3.8 main dungeon through depth 24. It uses all available CPU cores, reports matches live, and completes after the first 1,024 matches or the end of the seed space.",
                        )
                    }
                }

                item {
                    AboutSection("Not the game") {
                        Text(
                            "Seed Seeker is an independent utility and is not affiliated with or endorsed by Shattered Pixel Dungeon or its authors. Its interface and compass icon are original; no game UI components are used.",
                        )
                    }
                }

                item {
                    AboutSection("Artwork attribution") {
                        Text("The item sprites are an unchanged copy of Shattered Pixel Dungeon's items.png atlas.")
                        Spacer(Modifier.height(10.dp))
                        AttributionLine("Upstream", "Shattered Pixel Dungeon v3.3.8")
                        AttributionLine("Commit", "7b8b845a76fe76c6b7c031ae9e570852411f56db")
                        AttributionLine("Pixel Dungeon", "© 2012–2015 Oleg Dolya")
                        AttributionLine("Shattered Pixel Dungeon", "© 2014–2026 Evan Debenham")
                        AttributionLine("Atlas SHA-256", "ce2496368660e9b2…a294caacaf")
                    }
                }

                item {
                    AboutSection("GNU GPL v3 or later") {
                        Text(
                            "This program is free software. You may redistribute and modify it under GPL-3.0-or-later. It comes with no warranty. Source distributions must retain the license and copyright notices.",
                        )
                        Spacer(Modifier.height(8.dp))
                        TextButton(onClick = { showLicense = !showLicense }) {
                            Text(if (showLicense) "Hide full license" else "Read full license")
                        }
                    }
                }

                if (showLicense) {
                    item {
                        OutlinedCard(modifier = Modifier.fillMaxWidth()) {
                            SelectionContainer {
                                Text(
                                    licenseText,
                                    modifier = Modifier.padding(16.dp),
                                    style = MaterialTheme.typography.bodySmall,
                                    fontFamily = FontFamily.Monospace,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }

                item {
                    Text(
                        "Seed Seeker ${BuildConfig.VERSION_NAME} • Shattered Pixel Dungeon v3.3.8 profile",
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 14.dp),
                        textAlign = TextAlign.Center,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun AboutSection(title: String, content: @Composable ColumnScope.() -> Unit) {
    OutlinedCard(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.outlinedCardColors(containerColor = RaisedMoss),
        shape = RoundedCornerShape(20.dp),
    ) {
        Column(Modifier.padding(18.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(9.dp))
            content()
        }
    }
}

@Composable
private fun AttributionLine(label: String, value: String) {
    Column(Modifier.padding(vertical = 4.dp)) {
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
        Text(value, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun ItemSprite(
    item: CatalogItem,
    modifierName: String? = null,
    modifier: Modifier = Modifier,
) {
    val atlas = LocalItemAtlas.current
    val placeholderColor = MaterialTheme.colorScheme.outline
    Canvas(
        modifier = modifier.semantics { contentDescription = item.name },
    ) {
        if (modifierName != null) {
            val glow = if (item.kind == ItemKind.ARMOR) Mint else Amber
            drawCircle(glow, radius = size.minDimension * 0.46f, alpha = 0.23f)
            drawCircle(glow, radius = size.minDimension * 0.34f, alpha = 0.16f)
        }
        if (atlas != null) {
            drawImage(
                image = atlas,
                srcOffset = IntOffset(
                    x = (item.spriteIndex % 16) * 16,
                    y = (item.spriteIndex / 16) * 16,
                ),
                srcSize = IntSize(16, 16),
                dstOffset = IntOffset.Zero,
                dstSize = IntSize(size.width.toInt(), size.height.toInt()),
                filterQuality = FilterQuality.None,
            )
        } else {
            drawCircle(placeholderColor, radius = size.minDimension * 0.28f)
        }
    }
}

private fun compactCount(value: Long): String = when {
    value >= 1_000_000_000_000L -> String.format(Locale.US, "%.2fT", value / 1_000_000_000_000.0)
    value >= 1_000_000_000L -> String.format(Locale.US, "%.2fB", value / 1_000_000_000.0)
    value >= 1_000_000L -> String.format(Locale.US, "%.1fM", value / 1_000_000.0)
    value >= 1_000L -> String.format(Locale.US, "%.1fK", value / 1_000.0)
    else -> value.toString()
}
