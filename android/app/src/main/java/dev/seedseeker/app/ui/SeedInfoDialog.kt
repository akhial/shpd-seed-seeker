// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Info
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.ui.Alignment
import androidx.compose.ui.text.font.FontWeight
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
        icon = {
            ShapeBackdrop(SeekerShapes.Seed, MaterialTheme.colorScheme.tertiaryContainer, Modifier.size(52.dp)) {
                Icon(Icons.Filled.Info, contentDescription = null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
            }
        },
        title = {
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Seed information")
                Text(seed, fontFamily = FontFamily.Monospace,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.tertiary)
            }
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                // The reveal card is always there, pinned above the grid: it holds
                // a hint until an item is picked, so choosing, changing or clearing
                // a pick never moves the tiles you are tapping.
                val container by animateColorAsState(
                    if (selected != null) MaterialTheme.colorScheme.tertiaryContainer
                    else MaterialTheme.colorScheme.surfaceContainerHighest,
                    label = "reveal-container",
                )
                Surface(shape = MaterialTheme.shapes.large, color = container, modifier = Modifier.fillMaxWidth()) {
                    AnimatedContent(
                        targetState = selected,
                        transitionSpec = { fadeIn(tween(160)).togetherWith(fadeOut(tween(100))) },
                        label = "mapping-reveal",
                    ) { label ->
                        if (label == null) {
                            Text("Tap an unidentified item to reveal it",
                                modifier = Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 10.dp),
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurfaceVariant)
                        } else {
                            Text(label, modifier = Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 10.dp)
                                .then(if (label == selected) Modifier.testTag("mapping-detail") else Modifier),
                                style = MaterialTheme.typography.bodyLarge,
                                fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onTertiaryContainer)
                        }
                    }
                }
                ItemMappingGrid(mappings, selected, onSelect = { selected = if (selected == it) null else it },
                    modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).testTag("seed-mappings"))
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } },
    )
}
