// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.offset
import androidx.compose.material3.ripple
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.IntOffset
import kotlin.math.roundToInt
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.BadgeView
import dev.seedseeker.app.model.BoardEdit
import dev.seedseeker.app.model.BoardItemView
import dev.seedseeker.app.model.BoardView
import dev.seedseeker.app.model.ChipView
import dev.seedseeker.app.model.EffectView
import dev.seedseeker.app.model.ResinChipView
import dev.seedseeker.app.model.TagStyle
import dev.seedseeker.app.model.TagView
import dev.seedseeker.app.ui.theme.SpdGreen
import dev.seedseeker.app.ui.theme.SpdUpgrade
import dev.seedseeker.app.ui.theme.SpdYellow

/**
 * The requirement board: every chip is one item to find, and the two ways chips
 * relate are shown rather than described.
 *
 * - Dragging a chip onto another makes them *either/or* alternatives of one
 *   slot — they share a dashed capsule with a small "or" between them. Dragging
 *   a chip off its capsule pulls it back out on its own.
 * - A chip asking for several items of the same kind carries a `×N` badge, and
 *   one asking for a combined level carries `Σ ≥ N`. Both are properties of a
 *   chip rather than relationships between chips — a capsule's member carries
 *   its own (`{Frost ×2 | Disintegration}`: two Frosts, or one
 *   Disintegration), and the capsule none — so both are set in the editor a
 *   tap opens, never by a drag.
 *
 * Entries flow like words: a chip sits beside the last one when it fits and
 * starts a new line when it does not. A capsule flows the same way inside its
 * own edge, so its chips stack within the one dashed outline rather than the
 * capsule splitting into what would read as two slots. A chip wider than a
 * phone's line gives up its name first, ellipsised, so the tags that say what
 * it actually asks for are never the part that goes.
 *
 * Removal is a drop rather than a target to hit — the board opens a zone under
 * itself while a chip is held — with the editor a tap opens as the other way.
 *
 * What the chips say, which drops a chip takes, and what a drop writes back
 * are the requirement editor's: [board] is its answer for the whole list, of
 * which this draws the [blanket] section, and every drop is sent to
 * [onChange] as a [BoardEdit]. A tap opens the editor on a row through
 * [onEdit]. Hit-testing reads the join and refusal sets [board] already
 * carries, so a drag asks the editor nothing until it lands.
 *
 * [compact] draws every chip at its smaller size ([ChipMetrics.Compact]): a
 * shorter capsule, a smaller sprite, and smaller text, so a line holds more.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun RequirementBoard(
    board: BoardView,
    enabled: Boolean,
    compact: Boolean = false,
    blanket: Boolean = false,
    onChange: (BoardEdit) -> Unit,
    onEdit: (Long) -> Unit,
    onAdd: () -> Unit,
    /** The row whose entry to bring into view — where a save landed — reported through [onFocused] once it is. */
    focus: Long? = null,
    onFocused: () -> Unit = {},
    /** The Arcane Resin chip, drawn after the ordinary section's chips. */
    resin: ResinChipView? = null,
    onEditResin: () -> Unit = {},
    onRemoveResin: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val items = remember(board, blanket) { board.items.filter { it.blanket == blanket } }
    val haptics = LocalHapticFeedback.current
    val entrances = LocalEntranceMemory.current
    // Live layout handles, so a drop can name what it landed on. Bounds are
    // resolved against the board only at hit-test time, which keeps them right
    // through scrolling.
    val placements = remember { mutableStateMapOf<Long, LayoutCoordinates>() }
    // A capsule is a target in its own right: the "or" between its chips and
    // the padding around them are still the cluster's. Keyed by its anchor row.
    val capsules = remember { mutableStateMapOf<Long, LayoutCoordinates>() }
    var boardArea by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var deleteZone by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var dragging by remember { mutableStateOf<Long?>(null) }
    var resinPlacement by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var draggingResin by remember { mutableStateOf(false) }
    var dragPosition by remember { mutableStateOf(Offset.Zero) }
    // Where inside the held chip the finger landed, so the lifted copy that
    // follows the finger keeps that same grip.
    var grabOffset by remember { mutableStateOf(Offset.Zero) }

    fun rectOf(child: LayoutCoordinates?): Rect? {
        val root = boardArea?.takeIf { it.isAttached } ?: return null
        val target = child?.takeIf { it.isAttached } ?: return null
        return root.localBoundingBoxOf(target)
    }

    fun chipOf(key: Long): ChipView? = items.firstNotNullOfOrNull { item -> item.chips.firstOrNull { it.key == key } }

    /**
     * What the pointer is over: a chip or a capsule takes the drop when the
     * editor lets [source] join it, and says why when it refuses; anything
     * else there, the chip's own entry included, takes nothing.
     */
    fun targetAt(position: Offset, source: ChipView): DropTarget? {
        if (rectOf(deleteZone)?.contains(position) == true) return DropTarget.Remove
        val over = placements.entries.firstOrNull { (_, coordinates) -> rectOf(coordinates)?.contains(position) == true }?.key
            ?: items.firstOrNull { rectOf(capsules[it.anchor])?.contains(position) == true }?.anchor
            ?: return DropTarget.Board
        if (over in source.join) return DropTarget.Join(over)
        return source.refuse[over]?.let(DropTarget::Refused)
    }

    val held = dragging?.let(::chipOf)
    // Resin is its own requirement and cannot join an either/or item group.
    val target = if (draggingResin) {
        DropTarget.Remove.takeIf { rectOf(deleteZone)?.contains(dragPosition) == true }
    } else held?.let { targetAt(dragPosition, it) }
    val hovered = (target as? DropTarget.Join)?.key
    val metrics = if (compact) ChipMetrics.Compact else ChipMetrics.Regular
    // A light tick each time the held chip finds something new to land on.
    LaunchedEffect(target) {
        if (target is DropTarget.Join || target == DropTarget.Remove) {
            haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
        }
    }

    CompositionLocalProvider(LocalChipMetrics provides metrics) {
        Box(modifier = modifier.onGloballyPositioned { boardArea = it }) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(metrics.spacing),
                    verticalArrangement = Arrangement.spacedBy(metrics.spacing),
                ) {
                    items.forEachIndexed { position, item ->
                        key(item.anchor) {
                            // Chips spring in when added, not every time the board reappears.
                            val fresh = remember { entrances.firstTime("chip:${item.anchor}") }
                            val view = remember { BringIntoViewRequester() }
                            if (focus != null && focus in item.members) {
                                LaunchedEffect(focus) {
                                    view.bringIntoView()
                                    onFocused()
                                }
                            }
                            BoardEntry(
                                modifier = Modifier
                                    .bringIntoViewRequester(view)
                                    .springEntrance(enabled = fresh, delayMillis = if (fresh) (position % 10) * 30 else 0),
                                item = item,
                                enabled = enabled,
                                dragging = dragging,
                                hovered = hovered,
                                onPlaced = { key, coordinates -> placements[key] = coordinates },
                                onCapsulePlaced = { capsules[item.anchor] = it },
                                onEdit = onEdit,
                                onDragStart = { key, offset ->
                                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                    dragging = key
                                    grabOffset = offset
                                    dragPosition = (rectOf(placements[key])?.topLeft ?: Offset.Zero) + offset
                                },
                                onDrag = { delta -> dragPosition += delta },
                                onDragEnd = {
                                    val source = dragging?.let(::chipOf)
                                    dragging = null
                                    if (source == null) return@BoardEntry
                                    when (val drop = targetAt(dragPosition, source)) {
                                        is DropTarget.Join -> {
                                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                            onChange(BoardEdit.Join(source.key, drop.key))
                                        }
                                        // A rejected drop is not a drag out of
                                        // a capsule: keep the original group.
                                        is DropTarget.Refused -> Unit
                                        // A lone chip goes with its copies; a member
                                        // leaves the cluster and its stack behind.
                                        DropTarget.Remove -> {
                                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                            onChange(BoardEdit.Remove(source.key))
                                        }
                                        // Let go on the open board: a member leaves its capsule.
                                        DropTarget.Board -> if (source.canDetach) onChange(BoardEdit.Detach(source.key))
                                        null -> Unit
                                    }
                                },
                                onDragCancel = { dragging = null },
                            )
                        }
                    }
                    if (resin != null) {
                        ArcaneResinChip(
                            resin = resin,
                            enabled = enabled,
                            dimmed = draggingResin,
                            onPlaced = { resinPlacement = it },
                            onClick = onEditResin,
                            onDragStart = { offset ->
                                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                draggingResin = true
                                grabOffset = offset
                                dragPosition = (rectOf(resinPlacement)?.topLeft ?: Offset.Zero) + offset
                            },
                            onDrag = { delta -> dragPosition += delta },
                            onDragEnd = {
                                draggingResin = false
                                if (rectOf(deleteZone)?.contains(dragPosition) == true) {
                                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                    onRemoveResin()
                                }
                            },
                            onDragCancel = { draggingResin = false },
                        )
                    }
                    AddChip(enabled = enabled, onClick = onAdd)
                }
                if (held != null || draggingResin) {
                    RemoveZone(
                        over = target == DropTarget.Remove,
                        modifier = Modifier.onGloballyPositioned { deleteZone = it },
                    )
                    if (target is DropTarget.Refused) {
                        Text(
                            target.message,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            // The chip in hand: a lifted, tilted copy riding under the finger
            // while its place on the board waits, faded, for it to come back.
            // It is the one item the drag moves, so it wears no stack badges.
            val ghostResin = resin.takeIf { draggingResin }
            if (held != null || ghostResin != null) {
                val lift = remember { Animatable(0f) }
                LaunchedEffect(Unit) { lift.animateTo(1f, spring(dampingRatio = 0.5f, stiffness = 500f)) }
                val overBin by animateFloatAsState(
                    if (target == DropTarget.Remove) 0.86f else 1f,
                    spring(dampingRatio = 0.5f, stiffness = 500f),
                    label = "ghost-over-bin",
                )
                Box(
                    Modifier
                        .offset {
                            val corner = dragPosition - grabOffset
                            IntOffset(corner.x.roundToInt(), corner.y.roundToInt())
                        }
                        .graphicsLayer {
                            // Held over the bin, the chip shrinks as if about to be swallowed.
                            val scale = (1f + 0.06f * lift.value) * overBin
                            scaleX = scale
                            scaleY = scale
                            rotationZ = -2f * lift.value
                            // Float just above the finger so the chip underneath stays in view.
                            translationY = -14.dp.toPx() * lift.value
                            shadowElevation = 12.dp.toPx() * lift.value
                            shape = CircleShape
                            // Stays opaque: a translucent layer renders offscreen at its
                            // unscaled size, which would crop the enlarged capsule's ends.
                        }
                        .clearAndSetSemantics {},
                ) {
                    if (held != null) {
                        HeldChip(held)
                    } else if (ghostResin != null) {
                        ArcaneResinChip(
                            resin = ghostResin,
                            enabled = false,
                            dimmed = false,
                            onPlaced = {},
                            onClick = {},
                            onDragStart = {},
                            onDrag = {},
                            onDragEnd = {},
                            onDragCancel = {},
                        )
                    }
                }
            }
        }
    }
}

