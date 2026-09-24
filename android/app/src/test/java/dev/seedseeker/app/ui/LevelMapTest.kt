// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Rect
import android.graphics.RectF
import dev.seedseeker.app.engine.JniBindings
import dev.seedseeker.app.engine.LevelMapBundle
import dev.seedseeker.app.engine.LevelMapCodec
import dev.seedseeker.app.engine.LevelMapDocument
import dev.seedseeker.app.engine.LevelMapRequest
import dev.seedseeker.app.engine.LevelMaps
import dev.seedseeker.app.engine.MapCurve
import dev.seedseeker.app.engine.MapDraw
import dev.seedseeker.app.engine.MapEmitter
import dev.seedseeker.app.engine.MapGlow
import dev.seedseeker.app.engine.MapLayer
import dev.seedseeker.app.engine.MapParticle
import dev.seedseeker.app.engine.MapSprite
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35])
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class LevelMapTest {
    private val constant = MapCurve(listOf(0f to 1000f, 1000f to 1000f), false)
    private fun fill(r: Int, g: Int, b: Int, rect: RectF = RectF(0f, 0f, 16f, 16f)) =
        MapDraw(rect, rgba = intArrayOf(r, g, b, 255))
    private fun emitter(cell: Int, x: Float = 8f, wall: Boolean = true, chasm: Boolean = false) = MapEmitter(
        cell, 1000, null, true, wall, chasm, fill(100, 0, 0),
        0f, 0f, 0f, 0f, 0f, constant, constant, null,
        listOf(MapParticle(0, 1000, x, 8f, 1f, 0f)),
    )

    @Test fun gardenShaftsScaleWidthAndHeightIndependently() = runBlocking {
        val bundle = LevelMaps.load(LevelMapRequest("AAA-AAA-AAA", 4, 0, null))
        val shaft = bundle.map.emitters.first { it.scaleX != null }
        val particle = shaft.particles.first()
        val middle = requireNotNull(shaft.state(particle, particle.birth + particle.lifespan / 2))
        assertEquals(2f, middle.scaleX, 0.001f)
        assertEquals(24f, middle.scaleY, 0.001f)

        val e = emitter(0).copy(
            image = fill(100, 0, 0, RectF(0f, 0f, 1f, 1f)),
            scaleX = MapCurve(listOf(0f to 0f, 1000f to 4000f), false),
            scaleY = MapCurve(listOf(0f to 16000f, 1000f to 32000f), false),
        )
        val background = MapSprite(1, listOf(listOf(fill(0, 0, 0))))
        val map = document(listOf(background),
            listOf(MapLayer("terrain", false, intArrayOf(0, 0, 0))), listOf(e))
        val renderer = LevelMapRenderer(LevelMapBundle(map, emptyMap()), true)
        val output = Bitmap.createBitmap(48, 16, Bitmap.Config.ARGB_8888)
        renderer.draw(Canvas(output), 500)
        assertEquals(Color.rgb(100, 0, 0), output.getPixel(7, 0))
        assertEquals(Color.rgb(100, 0, 0), output.getPixel(8, 15))
        assertEquals(Color.BLACK, output.getPixel(6, 8))
        assertEquals(Color.BLACK, output.getPixel(9, 8))
        renderer.close()
    }
    private fun document(sprites: List<MapSprite>, layers: List<MapLayer>, emitters: List<MapEmitter> = emptyList()) = LevelMapDocument(
        "AAA-AAA-AAA", 1, 0, "regular", "test", 3, 1, 16, 1, emptyList(), emptyList(),
        sprites, layers, layers, emitters, emitters,
    )
    private fun render(bundle: LevelMapBundle, time: Long = 0, secrets: Boolean = false): Bitmap {
        val output = Bitmap.createBitmap(bundle.map.width * 16, bundle.map.height * 16, Bitmap.Config.ARGB_8888)
        val renderer = LevelMapRenderer(bundle, secrets)
        renderer.draw(Canvas(output), time)
        renderer.close()
        return output
    }

    @Test fun requestUsesCompletedChallengesAndExplicitNoTrinket() {
        val request = JSONObject(LevelMapRequest("AAA-AAA-AAA", 15, 8 or 256, null, 1).json())
        assertEquals("none", request.getString("trinket"))
        assertEquals(listOf("barren_land", "badder_bosses"), request.getJSONArray("challenges").let { values ->
            List(values.length()) { values.getString(it) }
        })
        assertEquals(1, request.getInt("branch"))
    }

    @Test fun rejectsUnsupportedSchemaBeforeDrawing() {
        assertThrows(IllegalArgumentException::class.java) {
            LevelMapCodec.decode("""{"format":"seed-seeker-level-map","schemaVersion":99}""")
        }
    }

    @Test fun scheduledParticlesDoNotWrapBeforeFirstBirthAndCurvesInterpolate() {
        val curve = MapCurve(listOf(0f to 0f, 1000f to 1000f), true)
        assertEquals(0.5f, curve.value(0.25f), 0.0001f)
        val emitter = emitter(0).copy(start = 2000, vx = 4f, ay = 8f, scaleY = curve)
        val particle = emitter.particles.single().copy(birth = 100, lifespan = 500)
        assertNull(emitter.state(particle, 2099))
        val state = requireNotNull(emitter.state(particle, 2350))
        assertEquals(9f, state.x, 0.0001f)
        assertEquals(8.25f, state.y, 0.0001f)
        assertEquals(kotlin.math.sqrt(0.5f), state.scaleY, 0.0001f)
        assertNull(emitter.state(particle, 2600))
        assertNotNull(emitter.state(particle, 3100))
    }

    @Test fun additiveEffectsUseSceneryAndRespectWallAndDarknessMasks() {
        val sprites = listOf(fill(20, 30, 40), fill(0, 90, 0, RectF(0f, 0f, 8f, 16f)), fill(0, 0, 0))
            .map { MapSprite(1, listOf(listOf(it))) }
        val layers = listOf(
            MapLayer("terrain", false, intArrayOf(0, 0, 0)),
            MapLayer("walls", false, intArrayOf(-1, 1, -1)),
            MapLayer("darkness", false, intArrayOf(-1, -1, 2)),
        )
        val image = render(LevelMapBundle(document(sprites, layers, (0..2).map(::emitter)), emptyMap()))
        assertEquals(Color.rgb(120, 30, 40), image.getPixel(4, 8))
        assertEquals(Color.rgb(0, 90, 0), image.getPixel(18, 8))
        assertEquals(Color.rgb(120, 30, 40), image.getPixel(28, 8))
        assertEquals(Color.BLACK, image.getPixel(40, 8))
    }

    @Test fun chasmParticlesCannotDriftOntoAdjacentFloor() {
        val sprite = MapSprite(1, listOf(listOf(fill(20, 30, 40))))
        val map = document(listOf(sprite), listOf(MapLayer("terrain", false, intArrayOf(0, 0, 0))),
            listOf(emitter(0, x = 24f, chasm = true)))
        val image = render(LevelMapBundle(map, emptyMap()))
        assertEquals(Color.rgb(20, 30, 40), image.getPixel(24, 8))
    }

    @Test fun hiddenScenesUseTheirCompleteAlternativeStackAndEmitterList() {
        val sprites = listOf(fill(255, 0, 0), fill(0, 0, 255)).map { MapSprite(1, listOf(listOf(it))) }
        val map = document(sprites, listOf(MapLayer("terrain", false, intArrayOf(0, 0, 0))), listOf(emitter(0)))
            .copy(concealedLayers = listOf(MapLayer("terrain", false, intArrayOf(1, 1, 1))), concealedEmitters = emptyList())
        assertEquals(Color.BLUE, render(LevelMapBundle(map, emptyMap())).getPixel(8, 8))
        assertEquals(Color.RED, render(LevelMapBundle(map, emptyMap()), secrets = true).getPixel(8, 8))
    }

    @Test fun oneFrameItemGlowUpdatesWithoutChangingSpriteFrames() {
        val texture = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888).apply { eraseColor(Color.rgb(100, 50, 0)) }
        val command = MapDraw(RectF(0f, 0f, 16f, 16f), "item", Rect(0, 0, 1, 1), glow = MapGlow(intArrayOf(255, 0, 0), 1000))
        val map = document(listOf(MapSprite(1, listOf(listOf(command)))), listOf(MapLayer("heaps", false, intArrayOf(0, 0, 0))))
        val bundle = LevelMapBundle(map, mapOf("item" to texture))
        val renderer = LevelMapRenderer(bundle, false)
        val output = Bitmap.createBitmap(48, 16, Bitmap.Config.ARGB_8888)
        renderer.draw(Canvas(output), 0)
        assertEquals(Color.rgb(100, 50, 0), output.getPixel(8, 8))
        renderer.draw(Canvas(output), 1000)
        assertEquals(193, Color.red(output.getPixel(8, 8)))
        assertEquals(20, Color.green(output.getPixel(8, 8)))
        renderer.draw(Canvas(output), 2000)
        assertEquals(Color.rgb(100, 50, 0), output.getPixel(8, 8))
        renderer.close()
    }

    @Test fun nativeV3SceneRendersEmbeddedAssetsAndBossFloor() = runBlocking {
        for ((seed, depth) in listOf("AAA-AAA-AAA" to 1, "AAA-AAA-AAA" to 15, "AAT-TST-BMT" to 17)) {
            val request = LevelMapRequest(seed, depth, 0, null)
            val bundle = LevelMaps.load(request)
            assertTrue(bundle.map.assets.isNotEmpty())
            assertEquals(depth, bundle.map.depth)
            val image = render(bundle, 1250, secrets = true)
            assertTrue((0 until image.width).any { x -> (0 until image.height).any { y -> image.getPixel(x, y) != Color.BLACK } })
            val output = File("build/outputs/level-maps/android-floor-$depth.png")
            output.parentFile?.mkdirs()
            output.outputStream().use { image.compress(Bitmap.CompressFormat.PNG, 100, it) }
            assertSame(bundle, LevelMaps.load(request))
        }
        assertThrows(IllegalArgumentException::class.java) {
            JniBindings.levelMap(LevelMapRequest("AAA-AAA-AAA", 10, 0, null).json().toByteArray())
        }
        Unit
    }

    @Test fun nativeQuestBranchesRenderTheirOwnEmbeddedAssets() = runBlocking {
        for (depths in listOf(12..14, 17..19)) {
            var found = false
            for (depth in depths) {
                val request = LevelMapRequest("AAA-AAA-AAA", depth, 0, null)
                val parent = LevelMaps.load(request)
                val branch = parent.map.branches.firstOrNull() ?: continue
                val bundle = LevelMaps.load(request.copy(branch = branch.branch))
                assertEquals(branch.kind, bundle.map.kind)
                assertTrue(bundle.map.branches.isEmpty())
                val image = render(bundle, 3500)
                val output = File("build/outputs/level-maps/android-${bundle.map.kind}.png")
                output.parentFile?.mkdirs()
                output.outputStream().use { image.compress(Bitmap.CompressFormat.PNG, 100, it) }
                found = true
                break
            }
            assertTrue("The seed must expose the quest branch in $depths", found)
        }
    }
}
