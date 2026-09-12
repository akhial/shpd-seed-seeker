// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.ClipData
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Info
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.platform.toClipEntry
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.util.lerp
import dev.seedseeker.app.engine.ScoutMatches
import dev.seedseeker.app.model.ScoutItem
import dev.seedseeker.app.model.ScoutWorld
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

@Composable
internal fun ScoutSummaryCard(
    world: ScoutWorld,
    matches: ScoutMatches?,
    progress: Float,
    onCollapseDistanceChanged: (Float) -> Unit,
    modifier: Modifier = Modifier,
) {
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    val floors = world.items.map(ScoutItem::depth).distinct().size
    val matchText = matches?.let { scoutMatchText(it.matchedSlots, it.totalSlots) }
    val labelStyle = MaterialTheme.typography.labelMedium
    val matchTextSize = rememberTextMeasurer().measure(matchText.orEmpty(), labelStyle, maxLines = 1).size
    Card(
        modifier = modifier.fillMaxWidth().testTag("scout-summary").semantics {
            if (progress == 1f) contentDescription = "${world.items.size} items, $floors floors"
        },
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
    ) {
        Layout(
            content = {
                Text(
                    world.seed,
                    modifier = Modifier.testTag("scout-seed"),
                    style = MaterialTheme.typography.headlineSmall.copy(
                        fontSize = lerp(24f, 20f, progress).sp,
                        lineHeight = lerp(32f, 28f, progress).sp,
                        fontFamily = FontFamily.Monospace,
                    ),
                    color = MaterialTheme.colorScheme.tertiary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                TextButton(onClick = {
                    scope.launch {
                        clipboard.setClipEntry(ClipData.newPlainText("Seed", world.seed).toClipEntry())
                    }
                }) { Text("Copy") }
                Row(
                    modifier = Modifier.graphicsLayer {
                        alpha = (1f - progress * 1.6f).coerceIn(0f, 1f)
                        translationX = 40.dp.toPx() * progress
                    }.then(if (progress >= 0.625f) Modifier.clearAndSetSemantics {} else Modifier),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    SummaryCount("${world.items.size} items", Modifier.weight(1f, fill = false))
                    SummaryCount("$floors floors", Modifier.weight(1f, fill = false))
                }
                if (matches != null) {
                    RequirementBadge(matches, progress)
                }
            },
        ) { measurables, constraints ->
            val padding = 18.dp.roundToPx()
            val gap = 8.dp.roundToPx()
            val width = constraints.maxWidth
            val innerWidth = (width - 2 * padding).coerceAtLeast(0)
            val loose = Constraints(maxWidth = innerWidth)
            val copy = measurables[1].measure(loose)
            val badgeHeight = maxOf(32.dp.roundToPx(), matchTextSize.height + 12.dp.roundToPx())
            val expandedBadgeWidth = if (matches == null) 0 else {
                (matchTextSize.width + 24.dp.roundToPx()).coerceAtMost((innerWidth * 0.55f).roundToInt())
            }
            val badgeWidth = lerp(expandedBadgeWidth, badgeHeight, progress)
            val badge = if (matches == null) null else measurables[3].measure(Constraints.fixed(badgeWidth, badgeHeight))
            val seedWidth = (innerWidth - copy.width - if (badge != null) {
                ((badgeHeight + 2 * gap) * progress).roundToInt()
            } else 0).coerceAtLeast(0)
            val seed = measurables[0].measure(Constraints(maxWidth = seedWidth))
            val counts = measurables[2].measure(Constraints(
                maxWidth = (innerWidth - expandedBadgeWidth - if (badge != null) gap else 0).coerceAtLeast(0),
            ))
            val rowHeight = maxOf(copy.height, seed.height, if (badge != null) badgeHeight else 0)
            val secondRowHeight = maxOf(counts.height, if (badge != null) badgeHeight else 0)
            val expandedHeight = 8.dp.roundToPx() + rowHeight + 6.dp.roundToPx() + secondRowHeight + 12.dp.roundToPx()
            val compactHeight = rowHeight + 8.dp.roundToPx()
            onCollapseDistanceChanged((expandedHeight - compactHeight).toFloat())
            val top = lerp(8.dp.roundToPx(), 4.dp.roundToPx(), progress)
            val secondRowTop = top + rowHeight + 6.dp.roundToPx()
            layout(width, lerp(expandedHeight, compactHeight, progress)) {
                seed.placeRelative(padding, top + (rowHeight - seed.height) / 2)
                copy.placeRelative(width - padding - copy.width, top + (rowHeight - copy.height) / 2)
                counts.placeRelative(padding, lerp(secondRowTop, top + (rowHeight - counts.height) / 2, progress))
                badge?.placeRelative(
                    lerp(width - padding - expandedBadgeWidth, width - padding - copy.width - gap - badgeHeight, progress),
                    lerp(secondRowTop, top + (rowHeight - badgeHeight) / 2, progress),
                )
            }
        }
    }
}

@Composable
private fun SummaryCount(text: String, modifier: Modifier) {
    Text(
        text,
        modifier = modifier.background(MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
private fun RequirementBadge(matches: ScoutMatches, progress: Float) {
    val hasMatches = matches.matchedSlots > 0
    val complete = hasMatches && matches.matchedSlots == matches.totalSlots
    val text = scoutMatchText(matches.matchedSlots, matches.totalSlots)
    val container = if (hasMatches) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHighest
    val content = if (hasMatches) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurfaceVariant
    Layout(
        modifier = Modifier.testTag("scout-requirements").clip(CircleShape).background(container)
            .clearAndSetSemantics { contentDescription = text },
        content = {
            Text(
                text,
                modifier = Modifier.graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                    .drawWithContent {
                        drawContent()
                        if (progress > 0f) {
                            // Wipe from the right of the full label, independent of the shrinking capsule.
                            val edge = size.width * (1f - progress)
                            drawRect(
                                brush = Brush.horizontalGradient(
                                    colors = listOf(Color.Black, Color.Transparent),
                                    startX = (edge - 24.dp.toPx()).coerceAtLeast(0f),
                                    endX = edge.coerceAtLeast(0.01f),
                                ),
                                blendMode = BlendMode.DstIn,
                            )
                        }
                    },
                style = MaterialTheme.typography.labelMedium,
                color = content,
                maxLines = 1,
            )
            Box(Modifier.size(20.dp).graphicsLayer {
                alpha = ((progress - 0.45f) / 0.55f).coerceIn(0f, 1f)
            }, contentAlignment = Alignment.Center) {
                Icon(if (complete) Icons.Filled.Check else Icons.Filled.Info, contentDescription = null, tint = content)
            }
        },
    ) { measurables, constraints ->
        val label = measurables[0].measure(Constraints())
        val icon = measurables[1].measure(Constraints())
        layout(constraints.maxWidth, constraints.maxHeight) {
            label.placeRelative(12.dp.roundToPx(), (constraints.maxHeight - label.height) / 2)
            icon.placeRelative((constraints.maxWidth - icon.width) / 2, (constraints.maxHeight - icon.height) / 2)
        }
    }
}
