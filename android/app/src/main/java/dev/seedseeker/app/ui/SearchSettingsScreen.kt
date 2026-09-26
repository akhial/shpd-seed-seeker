// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.Warning
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.text.font.FontWeight
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
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
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
            LargeFlexibleTopAppBar(
                title = { Text("Search settings", fontWeight = FontWeight.ExtraBold) },
                subtitle = {
                    Text(
                        scopeSummaryText(query.maximumDepth, query.requireBlacksmith, query.excludeBlacksmithRewards,
                            query.wandmakerQuest, query.challenges),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack, shapes = IconButtonDefaults.shapes()) {
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

                SettingsGroupTitle("Dungeon", MaterialShapes.Gem, Icons.Filled.Place)
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

                SettingsGroupTitle("Rooms and feelings", MaterialShapes.Flower, Icons.Filled.Home)
                SearchSettingsCard {
                    FarmingFloorsSection(query.floorRequirements, enabled,
                        onToggle = { onQueryChange(query.toggleFarmingFloor(it)) },
                        onRemove = { depth -> onQueryChange(query.copy(
                            floorRequirements = query.floorRequirements.filterNot { it.depth == depth })) })
                }
                query.floorRequirements.floorValidationProblem(query.maximumDepth)?.let { SettingsNotice(it) }

                SettingsGroupTitle("Quests and items", MaterialShapes.Sunny, Icons.Filled.Star)
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
                    SettingsGroupTitle("Performance", MaterialShapes.SoftBurst, Icons.Filled.Build)
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

                SettingsGroupTitle("Challenges · ${Integer.bitCount(query.challenges)} on", MaterialShapes.SoftBoom, Icons.Filled.Warning)
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
        icon = {
            ShapeBackdrop(MaterialShapes.Flower, MaterialTheme.colorScheme.tertiaryContainer, Modifier.size(52.dp)) {
                Icon(Icons.Filled.Star, contentDescription = null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
            }
        },
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

/** A slider's name and its value, which rolls like an odometer as the thumb moves. */
@Composable
private fun SearchSliderHeading(title: String, value: String) {
    val headingStyle = MaterialTheme.typography.titleMedium.copy(fontSize = 18.sp)
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(title, style = headingStyle, modifier = Modifier.weight(1f))
        AnimatedContent(
            targetState = value,
            transitionSpec = {
                val up = (targetState.substringBefore(' ').toIntOrNull() ?: 0) >
                    (initialState.substringBefore(' ').toIntOrNull() ?: 0)
                (slideInVertically { if (up) it else -it } + fadeIn())
                    .togetherWith(slideOutVertically { if (up) -it else it } + fadeOut())
                    .using(SizeTransform(clip = false))
            },
            label = "slider-value",
        ) { shown ->
            Text(shown, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.ExtraBold,
                color = MaterialTheme.colorScheme.tertiary)
        }
    }
}

@Composable
private fun SettingsGroupTitle(title: String, polygon: androidx.graphics.shapes.RoundedPolygon, icon: androidx.compose.ui.graphics.vector.ImageVector) {
    SectionHeading(title, polygon, icon, modifier = Modifier.padding(start = 6.dp, top = 20.dp, bottom = 4.dp),
        color = MaterialTheme.colorScheme.tertiary)
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
    val interaction = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    // Checked rows glow faintly in the primary colour.
    val container by animateColorAsState(
        if (checked && enabled) MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.45f)
        else MaterialTheme.colorScheme.surfaceContainerHigh,
        label = "setting-row",
    )
    Surface(shape = shape, color = container, modifier = Modifier.pressScale(interaction, pressed = 0.98f)) {
        Row(Modifier.fillMaxWidth()
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onCheckedChange,
                interactionSource = interaction, indication = ripple())
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

/** The finder's doorway into these settings; its cog turns a notch whenever it is pressed. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun SearchSettingsLink(summary: String, onClick: () -> Unit) {
    val interaction = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val turn by animateFloatAsState(if (pressed) 60f else 0f, MaterialTheme.motionScheme.defaultSpatialSpec(), label = "cog")
    Surface(onClick = onClick, shape = MaterialTheme.shapes.extraLarge, interactionSource = interaction,
        color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = Modifier.pressScale(interaction, pressed = 0.97f)) {
        Row(Modifier.fillMaxWidth().padding(20.dp), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            ShapeBackdrop(MaterialShapes.Cookie9Sided, MaterialTheme.colorScheme.primaryContainer, Modifier.size(48.dp)) {
                Icon(Icons.Filled.Settings, contentDescription = null, tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(24.dp).rotate(turn))
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