/**
 * The sizes a chip and everything on it are drawn at. [Regular] is the
 * default; [Compact] is the "Compact chips" setting, which trades a little
 * legibility for more chips on a line.
 */
private class ChipMetrics(
    /** Whether the chip's name and its badges use the smaller text styles. */
    val compact: Boolean,
    /** The sprite tile's side in dp; a chip is this plus its padding tall. */
    val tile: Int,
    /** Padding above and below the tile. */
    val verticalPadding: Dp,
    val startPadding: Dp,
    val endPadding: Dp,
    /** Between the tile and the name. */
    val spriteGap: Dp,
    /** Between one chip and the next on a line, and between lines. */
    val spacing: Dp,
    /** A tag's padding around its text; stack badges wear a little more. */
    val tagPadding: Dp,
    /** The effect-count badge's height, and the "any enchantment" dot's side. */
    val badgeHeight: Dp,
    val dotSize: Dp,
    val addIconSize: Dp,
) {
    /** A chip's height: its tile plus the padding above and below it. */
    val height: Dp = tile.dp + verticalPadding * 2

    /**
     * Half a chip's height, so a chip's ends are half circles rather than
     * rounded corners, and so the capsule a cluster wears stays concentric
     * with them.
     */
    val radius: Dp = height / 2

    companion object {
        val Regular = ChipMetrics(
            compact = false,
            tile = 34,
            verticalPadding = 5.dp,
            startPadding = 6.dp,
            endPadding = 10.dp,
            spriteGap = 8.dp,
            spacing = 6.dp,
            tagPadding = 5.dp,
            badgeHeight = 19.dp,
            dotSize = 13.dp,
            addIconSize = 20.dp,
        )
        val Compact = ChipMetrics(
            compact = true,
            tile = 24,
            verticalPadding = 3.dp,
            startPadding = 4.dp,
            endPadding = 8.dp,
            spriteGap = 6.dp,
            spacing = 5.dp,
            tagPadding = 4.dp,
            badgeHeight = 16.dp,
            dotSize = 11.dp,
            addIconSize = 16.dp,
        )
    }
}

