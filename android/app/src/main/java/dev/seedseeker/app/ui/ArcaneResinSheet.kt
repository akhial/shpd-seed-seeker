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
 * wands' filters, their words and their checks are the editor's, as in
 * [RequirementSheet]. Save hands [onSaved] the condition to adopt. A sheet
 * opened on the query's resin edits it, and [onRemove] takes it away; on a
 * query without resin the editor opens a new one, which Add stores.
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
                Text(form.title, Modifier.weight(1f).padding(start = 12.dp), style = MaterialTheme.typography.titleLarge)
                TextButton(onClick = onDismiss) { Text("Close") }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
                resin.modes.forEachIndexed { index, mode ->
                    ToggleButton(
                        checked = resin.auto == mode.value,
                        onCheckedChange = { edited.change(SheetChange.resinAuto(mode.value)) },
                        shapes = when (index) {
                            0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                            resin.modes.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                            else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                        },
                        colors = ToggleButtonDefaults.toggleButtonColors(
                            containerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                        ),
                        modifier = Modifier.weight(1f).semantics {
                            role = Role.RadioButton
                            selected = resin.auto == mode.value
                        },
                    ) { Text(mode.label) }
                }
            }
            // What Auto means stands in the amount field's place.
            if (resin.auto) Text(resin.caption)
            else OutlinedTextField(value = typed,
                onValueChange = {
                    typed = it
                    edited.change(SheetChange.resinAmount(typedAmount(it)))
                },
                label = { Text(resin.label) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true,
                // The resin section's one error is its amount's.
                isError = form.errors.isNotEmpty(),
                supportingText = form.errors.firstOrNull()?.let { error -> { Text(error) } })
            if (form.uncursed.visible) ResinSwitch(form.uncursed) { edited.change(SheetChange.uncursed(it)) }
            if (form.floorLimit.visible) Column {
                FloorLimit(
                    form.floorLimit,
                    onEnabled = { edited.change(SheetChange.floorLimitEnabled(it)) },
                    onFloor = { edited.change(SheetChange.floorLimit(it)) },
                    style = LocalTextStyle.current,
                )
            }
            if (form.source.visible) SourcePicker(form.source, "Wand source") { edited.change(SheetChange.source(it)) }
            if (resin.includeMageWand.visible) ResinSwitch(resin.includeMageWand) { edited.change(SheetChange.includeMageWand(it)) }
            edited.notice?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                if (!form.adding && onRemove != null) OutlinedButton(
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
                    Text(if (form.adding) "Add" else "Save", style = MaterialTheme.typography.titleMedium)
                }
            }
        }
    }
}

/** A switch of the resin section: its label and help text beside it, the switch named by the label. */
@Composable
private fun ResinSwitch(toggle: SheetToggle, onChange: (Boolean) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(toggle.label)
            toggle.caption?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
        }
        Switch(checked = toggle.value, onCheckedChange = onChange,
            modifier = Modifier.semantics { contentDescription = toggle.label })
    }
}

/** A typed amount as the field shows it: a whole number without its ".0". */
private fun amountText(amount: Double): String =
    if (amount % 1.0 == 0.0) amount.toLong().toString() else amount.toString()

/**
 * The number the amount field holds, for the editor to judge, or null when
 * it holds none. Only plain decimal digits count: Kotlin's own parser would
 * also read `3d`, `1e3`, `0x1p2` or `NaN` as numbers the field never meant.
 */
internal fun typedAmount(text: String): Double? {
    val trimmed = text.trim()
    return trimmed.toLongOrNull()?.toDouble() ?: trimmed.takeIf(PLAIN_DECIMAL::matches)?.toDouble()
}

private val PLAIN_DECIMAL = Regex("[+-]?([0-9]+\\.[0-9]*|\\.[0-9]+)")
