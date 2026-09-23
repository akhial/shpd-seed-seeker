// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.model.*
import kotlin.math.roundToInt

/** Query values remain owned by the app, so every change is saved before navigating back. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SearchSettingsScreen(
    query: PresetQuery,
    enabled: Boolean,
    challengesEnabled: Boolean,
    workerCount: Int,
    workerCeiling: Int,
    onQueryChange: (PresetQuery) -> Unit,
    onWorkerCountChange: (Int) -> Unit,
    onBack: () -> Unit,
) {
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    var chooseQuest by remember { mutableStateOf(false) }
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            LargeTopAppBar(
                title = { Text("Search settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                scrollBehavior = scrollBehavior,
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    scrolledContainerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.TopCenter) {
            Column(
                Modifier.widthIn(max = 680.dp).fillMaxWidth()
                    .verticalScroll(rememberScrollState()).testTag("search-settings-scroll")
                    .padding(start = 16.dp, end = 16.dp, bottom = 28.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (!enabled) SettingsNotice("Stop the search to change its settings.")

                SettingsGroupTitle("Dungeon")
                SearchSettingsCard {
                    SearchSliderHeading("Max floor", "${query.maximumDepth}")
                    Slider(
                        value = floorLimitIndex(query.maximumDepth).toFloat(),
                        onValueChange = {
                            val index = it.roundToInt().coerceIn(0, FLOOR_LIMIT_OPTIONS.lastIndex)
                            onQueryChange(query.copy(maximumDepth = FLOOR_LIMIT_OPTIONS[index]))
                        },
                        valueRange = 0f..FLOOR_LIMIT_OPTIONS.lastIndex.toFloat(),
                        steps = FLOOR_LIMIT_OPTIONS.size - 2,
                        colors = searchSliderColors(),
                        enabled = enabled,
                        modifier = Modifier.semantics {
                            contentDescription = "Max floor"
                            stateDescription = "Floor ${query.maximumDepth}"
                        },
                    )
                }

                SettingsGroupTitle("Rooms and feelings")
                SearchSettingsCard {
                    FarmingFloorsSection(query.floorRequirements, enabled,
                        onToggle = { onQueryChange(query.toggleFarmingFloor(it)) },
                        onRemove = { depth -> onQueryChange(query.copy(
                            floorRequirements = query.floorRequirements.filterNot { it.depth == depth })) })
                }
                query.floorRequirements.floorValidationProblem(query.maximumDepth)?.let { SettingsNotice(it) }

                SettingsGroupTitle("Quests and items")
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    SearchSettingSwitch(
                        title = "AutoTrinket",
                        supporting = "Applies a helpful trinket at +3 at the first brewing opportunity. Keeps it only when the match needs it.",
                        checked = query.autoApplyTrinket, enabled = enabled,
                        onCheckedChange = { onQueryChange(query.copy(autoApplyTrinket = it)) },
                        shape = settingRowShape(first = true),
                    )
                    Surface(onClick = { chooseQuest = true }, enabled = enabled,
                        color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = settingRowShape()) {
                        Row(Modifier.fillMaxWidth().padding(20.dp), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                Text("Wandmaker quest", style = MaterialTheme.typography.titleMedium)
                                Text(query.wandmakerQuest?.label ?: "Any quest",
                                    color = MaterialTheme.colorScheme.tertiary, style = MaterialTheme.typography.bodyLarge)
                            }
                            Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                    val blacksmithGuaranteed = query.maximumDepth >= ScoutQuestGiver.BLACKSMITH.depths.last
                    SearchSettingSwitch(
                        title = "Blacksmith reachable",
                        supporting = if (blacksmithGuaranteed) "Already included at this floor limit." else "Reach the Blacksmith within your floor limit.",
                        checked = blacksmithGuaranteed || query.requireBlacksmith,
                        enabled = enabled && !blacksmithGuaranteed,
                        onCheckedChange = { onQueryChange(query.copy(requireBlacksmith = it)) },
                        shape = settingRowShape(),
                    )
                    SearchSettingSwitch(
                        title = "Exclude smith rewards",
                        supporting = "Items may not come from the 2,000-favor Smith trade.",
                        checked = query.excludeBlacksmithRewards, enabled = enabled,
                        onCheckedChange = { onQueryChange(query.copy(excludeBlacksmithRewards = it)) },
                        shape = settingRowShape(last = true),
                    )
                }

                if (workerCeiling > 1) {
                    SettingsGroupTitle("Performance")
                    SearchSettingsCard {
                        val shown = workerCount.coerceIn(1, workerCeiling)
                        SearchSliderHeading("Workers", "$shown / $workerCeiling")
                        Slider(
                            value = shown.toFloat(),
                            onValueChange = { onWorkerCountChange(it.roundToInt().coerceIn(1, workerCeiling)) },
                            valueRange = 1f..workerCeiling.toFloat(), steps = workerCeiling - 2, enabled = enabled,
                            colors = searchSliderColors(),
                            modifier = Modifier.semantics {
                                contentDescription = "Workers"
                                stateDescription = "$shown of $workerCeiling cores"
                            },
                        )
                        Text("More workers search faster and use more battery.",
                            style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }

                SettingsGroupTitle("Challenges · ${Integer.bitCount(query.challenges)} on")
                Text("Used for both searches and scouting.",
                    style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 8.dp, bottom = 8.dp))
                if (!challengesEnabled) SettingsNotice("Challenges can be changed after the current search or scout stops.")
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Challenge.entries.forEachIndexed { index, challenge ->
                        SearchSettingSwitch(
                            title = challenge.displayName,
                            supporting = if (challenge.changesLevelGeneration) "Changes level generation" else "No effect on seed content",
                            checked = query.challenges and challenge.bit != 0,
                            enabled = enabled && challengesEnabled,
                            onCheckedChange = { checked -> onQueryChange(query.copy(challenges =
                                if (checked) query.challenges or challenge.bit else query.challenges and challenge.bit.inv())) },
                            shape = settingRowShape(first = index == 0, last = index == Challenge.entries.lastIndex),
                        )
                    }
                }
            }
        }
    }
    if (chooseQuest) AlertDialog(
        onDismissRequest = { chooseQuest = false },
        title = { Text("Wandmaker quest") },
        text = {
            Column(Modifier.selectableGroup().verticalScroll(rememberScrollState())) {
                (listOf(null) + WandmakerQuest.entries).forEach { quest ->
                    Row(
                        Modifier.fillMaxWidth().clip(MaterialTheme.shapes.medium)
                            .selectable(selected = query.wandmakerQuest == quest, enabled = enabled,
                                role = Role.RadioButton, onClick = {
                                    onQueryChange(query.copy(wandmakerQuest = quest))
                                    chooseQuest = false
                                }).padding(vertical = 14.dp, horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = query.wandmakerQuest == quest, onClick = null, enabled = enabled)
                        Text(quest?.label ?: "Any quest", modifier = Modifier.padding(start = 12.dp),
                            style = MaterialTheme.typography.bodyLarge)
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { chooseQuest = false }) { Text("Cancel") } },
    )
}

@Composable
private fun SearchSliderHeading(title: String, value: String) {
    val headingStyle = MaterialTheme.typography.titleMedium.copy(fontSize = 18.sp)
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(title, style = headingStyle, modifier = Modifier.weight(1f))
        Text(value, style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.tertiary)
    }
}

@Composable
private fun SettingsGroupTitle(title: String) {
    Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.tertiary,
        modifier = Modifier.padding(start = 8.dp, top = 20.dp, bottom = 4.dp))
}

@Composable
private fun SearchSettingsCard(content: @Composable ColumnScope.() -> Unit) {
    Surface(shape = MaterialTheme.shapes.extraLarge, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(8.dp), content = content)
    }
}

private fun settingRowShape(first: Boolean = false, last: Boolean = false) = RoundedCornerShape(
    topStart = if (first) 28.dp else 4.dp, topEnd = if (first) 28.dp else 4.dp,
    bottomStart = if (last) 28.dp else 4.dp, bottomEnd = if (last) 28.dp else 4.dp,
)

@Composable
private fun SearchSettingSwitch(
    title: String,
    supporting: String,
    checked: Boolean,
    enabled: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    shape: RoundedCornerShape,
) {
    Surface(shape = shape, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Row(Modifier.fillMaxWidth()
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onCheckedChange)
            .padding(20.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Text(supporting, style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Switch(checked = checked, onCheckedChange = null, enabled = enabled,
                thumbContent = if (checked) ({ Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(16.dp)) }) else null)
        }
    }
}

@Composable
private fun SettingsNotice(message: String) {
    Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.errorContainer) {
        Text(message, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onErrorContainer,
            modifier = Modifier.fillMaxWidth().padding(16.dp))
    }
}

@Composable
internal fun SearchSettingsLink(summary: String, onClick: () -> Unit) {
    Surface(onClick = onClick, shape = MaterialTheme.shapes.extraLarge,
        color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Row(Modifier.fillMaxWidth().padding(20.dp), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Surface(shape = MaterialTheme.shapes.medium, color = MaterialTheme.colorScheme.primaryContainer) {
                Icon(Icons.Filled.Settings, contentDescription = null, tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(12.dp).size(24.dp))
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Search settings", style = MaterialTheme.typography.titleMedium)
                Text(summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null,
                tint = MaterialTheme.colorScheme.primary)
        }
    }
}

@Composable
private fun searchSliderColors() = SliderDefaults.colors(
    inactiveTrackColor = MaterialTheme.colorScheme.surfaceVariant,
    inactiveTickColor = MaterialTheme.colorScheme.onSurfaceVariant,
)
