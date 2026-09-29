// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.model.CatalogItem
import dev.seedseeker.app.model.EditorSheet
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.RequirementEditor
import dev.seedseeker.app.model.SheetChange
import dev.seedseeker.app.model.SheetEffect
import dev.seedseeker.app.model.SheetFloors
import dev.seedseeker.app.model.SheetForm
import dev.seedseeker.app.model.SheetOption
import dev.seedseeker.app.model.SheetPicker
import dev.seedseeker.app.model.SheetSave
import java.util.Locale
import kotlin.math.roundToInt

private enum class SheetStep { ITEM, DETAILS }

/** The tier slider's name under each mode that takes a value (dialog chrome). */
private val TIER_SLIDER = mapOf("exact" to "Exact tier", "at_least" to "Minimum tier", "at_most" to "Maximum tier")

/** The upgrade slider's name under each mode that takes a value (dialog chrome). */
private val UPGRADE_SLIDER = mapOf("exact" to "Level", "at_least" to "At least")

/**
 * The requirement editor. It draws the shared core's form for [sheet] —
 * which controls show, what they offer, their ranges, labels, section labels
 * and help texts, the chip a save would produce, and why the draft cannot be
 * saved — and sends every control the user moves back to the core as a
 * change; the draft is the core's and stays opaque here. The sheet's own are
 * its dialog chrome: its two steps, its titles, its buttons, the headings of
 * its pickers and mode pickers, and its sliders' names.
 *
 * Save stores the draft onto [rows], the list as it is now, a new row taking
 * its key from [nextKey] on. [onSaved] gets what the save stored — the list
 * and the chip it landed in — for the caller to adopt; a save the editor
 * refuses keeps the sheet open on its reasons.
 *
 * [onPickResin] is where the Arcane Resin choice leads when the sheet offers
 * it: the query's resin has a sheet of its own.
 *
 * [onRemove], given for an existing chip, takes it off the board from here: the
 * board's chips flow across lines, so a tap is the sure way to reach one, and
 * the editor a tap opens is where its removal lives besides the drop zone.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun RequirementSheet(
    sheet: EditorSheet,
    rows: List<ItemRequirement>,
    nextKey: Long? = null,
    onDismiss: () -> Unit,
    onSaved: (SheetSave.Saved) -> Unit,
    onRemove: (() -> Unit)? = null,
    onPickResin: (() -> Unit)? = null,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val edited = rememberEditedSheet(sheet)
    val form = edited.sheet.form
    var step by rememberSaveable(sheet.draft) {
        mutableStateOf(if (sheet.form.adding) SheetStep.ITEM else SheetStep.DETAILS)
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        sheetGesturesEnabled = false,
        dragHandle = null,
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .fillMaxHeight(0.94f)
                .navigationBarsPadding()
                .padding(bottom = 16.dp),
        ) {
            Row(
                modifier = Modifier.padding(start = 20.dp, top = 12.dp, end = 20.dp, bottom = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(
                        when {
                            form.blanket -> if (form.adding) "Add blanket requirement" else "Edit blanket requirement"
                            form.adding -> "Add requirement"
                            form.inCluster -> "Edit alternative"
                            else -> "Edit requirement"
                        },
                        style = MaterialTheme.typography.titleLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    StepIndicator(step)
                }
                TextButton(onClick = onDismiss) { Text("Close") }
            }

            // Steps slide past each other like pages: forward to the details,
            // back to the item picker.
            AnimatedContent(
                targetState = step,
                transitionSpec = {
                    val forward = targetState == SheetStep.DETAILS
                    val spatial = spring<IntOffset>(dampingRatio = 0.85f, stiffness = 420f)
                    (slideInHorizontally(spatial) { if (forward) it / 2 else -it / 2 } + fadeIn(tween(200)))
                        .togetherWith(slideOutHorizontally(spatial) { if (forward) -it / 2 else it / 2 } + fadeOut(tween(120)))
                },
                modifier = Modifier.weight(1f),
                label = "sheet-step",
            ) { shownStep ->
            Column(Modifier.fillMaxSize()) {
            when (shownStep) {
                SheetStep.ITEM -> ItemStep(form, edited::change, onPickResin, onNext = { step = SheetStep.DETAILS })

                SheetStep.DETAILS -> {
                    // Details — a single scrollable column.
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .verticalScroll(rememberScrollState())
                            .padding(horizontal = 20.dp),
                    ) {
                        DetailControls(form, edited::change)
                        Spacer(Modifier.height(14.dp))
                    }

                    Column(Modifier.padding(horizontal = 20.dp)) {
                        RequirementPreview(form, edited.notice)
                        Spacer(Modifier.height(10.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            if (!form.adding && onRemove != null) {
                                OutlinedButton(
                                    onClick = onRemove,
                                    modifier = Modifier.height(52.dp),
                                    shapes = ButtonDefaults.shapes(),
                                    colors = ButtonDefaults.outlinedButtonColors(
                                        contentColor = MaterialTheme.colorScheme.error,
                                    ),
                                    contentPadding = PaddingValues(horizontal = 14.dp),
                                ) {
                                    Icon(
                                        Icons.Filled.Delete,
                                        contentDescription = if (form.inCluster) {
                                            "Remove alternative"
                                        } else {
                                            "Remove requirement"
                                        },
                                        modifier = Modifier.size(20.dp),
                                    )
                                }
                            }
                            OutlinedButton(
                                onClick = { step = SheetStep.ITEM },
                                modifier = Modifier.height(52.dp),
                                shapes = ButtonDefaults.shapes(),
                            ) {
                                Text("Back")
                            }
                            val saveInteraction = remember { MutableInteractionSource() }
                            Button(
                                onClick = { edited.save(rows, nextKey, onSaved) },
                                enabled = form.canSave,
                                modifier = Modifier
                                    .weight(1f)
                                    .height(52.dp)
                                    .pressScale(saveInteraction, pressed = 0.95f)
                                    .shakeOnChange(form.errors.firstOrNull()),
                                shapes = ButtonDefaults.shapes(),
                                interactionSource = saveInteraction,
                            ) {
                                Icon(
                                    if (form.adding) Icons.Filled.Add else Icons.Filled.Check,
                                    contentDescription = null,
                                    modifier = Modifier.size(20.dp),
                                )
                                Spacer(Modifier.width(6.dp))
                                Text(
                                    if (form.adding) "Add" else "Save",
                                    style = MaterialTheme.typography.titleMedium,
                                )
                            }
                        }
                    }
                }
            }
            }
            }
        }
    }
}

/**
 * An open sheet as the user edits it: every control moved goes to the
 * requirement editor, and the sheet it answers replaces [sheet]. A save the
 * editor refuses comes back the same way, with its reasons in the form.
 */
