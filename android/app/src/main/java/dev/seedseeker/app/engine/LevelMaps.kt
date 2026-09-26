// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.engine

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Rect
import android.graphics.RectF
import android.util.LruCache
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.sqrt

/** The resolved scout profile is explicit: null means no trinket, never AutoTrinket. */
internal data class LevelMapRequest(
    val seed: String,
    val depth: Int,
    val challenges: Int,
    val trinket: String?,
    val branch: Int = 0,
) {
    fun hasSameLocation(other: LevelMapRequest): Boolean =
        seed == other.seed && depth == other.depth && branch == other.branch && challenges == other.challenges

    fun json(): String = JSONObject().apply {
        put("seed", seed)
        put("depth", depth)
        put("branch", branch)
        put("challenges", JSONArray(EngineInfo.challengeNames.filterKeys { challenges and it != 0 }.values))
        put("trinket", trinket ?: "none")
    }.toString()
}

internal fun isMapDepthSupported(depth: Int) = depth in 1..24 && depth !in setOf(10, 20)

internal data class MapGlow(val color: IntArray, val periodMs: Int)
internal data class MapDraw(
    val destination: RectF,
    val asset: String? = null,
    val source: Rect? = null,
    val rgba: IntArray? = null,
    val opacity: Int = 255,
    val tint: IntArray? = null,
    val glow: MapGlow? = null,
)
internal data class MapSprite(val duration: Int, val frames: List<List<MapDraw>>) {
    fun frame(elapsed: Long): Int = ((elapsed / duration) % frames.size).toInt()
    val glowing = frames.any { frame -> frame.any { it.glow != null } }
}
internal data class MapLayer(val name: String, val additive: Boolean, val cells: IntArray)
internal data class MapCurve(val points: List<Pair<Float, Float>>, val squareRoot: Boolean) {
    fun value(progress: Float): Float {
        val p = progress.coerceIn(0f, 1f) * 1000f
        val right = points.indexOfFirst { it.first >= p }
        val value = when {
            right < 0 -> points.last().second
            right == 0 -> points.first().second
            else -> {
                val (x0, y0) = points[right - 1]
                val (x1, y1) = points[right]
                y0 + (y1 - y0) * (p - x0) / (x1 - x0)
            }
        } / 1000f
        return if (squareRoot) sqrt(value.coerceAtLeast(0f)) else value
    }
}
internal data class MapParticle(val birth: Long, val lifespan: Long, val x: Float, val y: Float, val scale: Float, val angle: Float)
internal data class ParticleState(val x: Float, val y: Float, val scale: Float, val scaleX: Float, val scaleY: Float, val alpha: Float, val angle: Float)
internal data class MapEmitter(
    val cell: Int, val loop: Long, val start: Long?, val additive: Boolean,
    val wallMask: Boolean, val chasm: Boolean, val image: MapDraw,
    val vx: Float, val vy: Float, val ax: Float, val ay: Float, val angularSpeed: Float,
    val alpha: MapCurve, val scale: MapCurve, val scaleY: MapCurve?, val particles: List<MapParticle>,
    val scaleX: MapCurve? = null,
) {
    fun state(particle: MapParticle, elapsed: Long): ParticleState? {
        val clock = elapsed.coerceAtLeast(0) - (start ?: 0)
        if (start != null && clock < particle.birth) return null
        val age = ((clock - particle.birth) % loop + loop) % loop
        if (age >= particle.lifespan) return null
        val seconds = age / 1000f
        val progress = age.toFloat() / particle.lifespan
        return ParticleState(
            particle.x + vx * seconds + ax * seconds * seconds / 2,
            particle.y + vy * seconds + ay * seconds * seconds / 2,
            particle.scale * scale.value(progress), scaleX?.value(progress) ?: 1f, scaleY?.value(progress) ?: 1f,
            alpha.value(progress), particle.angle + angularSpeed * seconds,
        )
    }
}
internal data class MapBranch(val branch: Int, val kind: String) {
    val label get() = if (kind == "imp_vault") "Imp Vault" else "Blacksmith Mine"
}
internal data class MapTooltipItem(
    val name: String, val description: String, val image: Int, val quantity: Int, val deterministic: Boolean,
    val icon: List<Int>? = null, val upgrade: Int? = null, val cursed: Boolean = false,
    val enchantment: String? = null, val curse: String? = null, val glow: MapGlow? = null,
)
internal data class MapItemTooltip(val cell: Int, val label: String, val hidden: Boolean, val items: List<MapTooltipItem>, val bounds: List<Int>? = null)
internal data class LevelMapDocument(
    val seed: String, val depth: Int, val branch: Int, val kind: String,
    val revision: String, val width: Int, val height: Int, val tileSize: Int,
    val secretCount: Int, val branches: List<MapBranch>, val assets: List<String>,
    val sprites: List<MapSprite>, val layers: List<MapLayer>, val concealedLayers: List<MapLayer>,
    val emitters: List<MapEmitter>, val concealedEmitters: List<MapEmitter>,
    val itemTooltips: List<MapItemTooltip> = emptyList(),
) {
    fun itemAt(x: Float, y: Float, secrets: Boolean): MapItemTooltip? {
        if (x < 0 || y < 0 || x >= width * tileSize || y >= height * tileSize) return null
        return itemTooltips.firstOrNull { tip ->
            val bounds = tip.bounds ?: listOf(0, 0, tileSize, tileSize)
            val left = (tip.cell % width) * tileSize + bounds[0]
            val top = (tip.cell / width) * tileSize + bounds[1]
            (secrets || !tip.hidden) && x >= left && y >= top && x < left + bounds[2] && y < top + bounds[3]
        }
    }
}
internal data class LevelMapBundle(val map: LevelMapDocument, val textures: Map<String, Bitmap>)

