// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.remember
import dev.seedseeker.app.engine.NativeSeedFinderFactory
import dev.seedseeker.app.ui.SeedFinderApp
import dev.seedseeker.app.ui.theme.SeedSeekerTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            SeedSeekerTheme {
                val engine = remember { NativeSeedFinderFactory.create() }
                SeedFinderApp(engine)
            }
        }
    }
}