@Stable
internal class EditedSheet(opened: EditorSheet) {
    var sheet by mutableStateOf(opened)
        private set

    /** Why the editor could not answer the last request; null once it does. */
    var notice by mutableStateOf<String?>(null)
        private set

    fun change(change: SheetChange) {
        runCatching { RequirementEditor.change(sheet.draft, change) }
            .onSuccess {
                sheet = it
                notice = null
            }
            .onFailure { notice = it.message ?: "The requirement editor could not answer." }
    }

    /** Saves onto [rows]; [onSaved] gets what was stored, while a refused draft stays open. */
    fun save(rows: List<ItemRequirement>, nextKey: Long?, onSaved: (SheetSave.Saved) -> Unit) {
        runCatching { RequirementEditor.save(sheet.draft, rows, nextKey) }
            .onSuccess { result ->
                notice = null
                when (result) {
                    is SheetSave.Saved -> onSaved(result)
                    is SheetSave.Refused -> sheet = result.sheet
                }
            }
            .onFailure { notice = it.message ?: "The requirement could not be saved." }
    }

    companion object {
        val Saver = listSaver<EditedSheet, String>(
            save = { listOf(it.sheet.draft, it.sheet.formJson) },
            restore = { (draft, form) -> EditedSheet(EditorSheet(draft, form)) },
        )
    }
}

