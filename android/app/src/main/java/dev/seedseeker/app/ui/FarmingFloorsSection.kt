// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.FARMING_FLOORS
import dev.seedseeker.app.model.FloorRequirement

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
        MultiChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
            FARMING_FLOORS.forEachIndexed { index, depth ->
                SegmentedButton(
                    checked = floors.any { it.depth == depth && it.isFarming },
                    onCheckedChange = { onToggle(depth) },
                    enabled = enabled,
                    shape = SegmentedButtonDefaults.itemShape(index, FARMING_FLOORS.size),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = MaterialTheme.colorScheme.primary,
                        activeContentColor = MaterialTheme.colorScheme.onPrimary,
                        activeBorderColor = MaterialTheme.colorScheme.primary,
                        inactiveContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    ),
                    modifier = Modifier.heightIn(min = 48.dp).semantics { contentDescription = "Floor $depth" },
                ) { Text("$depth", style = MaterialTheme.typography.titleMedium) }
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