private val LocalChipMetrics = compositionLocalOf { ChipMetrics.Regular }

/** The style a chip's name is set in. */
private val chipTitleStyle: TextStyle
    @Composable get() = if (LocalChipMetrics.current.compact) {
        MaterialTheme.typography.bodyMedium
    } else {
        MaterialTheme.typography.bodyLarge
    }

/** The style a chip's tags, badges, and the "or" between alternatives are set in. */
private val chipLabelStyle: TextStyle
    @Composable get() = if (LocalChipMetrics.current.compact) {
        MaterialTheme.typography.labelSmall
    } else {
        MaterialTheme.typography.labelMedium
    }

/** How far a capsule's dashed edge stands off the chips inside it. */
private val CAPSULE_INSET = 5.dp

/** Where a dragged chip may be let go. */
private sealed interface DropTarget {
    /** Become an either/or alternative of the row [key]. */
    data class Join(val key: Long) : DropTarget

    /** A chip or capsule the editor refuses the join onto, and why. */
    data class Refused(val message: String) : DropTarget

    /** The open board: a cluster member leaves its capsule. */
    data object Board : DropTarget

    /** Leave the board. */
    data object Remove : DropTarget
}

/** A chip, or the capsule holding an either/or cluster's chips. */
@Composable
private fun BoardEntry(
    item: BoardItemView,
    enabled: Boolean,
    dragging: Long?,
    hovered: Long?,
    onPlaced: (Long, LayoutCoordinates) -> Unit,
    onCapsulePlaced: (LayoutCoordinates) -> Unit,
    onEdit: (Long) -> Unit,
    onDragStart: (Long, Offset) -> Unit,
    onDrag: (Offset) -> Unit,
    onDragEnd: () -> Unit,
    onDragCancel: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (item.cluster == null) {
        val chip = item.chips.single()
        RequirementChip(
            chip = chip,
            enabled = enabled,
            dimmed = dragging == chip.key,
            highlighted = hovered == chip.key,
            modifier = modifier,
            onPlaced = { onPlaced(chip.key, it) },
            onClick = { onEdit(chip.key) },
            onDragStart = { offset -> onDragStart(chip.key, offset) },
            onDrag = onDrag,
            onDragEnd = onDragEnd,
            onDragCancel = onDragCancel,
        )
        return
    }
    val highlighted = hovered != null && hovered in item.members
    val edge = MaterialTheme.colorScheme.tertiary
    val metrics = LocalChipMetrics.current
    // A capsule a chip is about to join marches its dashes like ants and swells.
    val march = if (highlighted && LocalMotionEnabled.current) {
        rememberInfiniteTransition(label = "capsule-march").animateFloat(
            initialValue = 0f,
            targetValue = 14f,
            animationSpec = infiniteRepeatable(tween(420, easing = LinearEasing)),
            label = "capsule-phase",
        ).value
    } else 0f
    val swell by animateFloatAsState(
        if (highlighted) 1.04f else 1f,
        spring(dampingRatio = 0.45f, stiffness = 500f),
        label = "capsule-swell",
    )
    // The capsule is exactly as wide as what it holds, and what it holds flows
    // too: a member that will not fit beside the last drops to the next line
    // *inside* the dashed edge, its "or" going with it, so the capsule still
    // reads as one slot however tall it grows.
    FlowRow(
        modifier = modifier
            .onGloballyPositioned(onCapsulePlaced)
            // A new member joining the capsule makes it shout.
            .popOnChange(item.members.size, peak = 1.08f)
            .graphicsLayer {
                scaleX = swell
                scaleY = swell
            }
            .dashedOutline(
                color = if (highlighted) edge else edge.copy(alpha = 0.55f),
                fill = edge.copy(alpha = if (highlighted) 0.12f else 0.05f),
                radius = metrics.radius + CAPSULE_INSET,
                width = if (highlighted) 2.dp else 1.dp,
                phase = march,
            )
            .padding(CAPSULE_INSET),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        item.chips.forEachIndexed { position, chip ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (position > 0) {
                    Text(
                        "or",
                        modifier = Modifier.padding(horizontal = 5.dp),
                        style = chipLabelStyle,
                        fontFamily = FontFamily.Monospace,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.tertiary,
                    )
                }
                // A member carries its own stack badges: the copies follow
                // it, and are waived when another member is the one found.
                RequirementChip(
                    chip = chip,
                    enabled = enabled,
                    dimmed = dragging == chip.key,
                    highlighted = false,
                    modifier = Modifier.weight(1f, fill = false),
                    onPlaced = { onPlaced(chip.key, it) },
                    onClick = { onEdit(chip.key) },
                    onDragStart = { offset -> onDragStart(chip.key, offset) },
                    onDrag = onDrag,
                    onDragEnd = onDragEnd,
                    onDragCancel = onDragCancel,
                )
            }
        }
    }
}

