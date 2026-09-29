// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import dev.seedseeker.app.BuildConfig
import dev.seedseeker.app.engine.DemoNativeSeedFinder
import dev.seedseeker.app.model.*
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w360dp-h800dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class SharedLinkUiTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private lateinit var controller: SearchController
    private val preferences get() = compose.activity.getSharedPreferences("seed_seeker_settings", Context.MODE_PRIVATE)

    /**
     * A cold start opened by a share link. The saved search loads on an
     * unconfined dispatcher, so it finishes after the first composition but
     * before that composition's effects run: the window in which the link
     * used to report a running search and the saved query could win.
     */
    @Test fun aLinkOpeningTheAppLoadsWithoutClaimingASearchIsRunning() {
        preferences.edit().clear().commit()
        val saved = PresetQuery(
            requirements = listOf(ItemRequirement(1, null, 0, kind = ItemKind.RING, upgradeMatch = UpgradeMatch.ANY)),
            maximumDepth = 12,
        )
        val shared = PresetQuery(
            requirements = listOf(ItemRequirement(1, null, 3, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.AT_LEAST)),
        )
        val link = DeepLink.encodeLink(shared)
        compose.setContent {
            val scope = rememberCoroutineScope()
            val engine = remember { DemoNativeSeedFinder() }
            controller = remember {
                SearchController(engine, object : SearchCheckpointStore {
                    override fun load() = SearchSnapshot(query = saved)
                    override fun save(snapshot: SearchSnapshot) {}
                }, scope, startService = {}, ioDispatcher = Dispatchers.Unconfined)
            }
            SeedSeekerTheme {
                SeedFinderApp(engine, controller, fakeLatestVersion = BuildConfig.VERSION_NAME, sharedLink = SharedLink(link))
            }
        }
        compose.waitUntil { ::controller.isInitialized && controller.ready }
        compose.waitUntil(timeoutMillis = 5_000) {
            PresetStorage(preferences).loadCurrentQuery()?.maximumDepth == shared.maximumDepth
        }
        compose.waitForIdle()
        compose.onAllNodesWithText("Stop the search", substring = true).assertCountEquals(0)
        val current = requireNotNull(PresetStorage(preferences).loadCurrentQuery())
        assertEquals(
            shared.requirements.map { it.copy(key = 0) },
            current.requirements.map { it.copy(key = 0) },
        )
    }
}
