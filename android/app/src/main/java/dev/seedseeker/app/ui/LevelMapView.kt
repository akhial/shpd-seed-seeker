// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.Context
import android.database.ContentObserver
import android.graphics.Canvas
import android.graphics.Color as AndroidColor
import android.graphics.Rect
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.Settings
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import android.view.KeyEvent
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.ImageView
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.lightColorScheme
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Bitmap
import android.graphics.drawable.BitmapDrawable
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.seedseeker.app.engine.LevelMapBundle
import dev.seedseeker.app.engine.LevelMapRequest
import dev.seedseeker.app.engine.LevelMaps
import dev.seedseeker.app.model.ScoutWorld
import kotlinx.coroutines.CancellationException
import kotlin.math.abs
import kotlin.math.floor

/** Mounted by the lazy floor list. A trinket swap keeps the open map's floor and dialog. */
@Composable
@OptIn(ExperimentalMaterial3Api::class)
internal fun LevelMapView(
    world: ScoutWorld,
    initialDepth: Int,
    floors: List<Int>,
    challenges: Int,
    changingTrinket: Boolean,
    onSelectTrinket: (String) -> Unit,
) {
    var selection by remember(world.seed, challenges, initialDepth) { mutableStateOf<Pair<Int, Boolean>?>(null) }
    var depth by remember(world.seed, challenges, initialDepth) { mutableIntStateOf(initialDepth) }
    LevelMapPanel(world, initialDepth, floors, challenges, changingTrinket, onSelectTrinket,
        animated = selection == null, onExpand = { branch, secrets ->
            depth = initialDepth
            selection = branch to secrets
        })
    selection?.let { (branch, secrets) ->
        Dialog(onDismissRequest = { selection = null },
            properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
            LevelMapPanel(world, depth, floors, challenges, changingTrinket, onSelectTrinket,
                full = true, initialBranch = branch, initialSecrets = secrets,
                close = { selection = null }, navigate = { delta ->
                    floors.getOrNull(floors.indexOf(depth) + delta)?.let { depth = it }
                })
        }
    }
}