/**
 * One item to find: its sprite, its name, and the tiny tags that narrow it. A
 * tap edits it and a long press picks it up to drop on another chip; the effect
 * and the source it may come from stay in the editor and the spoken
 * description, since a phone's line has no room to spell them out. A chip the
 * editor finds a problem with wears the error colour and says the problem.
 */
@Composable
private fun RequirementChip(
    chip: ChipView,
    enabled: Boolean,
    dimmed: Boolean,
    highlighted: Boolean,
    modifier: Modifier = Modifier,
    onPlaced: (LayoutCoordinates) -> Unit,
    onClick: () -> Unit,
    onDragStart: (Offset) -> Unit,
    onDrag: (Offset) -> Unit,
    onDragEnd: () -> Unit,
    onDragCancel: () -> Unit,
    /** Whether the chip wears its stack badges; the one item a drag holds does not. */
    badges: Boolean = true,
) {
    val metrics = LocalChipMetrics.current
    BoardChip(
        description = listOfNotNull(chip.description, chip.problem).joinToString(", "),
        enabled = enabled,
        dimmed = dimmed,
        highlighted = highlighted,
        faulty = chip.problem != null,
        modifier = modifier,
        onPlaced = onPlaced,
        onClick = onClick,
        onDragStart = onDragStart,
        onDrag = onDrag,
        onDragEnd = onDragEnd,
        onDragCancel = onDragCancel,
    ) {
        SpriteTile(
            item = chip.item,
            wildcardKind = chip.kind,
            // "Any enchantment" settles on no colour of its own.
            glows = chip.effect?.takeUnless { it.anyEnchantment }?.let { ItemGlows.forEffects(it.effects) }.orEmpty(),
            tileSize = metrics.tile,
        )
        Spacer(Modifier.width(metrics.spriteGap))
        ChipTitle(chip.name)
        ChipTags(chip.tags)
        EffectBadge(chip.effect)
        ChipTags(chip.trailingTags)
        if (chip.uncursed) {
            Spacer(Modifier.width(5.dp))
            UncursedTag()
        }
        if (badges) StackBadges(count = chip.countBadge, total = chip.totalBadge, enabled = enabled, onClick = onClick)
    }
}

