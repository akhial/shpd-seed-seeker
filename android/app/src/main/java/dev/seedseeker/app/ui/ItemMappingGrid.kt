// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.ScoutItemMapping
import dev.seedseeker.app.model.ScoutItemMappings
import org.json.JSONObject
import kotlin.math.roundToInt

internal fun mappingLabel(entry: ScoutItemMapping) = "${entry.appearance} — ${entry.name}"

private data class MappingArt(val spriteSize: IntSize, val iconBase: Int, val iconSizes: List<IntSize>)

/** Six columns use the journal's shared frame sizes and one game-pixel gutters. */
@Composable
internal fun ItemMappingGrid(
    mappings: ScoutItemMappings,
    selectedLabel: String?,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val artwork = remember(context) {
        context.assets.open("third_party/shattered-pixel-dungeon/item-mapping-art.json")
            .bufferedReader().use { JSONObject(it.readText()) }
    }
    val slotSize = artwork.getInt("slotSize")
    val slotGap = artwork.getInt("slotGap")
    val groups = remember(mappings, artwork) {
        listOf("potions" to mappings.potions, "scrolls" to mappings.scrolls, "rings" to mappings.rings).map { (category, entries) ->
            val data = artwork.getJSONObject("categories").getJSONObject(category)
            val sprite = data.getJSONArray("spriteSize")
            val icons = data.getJSONArray("iconSizes")
            Triple(category, entries, MappingArt(IntSize(sprite.getInt(0), sprite.getInt(1)), data.getInt("iconBase"),
                List(icons.length()) { index -> icons.getJSONArray(index).let { IntSize(it.getInt(0), it.getInt(1)) } }))
        }
    }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        for ((category, entries, art) in groups) {
            Column {
                Row(Modifier.padding(bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("${category.replaceFirstChar { it.uppercaseChar() }}",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.tertiary)
                }
                BoxWithConstraints(Modifier.fillMaxWidth()) {
                    val pixel = maxWidth / (6 * slotSize + 5 * slotGap)
                    Column(Modifier.semantics { collectionInfo = CollectionInfo(2, 6) },
                        verticalArrangement = Arrangement.spacedBy(pixel * slotGap)) {
                        entries.chunked(6).forEachIndexed { row, rowEntries ->
                            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(pixel * slotGap)) {
                                rowEntries.forEachIndexed { column, entry ->
                                    val classIndex = row * 6 + column
                                    val label = mappingLabel(entry)
                                    val isSelected = selectedLabel == label
                                    val interaction = remember { MutableInteractionSource() }
                                    // The chosen tile swells onto a seal; the rest wait on soft pads.
                                    val lift by animateFloatAsState(
                                        if (isSelected) 1f else 0f,
                                        spring(dampingRatio = 0.45f, stiffness = 420f),
                                        label = "mapping-lift",
                                    )
                                    val pad = MaterialTheme.colorScheme.surfaceContainerHighest
                                    val seal = MaterialTheme.colorScheme.tertiary.copy(alpha = 0.3f)
                                    MappingTile(entry, art, classIndex, slotSize,
                                        Modifier.weight(1f).aspectRatio(1f).testTag("mapping-$category-$classIndex")
                                            .pressScale(interaction, pressed = 0.88f)
                                            .drawBehind {
                                                drawRoundRect(pad.copy(alpha = 0.55f * (1f - lift)), cornerRadius = CornerRadius(size.minDimension * 0.22f))
                                                if (lift > 0.01f) {
                                                    val side = size.minDimension * (0.7f + 0.45f * lift)
                                                    drawPolygon(SeekerShapes.Seed, Offset((size.width - side) / 2f, (size.height - side) / 2f),
                                                        side, seal, degrees = 40f * lift)
                                                }
                                            }
                                            .graphicsLayer {
                                                val scale = 1f + 0.12f * lift
                                                scaleX = scale
                                                scaleY = scale
                                            }
                                            .clickable(interactionSource = interaction, indication = null, role = Role.Button) { onSelect(label) }
                                            .semantics { contentDescription = label; selected = isSelected })
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** Full sprite frame centered in a 17×17 tile; identity glyph flush top right. */
@Composable
private fun MappingTile(entry: ScoutItemMapping, art: MappingArt, classIndex: Int, slotSize: Int, modifier: Modifier) {
    val atlas = LocalItemAtlas.current
    val iconAtlas = LocalItemIconAtlas.current
    Canvas(modifier) {
        val scale = size.width / slotSize
        fun scaled(frame: IntSize) = IntSize((frame.width * scale).roundToInt(), (frame.height * scale).roundToInt())
        val spriteSize = scaled(art.spriteSize)
        if (atlas != null) {
            drawImage(atlas,
                srcOffset = IntOffset(entry.spriteIndex % 16 * 16, entry.spriteIndex / 16 * 16),
                srcSize = art.spriteSize,
                dstOffset = IntOffset(((size.width - spriteSize.width) / 2).roundToInt(), ((size.height - spriteSize.height) / 2).roundToInt()),
                dstSize = spriteSize, filterQuality = FilterQuality.None)
        }
        if (iconAtlas != null) {
            val icon = art.iconBase + classIndex
            val frame = art.iconSizes[classIndex]
            val iconSize = scaled(frame)
            drawImage(iconAtlas,
                srcOffset = IntOffset(icon % 16 * 8, icon / 16 * 8), srcSize = frame,
                dstOffset = IntOffset((size.width - iconSize.width).roundToInt(), 0),
                dstSize = iconSize, filterQuality = FilterQuality.None)
        }
    }
}
