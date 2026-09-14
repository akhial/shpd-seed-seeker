// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ColorMatrixColorFilter
import android.graphics.Paint
import android.graphics.Path
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import dev.seedseeker.app.engine.LevelMapBundle
import dev.seedseeker.app.engine.MapDraw
import dev.seedseeker.app.engine.MapEmitter
import dev.seedseeker.app.engine.MapLayer

/** Nearest-neighbour scene rendering, with dirty tiles and display-rate effects. */
internal class LevelMapRenderer(private val bundle: LevelMapBundle, secrets: Boolean) {
    private val map = bundle.map
    private val layers = if (secrets) map.layers else map.concealedLayers
    private val emitters = if (secrets) map.emitters else map.concealedEmitters
    val width = map.width * map.tileSize
    val height = map.height * map.tileSize
    val animated = map.sprites.any { it.frames.size > 1 || it.glowing } || emitters.isNotEmpty()
    private val paint = Paint().apply { isFilterBitmap = false; isAntiAlias = false }
    private val copyPaint = Paint().apply { isFilterBitmap = false }
    private val add = PorterDuffXfermode(PorterDuff.Mode.ADD)
    private val maskIn = PorterDuffXfermode(PorterDuff.Mode.DST_IN)
    private val scenery = bitmap()
    private val sceneryCanvas = Canvas(scenery)
    private val walls = bitmap()
    private val wallScenery = bitmap()
    private val wallSceneryCanvas = Canvas(wallScenery)
    private val lastFrames = IntArray(map.sprites.size) { -1 }
    private var lastTime = -1L
    private val chasms = Path().apply {
        emitters.filter { it.chasm }.forEach {
            val x = (it.cell % map.width) * map.tileSize.toFloat()
            val y = (it.cell / map.width) * map.tileSize.toFloat()
            addRect(x, y, x + map.tileSize, y + map.tileSize, Path.Direction.CW)
        }
    }

    init {
        val mask = Canvas(walls)
        layers.filter { it.name in setOf("raised", "walls", "room_walls", "boss_walls") }
            .forEach { drawLayer(mask, it, 0) }
    }

    private fun bitmap() = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)

    fun draw(target: Canvas, elapsed: Long) {
        updateScenery(elapsed)
        if (emitters.isEmpty()) {
            target.drawBitmap(scenery, 0f, 0f, copyPaint)
            return
        }
        // Additive particles blend with real scenery pixels, including in overlapping emitters.
        target.drawBitmap(scenery, 0f, 0f, copyPaint)
        target.save()
        target.clipPath(chasms)
        emitters.filter { it.chasm }.forEach { drawEmitter(target, it, elapsed) }
        target.restore()
        emitters.filter { it.wallMask && !it.chasm }.forEach { drawEmitter(target, it, elapsed) }
        target.drawBitmap(wallScenery, 0f, 0f, copyPaint)
        emitters.filter { !it.wallMask && !it.chasm }.forEach { drawEmitter(target, it, elapsed) }
        // Status icons also obey the engine's geometric darkness.
        layers.filter { it.name == "darkness" }.forEach { drawLayer(target, it, elapsed) }
    }

    private fun updateScenery(elapsed: Long) {
        val frames = IntArray(map.sprites.size) { map.sprites[it].frame(elapsed) }
        val changed = BooleanArray(frames.size) {
            frames[it] != lastFrames[it] || (map.sprites[it].glowing && lastTime != elapsed)
        }
        var repaint = false
        for (cell in 0 until map.width * map.height) {
            if (lastTime >= 0 && layers.none { it.cells[cell] >= 0 && changed[it.cells[cell]] }) continue
            repaint = true
            val x = (cell % map.width) * map.tileSize.toFloat()
            val y = (cell / map.width) * map.tileSize.toFloat()
            sceneryCanvas.save()
            sceneryCanvas.translate(x, y)
            sceneryCanvas.clipRect(0f, 0f, map.tileSize.toFloat(), map.tileSize.toFloat())
            sceneryCanvas.drawColor(Color.BLACK)
            for (layer in layers) {
                val sprite = layer.cells[cell]
                if (sprite >= 0) map.sprites[sprite].frames[frames[sprite]].forEach {
                    drawCommand(sceneryCanvas, it, elapsed, layer.additive)
                }
            }
            sceneryCanvas.restore()
        }
        frames.copyInto(lastFrames)
        lastTime = elapsed
        if (repaint && emitters.isNotEmpty()) {
            wallSceneryCanvas.drawBitmap(scenery, 0f, 0f, copyPaint)
            paint.colorFilter = null
            paint.alpha = 255
            paint.xfermode = maskIn
            wallSceneryCanvas.drawBitmap(walls, 0f, 0f, paint)
            paint.xfermode = null
        }
    }

    private fun drawLayer(canvas: Canvas, layer: MapLayer, elapsed: Long) {
        layer.cells.forEachIndexed { cell, sprite ->
            if (sprite < 0) return@forEachIndexed
            canvas.save()
            canvas.translate((cell % map.width) * map.tileSize.toFloat(), (cell / map.width) * map.tileSize.toFloat())
            val film = map.sprites[sprite]
            film.frames[film.frame(elapsed)].forEach { drawCommand(canvas, it, elapsed, layer.additive) }
            canvas.restore()
        }
    }

    private fun drawEmitter(canvas: Canvas, emitter: MapEmitter, elapsed: Long) {
        val x = (emitter.cell % map.width) * map.tileSize.toFloat()
        val y = (emitter.cell / map.width) * map.tileSize.toFloat()
        for (particle in emitter.particles) {
            val state = emitter.state(particle, elapsed) ?: continue
            if (state.scale <= 0 || state.alpha <= 0) continue
            canvas.save()
            canvas.translate(x + state.x, y + state.y)
            canvas.rotate(state.angle)
            canvas.scale(state.scale, state.scale * state.scaleY)
            val destination = emitter.image.destination
            canvas.translate(-destination.centerX(), -destination.centerY())
            drawCommand(canvas, emitter.image, elapsed, emitter.additive, state.alpha)
            canvas.restore()
        }
    }

    private fun drawCommand(canvas: Canvas, draw: MapDraw, elapsed: Long, additive: Boolean, alpha: Float = 1f) {
        paint.xfermode = if (additive) add else null
        paint.colorFilter = null
        if (draw.asset == null) {
            val rgba = requireNotNull(draw.rgba)
            paint.color = Color.argb((rgba[3] * alpha).toInt().coerceIn(0, 255), rgba[0], rgba[1], rgba[2])
            canvas.drawRect(draw.destination, paint)
            return
        }
        paint.alpha = (draw.opacity * alpha).toInt().coerceIn(0, 255)
        if (draw.tint != null || draw.glow != null) {
            val phase = draw.glow?.let { (elapsed.toDouble() / it.periodMs % 2).toFloat() } ?: 0f
            val glow = minOf(phase, 2 - phase) * 0.6f
            val matrix = FloatArray(20)
            for (axis in 0..2) {
                matrix[axis * 6] = (draw.tint?.get(axis)?.div(255f) ?: 1f) * (1 - glow)
                matrix[axis * 5 + 4] = (draw.glow?.color?.get(axis) ?: 0) * glow
            }
            matrix[18] = 1f
            paint.colorFilter = ColorMatrixColorFilter(matrix)
        }
        canvas.drawBitmap(bundle.textures.getValue(draw.asset), draw.source, draw.destination, paint)
    }

    fun close() {
        listOf(scenery, walls, wallScenery).forEach(Bitmap::recycle)
    }
}