/**
 * The one item a drag holds: [chip]'s sprite, name and tags, without the
 * `×N` or `Σ` it wears on the board, since only one of its items moves.
 */
@Composable
internal fun HeldChip(chip: ChipView) {
    RequirementChip(
        chip = chip,
        enabled = false,
        dimmed = false,
        highlighted = false,
        onPlaced = {},
        onClick = {},
        onDragStart = {},
        onDrag = {},
        onDragEnd = {},
        onDragCancel = {},
        badges = false,
    )
}

@Composable
private fun ArcaneResinChip(
    resin: ResinChipView,
    enabled: Boolean,
    dimmed: Boolean,
    onPlaced: (LayoutCoordinates) -> Unit,
    onClick: () -> Unit,
    onDragStart: (Offset) -> Unit,
    onDrag: (Offset) -> Unit,
    onDragEnd: () -> Unit,
    onDragCancel: () -> Unit,
) {
    val metrics = LocalChipMetrics.current
    BoardChip(
        description = resin.description,
        enabled = enabled,
        dimmed = dimmed,
        highlighted = false,
        faulty = false,
        onPlaced = onPlaced,
        onClick = onClick,
        onDragStart = onDragStart,
        onDrag = onDrag,
        onDragEnd = onDragEnd,
        onDragCancel = onDragCancel,
    ) {
        SpriteTile(item = arcaneResinItem, tileSize = metrics.tile)
        Spacer(Modifier.width(metrics.spriteGap))
        ChipTitle(resin.name)
        ChipTags(resin.tags)
        if (resin.uncursed) {
            Spacer(Modifier.width(5.dp))
            UncursedTag()
        }
    }
}

