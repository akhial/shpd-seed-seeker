// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.ScoutItemMappings

@Composable
internal fun SeedInfoDialog(seed: String, mappings: ScoutItemMappings, onDismiss: () -> Unit) {
    var selected by remember(seed) { mutableStateOf<String?>(null) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Seed information")
                Text(seed, fontFamily = FontFamily.Monospace,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.tertiary)
            }
        },
        text = {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).testTag("seed-mappings"),
                verticalArrangement = Arrangement.spacedBy(12.dp)) {
                selected?.let {
                    Text(it, modifier = Modifier.testTag("mapping-detail"), style = MaterialTheme.typography.bodyMedium)
                }
                ItemMappingGrid(mappings, selected, onSelect = { selected = if (selected == it) null else it })
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } },
    )
}
