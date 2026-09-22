// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.Context
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import androidx.activity.ComponentActivity
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Density
import dev.seedseeker.app.BuildConfig
import dev.seedseeker.app.engine.JniNativeSeedFinder
import dev.seedseeker.app.model.*
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import java.io.File
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w360dp-h800dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class SearchSettingsUiTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private var query by mutableStateOf(PresetQuery(emptyList(), maximumDepth = 4))
    private var workers by mutableIntStateOf(4)

    private fun show(enabled: Boolean = true, challengesEnabled: Boolean = true, fontScale: Float = 1f) {
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalDensity provides Density(compose.density.density, fontScale)) {
                    SearchSettingsScreen(query, enabled, challengesEnabled, workers, 8,
                        onQueryChange = { query = it }, onWorkerCountChange = { workers = it }, onBack = {})
                }
            }
        }
    }

    @Test fun settingsEditOneQueryAndKeepImportedFloors() {
        val imported = FloorRequirement(3, FloorFeeling.GRASS)
        query = query.copy(floorRequirements = listOf(imported))
        show()
        screenshot("overview")
        compose.onNodeWithContentDescription("Floor 7").performScrollTo().performClick().assertIsOn()
        compose.onNodeWithContentDescription("Floor 22").performClick().assertIsOn()
        compose.runOnIdle {
            assertEquals(22, query.maximumDepth)
            assertTrue(query.floorRequirements.contains(imported))
        }
        screenshot("farming-selected")
        compose.onNodeWithText("AutoTrinket").performScrollTo().performClick().assertIsOff()
        compose.onNodeWithText("Wandmaker quest").performScrollTo().performClick()
        compose.onNodeWithText(WandmakerQuest.ROTBERRY.label).performClick()
        compose.onNodeWithText("Blacksmith reachable").performScrollTo().assertIsNotEnabled().assertIsOn()
        compose.onNodeWithText("Exclude smith rewards").performScrollTo().performClick().assertIsOn()
        screenshot("quest-settings")
        compose.onNodeWithContentDescription("Workers").performScrollTo()
            .performSemanticsAction(SemanticsActions.SetProgress) { it(2f) }
        compose.onNodeWithText(Challenge.entries.first().displayName).performScrollTo().performClick().assertIsOn()
        screenshot("challenges")
        compose.runOnIdle {
            assertFalse(query.autoApplyTrinket)
            assertEquals(WandmakerQuest.ROTBERRY, query.wandmakerQuest)
            assertTrue(query.excludeBlacksmithRewards)
            assertEquals(2, workers)
            assertEquals(Challenge.entries.first().bit, query.challenges)
        }
        compose.onNodeWithContentDescription("Max floor").performScrollTo()
            .performSemanticsAction(SemanticsActions.SetProgress) { it(floorLimitIndex(4).toFloat()) }
        compose.onNodeWithText("Floor 7 exceeds the floor limit of 4.").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(imported.description).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Remove").performClick()
        compose.runOnIdle { assertFalse(query.floorRequirements.contains(imported)) }
    }

    @Test fun aRunningSearchLocksAllSearchControls() {
        show(enabled = false, challengesEnabled = false)
        compose.onNodeWithContentDescription("Max floor").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Floor 7").performScrollTo().assertIsNotEnabled()
        for (title in listOf("AutoTrinket", "Wandmaker quest", "Blacksmith reachable", "Exclude smith rewards")) {
            compose.onNodeWithText(title).performScrollTo().assertIsNotEnabled()
        }
        compose.onNodeWithContentDescription("Workers").performScrollTo().assertIsNotEnabled()
        for (challenge in Challenge.entries) {
            compose.onNodeWithText(challenge.displayName).performScrollTo().assertIsNotEnabled()
        }
    }

    @Test fun scoutingLocksChallengesButAllowsEditingTheNextSearch() {
        show(challengesEnabled = false)
        compose.onNodeWithContentDescription("Floor 17").performScrollTo().performClick().assertIsOn()
        compose.onNodeWithText(Challenge.entries.first().displayName).performScrollTo().assertIsNotEnabled()
        compose.runOnIdle { assertEquals(17, query.maximumDepth) }
    }

    @Test
    @Config(qualifiers = "w320dp-h800dp-xhdpi")
    fun largeTextOnANarrowScreenKeepsControlsReadableAndReachable() {
        query = query.toggleFarmingFloor(7).toggleFarmingFloor(22)
        show(fontScale = 2f)
        screenshot("large-text-overview")
        for (title in listOf("Max floor", "Ring of Wealth farming floors", "AutoTrinket", "Wandmaker quest", "Exclude smith rewards")) {
            val node = compose.onNodeWithText(title, useUnmergedTree = true)
            node.performScrollTo().assertIsDisplayed()
            var layout: TextLayoutResult? = null
            node.performSemanticsAction(SemanticsActions.GetTextLayoutResult) { action ->
                val results = mutableListOf<TextLayoutResult>()
                action(results)
                layout = results.single()
            }
            val textLayout = requireNotNull(layout)
            // Paragraph metrics are fractional pixels; the measured size rounds to whole pixels.
            assertTrue("$title clips vertically", textLayout.multiParagraph.height <= textLayout.size.height + 1f)
            for (line in 0 until textLayout.lineCount) {
                assertFalse("$title is ellipsized", textLayout.isLineEllipsized(line))
                assertTrue("$title clips horizontally", textLayout.getLineRight(line) <= textLayout.size.width + 1f)
            }
        }
        compose.onNodeWithContentDescription("Floor 22").performScrollTo().assertIsOn()
        screenshot("large-text-floors")
        compose.onNodeWithContentDescription("Floor 17").performClick().assertIsOn()
        compose.onNodeWithText(Challenge.entries.last().displayName).performScrollTo().performClick().assertIsOn()
        screenshot("large-text-challenges")
    }

    @Test fun finderAndAppSettingsNavigateBackAndPersistTheSameQuery() {
        val preferences = compose.activity.getSharedPreferences("seed_seeker_settings", Context.MODE_PRIVATE)
        preferences.edit().clear().commit()
        PresetStorage(preferences).saveCurrentQuery(query)
        compose.setContent {
            val scope = rememberCoroutineScope()
            val engine = remember { JniNativeSeedFinder() }
            val controller = remember {
                SearchController(engine, object : SearchCheckpointStore {
                    override fun load() = SearchSnapshot()
                    override fun save(snapshot: SearchSnapshot) {}
                }, scope, startService = {})
            }
            SeedSeekerTheme { SeedFinderApp(engine, controller, fakeLatestVersion = BuildConfig.VERSION_NAME) }
        }
        compose.onNodeWithContentDescription("Show requirements").performClick()
        compose.onNodeWithText("Scope").assertDoesNotExist()
        compose.onNodeWithText("Search settings").performScrollTo().performClick()
        compose.onNodeWithContentDescription("Floor 22").performScrollTo().performClick()
        compose.onNodeWithText("AutoTrinket").performScrollTo().performClick()
        compose.onNodeWithContentDescription("Back").performClick()
        compose.onNodeWithText("Required floors: 22", substring = true).assertIsDisplayed()
        screenshot("finder-entry")
        compose.runOnIdle {
            val saved = requireNotNull(PresetStorage(preferences).loadCurrentQuery())
            assertEquals(22, saved.maximumDepth)
            assertEquals(listOf(22), saved.floorRequirements.map { it.depth })
            assertFalse(saved.autoApplyTrinket)
        }
        compose.onNodeWithContentDescription("Settings").performClick()
        compose.onNodeWithText("App settings").assertIsDisplayed()
        compose.onNodeWithText("Search settings").performClick()
        compose.onNodeWithContentDescription("Floor 22").performScrollTo().assertIsOn()
        compose.runOnIdle { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.onNodeWithText("App settings").assertIsDisplayed()
        compose.onNodeWithContentDescription("Back").performClick()
        compose.onNodeWithText("Seed Seeker").assertIsDisplayed()
    }

    private fun screenshot(name: String) {
        System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
        val window = compose.activity.window
        val bitmap = Bitmap.createBitmap(window.decorView.width, window.decorView.height, Bitmap.Config.ARGB_8888)
        var result: Int? = null
        compose.runOnIdle {
            PixelCopy.request(window, bitmap, { result = it }, Handler(Looper.getMainLooper()))
        }
        compose.waitUntil { result != null }
        assertEquals(PixelCopy.SUCCESS, result)
        val output = File("build/outputs/search-settings-ui/$name.png")
        output.parentFile?.mkdirs()
        output.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }
}
