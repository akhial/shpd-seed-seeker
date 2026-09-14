// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.R
import dev.seedseeker.app.engine.isMapDepthSupported
import dev.seedseeker.app.model.ScoutAccessibility
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutWorld

internal fun scoutFloors(world: ScoutWorld?): Map<Int, List<IndexedValue<ScoutItem>>> {
    if (world == null) return emptyMap()
    val floors = world.items.withIndex().groupBy { it.value.depth }.toSortedMap()
    world.floorFeelings.keys.filter(::isMapDepthSupported).forEach { floors.getOrPut(it) { emptyList() } }
    for (depth in 1..(floors.keys.maxOrNull() ?: 0)) {
        if (isMapDepthSupported(depth)) floors.getOrPut(depth) { emptyList() }
    }
    return floors
}

internal fun matchedScoutChoices(items: List<ScoutItem>, matched: Set<Int>): Map<Int, Int> = buildMap {
    matched.forEach { index -> (items.getOrNull(index)?.accessibility as? ScoutAccessibility.Choice)?.let { put(it.group, it.option) } }
}

internal fun isAlternateScoutChoice(access: ScoutAccessibility, matched: Boolean, choices: Map<Int, Int>): Boolean =
    !matched && access is ScoutAccessibility.Choice && choices[access.group]?.let { it != access.option } == true

internal fun scoutGroupLetter(group: Int): Char = ('A'.code + group % 26).toChar()

@Composable
internal fun ChoiceGroupChip(choice: ScoutAccessibility.Choice) {
    val letter = scoutGroupLetter(choice.group)
    val color = MaterialTheme.colorScheme.onSurfaceVariant
    Surface(shape = MaterialTheme.shapes.extraSmall, color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.semantics(mergeDescendants = true) { contentDescription = "One reward of choice group $letter (option ${choice.option + 1})" }) {
        Row(Modifier.padding(horizontal = 6.dp, vertical = 1.dp), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Icon(painterResource(R.drawable.ic_git_fork), null, Modifier.size(12.dp), tint = color)
            Text(letter.toString(), style = MaterialTheme.typography.labelSmall, color = color)
        }
    }
}
