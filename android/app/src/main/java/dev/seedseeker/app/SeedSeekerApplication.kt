// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app

import android.app.Application
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.engine.EngineInfo
import dev.seedseeker.app.engine.NativeSeedFinderFactory
import dev.seedseeker.app.ui.FileSearchCheckpointStore
import dev.seedseeker.app.ui.SearchController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import java.io.File

open class SeedSeekerApplication : Application() {
    internal val engine by lazy { NativeSeedFinderFactory.create() }
    internal open val searchController by lazy {
        SearchController(
            engine,
            FileSearchCheckpointStore(File(noBackupFilesDir, "search-session.json"),
                "${BuildConfig.VERSION_CODE}:${EngineInfo.shpdCommit}:${BuildConfig.USE_DEMO_ENGINE}"),
            CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
            startService = { SearchService.start(this) },
        )
    }

    override fun onCreate() {
        super.onCreate()
        // The item catalog is parsed from the packaged asset. Binding it here,
        // rather than in an Activity, means every entry point into the process
        // (launcher, App Link, anything added later) finds it already in place.
        ItemCatalog.install { path -> assets.open(path) }
    }
}
