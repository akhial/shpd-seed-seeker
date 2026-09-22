// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.R

/** Appearance and Android preferences; query controls live in Search settings. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    compactChips: Boolean,
    onCompactChipsChange: (Boolean) -> Unit,
    onSearchSettings: () -> Unit,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("App settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
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
                    .widthIn(max = 680.dp)
                    .navigationBarsPadding(),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                item {
                    SearchSettingsLink("Floors, quests, challenges and workers", onClick = onSearchSettings)
                }
                item {
                    SectionHeading("Appearance")
                }
                item {
                    SettingCard(
                        title = "Compact chips",
                        supporting = "Shorter chips with smaller sprites and text, " +
                            "so more of the board fits on a line.",
                        checked = compactChips,
                        enabled = true,
                        onCheckedChange = onCompactChipsChange,
                    )
                }
                item {
                    SectionHeading("Background search", modifier = Modifier.padding(top = 14.dp))
                    Text(
                        "Search continues with the screen locked. If battery optimization pauses it, " +
                            "allow unrestricted battery use for Seed Seeker in Android settings. " +
                            "This uses more battery. Interrupted searches continue when you reopen the app.",
                        modifier = Modifier.padding(top = 8.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                item {
                    OutlinedCard(
                        onClick = {
                            runCatching {
                                context.startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
                            }.onFailure {
                                context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                                    Uri.parse("package:${context.packageName}")))
                            }
                        },
                        modifier = Modifier.fillMaxWidth(),
                        shape = MaterialTheme.shapes.medium,
                        colors = CardDefaults.outlinedCardColors(
                            containerColor = MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.35f),
                            contentColor = MaterialTheme.colorScheme.secondary,
                        ),
                        border = BorderStroke(1.dp, MaterialTheme.colorScheme.secondary.copy(alpha = 0.4f)),
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 18.dp, vertical = 14.dp),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                "Battery settings",
                                modifier = Modifier.weight(1f),
                                style = MaterialTheme.typography.bodyLarge,
                                textDecoration = TextDecoration.Underline,
                            )
                            Icon(
                                painterResource(R.drawable.ic_open_in_new),
                                contentDescription = "Opens Android settings",
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                }

            }
        }
    }
}

@Composable
private fun SectionHeading(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier = modifier.padding(horizontal = 2.dp),
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.primary,
    )
}

/** One toggled setting: its name, a line on what it does, and the switch. */
@Composable
private fun SettingCard(
    title: String,
    supporting: String,
    checked: Boolean,
    enabled: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled) { onCheckedChange(!checked) },
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        ),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 18.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.bodyLarge)
                Text(
                    supporting,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Switch(checked = checked, onCheckedChange = onCheckedChange, enabled = enabled)
        }
    }
}
