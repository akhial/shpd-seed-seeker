// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.engine.ScoutMatches
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.BuiltInPresets
import dev.seedseeker.app.model.CatalogItem
import dev.seedseeker.app.model.EffectFilter
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.PresetQuery
import dev.seedseeker.app.model.RingGems
import dev.seedseeker.app.model.ScoutAccessibility
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutItemSource
import dev.seedseeker.app.model.ScoutWorld
import dev.seedseeker.app.model.SearchState
import dev.seedseeker.app.model.SearchStatus
import dev.seedseeker.app.model.SeedResult
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assume.assumeTrue
import org.junit.BeforeClass
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

/**
 * Renders every screen of the expressive redesign to PNGs under
 * `build/outputs/showcase/` for visual review. Opt-in, since it exists for
 * eyes rather than assertions: run with `SEEDSEEKER_SHOWCASE=1`.
 */
@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w412dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ExpressiveShowcaseTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    companion object {
        /** Skips the whole class before any activity launches unless asked for. */
        @JvmStatic @BeforeClass fun optIn() = assumeTrue(System.getenv("SEEDSEEKER_SHOWCASE") == "1")
    }

    private fun find(id: String): CatalogItem = requireNotNull(ItemCatalog.findById(id)) { id }

    private val board = listOf(
        ItemRequirement(key = 1, item = find("wand_fireblast"), upgrade = 3),
        ItemRequirement(key = 2, item = null, kind = ItemKind.RING, upgrade = 2),
        ItemRequirement(key = 3, item = null, kind = ItemKind.WEAPON, upgrade = 2, effect = EffectFilter.AnyEnchantment, maximumDepth = 10),
        ItemRequirement(key = 4, item = find("wand_lightning"), upgrade = 2, alternativeGroup = 1),
        ItemRequirement(key = 5, item = find("wand_frost"), upgrade = 2, alternativeGroup = 1),
    )

    private fun host(settle: Boolean = true, content: @Composable () -> Unit) {
        if (!settle) compose.mainClock.autoAdvance = false
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        val iconAtlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/item_icons.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas, LocalItemIconAtlas provides iconAtlas) {
                    content()
                }
            }
        }
        if (settle) compose.waitForIdle() else pump()
    }

    /** The newest dialog window, pumping frames until one has opened. */
    private fun dialogWindow(): android.view.Window {
        repeat(10) {
            org.robolectric.shadows.ShadowDialog.getLatestDialog()?.window?.let { return it }
            pump(30)
        }
        return requireNotNull(org.robolectric.shadows.ShadowDialog.getLatestDialog()?.window) { "no dialog opened" }
    }

    private fun shot(name: String, window: android.view.Window = compose.activity.window, settle: Boolean = true) {
        if (settle) compose.waitForIdle() else pump()
        System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
        val bitmap = Bitmap.createBitmap(window.decorView.width, window.decorView.height, Bitmap.Config.ARGB_8888)
        var result: Int? = null
        compose.activity.runOnUiThread { PixelCopy.request(window, bitmap, { result = it }, Handler(Looper.getMainLooper())) }
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
        repeat(50) { if (result == null) org.robolectric.shadows.ShadowLooper.idleMainLooper() }
        val output = File("build/outputs/showcase/$name.png")
        output.parentFile?.mkdirs()
        output.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    /** Advances frames by hand for screens that never report idle under Robolectric. */
    private fun pump(frames: Int = 90) = repeat(frames) {
        compose.mainClock.advanceTimeByFrame()
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
    }

    @Composable
    private fun Finder(
        requirements: List<ItemRequirement> = board,
        results: List<SeedResult> = emptyList(),
        searching: Boolean = false,
        status: SearchStatus? = null,
        validation: String? = null,
    ) = FinderScreen(
        requirements = requirements, maximumDepth = 24, autoApplyTrinket = true, floorRequirements = emptyList(),
        arcaneResin = 0, arcaneResinAuto = true, arcaneResinFilter = ArcaneResinFilter(),
        requireBlacksmith = false, excludeBlacksmithRewards = false, wandmakerQuest = null, challenges = 0,
        presets = BuiltInPresets.all, compactChips = false, results = results, foundCount = results.size,
        status = status, seedsPerSecond = 184_300.0, elapsedSeconds = 263, isSearching = searching,
        refinePhase = null, refineProgress = null, error = null, snackbarHostState = SnackbarHostState(),
        onAbout = {}, onSettings = {}, onSearchSettings = {}, onApplyPreset = {}, onSavePreset = {},
        onDeletePreset = {}, onEditResin = {}, onRemoveResin = {}, onAdd = {}, onEdit = { _, _ -> },
        onRequirementsChange = {}, onRemove = {}, validationMessage = validation, onSearch = {}, onCancel = {},
        canExportResults = true, canClearResults = true, importNotice = null, onExportResults = {},
        onImportResults = {}, onImportClipboard = {}, onClearResults = {}, onShareQuery = {}, onScoutSeed = {},
        bottomBar = { Box(Modifier.fillMaxWidth().height(80.dp)) },
    )

    private val seeds = listOf("EQI-HLQ-RTU", "ABC-DEF-GHI", "KZP-QRM-WAA", "TUV-XYZ-JKL", "MNO-PQR-STU", "BBQ-LOL-OMG")
        .mapIndexed { index, seed -> SeedResult(seed, 5, if (index % 2 == 0) "mimic_tooth" else null) }

    @Test fun finderBoard() {
        host { Finder() }
        shot("finder-board")
    }

    @Test fun boardDrag() {
        host { Finder() }
        val from = compose.onNodeWithContentDescription("Wand of Fireblast", substring = true)
            .fetchSemanticsNode().boundsInRoot.center
        val to = compose.onNodeWithContentDescription("Any ring", substring = true)
            .fetchSemanticsNode().boundsInRoot.center
        compose.onRoot().performTouchInput {
            down(from)
            advanceEventTime(900)
            moveTo(from + androidx.compose.ui.geometry.Offset(10f, 10f))
            moveTo(androidx.compose.ui.geometry.Offset((from.x + to.x) / 2f, to.y))
            moveTo(to)
        }
        shot("board-drag")
    }

    @Test fun finderEmptyBoard() {
        host { Finder(requirements = emptyList(), validation = "Add at least one requirement.") }
        shot("finder-empty")
    }

    @Test fun finderSearching() {
        host {
            Finder(
                results = seeds, searching = true,
                status = SearchStatus(SearchState.RUNNING, 48_400_000, 1L shl 40, matchProbability = 2.1e-6),
            )
        }
        shot("finder-searching")
    }

    @Test fun finderSearchingEmpty() {
        host { Finder(searching = true, status = SearchStatus(SearchState.RUNNING, 1_200_000, 1L shl 40, matchProbability = 2.1e-6)) }
        shot("finder-searching-empty")
    }

    @Test fun finderDone() {
        host { Finder(results = seeds, status = SearchStatus(SearchState.COMPLETED, 48_400_000, 48_400_000)) }
        shot("finder-done")
    }

    @Test fun presets() {
        host { Finder() }
        compose.mainClock.autoAdvance = false
        compose.onNodeWithText("Presets").performClick()
        shot("presets", dialogWindow(), settle = false)
    }

    @Test fun requirementSheet() {
        host(settle = false) { RequirementSheet(editing = null, onDismiss = {}, onSave = { _, _, _, _ -> }) }
        shot("sheet-item", dialogWindow(), settle = false)
        compose.onNodeWithText("Next").performClick()
        pump()
        shot("sheet-details", dialogWindow(), settle = false)
        compose.onNodeWithText("Specific…").performClick()
        pump()
        shot("sheet-effects", dialogWindow(), settle = false)
    }

    @Test fun resinSheet() {
        host(settle = false) { ArcaneResinSheet(amount = 6, filter = ArcaneResinFilter(), onDismiss = {}, onSave = { _, _, _ -> }, onRemove = {}) }
        shot("sheet-resin", dialogWindow(), settle = false)
    }

    private fun item(catalog: CatalogItem, depth: Int, upgrade: Int = 0, effect: String? = null, cursed: Boolean = false) = ScoutItem(
        item = catalog, depth = depth, upgrade = upgrade, effect = effect, cursed = cursed,
        source = ScoutItemSource.HEAP, accessibility = ScoutAccessibility.Independent,
    )

    private val world by lazy {
        val offers = listOf("cracked_spyglass", "dimensional_sundial", "mimic_tooth", "trap_mechanism").map(::find)
        ScoutWorld(
            seed = "EQI-HLQ-RTU",
            items = offers.map { item(it, 1) } + listOf(
                item(find("wand_fireblast"), 2, upgrade = 3),
                item(find("fishing_spear"), 2, upgrade = 1, effect = "Blazing"),
                item(find("force_cube"), 3, upgrade = 2, effect = "Chilling"),
                item(find("wand_frost"), 4, upgrade = 2),
                item(find("fishing_spear"), 7, cursed = true, effect = "Annoying"),
                item(find("wand_lightning"), 12, upgrade = 2),
                item(find("fishing_spear"), 17),
                item(find("wand_disintegration"), 2, cursed = true).copy(
                    source = ScoutItemSource.entries.first(),
                    accessibility = ScoutAccessibility.Choice(group = 8, option = 0),
                ),
            ),
            quests = emptyList(),
            ringGems = RingGems.CATALOG,
            trinketOrder = offers,
            itemMappings = dev.seedseeker.app.engine.JniNativeSeedFinder().scoutSeed("EQI-HLQ-RTU").itemMappings,
        )
    }

    @Composable
    private fun Scout(result: ScoutWorld?, scouting: Boolean = false) = ScoutScreen(
        seedInput = "EQI-HLQ-RTU", result = result, isScouting = scouting, error = null,
        matches = if (result == null) null else ScoutMatches(setOf(4, 7, 11), 3, 4),
        resultSeeds = seeds.map { it.seed }, scoutedSeed = result?.seed,
        onScoutSeed = {}, onSeedChange = {}, onScout = {}, onSelectTrinket = {},
        onSettings = {}, onAbout = {}, bottomBar = { Box(Modifier.fillMaxWidth().height(80.dp)) },
    )

    @Test fun scoutEmpty() {
        host { Scout(null) }
        shot("scout-empty")
    }

    @Test fun scoutLoading() {
        host { Scout(null, scouting = true) }
        shot("scout-loading")
    }

    @Test fun scoutWorld() {
        host { Scout(world) }
        shot("scout-world")
    }

    @Test @Config(qualifiers = "w360dp-h800dp-xhdpi") fun scoutCards() {
        host {
          val base = androidx.compose.ui.platform.LocalDensity.current
          CompositionLocalProvider(androidx.compose.ui.platform.LocalDensity provides androidx.compose.ui.unit.Density(base.density, 1.15f)) {
            androidx.compose.foundation.layout.Column(
                Modifier.padding(16.dp),
                verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
            ) {
                val cursedChoice = item(find("wand_disintegration"), 2, cursed = true).copy(
                    source = ScoutItemSource.entries.first(),
                    accessibility = ScoutAccessibility.Choice(group = 8, option = 0),
                )
                ScoutItemCard(cursedChoice, RingGems.CATALOG, matches = true)
                ScoutItemCard(cursedChoice.copy(effect = "Annoying"), RingGems.CATALOG, matches = true)
                ScoutItemCard(item(find("wand_fireblast"), 2, upgrade = 3), RingGems.CATALOG, matches = true)
            }
          }
        }
        shot("scout-cards")
    }

    @Test fun seedInfo() {
        host { Scout(world) }
        compose.onNodeWithContentDescription("Seed information").performClick()
        compose.mainClock.autoAdvance = false
        shot("seed-info", dialogWindow(), settle = false)
    }

    @Test fun settings() {
        host { SettingsScreen(compactChips = false, onCompactChipsChange = {}, onBack = {}) }
        shot("settings")
    }

    @Test fun searchSettings() {
        host {
            SearchSettingsScreen(
                query = PresetQuery(requirements = board), enabled = true, challengesEnabled = true,
                workerCount = 6, workerCeiling = 8, onQueryChange = {}, onWorkerCountChange = {}, onBack = {},
            )
        }
        shot("search-settings")
    }

    @Test fun about() {
        host { AboutScreen(onBack = {}) }
        shot("about")
    }
}
