// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import dev.seedseeker.app.BuildConfig
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.DemoNativeSeedFinder
import dev.seedseeker.app.model.*
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The app's own wiring around the shared requirement editor: the sheets it
 * opens from the board, what it adopts from their saves and the board's
 * edits, and how Share is gated. The editor's rules are the core's; these
 * run them through the real engine inside the whole app.
 */
@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w412dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class RequirementEditingAppTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    init { PackagedCatalog.install() }

    private lateinit var controller: SearchController
    private val preferences get() = compose.activity.getSharedPreferences("seed_seeker_settings", Context.MODE_PRIVATE)

    /** The query the app keeps as its draft, which follows every adopted edit. */
    private val draft get() = requireNotNull(PresetStorage(preferences).loadCurrentQuery())

    private fun show(query: PresetQuery) {
        preferences.edit().clear().commit()
        PresetStorage(preferences).saveCurrentQuery(query)
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

    private fun items() = draft.requirements.map { it.item?.id }

    @Test fun aChipIsAddedResavedAndRemovedThroughTheSheet() {
        show(PresetQuery(listOf(ItemRequirement(1, ItemCatalog.findById("wand_frost")!!, 2))))
        compose.onNodeWithContentDescription("Show requirements").performClick()
        compose.onNodeWithContentDescription("Add requirement").performClick()
        compose.onNodeWithText("Add requirement").assertIsDisplayed()
        compose.onNodeWithText("Ring").performClick()
        compose.onNodeWithText("Ring of Might").performClick()
        compose.onNodeWithText("Next").performClick()
        // The sheet's Add, not the board's "+ Add" chip behind it.
        compose.onNode(hasText("Add") and !hasContentDescription("Add requirement")).performClick()
        compose.onNodeWithText("Add requirement").assertDoesNotExist()
        compose.runOnIdle { assertEquals(listOf("wand_frost", "ring_might"), items()) }

        // Saved as it is, the chip changes nothing.
        val before = draft
        compose.onNodeWithContentDescription("Ring of Might", substring = true).performClick()
        compose.onNodeWithText("Edit requirement").assertIsDisplayed()
        compose.onNodeWithText("Save").performClick()
        compose.onNodeWithText("Edit requirement").assertDoesNotExist()
        compose.runOnIdle { assertEquals(before, draft) }

        compose.onNodeWithContentDescription("Ring of Might", substring = true).performClick()
        compose.onNodeWithContentDescription("Remove requirement").performClick()
        compose.onNodeWithText("Edit requirement").assertDoesNotExist()
        compose.runOnIdle { assertEquals(listOf("wand_frost"), items()) }
    }

    private val blanketOnly = listOf(
        ItemRequirement(1, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY, blanket = true),
    )

    /** Shares the query, and finds [message] in the dialog that refuses it. */
    private fun shareIsRefusedWith(message: String) {
        compose.onNodeWithContentDescription("More options").performClick()
        compose.onNodeWithText("Share search…").performClick()
        compose.onNodeWithText("Shared search").assertIsDisplayed()
        compose.onNode(hasText(message) and hasAnyAncestor(isDialog())).assertIsDisplayed()
    }

    @Test fun shareIsRefusedWithTheListsFirstProblem() {
        show(PresetQuery(blanketOnly))
        shareIsRefusedWith("Add at least one ordinary requirement.")
    }

    @Test fun shareChecksTheQueryItselfBeforeTheList() {
        show(PresetQuery(blanketOnly, maximumDepth = 8, floorRequirements = listOf(FloorRequirement(12, FloorFeeling.GRASS))))
        shareIsRefusedWith("Floor 12 exceeds the floor limit of 8.")
    }
}
