// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.asComposePath
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.graphics.shapes.Morph
import androidx.graphics.shapes.RoundedPolygon
import androidx.graphics.shapes.toPath
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.sin
import kotlin.random.Random

/*
 * The app's expressive motion kit: Material 3 Expressive's shape library and
 * springs, put to work. Everything here is decoration layered on top of a
 * screen's own layout — it scales, rotates, draws or fades in the draw and
 * layer phases, never re-measures — so a test that asserts bounds sees the
 * same boxes with or without it, and a user who turns animations off sees
 * the resting state of every effect.
 */

/**
 * False when the user has turned system animations off. Every looping or
 * celebratory effect below checks it and settles on its resting frame.
 */
val LocalMotionEnabled = staticCompositionLocalOf { true }

/** Whether the system animator scale allows motion, read once per context. */
@Composable
fun rememberMotionEnabled(): Boolean {
    val context = LocalContext.current
    return remember(context) { animationsEnabled(context) }
}

/**
 * The shape families the app speaks in. Each region of the dungeon has its
 * own silhouette, so a floor's marker says where it is at a glance even
 * before its colour does.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
object SeekerShapes {
    val Seed: RoundedPolygon get() = MaterialShapes.Cookie9Sided
    val Match: RoundedPolygon get() = MaterialShapes.SoftBurst

    /** A calm cycle for idle "waiting for you" illustrations. */
    val Idle: List<RoundedPolygon>
        get() = listOf(
            MaterialShapes.Cookie9Sided,
            MaterialShapes.Clover4Leaf,
            MaterialShapes.Puffy,
            MaterialShapes.Gem,
            MaterialShapes.Flower,
        )

    /** A punchier cycle for "working on it" moments. */
    val Busy: List<RoundedPolygon>
        get() = listOf(
            MaterialShapes.SoftBurst,
            MaterialShapes.Cookie9Sided,
            MaterialShapes.Pentagon,
            MaterialShapes.Pill,
            MaterialShapes.Sunny,
            MaterialShapes.Cookie4Sided,
            MaterialShapes.Oval,
        )

    /** Small sparkle silhouettes a celebration throws. */
    val Sparkles: List<RoundedPolygon>
        get() = listOf(
            MaterialShapes.Sunny,
            MaterialShapes.Clover4Leaf,
            MaterialShapes.SoftBurst,
            MaterialShapes.Cookie4Sided,
            MaterialShapes.Heart,
        )
}

/** Fills [polygon] (normalised to a unit square) across [size], turned by [degrees]. */
private fun polygonPath(polygon: RoundedPolygon, size: Size, degrees: Float = 0f): Path =
    scaled(polygon.toPath(), size, degrees)

private fun morphPath(morph: Morph, progress: Float, size: Size, degrees: Float = 0f): Path =
    scaled(morph.toPath(progress, android.graphics.Path()), size, degrees)

private fun scaled(path: android.graphics.Path, size: Size, degrees: Float): Path {
    val matrix = android.graphics.Matrix()
    matrix.setScale(size.width, size.height)
    if (degrees != 0f) matrix.preRotate(degrees, 0.5f, 0.5f)
    path.transform(matrix)
    return path.asComposePath()
}

/** A clip or background shape cut from one of Material's expressive polygons. */
class PolygonShape(private val polygon: RoundedPolygon, private val degrees: Float = 0f) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline =
        Outline.Generic(polygonPath(polygon, size, degrees))
}

/** A shape caught part-way through morphing from one polygon to another. */
class MorphShape(private val morph: Morph, private val progress: Float, private val degrees: Float = 0f) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline =
        Outline.Generic(morphPath(morph, progress, size, degrees))
}

/**
 * Content set on an expressive polygon. With [spinMillis] the backdrop turns
 * a full revolution in that time while the content stays upright, which is
 * how the app says "alive" without saying "busy".
 */