/** All geometry, objects and hazard schedules come from the scene, never loot metadata. */
internal object LevelMapCodec {
    fun decode(text: String): LevelMapDocument {
        val root = JSONObject(text)
        require(root.getString("format") == "seed-seeker-level-map" && root.getInt("schemaVersion") == 3) {
            "Unsupported level map format"
        }
        val width = root.getInt("width")
        val height = root.getInt("height")
        require(width in 1..256 && height in 1..256) { "Invalid map dimensions" }
        val scene = root.getJSONObject("scene")
        require(scene.getInt("tileSize") == 16) { "Unsupported map tile size" }
        val sprites = scene.getJSONArray("sprites").objects { sprite ->
            val duration = sprite.getInt("frameDurationMs")
            val frames = sprite.getJSONArray("frames").arrays { frame -> frame.objects(::draw) }
            require(duration > 0 && frames.isNotEmpty()) { "Invalid sprite animation" }
            MapSprite(duration, frames)
        }
        fun layers(name: String) = scene.getJSONArray(name).objects { layer ->
            val cells = layer.getJSONArray("cells")
            require(cells.length() == width * height) { "Invalid map layer size" }
            MapLayer(layer.getString("name"), layer.optString("blend") == "add", IntArray(cells.length()) { i ->
                if (cells.isNull(i)) -1 else cells.getInt(i).also { require(it in sprites.indices) }
            })
        }
        return LevelMapDocument(
            root.getString("seed"), root.getInt("depth"), root.getInt("branch"), root.getString("kind"),
            root.getString("assetRevision"), width, height, 16,
            listOf("secretRooms", "secretDoors", "secretTraps").sumOf { root.getJSONArray(it).length() },
            root.getJSONArray("branches").objects { MapBranch(it.getInt("branch"), it.getString("kind")) },
            root.getJSONArray("assets").objects { it.getString("id") },
            sprites, layers("layers"), layers("concealedLayers"),
            scene.getJSONArray("emitters").objects(::emitter), scene.getJSONArray("concealedEmitters").objects(::emitter),
            root.optJSONArray("itemTooltips")?.objects { tip ->
                MapItemTooltip(tip.getInt("cell"), tip.optString("label"), tip.optBoolean("hidden"),
                    tip.getJSONArray("items").objects { item ->
                        MapTooltipItem(item.getString("name"), item.optString("description"), item.getInt("image"), item.getInt("quantity"), item.optBoolean("deterministic", true),
                            icon = item.optJSONArray("icon")?.let { a -> List(4) { a.getInt(it) } },
                            upgrade = if (item.isNull("upgrade")) null else item.getInt("upgrade"),
                            cursed = item.optBoolean("cursed"),
                            enchantment = if (item.isNull("enchantment")) null else item.getString("enchantment"),
                            curse = if (item.isNull("curse")) null else item.getString("curse"),
                            glow = item.optJSONObject("glow")?.let { MapGlow(it.getJSONArray("color").ints(), it.getInt("periodMs")) })
                    }, tip.optJSONArray("bounds")?.let { a -> List(4) { a.getInt(it) } })
            } ?: emptyList(),
        )
    }

