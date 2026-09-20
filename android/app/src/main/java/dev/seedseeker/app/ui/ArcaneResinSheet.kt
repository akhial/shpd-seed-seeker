// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.*
import kotlin.math.roundToInt

internal val arcaneResinItem = CatalogItem("arcane_resin", "Arcane Resin", ItemKind.WAND, 317)

internal fun resinFilterDescription(filter: ArcaneResinFilter): String = listOfNotNull(
    if (filter.uncursed) "uncursed wands" else "any wands",
    filter.maximumDepth?.let { "≤ floor $it" }, filter.source?.label,
).joinToString(" · ")

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ArcaneResinSheet(
    amount: Int,
    filter: ArcaneResinFilter,
    auto: Boolean = false,
    onDismiss: () -> Unit,
    onSave: (Int, ArcaneResinFilter, Boolean) -> Unit,
    onRemove: () -> Unit,
) {
    var automatic by remember { mutableStateOf(auto) }
    var minimum by remember { mutableStateOf((amount.takeIf { it > 0 } ?: 2).toString()) }
    var uncursed by remember { mutableStateOf(filter.uncursed) }
    var depth by remember { mutableStateOf(filter.maximumDepth) }
    var source by remember { mutableStateOf(filter.source) }
    var sourceExpanded by remember { mutableStateOf(false) }
    val parsed = minimum.toIntOrNull()?.takeIf { it in 1..65535 }
    ModalBottomSheet(onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        sheetGesturesEnabled = false,
        dragHandle = null,
        containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())
            .navigationBarsPadding().padding(start = 20.dp, top = 12.dp, end = 20.dp, bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ItemSprite(arcaneResinItem, modifier = Modifier.size(40.dp))
                Text("Arcane Resin", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f).padding(start = 12.dp))
                TextButton(onClick = onDismiss) { Text("Close") }
            }
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                listOf("Amount", "Auto").forEachIndexed { index, label ->
                    SegmentedButton(selected = automatic == (index == 1), onClick = { automatic = index == 1 },
                        shape = SegmentedButtonDefaults.itemShape(index, 2)) { Text(label) }
                }
            }
            if (automatic) Text("Find enough resin to upgrade every matched wand to +3.")
            else OutlinedTextField(value = minimum, onValueChange = { minimum = it }, label = { Text("Minimum resin") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true,
                isError = parsed == null,
                supportingText = if (parsed == null) { { Text("Enter a whole number.") } } else null)
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Require uncursed wands", modifier = Modifier.weight(1f))
                Switch(checked = uncursed, onCheckedChange = { uncursed = it })
            }
            Text(depth?.let { "Wands within floor $it" } ?: "Wands within search limit")
            Slider(value = (depth?.let { floorLimitIndex(it) + 1 } ?: 0).toFloat(),
                onValueChange = { val index = it.roundToInt(); depth = if (index == 0) null else FLOOR_LIMIT_OPTIONS[index - 1] },
                valueRange = 0f..FLOOR_LIMIT_OPTIONS.size.toFloat(), steps = FLOOR_LIMIT_OPTIONS.size - 1)
            ExposedDropdownMenuBox(expanded = sourceExpanded, onExpandedChange = { sourceExpanded = it }) {
                OutlinedTextField(value = source?.label ?: "Any source", onValueChange = {}, readOnly = true,
                    label = { Text("Wand source") }, modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = sourceExpanded) })
                ExposedDropdownMenu(expanded = sourceExpanded, onDismissRequest = { sourceExpanded = false }) {
                    (listOf(null) + ScoutItemSource.entries).forEach { entry ->
                        DropdownMenuItem(text = { Text(entry?.label ?: "Any source") }, onClick = { source = entry; sourceExpanded = false })
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                if (auto || amount > 0) OutlinedButton(onClick = onRemove) { Text("Remove") }
                Button(onClick = { if (automatic || parsed != null) onSave(if (automatic) 0 else parsed!!, ArcaneResinFilter(uncursed, depth, source), automatic) },
                    enabled = automatic || parsed != null, modifier = Modifier.weight(1f)) { Text(if (auto || amount > 0) "Save" else "Add") }
            }
        }
    }
}