@Composable
fun ShapeBackdrop(
    polygon: RoundedPolygon,
    color: Color,
    modifier: Modifier = Modifier,
    spinMillis: Int? = null,
    content: @Composable BoxScope.() -> Unit = {},
) {
    val motion = LocalMotionEnabled.current
    val rotation = if (spinMillis != null && motion) {
        val transition = rememberInfiniteTransition(label = "backdrop-spin")
        transition.animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = infiniteRepeatable(tween(spinMillis, easing = LinearEasing)),
            label = "backdrop-rotation",
        )
    } else null
    Box(
        // The outline is built once per size; a spin only rotates it.
        modifier.drawWithCache {
            val path = polygonPath(polygon, size)
            onDrawBehind { rotate(rotation?.value ?: 0f) { drawPath(path, color) } }
        },
        contentAlignment = androidx.compose.ui.Alignment.Center,
        content = content,
    )
}

/**
 * A backdrop that never stops changing shape: it holds each of [shapes],
 * then springs into the next, turning as it goes when [turning] (for busy
 * states). Frozen on the first shape when motion is off.
 */
@Composable
fun MorphingBackdrop(
    shapes: List<RoundedPolygon>,
    color: Color,
    modifier: Modifier = Modifier,
    stepMillis: Int = 1400,
    turning: Boolean = false,
    content: @Composable BoxScope.() -> Unit = {},
) {
    val motion = LocalMotionEnabled.current
    val morphs = remember(shapes) { shapes.indices.map { Morph(shapes[it], shapes[(it + 1) % shapes.size]) } }
    val clock = if (motion) {
        rememberInfiniteTransition(label = "morph-cycle").animateFloat(
            initialValue = 0f,
            targetValue = shapes.size.toFloat(),
            animationSpec = infiniteRepeatable(tween(stepMillis * shapes.size, easing = LinearEasing)),
            label = "morph-clock",
        )
    } else null
    Box(
        modifier.drawBehind {
            val t = clock?.value ?: 0f
            val index = floor(t).toInt().coerceIn(0, morphs.lastIndex)
            val local = t - index
            // Hold, then an overshooting spring-like ease into the next shape.
            val progress = springEase(((local - 0.45f) / 0.55f).coerceIn(0f, 1f))
            val path = morphPath(morphs[index], progress.coerceIn(0f, 1f), size)
            rotate(if (turning) t * 72f else 0f) { drawPath(path, color) }
        },
        contentAlignment = androidx.compose.ui.Alignment.Center,
        content = content,
    )
}

/** An underdamped ease: overshoots a touch past 1 and settles, like a spring. */
internal fun springEase(x: Float): Float {
    if (x <= 0f) return 0f
    if (x >= 1f) return 1f
    val decay = kotlin.math.exp(-6f * x)
    return 1f - decay * cos(x * 9f)
}

/**
 * What has already sprung into view this session. Screens leave composition
 * whenever the user switches tabs; asking here instead of remembering locally
 * keeps a row, floor or chip from replaying its entrance on every return.
 */
class EntranceMemory {
    private val seen = mutableSetOf<Any>()

    /** True the first time [key] is asked about, false ever after. */
    fun firstTime(key: Any): Boolean = seen.add(key)
}

/** The app's entrance memory; a screen shown on its own gets a fresh one. */
val LocalEntranceMemory = staticCompositionLocalOf { EntranceMemory() }

/** Squashes a control while it is held, and springs it back on release. */
fun Modifier.pressScale(interactionSource: InteractionSource, pressed: Float = 0.92f): Modifier = composed {
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed) pressed else 1f,
        animationSpec = MaterialTheme.motionScheme.fastSpatialSpec(),
        label = "press-scale",
    )
    this.graphicsLayer {
        scaleX = scale
        scaleY = scale
    }
}

/**
 * "Shouts" when [key] changes after the first composition: a quick swell
 * past full size and a bouncy settle back.
 */