    private fun draw(json: JSONObject): MapDraw {
        val d = json.getJSONArray("destination")
        val destination = RectF(d.f(0), d.f(1), d.f(0) + d.f(2), d.f(1) + d.f(3))
        return when (json.getString("kind")) {
            "fill" -> MapDraw(destination, rgba = json.getJSONArray("rgba").ints())
            "blit" -> {
                val s = json.getJSONArray("source")
                MapDraw(destination, json.getString("asset"), Rect(s.getInt(0), s.getInt(1), s.getInt(0) + s.getInt(2), s.getInt(1) + s.getInt(3)),
                    opacity = json.optInt("opacity", 255), tint = json.optJSONArray("tint")?.ints(),
                    glow = json.optJSONObject("glow")?.let { MapGlow(it.getJSONArray("color").ints(), it.getInt("periodMs")) })
            }
            else -> error("Unsupported map drawing command")
        }
    }
    private fun curve(json: JSONObject) = MapCurve(json.getJSONArray("points").arrays { it.f(0) to it.f(1) }.also {
        require(it.isNotEmpty())
    }, json.optBoolean("sqrt"))
    private fun emitter(json: JSONObject): MapEmitter {
        val v = json.getJSONArray("velocity")
        val a = json.getJSONArray("acceleration")
        return MapEmitter(
            json.getInt("cell"), json.getLong("loopMs").also { require(it > 0) },
            if (json.has("startMs")) json.getLong("startMs") else null,
            json.optString("blend") == "add", json.optBoolean("wallMask"), json.optBoolean("clipToChasm"),
            draw(json.getJSONObject("image")), v.f(0), v.f(1), a.f(0), a.f(1), json.getDouble("angularSpeed").toFloat(),
            curve(json.getJSONObject("alpha")), curve(json.getJSONObject("scale")), json.optJSONObject("scaleY")?.let(::curve),
            scaleX = json.optJSONObject("scaleX")?.let(::curve),
            particles = json.getJSONArray("particles").objects {
                val p = it.getJSONArray("position")
                MapParticle(it.getLong("birthMs"), it.getLong("lifespanMs").also { lifespan -> require(lifespan > 0) },
                    p.f(0) / 1000, p.f(1) / 1000, it.getDouble("scale").toFloat() / 1000, it.getDouble("angle").toFloat())
            },
        )
    }
    private fun JSONArray.f(index: Int) = getDouble(index).toFloat()
    private fun JSONArray.ints() = IntArray(length()) { getInt(it) }
    private fun <T> JSONArray.objects(block: (JSONObject) -> T) = List(length()) { block(getJSONObject(it)) }
    private fun <T> JSONArray.arrays(block: (JSONArray) -> T) = List(length()) { block(getJSONArray(it)) }
}

/** JNI and decoding never run on the UI thread. Bounded caches share embedded atlases. */
internal object LevelMaps {
    private val lock = Mutex()
    private val documents = LruCache<LevelMapRequest, LevelMapBundle>(6)
    private val assets = object : LruCache<String, Bitmap>(24 * 1024 * 1024) {
        override fun sizeOf(key: String, value: Bitmap) = value.allocationByteCount
    }

    suspend fun load(request: LevelMapRequest): LevelMapBundle = withContext(Dispatchers.Default) {
        lock.withLock {
            documents.get(request)?.let { return@withLock it }
            val map = LevelMapCodec.decode(JniBindings.levelMap(request.json().toByteArray(Charsets.UTF_8)).toString(Charsets.UTF_8))
            check(map.seed == request.seed && map.depth == request.depth && map.branch == request.branch) { "Map profile mismatch" }
            val textures = map.assets.associateWith { id ->
                val key = "${map.revision}:$id"
                assets.get(key) ?: JniBindings.levelMapAsset(id.toByteArray(Charsets.UTF_8)).let { bytes ->
                    requireNotNull(BitmapFactory.decodeByteArray(bytes, 0, bytes.size)) { "Could not decode map texture" }
                        .also { assets.put(key, it) }
                }
            }
            LevelMapBundle(map, textures).also { documents.put(request, it) }
        }
    }
}
