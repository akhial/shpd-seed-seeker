// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.spring
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
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
                Text("Tap an unidentified item to reveal it",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        },
        text = {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).testTag("seed-mappings"),
                verticalArrangement = Arrangement.spacedBy(12.dp)) {
                // The reveal: what this seed's unknown appearance really is,
                // popping in on a bright card and swapping as you tap around.
                AnimatedVisibility(
                    visible = selected != null,
                    enter = expandVertically(spring(dampingRatio = 0.7f, stiffness = 420f)) + fadeIn(),
                    exit = shrinkVertically() + fadeOut(),
                ) {
                    var shown by remember { mutableStateOf(selected.orEmpty()) }
                    selected?.let { shown = it }
                    Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.tertiaryContainer,
                        modifier = Modifier.fillMaxWidth()) {
                        AnimatedContent(
                            targetState = shown,
                            transitionSpec = {
                                (slideInVertically { it / 2 } + fadeIn()).togetherWith(slideOutVertically { -it / 2 } + fadeOut())
                            },
                            label = "mapping-reveal",
                        ) { label ->
                            Text(label, modifier = Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 10.dp)
                                .then(if (label == selected) Modifier.testTag("mapping-detail") else Modifier),
                                style = MaterialTheme.typography.bodyLarge,
                                fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onTertiaryContainer)
                        }
                    }
                }
                ItemMappingGrid(mappings, selected, onSelect = { selected = if (selected == it) null else it })
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } },
    )
}