fun Modifier.popOnChange(key: Any?, peak: Float = 1.22f, fireIf: Boolean = true): Modifier = composed {
    val motion = LocalMotionEnabled.current
    val scale = remember { Animatable(1f) }
    var seen by remember { mutableStateOf(false) }
    LaunchedEffect(key) {
        if (!seen) {
            seen = true
            return@LaunchedEffect
        }
        if (!motion || !fireIf) return@LaunchedEffect
        scale.animateTo(peak, tween(90))
        scale.animateTo(1f, spring(dampingRatio = 0.32f, stiffness = 420f))
    }
    this.graphicsLayer {
        scaleX = scale.value
        scaleY = scale.value
    }
}

/** A short horizontal "no!" shake whenever [key] changes after the first composition. */
fun Modifier.shakeOnChange(key: Any?): Modifier = composed {
    val motion = LocalMotionEnabled.current
    val offset = remember { Animatable(0f) }
    var seen by remember { mutableStateOf(false) }
    LaunchedEffect(key) {
        if (!seen) {
            seen = true
            return@LaunchedEffect
        }
        if (!motion) return@LaunchedEffect
        offset.animateTo(0f, keyframes {
            durationMillis = 420
            -12f at 60
            10f at 130
            -7f at 200
            5f at 270
            -2f at 340
        })
    }
    this.graphicsLayer { translationX = offset.value * density }
}

/**
 * Springs a newly composed element into place: it rises a little, grows from
 * slightly small and fades in, [delayMillis] after it arrives. Pass
 * `enabled = false` for elements that were already on screen.
 */
fun Modifier.springEntrance(enabled: Boolean = true, delayMillis: Int = 0, rise: Float = 18f): Modifier = composed {
    val motion = LocalMotionEnabled.current
    val progress = remember { Animatable(if (enabled && motion) 0f else 1f) }
    LaunchedEffect(Unit) {
        if (progress.value < 1f) {
            // Wait on the frame clock rather than a timer, so the pause runs
            // on the same clock as the spring after it.
            if (delayMillis > 0) progress.animateTo(0.0001f, tween(delayMillis))
            progress.animateTo(1f, spring(dampingRatio = 0.62f, stiffness = 260f))
        }
    }
    this.graphicsLayer {
        val p = progress.value
        alpha = p.coerceIn(0f, 1f)
        translationY = (1f - p) * rise * density
        val scale = 0.9f + 0.1f * p
        scaleX = scale
        scaleY = scale
    }
}

/**
 * A slow, gentle breathing pulse for things that invite a tap, or that are
 * alive and working. Resting at full size when motion is off.
 */
fun Modifier.breathe(enabled: Boolean = true, amount: Float = 0.06f, periodMillis: Int = 1600): Modifier = composed {
    val motion = LocalMotionEnabled.current
    if (!enabled || !motion) return@composed this
    val transition = rememberInfiniteTransition(label = "breathe")
    val scale by transition.animateFloat(
        initialValue = 1f,
        targetValue = 1f + amount,
        animationSpec = infiniteRepeatable(tween(periodMillis / 2), RepeatMode.Reverse),
        label = "breathe-scale",
    )
    this.graphicsLayer {
        scaleX = scale
        scaleY = scale
    }
}

/** A dot that pulses a halo around itself — the universal "live" light. */
@Composable
fun PulsingDot(color: Color, modifier: Modifier = Modifier, active: Boolean = true) {
    val motion = LocalMotionEnabled.current && active
    val halo = if (motion) {
        rememberInfiniteTransition(label = "live-dot").animateFloat(
            initialValue = 0f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(tween(1300, easing = LinearEasing)),
            label = "live-halo",
        )
    } else null
    Box(
        modifier.drawBehind {
            val radius = size.minDimension / 2f
            val h = halo?.value
            if (h != null) {
                drawCircle(color.copy(alpha = 0.45f * (1f - h)), radius = radius * (0.6f + 1.2f * h))
            }
            drawCircle(color, radius = radius * 0.55f)
        },
    )
}