/** Inline and expanded maps retain separate loads, branches and native viewports. */
@Composable
@OptIn(ExperimentalMaterial3Api::class)
private fun LevelMapPanel(
    world: ScoutWorld,
    depth: Int,
    floors: List<Int>,
    challenges: Int,
    changingTrinket: Boolean,
    onSelectTrinket: (String) -> Unit,
    full: Boolean = false,
    animated: Boolean = true,
    initialBranch: Int = 0,
    initialSecrets: Boolean = false,
    onExpand: (Int, Boolean) -> Unit = { _, _ -> },
    close: () -> Unit = {},
    navigate: (Int) -> Unit = {},
) {
    var secrets by remember(world.seed, challenges) { mutableStateOf(initialSecrets) }
    val profile = LevelMapRequest(world.seed, depth, challenges, world.selectedTrinket)
    val location = profile.copy(trinket = null)
    // Initial branch applies only when opening the panel, not when navigating floors.
    val initialLocation = remember { location }
    var branch by remember(location) { mutableIntStateOf(if (location == initialLocation) initialBranch else 0) }
    var parent by remember(location) { mutableStateOf<Pair<LevelMapRequest, LevelMapBundle>?>(null) }
    var retry by remember { mutableIntStateOf(0) }
    val request = profile.copy(branch = branch)
    val loaded by produceState<Pair<LevelMapRequest, Result<LevelMapBundle>>?>(null, request, retry) {
        value = null
        value = try {
            val main = if (parent?.first == profile) parent!!.second else LevelMaps.load(profile)
            parent = profile to main
            if (branch != 0 && main.map.branches.none { it.branch == branch }) {
                branch = 0
                null
            } else {
                request to Result.success(if (branch == 0) main else LevelMaps.load(request))
            }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (failure: Throwable) {
            request to Result.failure(failure)
        }
    }
    val current = loaded?.takeIf { it.first == request }?.second
    val bundle = current?.getOrNull()
    val title = if (branch == 0) "Floor $depth layout" else if (bundle?.map?.kind == "imp_vault") "Imp Vault" else "Blacksmith Mine"

    val secretToggle: @Composable () -> Unit = {
        FilterChip(selected = secrets, onClick = { secrets = !secrets },
            enabled = (bundle?.map?.secretCount ?: 0) > 0,
            label = {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Icon(if (secrets) Icons.Filled.Check else Icons.Filled.Close, null, Modifier.size(16.dp))
                    Text("Secrets")
                }
            })
    }
    val toolbar: @Composable () -> Unit = {
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (!parent?.second?.map?.branches.isNullOrEmpty()) {
                    FilterChip(selected = branch == 0, onClick = { branch = 0 }, label = { Text("Main") })
                    parent?.second?.map?.branches?.forEach { area ->
                        FilterChip(selected = branch == area.branch, onClick = { branch = area.branch },
                            modifier = Modifier.semantics { contentDescription = area.label },
                            label = { Text(if (area.kind == "imp_vault") "Vault" else "Mine") })
                    }
                }
            }
            secretToggle()
        }
    }
    val stage: @Composable (Modifier, Boolean) -> Unit = { modifier, full ->
        Box(modifier.background(Color.Black), contentAlignment = Alignment.Center) {
            MapCanvas(bundle, request, secrets, title, full, animated, navigate)
            when {
                current?.isFailure == true -> Column(Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("Couldn’t load this map.", color = Color.White)
                    Text(current.exceptionOrNull()?.message ?: "Map generation failed", color = Color.LightGray,
                        style = MaterialTheme.typography.bodySmall)
                    TextButton(onClick = { retry++; }) { Text("Try again") }
                }
                bundle == null -> Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    CircularProgressIndicator(Modifier.size(24.dp))
                    Text("Charting ${if (branch == 0) "floor $depth" else "the quest level"}…", color = Color.LightGray)
                }
            }
            if (!full) Surface(modifier = Modifier.align(Alignment.TopEnd).padding(6.dp), shape = MaterialTheme.shapes.medium,
                color = MaterialTheme.colorScheme.surfaceContainer.copy(alpha = 0.92f)) {
                TextButton(onClick = { onExpand(branch, secrets) }) { Text("Expand map") }
            }
        }
    }
    if (!full) Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp)) {
        Column {
            toolbar()
            stage(Modifier.fillMaxWidth().height(280.dp), false)
        }
    }
    else {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            Column(Modifier.fillMaxSize().safeDrawingPadding()) {
                FloorHeading(
                    depth = depth,
                    itemCount = world.items.count { it.depth == depth },
                    feeling = world.floorFeelings[depth],
                    questLabel = world.quests.firstOrNull { it.depth == depth }?.variant?.label,
                    farming = world.isFarmingFloor(depth),
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                    onCloseMap = close,
                )
                val hasSublevels = !parent?.second?.map?.branches.isNullOrEmpty()
                Row(
                    Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp, vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = if (hasSublevels) Arrangement.Center else Arrangement.SpaceBetween,
                ) {
                    if (world.trinketOrder.isNotEmpty()) TrinketShortcuts(
                        offers = world.trinketOrder,
                        selectedTrinket = world.selectedTrinket,
                        enabled = !changingTrinket,
                        onSelect = onSelectTrinket,
                    )
                    if (!hasSublevels) secretToggle()
                }
                if (hasSublevels) toolbar()
                Box(Modifier.fillMaxWidth().weight(1f)) {
                    stage(Modifier.fillMaxSize(), true)
                    Surface(
                        modifier = Modifier.align(Alignment.BottomCenter).padding(12.dp),
                        shape = MaterialTheme.shapes.large,
                        color = MaterialTheme.colorScheme.surfaceContainer.copy(alpha = 0.92f),
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            IconButton(onClick = { navigate(-1) }, enabled = floors.indexOf(depth) > 0) {
                                Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, "Previous floor")
                            }
                            VerticalDivider(Modifier.height(24.dp))
                            IconButton(onClick = { navigate(1) }, enabled = floors.indexOf(depth) < floors.lastIndex) {
                                Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, "Next floor")
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun MapCanvas(bundle: LevelMapBundle?, request: LevelMapRequest, secrets: Boolean, label: String, expanded: Boolean, animated: Boolean, navigate: (Int) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val icons = LocalItemIconAtlas.current?.asAndroidBitmap()
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { context -> NativeLevelMapView(context) },
        update = { it.setTooltipTheme(colors, icons); it.bind(bundle, request, secrets, expanded, animated, navigate); it.contentDescription = label },
        onRelease = { it.release() },
    )
}

/** Android gestures, accessibility scrolling and animation lifetime stay native. */
internal class NativeLevelMapView(context: Context) : FrameLayout(context) {
    private var tooltipColors: ColorScheme = lightColorScheme()
    private var tooltipIcons: Bitmap? = null
    fun setTooltipTheme(colors: ColorScheme, icons: Bitmap?) {
        if (tooltipColors != colors || tooltipIcons !== icons) hideItem()
        tooltipColors = colors; tooltipIcons = icons
    }
    private fun itemArtwork(atlas: Bitmap, image: Int, icon: List<Int>?): Bitmap {
        val sx = image % 16 * 16; val sy = image / 16 * 16
        var left = 16; var top = 16; var right = -1; var bottom = -1
        for (y in 0 until 16) for (x in 0 until 16) {
            if (atlas.getPixel(sx + x, sy + y) ushr 24 > 8) {
                left = minOf(left, x); top = minOf(top, y); right = maxOf(right, x); bottom = maxOf(bottom, y)
            }
        }
        if (right < 0) { left = 0; top = 0; right = 15; bottom = 15 }
        val pixels = (32 * resources.displayMetrics.density).toInt().coerceAtLeast(1)
        val result = Bitmap.createBitmap(pixels, pixels, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(result); val paint = Paint().apply { isFilterBitmap = false }
        val scale = pixels / 16f
        val w = (right - left + 1) * scale; val h = (bottom - top + 1) * scale
        canvas.drawBitmap(atlas, Rect(sx + left, sy + top, sx + right + 1, sy + bottom + 1),
            RectF((pixels - w) / 2, (pixels - h) / 2, (pixels + w) / 2, (pixels + h) / 2), paint)
        if (icon != null) tooltipIcons?.let { sheet ->
            canvas.drawBitmap(sheet, Rect(icon[0], icon[1], icon[0] + icon[2], icon[1] + icon[3]),
                RectF(pixels - icon[2] * scale, 0f, pixels.toFloat(), icon[3] * scale), paint)
        }
        return result
    }
    private var inspectedCell: Int? = null
    private var itemCard: ScrollView? = null
    private fun hideItem() { itemCard?.let(::removeView); itemCard = null; inspectedCell = null }
    internal fun inspectItem(x: Float, y: Float) {
        val margin = 18 * resources.displayMetrics.density
        itemCard?.let { if (x >= it.left - margin && x <= it.right + margin && y >= it.top - margin && y <= it.bottom + margin) return }
        val map = bundle?.map ?: return
        val scale = fit() * zoom
        if (scale <= 0) return
        val tip = map.itemAt((x - width / 2f - panX) / scale + map.width * map.tileSize / 2f,
            (y - height / 2f - panY) / scale + map.height * map.tileSize / 2f, secrets)
        if (tip?.cell == inspectedCell) return
        hideItem()
        if (tip == null) return
        inspectedCell = tip.cell
        val density = resources.displayMetrics.density
        fun dp(value: Int) = (value * density).toInt()
        val body = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(14), dp(12), dp(14), dp(12))
        }
        fun text(value: String, size: Float, color: Int, bold: Boolean = false) {
            body.addView(TextView(context).apply {
                this.text = value; textSize = size; setTextColor(color)
                if (bold) setTypeface(typeface, Typeface.BOLD)
                setPadding(0, dp(4), 0, dp(4))
            })
        }
        if (tip.label.isNotEmpty()) {
            text(tip.label, 11f, tooltipColors.onSurfaceVariant.toArgb(), true)
            body.addView(View(context), LinearLayout.LayoutParams(1, dp(12)))
        }
        for (item in tip.items) {
            val heading = LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL; gravity = android.view.Gravity.CENTER_VERTICAL }
            bundle?.textures?.get("items.png")?.let { atlas ->
                val sx = item.image % 16 * 16
                val sy = item.image / 16 * 16
                if (sx + 16 <= atlas.width && sy + 16 <= atlas.height) {
                    heading.addView(ImageView(context).apply {
                        setImageDrawable(BitmapDrawable(resources, itemArtwork(atlas, item.image, item.icon)).apply { isFilterBitmap = false })
                        importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
                    }, LinearLayout.LayoutParams(dp(32), dp(32)).apply { marginEnd = dp(10) })
                }
            }
            heading.addView(TextView(context).apply {
                text = item.name + if (item.quantity > 1) "  ×${item.quantity}" else ""
                textSize = 16f; setTextColor(tooltipColors.onSurface.toArgb()); setTypeface(typeface, Typeface.BOLD)
            }, LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
            body.addView(heading)
            if (!item.deterministic) text("Varies with play", 11f, tooltipColors.onSurfaceVariant.toArgb())
            if (item.description.isNotEmpty()) text(item.description, 12f, tooltipColors.onSurfaceVariant.toArgb())
        }
        val cardWidth = minOf(dp(310), width - dp(16)).coerceAtLeast(1)
        val card = ScrollView(context).apply {
            addView(body)
            elevation = dp(8).toFloat()
            background = GradientDrawable().apply {
                setColor(tooltipColors.surfaceContainer.toArgb()); cornerRadius = 0f; setStroke(dp(1).coerceAtLeast(1), tooltipColors.outlineVariant.toArgb())
            }
            androidx.core.view.ViewCompat.setAccessibilityPaneTitle(this, tip.items.first().name)
        }
        card.measure(MeasureSpec.makeMeasureSpec(cardWidth, MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(minOf(dp(320), height - dp(16)).coerceAtLeast(1), MeasureSpec.AT_MOST))
        val cardHeight = card.measuredHeight
        val left = (x + dp(16)).toInt().coerceIn(dp(8), maxOf(dp(8), width - cardWidth - dp(8)))
        val top = (if (y + dp(16) + cardHeight < height) y + dp(16) else y - cardHeight - dp(16)).toInt().coerceIn(dp(8), maxOf(dp(8), height - cardHeight - dp(8)))
        addView(card, LayoutParams(cardWidth, cardHeight).apply { leftMargin = left; topMargin = top })
        itemCard = card
    }
    override fun onHoverEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_HOVER_ENTER, MotionEvent.ACTION_HOVER_MOVE -> inspectItem(event.x, event.y)
            MotionEvent.ACTION_HOVER_EXIT -> {
                val card = itemCard
                if (card == null || event.x < card.left || event.x > card.right || event.y < card.top || event.y > card.bottom) hideItem()
            }
        }
        return true
    }
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_ESCAPE && itemCard != null) { hideItem(); return true }
        return super.onKeyDown(keyCode, event)
    }
    private var renderer: LevelMapRenderer? = null
    private var bundle: LevelMapBundle? = null
    private var secrets = false
    private var expanded = false
    private var navigate: (Int) -> Unit = {}
    internal var zoom = 1f
        private set
    internal var panX = 0f
        private set
    internal var panY = 0f
        private set
    private var request: LevelMapRequest? = null
    private var active = true
    private var start = SystemClock.uptimeMillis()
    private var animate = true
    private val visible = Rect()
    private val durationObserver = object : ContentObserver(Handler(Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) { updateMotion(); invalidate() }
    }
    private val pinch = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean { parent?.requestDisallowInterceptTouchEvent(true); return true }
        override fun onScale(detector: ScaleGestureDetector): Boolean {
            zoomAt(zoom * detector.scaleFactor, detector.focusX - width / 2f, detector.focusY - height / 2f)
            return true
        }
    })
    private val gestures = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(e: MotionEvent) = true
        override fun onSingleTapConfirmed(e: MotionEvent): Boolean { if (!multiTouch) inspectItem(e.x, e.y); return true }
        override fun onDoubleTap(e: MotionEvent): Boolean {
            zoomAt(if (zoom > 1f) 1f else 2.5f, e.x - width / 2f, e.y - height / 2f)
            return true
        }
        override fun onScroll(e1: MotionEvent?, e2: MotionEvent, distanceX: Float, distanceY: Float): Boolean {
            if (zoom > 1 && !pinch.isInProgress) {
                panX -= distanceX; panY -= distanceY; constrain(); invalidate()
            }
            return true
        }
    })
    private var downX = 0f
    private var downY = 0f
    private var multiTouch = false

    init { setWillNotDraw(false); isFocusable = true; importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_YES }

    fun bind(next: LevelMapBundle?, request: LevelMapRequest, reveal: Boolean, full: Boolean, animated: Boolean, onNavigate: (Int) -> Unit) {
        expanded = full
        navigate = onNavigate
        active = animated
        if (this.request?.hasSameLocation(request) != true) reset()
        this.request = request
        invalidate()
        if (next === bundle && secrets == reveal) return
        hideItem()
        val newMap = next !== bundle
        renderer?.close()
        renderer = next?.let { LevelMapRenderer(it, reveal) }
        bundle = next
        secrets = reveal
        if (newMap) start = SystemClock.uptimeMillis()
        constrain()
        invalidate()
    }
    fun release() { hideItem(); renderer?.close(); renderer = null; bundle = null }
    fun reset() { hideItem(); zoom = 1f; panX = 0f; panY = 0f; invalidate() }
    private fun zoomAt(next: Float, x: Float, y: Float) {
        hideItem()
        val clamped = next.coerceIn(1f, 8f)
        val ratio = clamped / zoom
        panX = x - (x - panX) * ratio
        panY = y - (y - panY) * ratio
        zoom = clamped
        constrain(); invalidate()
    }
    private fun fit(): Float {
        val scene = renderer ?: return 1f
        val fit = minOf(width.toFloat() / scene.width, height.toFloat() / scene.height)
        return if (fit >= 1) floor(fit) else fit
    }
    private fun constrain() {
        val scene = renderer ?: return
        val limitX = ((scene.width * fit() * zoom - width) / 2).coerceAtLeast(0f)
        val limitY = ((scene.height * fit() * zoom - height) / 2).coerceAtLeast(0f)
        panX = panX.coerceIn(-limitX, limitX); panY = panY.coerceIn(-limitY, limitY)
    }
    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) { hideItem(); constrain() }
    override fun onDraw(canvas: Canvas) {
        // Compose's AndroidView does not clip drawing to its layout bounds.
        // Keep drawColor and zoomed particles inside the map, below the toolbar.
        canvas.save()
        canvas.clipRect(0, 0, width, height)
        canvas.drawColor(AndroidColor.BLACK)
        val scene = renderer
        if (scene == null) { canvas.restore(); return }
        val scale = fit() * zoom
        canvas.save()
        canvas.translate((width - scene.width * scale) / 2 + panX, (height - scene.height * scale) / 2 + panY)
        canvas.scale(scale, scale)
        scene.draw(canvas, if (animate) SystemClock.uptimeMillis() - start else 0)
        canvas.restore()
        canvas.restore()
        if (active && scene.animated && animate && windowVisibility == VISIBLE && isShown && getGlobalVisibleRect(visible)) postInvalidateOnAnimation()
    }
    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        context.contentResolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, durationObserver)
        updateMotion(); invalidate()
    }
    override fun onDetachedFromWindow() {
        hideItem()
        context.contentResolver.unregisterContentObserver(durationObserver)
        super.onDetachedFromWindow()
    }
    override fun onWindowVisibilityChanged(visibility: Int) { super.onWindowVisibilityChanged(visibility); if (visibility == VISIBLE) invalidate() }
    private fun updateMotion() { animate = Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) > 0f }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                hideItem()
                downX = event.x; downY = event.y; multiTouch = false
                parent?.requestDisallowInterceptTouchEvent(expanded || zoom > 1f)
            }
            MotionEvent.ACTION_POINTER_DOWN -> { multiTouch = true; parent?.requestDisallowInterceptTouchEvent(true) }
            MotionEvent.ACTION_UP -> {
                val dx = event.x - downX
                val dy = event.y - downY
                if (expanded && zoom == 1f && !multiTouch && abs(dx) > 60 * resources.displayMetrics.density && abs(dx) > abs(dy) * 1.5f) {
                    navigate(if (dx < 0) 1 else -1)
                }
                if (!multiTouch && abs(dx) < 8 && abs(dy) < 8) performClick()
                parent?.requestDisallowInterceptTouchEvent(false)
            }
            MotionEvent.ACTION_CANCEL -> parent?.requestDisallowInterceptTouchEvent(false)
        }
        pinch.onTouchEvent(event)
        gestures.onTouchEvent(event)
        return true
    }
    override fun performClick(): Boolean { super.performClick(); return true }
}
