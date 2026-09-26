// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.FARMING_FLOORS
import dev.seedseeker.app.model.FloorRequirement

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun FarmingFloorsSection(
    floors: List<FloorRequirement>,
    enabled: Boolean,
    onToggle: (Int) -> Unit,
    onRemove: (Int) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("Ring of Wealth farming floors", style = MaterialTheme.typography.titleMedium)
            Text("Dark floor with a garden.", style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        // A connected button group: a chosen floor rounds out into a pill
        // and fills, its neighbours keep their squared shoulders.
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
            FARMING_FLOORS.forEachIndexed { index, depth ->
                val checked = floors.any { it.depth == depth && it.isFarming }
                ToggleButton(
                    checked = checked,
                    onCheckedChange = { onToggle(depth) },
                    enabled = enabled,
                    shapes = when (index) {
                        0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                        FARMING_FLOORS.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                        else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                    },
                    colors = ToggleButtonDefaults.toggleButtonColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                    ),
                    modifier = Modifier.weight(1f).heightIn(min = 52.dp).semantics { contentDescription = "Floor $depth" },
                ) {
                    AnimatedVisibility(checked) {
                        Row {
                            Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                        }
                    }
                    Text("$depth", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                }
            }
        }
        floors.filterNot { it.isFarming }.forEach { floor ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(floor.description, modifier = Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
                TextButton(onClick = { onRemove(floor.depth) }, enabled = enabled) { Text("Remove") }
            }
        }
    }
}