@Composable
private fun RowScope.ChipTitle(title: String) {
    Text(
        title,
        modifier = Modifier.weight(1f, fill = false),
        style = chipTitleStyle,
        fontWeight = FontWeight.SemiBold,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}

/**
 * Shared appearance and gestures for every draggable requirement chip; a
 * [faulty] one is outlined in the error colour.
 */
@Composable
private fun BoardChip(
    description: String,
    enabled: Boolean,
    dimmed: Boolean,
    highlighted: Boolean,
    faulty: Boolean,
    modifier: Modifier = Modifier,
    onPlaced: (LayoutCoordinates) -> Unit,
    onClick: () -> Unit,
    onDragStart: (Offset) -> Unit,
    onDrag: (Offset) -> Unit,
    onDragEnd: () -> Unit,
    onDragCancel: () -> Unit,
    content: @Composable RowScope.() -> Unit,
) {
    val outline = when {
        highlighted -> MaterialTheme.colorScheme.tertiary
        faulty -> MaterialTheme.colorScheme.error
        else -> MaterialTheme.colorScheme.outlineVariant
    }
    val metrics = LocalChipMetrics.current
    // A drop target swells toward the chip in hand; the chip's own empty place shrinks back.
    val scale by animateFloatAsState(
        when {
            highlighted -> 1.08f
            dimmed -> 0.92f
            else -> 1f
        },
        spring(dampingRatio = 0.42f, stiffness = 520f),
        label = "chip-scale",
    )
    val fade by animateFloatAsState(if (dimmed) 0.35f else 1f, tween(160), label = "chip-fade")
    // A tap squashes the chip a little before the editor opens.
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val press by animateFloatAsState(
        if (pressed) 0.94f else 1f,
        MaterialTheme.motionScheme.fastSpatialSpec(),
        label = "chip-press",
    )
    // Keep a held gesture alive through recomposition while using current state.
    val currentOnDragStart by rememberUpdatedState(onDragStart)
    val currentOnDrag by rememberUpdatedState(onDrag)
    val currentOnDragEnd by rememberUpdatedState(onDragEnd)
    val currentOnDragCancel by rememberUpdatedState(onDragCancel)
    Surface(
        // A capsule, not a rounded rectangle: the ends stay half circles
        // however tall the chip grows at a larger font scale.
        shape = CircleShape,
        color = if (highlighted) MaterialTheme.colorScheme.tertiaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
        border = BorderStroke(if (highlighted) 2.dp else 1.dp, outline),
        modifier = modifier
            // Position and drag deltas are read outside the scale below, so
            // a swelling or shrinking chip never skews where the finger is.
            .onGloballyPositioned(onPlaced)
            .pointerInput(enabled) {
                if (!enabled) return@pointerInput
                detectDragGesturesAfterLongPress(
                    onDragStart = { currentOnDragStart(it) },
                    onDrag = { change, delta ->
                        change.consume()
                        currentOnDrag(delta)
                    },
                    onDragEnd = { currentOnDragEnd() },
                    onDragCancel = { currentOnDragCancel() },
                )
            }
            .graphicsLayer {
                alpha = fade
                scaleX = scale * press
                scaleY = scale * press
            }
            .clickable(interactionSource = interaction, indication = ripple(), enabled = enabled, onClick = onClick)
            .semantics { contentDescription = description },
    ) {
        Row(
            modifier = Modifier.padding(
                start = metrics.startPadding,
                top = metrics.verticalPadding,
                end = metrics.endPadding,
                bottom = metrics.verticalPadding,
            ),
            verticalAlignment = Alignment.CenterVertically,
            content = content,
        )
    }
}

/**
 * How many of the chip (`×N` / `≤N`) and the level they reach together
 * (`Σ ≥ N`, tighter on compact chips), each read out as what it means.
 */
@Composable
private fun RowScope.StackBadges(count: BadgeView?, total: BadgeView?, enabled: Boolean, onClick: () -> Unit) {
    val compact = LocalChipMetrics.current.compact
    count?.let {
        Spacer(Modifier.width(6.dp))
        StackBadge(
            text = if (compact) it.compactText else it.text,
            container = SpdGreen,
            content = MaterialTheme.colorScheme.onPrimary,
            description = it.tooltip,
            enabled = enabled,
            onClick = onClick,
        )
    }
    total?.let {
        Spacer(Modifier.width(5.dp))
        StackBadge(
            text = if (compact) it.compactText else it.text,
            container = SpdYellow,
            content = Color.Black,
            description = it.tooltip,
            enabled = enabled,
            onClick = onClick,
        )
    }
}

@Composable
private fun StackBadge(
    text: String,
    container: Color,
    content: Color,
    description: String,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    Surface(
        shape = RoundedCornerShape(10.dp),
        color = container,
        // No stepper on the chip: a badge is small, and a tap opening the editor
        // is one target instead of three.
        modifier = Modifier
            .clickable(enabled = enabled, onClick = onClick)
            .semantics { contentDescription = description },
    ) {
        val padding = LocalChipMetrics.current.tagPadding
        Text(
            text,
            modifier = Modifier.padding(horizontal = padding + 2.dp, vertical = padding - 3.dp),
            style = chipLabelStyle,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
            color = content,
        )
    }
}

/**
 * What one pulse of the sprite cannot say. A single effect wants no badge — the
 * sprite is already pulsing that very colour, black for a curse — but several
 * at once do, and so does "any enchantment", which settles on no colour at all.
 */
@Composable
private fun EffectBadge(effect: EffectView?) {
    when {
        effect == null -> Unit
        effect.anyEnchantment -> {
            Spacer(Modifier.width(5.dp))
            AnyEnchantmentDot(effect.label)
        }
        effect.effects.size > 1 -> {
            Spacer(Modifier.width(5.dp))
            EffectCountBadge(ItemGlows.forEffects(effect.effects), effect.label)
        }
    }
}

/** Several effects: the count, ringed in the colours the item may arrive in. */
@Composable
private fun EffectCountBadge(glows: List<Glow>, description: String) {
    val middle = MaterialTheme.colorScheme.surfaceContainerHigh
    val ring = remember(glows) { effectRing(glows) }
    val badgeHeight = LocalChipMetrics.current.badgeHeight
    Box(
        modifier = Modifier
            .height(badgeHeight)
            .widthIn(min = badgeHeight)
            .drawBehind {
                val radius = CornerRadius(size.height / 2f)
                drawRoundRect(brush = Brush.sweepGradient(*ring), cornerRadius = radius)
                val inset = 2.dp.toPx()
                drawRoundRect(
                    color = middle,
                    topLeft = Offset(inset, inset),
                    size = Size(size.width - inset * 2, size.height - inset * 2),
                    cornerRadius = CornerRadius(size.height / 2f - inset),
                )
            }
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            "${glows.size}",
            modifier = Modifier.padding(horizontal = 5.dp),
            style = chipLabelStyle,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
        )
    }
}

