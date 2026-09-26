// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.getValue
import androidx.compose.ui.draw.rotate
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.seedseeker.app.model.FloorFeeling

/** Shared floor identity and informational badges; metadata wraps on narrow screens. */
@Composable
internal fun FloorHeading(
    depth: Int,
    itemCount: Int,
    feeling: FloorFeeling? = null,
    modifier: Modifier = Modifier,
    questLabel: String? = null,
    farming: Boolean = false,
    mapExpanded: Boolean = false,
    onMapToggle: (() -> Unit)? = null,
    onCloseMap: (() -> Unit)? = null,
) {
    val region = floorRegionColor(depth)
    Box(
        modifier.fillMaxWidth().heightIn(min = 48.dp).then(
            if (onMapToggle != null) Modifier.clickable(
                role = Role.Button,
                onClickLabel = "${if (mapExpanded) "Hide" else "Show"} floor $depth map",
                onClick = onMapToggle,
            ).semantics { stateDescription = if (mapExpanded) "Map expanded" else "Map collapsed" }
            else Modifier,
        ),
        contentAlignment = Alignment.CenterStart,
    ) {
        Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            FlowRow(
                Modifier.weight(1f),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalArrangement = Arrangement.spacedBy(4.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    // Each region wears its own silhouette: soft sewers, boxy prisons,
                    // faceted caves, an ornate city and the spiked halls.
                    ShapeBackdrop(SeekerShapes.forDepth(depth), region, Modifier.size(16.dp))
                    Text("FLOOR $depth", style = MaterialTheme.typography.labelLarge,
                        letterSpacing = 1.1.sp, color = MaterialTheme.colorScheme.onSurface)
                    if (feeling != null && feeling != FloorFeeling.NONE) FloorFeelingSprite(feeling)
                    Text(floorRegion(depth), style = MaterialTheme.typography.labelMedium, color = region)
                }
                FlowRow(Modifier.padding(start = 8.dp), itemVerticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    questLabel?.let {
                        Surface(shape = CircleShape, color = region.copy(alpha = 0.14f)) {
                            Text(it, Modifier.padding(horizontal = 8.dp, vertical = 1.dp),
                                style = MaterialTheme.typography.labelSmall, color = region)
                        }
                    }
                    if (farming) {
                        Surface(shape = CircleShape, color = MaterialTheme.colorScheme.secondaryContainer) {
                            Text("Garden", Modifier.padding(horizontal = 8.dp, vertical = 1.dp),
                                style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSecondaryContainer)
                        }
                    }
                    Text(if (itemCount == 1) "1 item" else "$itemCount items",
                        style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            if (onCloseMap != null) {
                Spacer(Modifier.width(8.dp))
                IconButton(onClick = onCloseMap, modifier = Modifier.size(32.dp)) {
                    Icon(Icons.Filled.Close, "Close map", Modifier.size(20.dp))
                }
            } else if (onMapToggle != null) {
                Spacer(Modifier.width(8.dp))
                // The map chevron turns down as its map unfolds.
                val turn by animateFloatAsState(
                    if (mapExpanded) 90f else 0f,
                    MaterialTheme.motionScheme.fastSpatialSpec(),
                    label = "map-chevron",
                )
                Surface(
                    shape = CircleShape,
                    color = if (mapExpanded) region.copy(alpha = 0.22f) else MaterialTheme.colorScheme.surfaceContainerHigh,
                    modifier = Modifier.size(28.dp),
                ) {
                    Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, null,
                        Modifier.padding(4.dp).rotate(turn),
                        tint = if (mapExpanded) region else MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}