/**
 * Throws a burst of expressive sparkles out of this element every time
 * [level] rises after the first composition — never when it falls, so a
 * cleared list or a restarted search stays quiet. The sparkles are drawn over
 * the element and may spill past its edges; nothing is measured or placed.
 */
fun Modifier.celebrate(level: Int, colors: List<Color>, count: Int = 16, reach: Float = 64f): Modifier = composed {
    val motion = LocalMotionEnabled.current
    val progress = remember { Animatable(1f) }
    var previous by remember { mutableIntStateOf(level) }
    var seed by remember { mutableIntStateOf(0) }
    val sparkles = remember { SeekerShapes.Sparkles }
    LaunchedEffect(level) {
        val rose = level > previous
        previous = level
        if (!rose || !motion) return@LaunchedEffect
        seed++
        progress.snapTo(0f)
        progress.animateTo(1f, tween(900, easing = LinearEasing))
    }
    val particles = remember(seed) {
        val random = Random(seed * 7919 + 17)
        List(count) {
            Particle(
                angle = (it.toFloat() / count) * 2f * PI.toFloat() + random.nextFloat() * 0.5f,
                distance = 0.55f + random.nextFloat() * 0.6f,
                size = 5f + random.nextFloat() * 7f,
                spin = (random.nextFloat() - 0.5f) * 540f,
                color = colors[it % colors.size],
                shape = sparkles[random.nextInt(sparkles.size)],
            )
        }
    }
    this.drawWithContent {
        drawContent()
        val p = progress.value
        if (p < 1f) drawBurst(particles, p, reach * density)
    }
}

private class Particle(
    val angle: Float,
    val distance: Float,
    val size: Float,
    val spin: Float,
    val color: Color,
    val shape: RoundedPolygon,
)

private fun DrawScope.drawBurst(particles: List<Particle>, p: Float, reach: Float) {
    // Ease out: fast launch, drifting stop, with a little gravity at the end.
    val travel = 1f - (1f - p) * (1f - p) * (1f - p)
    val center = Offset(size.width / 2f, size.height / 2f)
    val radiusX = size.width / 2f
    val radiusY = size.height / 2f
    for (particle in particles) {
        val dx = cos(particle.angle)
        val dy = sin(particle.angle)
        val x = center.x + dx * (radiusX * 0.6f + reach * particle.distance * travel)
        val y = center.y + dy * (radiusY * 0.6f + reach * particle.distance * travel) + p * p * reach * 0.35f
        val grow = if (p < 0.15f) p / 0.15f else 1f - ((p - 0.15f) / 0.85f)
        val side = particle.size * density * grow.coerceIn(0f, 1f)
        if (side <= 0.5f) continue
        val path = polygonPath(particle.shape, Size(side, side))
        translate(x - side / 2f, y - side / 2f) {
            rotate(particle.spin * p, pivot = Offset(side / 2f, side / 2f)) {
                drawPath(path, particle.color.copy(alpha = (1f - p * p).coerceIn(0f, 1f)))
            }
        }
    }
}

/** Paints [polygon] into the square at [topLeft] with side [side], turned by [degrees]. */
fun DrawScope.drawPolygon(polygon: RoundedPolygon, topLeft: Offset, side: Float, color: Color, degrees: Float = 0f) {
    val path = polygonPath(polygon, Size(side, side), degrees)
    translate(topLeft.x, topLeft.y) { drawPath(path, color) }
}

/** The celebration palette: the game's own gold, upgrade green, teal and a curse-free pink. */
val CelebrationColors = listOf(
    Color(0xFFFFFF55),
    Color(0xFF83FC64),
    Color(0xFF58C2B4),
    Color(0xFFFF88C2),
    Color(0xFFB388FF),
)