/** "Any enchantment" settles on no colour, so its dot shows every one. */
@Composable
private fun AnyEnchantmentDot(description: String) {
    Box(
        Modifier
            .size(LocalChipMetrics.current.dotSize)
            .clip(CircleShape)
            .background(Brush.sweepGradient(*RAINBOW))
            .semantics { contentDescription = description },
    )
}

/** Stationary, evenly spaced colours blend smoothly around the effect count. */
private fun effectRing(glows: List<Glow>): Array<Pair<Float, Color>> {
    val band = 1f / glows.size
    return (
        glows.mapIndexed { index, glow -> (index * band) to glow.color } +
            listOf(1f to glows.first().color)
        ).toTypedArray()
}

private val RAINBOW = arrayOf(
    0f to Color(0xFFFF5555),
    1f / 6f to Color(0xFFFFFF55),
    2f / 6f to Color(0xFF55FF55),
    3f / 6f to Color(0xFF55FFFF),
    4f / 6f to Color(0xFF5555FF),
    5f / 6f to Color(0xFFFF55FF),
    1f to Color(0xFFFF5555),
)

/**
 * The trailing "+ Add" chip that opens the editor on a new requirement. It
 * stands as tall as a chip, so the line it shares with one stays level. Its
 * plus turns a quarter on press.
 */
@Composable
private fun AddChip(enabled: Boolean, onClick: () -> Unit) {
    val metrics = LocalChipMetrics.current
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val turn by animateFloatAsState(
        if (pressed) 90f else 0f,
        MaterialTheme.motionScheme.fastSpatialSpec(),
        label = "add-turn",
    )
    Row(
        modifier = Modifier
            .pressScale(interaction)
            .clip(CircleShape)
            .dashedOutline(MaterialTheme.colorScheme.outline, radius = metrics.radius)
            .clickable(
                interactionSource = interaction,
                indication = ripple(),
                enabled = enabled,
                onClick = onClick,
            )
            .semantics { contentDescription = "Add requirement" }
            .height(metrics.height)
            .padding(start = metrics.startPadding + 6.dp, end = metrics.endPadding + 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            Icons.Filled.Add,
            contentDescription = null,
            modifier = Modifier.size(metrics.addIconSize).rotate(turn),
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.width(5.dp))
        Text(
            "Add",
            style = chipTitleStyle,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * The zone the board opens under itself while a chip is held. It stands as
 * tall as a chip, so the chip it swallows and the row it replaces match.
 * It springs open when a chip is picked up, and when the chip hovers over it
 * the zone fills, swells and wags its bin.
 */
@Composable
private fun RemoveZone(over: Boolean, modifier: Modifier = Modifier) {
    val danger = MaterialTheme.colorScheme.error
    val fill = MaterialTheme.colorScheme.errorContainer
    val swell by animateFloatAsState(
        if (over) 1.04f else 1f,
        spring(dampingRatio = 0.4f, stiffness = 600f),
        label = "remove-swell",
    )
    val wag = if (over && LocalMotionEnabled.current) {
        rememberInfiniteTransition(label = "bin-wag").animateFloat(
            initialValue = -14f,
            targetValue = 14f,
            animationSpec = infiniteRepeatable(tween(130), RepeatMode.Reverse),
            label = "bin-angle",
        ).value
    } else 0f
    Row(
        modifier = modifier
            .springEntrance(rise = 10f)
            .graphicsLayer {
                scaleX = swell
                scaleY = swell
            }
            .fillMaxWidth()
            .then(
                if (over) {
                    Modifier.clip(RoundedCornerShape(LocalChipMetrics.current.radius)).background(fill)
                } else {
                    Modifier.dashedOutline(danger.copy(alpha = 0.6f), radius = LocalChipMetrics.current.radius)
                },
            )
            .height(LocalChipMetrics.current.height),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            Icons.Filled.Delete,
            contentDescription = null,
            modifier = Modifier.size(if (over) 18.dp else 14.dp).rotate(wag),
            tint = if (over) MaterialTheme.colorScheme.onErrorContainer else danger,
        )
        Spacer(Modifier.width(6.dp))
        Text(
            "Drop to remove",
            style = MaterialTheme.typography.labelMedium,
            color = if (over) MaterialTheme.colorScheme.onErrorContainer else danger,
        )
    }
}

/** A dashed outline, for the shapes that are an invitation rather than a thing. */
private fun Modifier.dashedOutline(
    color: Color,
    fill: Color = Color.Transparent,
    radius: Dp = 16.dp,
    width: Dp = 1.dp,
    phase: Float = 0f,
): Modifier = drawBehind {
    if (fill != Color.Transparent) {
        drawRoundRect(color = fill, cornerRadius = CornerRadius(radius.toPx()))
    }
    val stroke = width.toPx()
    drawRoundRect(
        color = color,
        topLeft = Offset(stroke / 2f, stroke / 2f),
        size = Size(size.width - stroke, size.height - stroke),
        cornerRadius = CornerRadius((radius.toPx() - stroke / 2f).coerceAtLeast(0f)),
        style = Stroke(
            width = stroke,
            pathEffect = PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 3.dp.toPx()), phase.dp.toPx()),
        ),
    )
}

