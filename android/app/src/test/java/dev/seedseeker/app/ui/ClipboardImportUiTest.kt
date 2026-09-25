// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.activity.ComponentActivity
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import dev.seedseeker.app.BuildConfig
import dev.seedseeker.app.engine.DemoNativeSeedFinder
import dev.seedseeker.app.engine.EngineInfo
import dev.seedseeker.app.engine.JniNativeSeedFinder
import dev.seedseeker.app.model.*
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w360dp-h800dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ClipboardImportUiTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private lateinit var controller: SearchController
    private val clipboard get() = compose.activity.getSystemService(ClipboardManager::class.java)
    private val preferences get() = compose.activity.getSharedPreferences("seed_seeker_settings", Context.MODE_PRIVATE)

    private fun show() {
        preferences.edit().clear().commit()
        compose.setContent {
            val scope = rememberCoroutineScope()
            val engine = remember { DemoNativeSeedFinder() }
            controller = remember {
                SearchController(engine, object : SearchCheckpointStore {
                    override fun load() = SearchSnapshot()
                    override fun save(snapshot: SearchSnapshot) {}
                }, scope, startService = {})
            }
            SeedSeekerTheme { SeedFinderApp(engine, controller, fakeLatestVersion = BuildConfig.VERSION_NAME) }
        }
        compose.waitUntil { ::controller.isInitialized && controller.ready }
    }

    private fun importClipboard() {
        compose.onNodeWithContentDescription("More options").performClick()
        compose.onNodeWithText("Import from clipboard").performClick()
    }

    private fun waitForText(text: String) {
        compose.waitUntil(timeoutMillis = 5_000) {
            compose.onAllNodesWithText(text, substring = true).fetchSemanticsNodes().isNotEmpty()
        }
    }

    @Test fun clipboardRestoresQueryAndTrinketsAndMergesTheSeedPool() {
        show()
        val query = PresetQuery(
            requirements = listOf(ItemRequirement(1, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY)),
            maximumDepth = 12,
            challenges = Challenge.NO_HERBALISM.bit,
            floorRequirements = listOf(FloorRequirement(3, FloorFeeling.GRASS)),
        )
        val trinket = JniNativeSeedFinder().scoutSeed("AAA-AAA-BUH").trinketOrder.first().id
        val json = ResultsExport.encode(query, listOf("AAA-AAA-BUH"), "test", listOf(trinket))
        compose.runOnIdle { clipboard.setPrimaryClip(ClipData.newPlainText("Results", json)) }
        importClipboard()
        waitForText("Imported 1 seed from clipboard")
        compose.runOnIdle {
            assertEquals(query, controller.snapshot.query)
            assertEquals(trinket, controller.snapshot.results.single().selectedTrinket)
            val saved = requireNotNull(PresetStorage(preferences).loadCurrentQuery())
            assertEquals(query.copy(requirements = saved.requirements), saved)
            assertEquals(query.requirements.map { it.copy(key = 0) }, saved.requirements.map { it.copy(key = 0) })
            clipboard.setPrimaryClip(ClipData.newPlainText("Results",
                ResultsExport.encode(query, listOf("ABC-DEF-GHI"), "test")))
        }
        importClipboard()
        compose.waitUntil { controller.snapshot.results.map { it.seed } == listOf("ABC-DEF-GHI") }
        compose.runOnIdle {
            assertEquals(setOf("AAA-AAA-BUH", "ABC-DEF-GHI"), controller.snapshot.target!!.results.map { it.seed }.toSet())
        }
    }

    @Test fun invalidOrOversizedJsonLeavesTheCurrentQueryAndResultsUntouched() {
        show()
        val query = PresetQuery(listOf(ItemRequirement(1, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY)))
        compose.runOnIdle {
            clipboard.setPrimaryClip(ClipData.newPlainText("Results",
                ResultsExport.encode(query, listOf("AAA-AAA-BUH"), "test")))
        }
        importClipboard()
        waitForText("Imported 1 seed from clipboard")
        val before = compose.runOnIdle { controller.snapshot }
        val draft = compose.runOnIdle { PresetStorage(preferences).loadCurrentQuery() }
        for (text in listOf("{broken", " ".repeat(EngineInfo.resultsFileMaxBytes + 1) + "{}")) {
            compose.runOnIdle { clipboard.setPrimaryClip(ClipData.newPlainText("Results", text)) }
            importClipboard()
            waitForText("Results transfer")
            compose.runOnIdle {
                assertEquals(before, controller.snapshot)
                assertEquals(draft, PresetStorage(preferences).loadCurrentQuery())
            }
            compose.onNodeWithText("OK").performClick()
        }
    }

    @Test fun emptyAndNonTextClipboardsExplainHowToImport() {
        show()
        for (clip in listOf(null, ClipData.newPlainText("Empty", " \n"), ClipData.newIntent("Intent", Intent("test")))) {
            compose.runOnIdle {
                if (clip == null) clipboard.clearPrimaryClip() else clipboard.setPrimaryClip(clip)
            }
            importClipboard()
            waitForText("The clipboard has no text. Copy results JSON and try again.")
            compose.runOnIdle { assertTrue(controller.snapshot.results.isEmpty()) }
            compose.onNodeWithText("OK").performClick()
        }
    }

    @Test fun runningSearchDisablesBothImportActions() {
        show()
        compose.runOnIdle {
            controller.start(SearchRequest(listOf(ItemRequirement(1, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY))), workers = 1)
        }
        compose.onNodeWithContentDescription("More options").performClick()
        compose.onNodeWithText("Import results…").assertIsNotEnabled()
        compose.onNodeWithText("Import from clipboard").assertIsNotEnabled()
    }
}