/** [opened] as the user edits it, kept in saved state as the editor's draft and form. */
@Composable
internal fun rememberEditedSheet(opened: EditorSheet): EditedSheet =
    rememberSaveable(opened.draft, saver = EditedSheet.Saver) { EditedSheet(opened) }

/** The item step: the category, the wildcard and weapon type, and the item grid. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ColumnScope.ItemStep(
    form: SheetForm,
    onChange: (SheetChange) -> Unit,
    onPickResin: (() -> Unit)?,
    onNext: () -> Unit,
) {
    // Category — connected toggle-button group (fixed chrome).
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        form.category.options.forEach { option ->
            ToggleButton(
                checked = option.value == form.category.value,
                onCheckedChange = { checked -> if (checked) onChange(SheetChange.category(option.value)) },
                colors = ToggleButtonDefaults.toggleButtonColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                ),
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 10.dp),
            ) {
                Text(option.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }

    // The wildcard and Arcane Resin lead the item picker's options; the
    // items themselves are the grid's.
    val (choices, named) = remember(form.item.options) {
        form.item.options.partition { it.value == null || it.value == SheetChange.ARCANE_RESIN }
    }
    val tiles = remember(named) {
        named.mapNotNull { option -> option.value?.let(ItemCatalog::findById)?.let { option to it } }
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        choices.forEach { option ->
            if (option.value == SheetChange.ARCANE_RESIN) {
                // The query's resin is edited on a sheet of its own.
                if (onPickResin != null) FilterChip(selected = false, onClick = onPickResin, label = { Text(option.label) })
            } else {
                FilterChip(
                    selected = form.item.value == null,
                    onClick = { onChange(SheetChange.item(null)) },
                    label = { Text(option.label, maxLines = 1, softWrap = false) },
                )
            }
        }
        if (form.weaponType.visible) {
            form.weaponType.options.forEach { option ->
                FilterChip(
                    selected = option.value == form.weaponType.value,
                    onClick = { onChange(SheetChange.weaponType(option.value)) },
                    label = { Text(option.label, maxLines = 1, softWrap = false) },
                )
            }
        }
    }

    // Item picker — the only scrollable region on this step.
    LazyVerticalGrid(
        columns = GridCells.Adaptive(92.dp),
        modifier = Modifier
            .fillMaxWidth()
            .weight(1f),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        items(tiles, key = { (_, item) -> item.id }) { (option, item) ->
            ItemTile(
                item = item,
                label = option.label,
                selected = option.value == form.item.value,
                onClick = { onChange(SheetChange.item(option.value)) },
            )
        }
    }

    val nextInteraction = remember { MutableInteractionSource() }
    Button(
        onClick = onNext,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp)
            .padding(top = 10.dp)
            .height(56.dp)
            .pressScale(nextInteraction, pressed = 0.95f),
        shapes = ButtonDefaults.shapes(),
        interactionSource = nextInteraction,
    ) {
        Text("Next", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.width(8.dp))
        Icon(Icons.AutoMirrored.Filled.ArrowForward, contentDescription = null, modifier = Modifier.size(20.dp))
    }
}

/** The details step's controls, each drawn only while the form shows it. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun DetailControls(form: SheetForm, onChange: (SheetChange) -> Unit) {
    val transmutations = form.transmutations
    if (transmutations.visible) {
        SwitchRow(transmutations.label, transmutations.enabled, { onChange(SheetChange.transmutationsEnabled(it)) })
        Spacer(Modifier.height(12.dp))
        if (transmutations.enabled) {
            // The stepper reads its own value ("At most 3") under the switch that names it.
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                Stepper(transmutations.value, transmutations.valueLabel, transmutations.min..transmutations.max) {
                    onChange(SheetChange.transmutations(it))
                }
            }
        }
        if (transmutations.captionVisible) HelpText(transmutations.caption)
    }
    if (form.selectTrinket.visible) {
        SwitchRow(form.selectTrinket.label, form.selectTrinket.value, { onChange(SheetChange.selectTrinket(it)) })
        Spacer(Modifier.height(8.dp))
        HelpText(form.selectTrinket.caption)
    }

    val tier = form.tier
    if (tier.visible) {
        Text("Tier", style = MaterialTheme.typography.titleSmall)
        Spacer(Modifier.height(8.dp))
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            tier.modes.forEach { mode ->
                FilterChip(
                    selected = tier.mode == mode.value,
                    onClick = { onChange(SheetChange.tierMode(mode.value)) },
                    label = { Text(mode.label) },
                )
            }
        }
        if (tier.valueVisible) {
            Column(Modifier.padding(vertical = 4.dp)) {
                ValueRow(TIER_SLIDER[tier.mode].orEmpty(), tier.valueLabel)
                StepSlider(tier.value, tier.min..tier.max, tier.valueLabel) { onChange(SheetChange.tier(it)) }
            }
        }
        Spacer(Modifier.height(18.dp))
    }

    val upgrade = form.upgrade
    if (upgrade.visible) {
        Text("Upgrade", style = MaterialTheme.typography.titleSmall)
        Spacer(Modifier.height(8.dp))
        ModeButtons(upgrade.modes, upgrade.mode) { onChange(SheetChange.upgradeMode(it)) }
        if (upgrade.valueVisible) {
            Spacer(Modifier.height(8.dp))
            Column {
                ValueRow(UPGRADE_SLIDER[upgrade.mode].orEmpty(), upgrade.valueLabel)
                StepSlider(upgrade.value, upgrade.min..upgrade.max, upgrade.valueLabel) { onChange(SheetChange.upgrade(it)) }
            }
        }
    }

    val effect = form.effect
    if (effect.visible) {
        Spacer(Modifier.height(18.dp))
        Text(effect.label, style = MaterialTheme.typography.titleSmall)
        Spacer(Modifier.height(8.dp))
        ModeButtons(effect.modes, effect.mode) { onChange(SheetChange.effectMode(it)) }
        if (effect.choicesVisible) {
            Spacer(Modifier.height(8.dp))
            effect.groups.forEachIndexed { index, group ->
                if (index > 0) Spacer(Modifier.height(6.dp))
                EffectGrid(
                    heading = group.label.uppercase(Locale.ROOT),
                    choices = effect.choices.filter { it.group == group.value },
                    headingColor = if (group.value == "curse") MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                    onToggle = { onChange(SheetChange.toggleEffect(it)) },
                )
            }
            HelpText(effect.caption)
        }
    }

    if (form.uncursed.visible) {
        Spacer(Modifier.height(10.dp))
        CheckRow(form.uncursed.label, form.uncursed.value) { onChange(SheetChange.uncursed(it)) }
        HelpText(form.uncursed.caption)
    }
    if (form.source.visible) {
        Spacer(Modifier.height(10.dp))
        SourcePicker(form.source, "Source", shape = MaterialTheme.shapes.medium) { onChange(SheetChange.source(it)) }
    }
    if (form.floorLimit.visible) {
        Spacer(Modifier.height(18.dp))
        FloorLimit(
            form.floorLimit,
            onEnabled = { onChange(SheetChange.floorLimitEnabled(it)) },
            onFloor = { onChange(SheetChange.floorLimit(it)) },
        )
    }

    if (form.excludeResin.visible) {
        CheckRow(form.excludeResin.label, form.excludeResin.value) { onChange(SheetChange.excludeResin(it)) }
        HelpText(form.excludeResin.caption)
    }

    val stack = form.stack
    if (stack.visible) {
        Spacer(Modifier.height(18.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(stack.label, style = MaterialTheme.typography.titleSmall)
            Spacer(Modifier.weight(1f))
            Stepper(stack.count, stack.valueLabel, stack.min..stack.max) { onChange(SheetChange.count(it)) }
        }
        if (stack.copyDepth.visible) {
            Spacer(Modifier.height(12.dp))
            FloorLimit(
                stack.copyDepth,
                onEnabled = { onChange(SheetChange.copyDepthEnabled(it)) },
                onFloor = { onChange(SheetChange.copyDepth(it)) },
            )
        }
        val levels = stack.countLevels
        if (levels.visible) {
            Spacer(Modifier.height(12.dp))
            SwitchRow(
                levels.label,
                levels.enabled,
                { onChange(SheetChange.countLevels(it)) },
                caption = levels.caption.takeIf { levels.captionVisible },
                style = MaterialTheme.typography.titleSmall,
            )
            if (levels.enabled) {
                // The whole reading ("≥ 5 across up to 2") under the switch that names it.
                Text(levels.valueLabel, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                StepSlider(levels.value, levels.min..levels.max, levels.valueLabel) { onChange(SheetChange.total(it)) }
            }
        }
    }
}

/** A connected row of mode buttons, one per [modes] entry, [selected] checked. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ModeButtons(modes: List<SheetOption<String>>, selected: String, onSelect: (String) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        modes.forEach { mode ->
            ToggleButton(
                checked = selected == mode.value,
                onCheckedChange = { checked -> if (checked) onSelect(mode.value) },
                modifier = Modifier.weight(1f),
                colors = ToggleButtonDefaults.toggleButtonColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                ),
                contentPadding = PaddingValues(horizontal = 4.dp, vertical = 10.dp),
            ) {
                Text(mode.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** A label and, at the far end, a value in the accent colour. */
