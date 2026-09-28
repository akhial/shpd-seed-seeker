// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import dev.seedseeker.app.ui.theme.SpdSecret
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.*

internal val arcaneResinItem = CatalogItem("arcane_resin", "Arcane Resin", ItemKind.WAND, 317)

/**
 * The query's Arcane Resin condition, on the requirement editor's resin
 * section: [sheet] is opened on the query's resin, and the amount, the donor
 * wands' filters and their checks are the editor's, as in [RequirementSheet].
 * Save hands [onSaved] the condition to adopt; [onRemove], given while the
 * query asks for resin, takes it away, and without it the sheet adds one.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ArcaneResinSheet(
    sheet: EditorSheet,
    rows: List<ItemRequirement>,
    nextKey: Long? = null,
    onDismiss: () -> Unit,
    onSaved: (SheetSave.Saved) -> Unit,
    onRemove: (() -> Unit)? = null,
) {
    val edited = rememberEditedSheet(sheet)
    val form = edited.sheet.form
    val resin = form.resin
    // The amount as typed; the draft holds the number it reads as.
    var typed by rememberSaveable(sheet.draft) { mutableStateOf(resin.amount?.let(::amountText).orEmpty()) }
    ModalBottomSheet(onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        sheetGesturesEnabled = false,
        dragHandle = null,
        containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())
            .navigationBarsPadding().padding(start = 20.dp, top = 12.dp, end = 20.dp, bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                // The resin sits on a violet sunburst.
                ShapeBackdrop(
                    MaterialShapes.Sunny,
                    SpdSecret.copy(alpha = 0.22f),
                    Modifier.size(52.dp),
                ) {
                    ItemSprite(arcaneResinItem, modifier = Modifier.size(36.dp).popOnChange(resin.auto))
                }
                Column(Modifier.weight(1f).padding(start = 12.dp)) {
                    Text(form.title, style = MaterialTheme.typography.titleLarge)
                    Text(
                        if (resin.auto) "Enough to take every kept wand to +3" else "A fixed amount, at least",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(onClick = onDismiss) { Text("Close") }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
                listOf("Amount", "Auto").forEachIndexed { index, label ->
                    ToggleButton(
                        checked = resin.auto == (index == 1),
                        onCheckedChange = { edited.change(SheetChange.resinAuto(index == 1)) },
                        shapes = if (index == 0) ButtonGroupDefaults.connectedLeadingButtonShapes()
                        else ButtonGroupDefaults.connectedTrailingButtonShapes(),
                        colors = ToggleButtonDefaults.toggleButtonColors(
                            containerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                        ),
                        modifier = Modifier.weight(1f).semantics {
                            role = Role.RadioButton
                            selected = resin.auto == (index == 1)
                        },
                    ) { Text(label) }
                }
            }
            if (resin.auto) Text("Upgrade each kept wand to +3. Excluded wands and extra copies reserved for reforging need no resin.")
            else OutlinedTextField(value = typed,
                onValueChange = {
                    typed = it
                    edited.change(SheetChange.resinAmount(it.trim().toDoubleOrNull()))
                },
                label = { Text("Minimum resin") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true,
                // The resin section's one error is its amount's.
                isError = form.errors.isNotEmpty(),
                supportingText = form.errors.firstOrNull()?.let { error -> { Text(error) } })
            if (form.uncursed.visible) Row(verticalAlignment = Alignment.CenterVertically) {
                Text(form.uncursed.label, modifier = Modifier.weight(1f))
                Switch(checked = form.uncursed.value, onCheckedChange = { edited.change(SheetChange.uncursed(it)) })
            }
            if (form.floorLimit.visible) Column {
                FloorLimit(
                    form.floorLimit,
                    onEnabled = { edited.change(SheetChange.floorLimitEnabled(it)) },
                    onFloor = { edited.change(SheetChange.floorLimit(it)) },
                    style = LocalTextStyle.current,
                )
            }
            if (form.source.visible) SourcePicker(form.source, "Wand source") { edited.change(SheetChange.source(it)) }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("Include Mage’s starting wand")
                    Text("Adds 2 resin from Magic Missile. Assumes you recover it with Wand Preservation and dismantle it after imbuing.",
                        style = MaterialTheme.typography.bodySmall)
                }
                Switch(checked = resin.includeMageWand, onCheckedChange = { edited.change(SheetChange.includeMageWand(it)) },
                    modifier = Modifier.semantics { contentDescription = "Include Mage’s starting wand" })
            }
            edited.notice?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                if (onRemove != null) OutlinedButton(
                    onClick = onRemove,
                    shapes = ButtonDefaults.shapes(),
                    modifier = Modifier.height(52.dp),
                    colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
                ) { Text("Remove") }
                val saveInteraction = remember { MutableInteractionSource() }
                Button(onClick = { edited.save(rows, nextKey, onSaved) },
                    enabled = form.canSave,
                    shapes = ButtonDefaults.shapes(),
                    interactionSource = saveInteraction,
                    modifier = Modifier.weight(1f).height(52.dp).pressScale(saveInteraction, pressed = 0.95f)) {
                    Text(if (onRemove != null) "Save" else "Add", style = MaterialTheme.typography.titleMedium)
                }
            }
        }
    }
}

/** A typed amount as the field shows it: a whole number without its ".0". */
private fun amountText(amount: Double): String =
    if (amount % 1.0 == 0.0) amount.toLong().toString() else amount.toString()