/** The qualifier badges beside a chip's name, in the order given. */
@Composable
private fun ChipTags(tags: List<TagView>) {
    tags.forEach { tag ->
        Spacer(Modifier.width(5.dp))
        ChipTag(tag)
    }
}

/**
 * One qualifier badge, tinted by its style: an upgrade in the upgrade colour,
 * everything else — the resin a chip credits (`≥4`, `Auto`, `Mage +2`)
 * included, as the resin chip has always drawn them here — in the qualifier
 * tint. A tag with a tooltip of its own shows it while a mouse rests on it;
 * touch keeps the long press for picking the chip up.
 */
@Composable
private fun ChipTag(tag: TagView) {
    val container = when (tag.style) {
        TagStyle.UPGRADE -> SpdUpgrade.copy(alpha = 0.12f)
        TagStyle.PLAIN, TagStyle.CREDIT -> MaterialTheme.colorScheme.tertiaryContainer
    }
    val content = when (tag.style) {
        TagStyle.UPGRADE -> SpdUpgrade
        TagStyle.PLAIN, TagStyle.CREDIT -> MaterialTheme.colorScheme.onTertiaryContainer
    }
    val padding = LocalChipMetrics.current.tagPadding
    HoverTooltip(tag.tooltip) { modifier ->
        Surface(shape = RoundedCornerShape(6.dp), color = container, modifier = modifier) {
            Text(
                tag.text,
                modifier = Modifier.padding(horizontal = padding, vertical = padding - 4.dp),
                style = chipLabelStyle,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.SemiBold,
                color = content,
            )
        }
    }
}

/**
 * [content], with [tooltip] shown above it while a mouse hovers it; just
 * [content] without one. The tooltip takes no touch input, so a long press
 * still reaches the chip underneath, and adds nothing to what TalkBack
 * reads: the chip's description already says it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HoverTooltip(tooltip: String?, content: @Composable (Modifier) -> Unit) {
    if (tooltip == null) {
        content(Modifier)
        return
    }
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    val state = rememberTooltipState(isPersistent = true)
    LaunchedEffect(hovered) {
        // Leaving cancels the show, which dismisses the tooltip.
        if (hovered) state.show()
    }
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = { PlainTooltip { Text(tooltip) } },
        state = state,
        enableUserInput = false,
    ) {
        content(Modifier.hoverable(hover))
    }
}

@Composable
private fun UncursedTag() {
    val labelHeight = with(LocalDensity.current) { chipLabelStyle.lineHeight.toDp() }
    val verticalPadding = LocalChipMetrics.current.tagPadding - 4.dp
    Surface(
        shape = RoundedCornerShape(6.dp),
        color = SpdGreen.copy(alpha = 0.14f),
        // Match text tags at both chip sizes and follow the user's font scale.
        modifier = Modifier.size(labelHeight + verticalPadding * 2),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Icon(
                Icons.Filled.Check,
                // The parent chip already announces "uncursed".
                contentDescription = null,
                tint = SpdGreen,
                modifier = Modifier.size(labelHeight),
            )
        }
    }
}