@Composable
private fun ValueRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge)
        Text(value, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
    }
}

/** A whole row that flips a switch, with an optional [caption] under its label. */
@Composable
internal fun SwitchRow(
    label: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    caption: String? = null,
    style: TextStyle = LocalTextStyle.current,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(value = checked, role = Role.Switch, onValueChange = onChange),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = style)
            caption?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        Spacer(Modifier.width(12.dp))
        Switch(checked = checked, onCheckedChange = null)
    }
}

/** A whole row that ticks a check box. */
@Composable
private fun CheckRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(value = checked, role = Role.Checkbox, onValueChange = onChange),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = checked, onCheckedChange = null)
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

/** A control's help text, as the form words it; nothing for a control without one. */
@Composable
internal fun HelpText(text: String?) {
    text ?: return
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/**
 * A slider over [range] at [value]. A drag reports only the whole steps it
 * crosses, so the editor hears one change per step rather than one per
 * pointer move; a range of one value has nothing to slide.
 */
@Composable
private fun StepSlider(value: Int, range: IntRange, description: String, onChange: (Int) -> Unit) {
    if (range.last <= range.first) return
    Slider(
        value = value.toFloat(),
        onValueChange = { moved -> moved.roundToInt().coerceIn(range).takeIf { it != value }?.let(onChange) },
        valueRange = range.first.toFloat()..range.last.toFloat(),
        steps = range.last - range.first - 1,
        modifier = Modifier.semantics { stateDescription = description },
    )
}

/** A floor switch, its label in [style], and the slider over the floors the form offers. */
@Composable
internal fun FloorLimit(
    floors: SheetFloors,
    onEnabled: (Boolean) -> Unit,
    onFloor: (Int) -> Unit,
    style: TextStyle = MaterialTheme.typography.titleSmall,
) {
    SwitchRow(floors.label, floors.enabled, onEnabled, style = style)
    if (floors.enabled) {
        Spacer(Modifier.height(6.dp))
        Text(floors.valueLabel, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        val index = floors.options.indexOfFirst { it.value == floors.value }.coerceAtLeast(0)
        StepSlider(index, 0..floors.options.lastIndex, floors.valueLabel) { onFloor(floors.options[it].value) }
    }
}

/** A dropdown of the sources the form offers, under [label]. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SourcePicker(
    source: SheetPicker<String?>,
    label: String,
    shape: Shape = OutlinedTextFieldDefaults.shape,
    onChange: (String?) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { expanded = it },
    ) {
        OutlinedTextField(
            value = source.label.orEmpty(),
            onValueChange = { },
            readOnly = true,
            singleLine = true,
            shape = shape,
            label = { Text(label) },
            trailingIcon = {
                ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded)
            },
            modifier = Modifier
                .menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable, enabled = true)
                .fillMaxWidth(),
        )
        ExposedDropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
        ) {
            source.options.forEach { option ->
                DropdownMenuItem(
                    text = { Text(option.label) },
                    onClick = {
                        onChange(option.value)
                        expanded = false
                    },
                )
            }
        }
    }
}

/**
 * A compact −/+ stepper for the small bounded counts the board deals in,
 * showing [label] for [value]. The value rolls up or down like an odometer as
 * it changes.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun Stepper(
    value: Int,
    label: String,
    range: IntRange,
    onChange: (Int) -> Unit,
) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        FilledTonalIconButton(
            onClick = { onChange(value - 1) },
            enabled = value > range.first,
            shapes = IconButtonDefaults.shapes(),
        ) {
            Text("−", style = MaterialTheme.typography.titleLarge)
        }
        AnimatedContent(
            targetState = value to label,
            transitionSpec = {
                val up = targetState.first > initialState.first
                (slideInVertically { if (up) it else -it } + fadeIn())
                    .togetherWith(slideOutVertically { if (up) -it else it } + fadeOut())
                    .using(SizeTransform(clip = false))
            },
            label = "stepper-value",
        ) { (_, shown) ->
            Text(
                shown,
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.widthIn(min = 48.dp),
                textAlign = TextAlign.Center,
            )
        }
        FilledTonalIconButton(
            onClick = { onChange(value + 1) },
            enabled = value < range.last,
            shapes = IconButtonDefaults.shapes(),
        ) {
            Text("+", style = MaterialTheme.typography.titleLarge)
        }
    }
}

/** Which step of the editor is showing, as two pills — the current one stretched long. */
@Composable
private fun StepIndicator(step: SheetStep) {
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
        SheetStep.entries.forEach { entry ->
            val active = entry == step
            val width by animateDpAsState(
                if (active) 22.dp else 8.dp,
                MaterialTheme.motionScheme.fastSpatialSpec(),
                label = "step-width",
            )
            Box(
                Modifier
                    .height(8.dp)
                    .width(width)
                    .clip(CircleShape)
                    .background(if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant),
            )
        }
        Spacer(Modifier.width(8.dp))
        Text(
            if (step == SheetStep.ITEM) "Item" else "Details",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * Effects under a small heading, as chips. Each chip carries the colour the
 * effect makes an item glow in the game, pulsing when picked.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EffectGrid(
    heading: String,
    choices: List<SheetEffect>,
    headingColor: Color,
    onToggle: (String) -> Unit,
) {
    Text(
        heading,
        modifier = Modifier.padding(vertical = 4.dp),
        style = MaterialTheme.typography.labelSmall,
        letterSpacing = 1.sp,
        color = headingColor,
    )
    // Chips wrap tightly: the 48dp touch margin would double every row gap
    // on a list this long, and each chip is still 32dp tall and full-width tappable.
    CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides 0.dp) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
        modifier = Modifier.fillMaxWidth().padding(bottom = 4.dp),
    ) {
        choices.forEach { choice ->
            val checked = choice.selected
            val glow = ItemGlows.forEffect(choice.value)
            FilterChip(
                selected = checked,
                onClick = { onToggle(choice.value) },
                label = { Text(choice.label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                leadingIcon = {
                    val dot = glow?.color ?: MaterialTheme.colorScheme.outline
                    Box(
                        Modifier
                            .size(if (checked) 14.dp else 10.dp)
                            .popOnChange(checked, peak = 1.5f)
                            .clip(CircleShape)
                            // Black glows (curses, Grim, Stone) get a light rim to read on dark.
                            .background(MaterialTheme.colorScheme.onSurface.copy(alpha = if (dot.luminanceLow()) 0.5f else 0f))
                            .padding(if (dot.luminanceLow()) 1.5.dp else 0.dp)
                            .clip(CircleShape)
                            .background(dot),
                    )
                },
                shape = CircleShape,
            )
        }
    }
    }
}

private fun Color.luminanceLow(): Boolean = (red * 0.299f + green * 0.587f + blue * 0.114f) < 0.18f

/**
 * One pickable item, named [label]. Picking it grows a seal behind its
 * sprite, springs the tile and tints it in the primary colours.
 */
@Composable
private fun ItemTile(item: CatalogItem, label: String, selected: Boolean, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val seal by animateFloatAsState(
        if (selected) 1f else 0f,
        spring(dampingRatio = 0.5f, stiffness = 400f),
        label = "tile-seal",
    )
    val container by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
        label = "tile-container",
    )
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .height(118.dp)
            .pressScale(interaction)
            .selectable(selected = selected, interactionSource = interaction, indication = ripple(), onClick = onClick),
        shape = MaterialTheme.shapes.large,
        color = container,
        border = if (selected) {
            BorderStroke(2.dp, MaterialTheme.colorScheme.primary)
        } else {
            null
        },
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 9.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            // The seal is painted behind the sprite's own fixed box, spilling past
            // it, so its spring never moves the name below.
            val sealColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.28f)
            Box(
                Modifier.size(42.dp).drawBehind {
                    val side = 52.dp.toPx() * seal
                    if (side > 0.5f) {
                        drawPolygon(SeekerShapes.Seed, Offset((size.width - side) / 2f, (size.height - side) / 2f), side, sealColor)
                    }
                },
                contentAlignment = Alignment.Center,
            ) {
                ItemSprite(item, modifier = Modifier.fillMaxSize().graphicsLayer {
                    val scale = 1f + 0.12f * seal
                    scaleX = scale
                    scaleY = scale
                })
            }
            Spacer(Modifier.height(5.dp))
            Text(
                label,
                style = MaterialTheme.typography.labelSmall,
                fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            item.tier?.let {
                Text(
                    "Tier $it",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/**
 * The chip a save would put on the board — its sprite, title, details and
 * relations — or, while the draft cannot be saved, why not; [notice] is why
 * the editor could not answer at all.
 */
@Composable
private fun RequirementPreview(form: SheetForm, notice: String?) {
    val preview = form.preview.takeIf { notice == null }
    Surface(
        shape = MaterialTheme.shapes.extraLarge,
        color = if (preview != null) MaterialTheme.colorScheme.surfaceContainerHighest else MaterialTheme.colorScheme.errorContainer,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SpriteTile(
                item = preview?.item ?: form.item.value?.let(ItemCatalog::findById),
                wildcardKind = preview?.kind ?: form.kind,
                glows = preview?.effect?.takeUnless { it.anyEnchantment }?.let { ItemGlows.forEffects(it.effects) }.orEmpty(),
                tileSize = 44,
                // Every change to the draft bounces its preview.
                modifier = Modifier.popOnChange(preview, peak = 1.12f),
            )
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                if (preview == null) {
                    Text(
                        (listOfNotNull(notice) + form.errors).ifEmpty { listOf(form.title) }.joinToString("\n"),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                } else {
                    Text(preview.title, style = MaterialTheme.typography.titleSmall)
                    (listOf(preview.details.joinToString(" · ")) + preview.relations).filter { it.isNotEmpty() }.forEach {
                        Text(
                            it,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
        }
    }
}
