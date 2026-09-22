// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import org.json.JSONArray
import org.json.JSONObject

val FARMING_FLOORS = listOf(7, 17, 22)

data class FloorRequirement(
    val depth: Int,
    val feeling: FloorFeeling? = null,
    val rooms: List<String> = emptyList(),
    val anyRooms: List<String> = emptyList(),
) {
    init {
        require(depth in 1..SearchLimits.MAX_DEPTH && depth % 5 != 0) { "Choose a regular floor from 1 through 24." }
        require(feeling != null || rooms.isNotEmpty() || anyRooms.isNotEmpty()) { "Choose a feeling or room for each floor." }
    }

    val isFarming: Boolean get() = depth in FARMING_FLOORS && feeling == FloorFeeling.DARK &&
        rooms.isEmpty() && anyRooms.size == 2 && anyRooms.toSet() == setOf("garden", "secret_garden")

    val description: String get() = listOfNotNull(
        "Floor $depth", feeling?.label,
        rooms.takeIf { it.isNotEmpty() }?.joinToString(", ") { it.replace('_', ' ') },
        anyRooms.takeIf { it.isNotEmpty() }?.joinToString(" / ") { it.replace('_', ' ') },
    ).joinToString(" · ")
}

fun List<FloorRequirement>.floorValidationProblem(maximumDepth: Int): String? = when {
    map { it.depth }.distinct().size != size -> "Each floor can have only one requirement."
    any { it.depth > maximumDepth } -> "Floor ${first { it.depth > maximumDepth }.depth} exceeds the floor limit of $maximumDepth."
    else -> null
}

fun PresetQuery.toggleFarmingFloor(depth: Int): PresetQuery {
    require(depth in FARMING_FLOORS)
    val selected = floorRequirements.any { it.depth == depth && it.isFarming }
    return copy(
        maximumDepth = if (selected) maximumDepth else maxOf(maximumDepth, depth),
        floorRequirements = (floorRequirements.filterNot { it.depth == depth } +
            if (selected) emptyList() else listOf(FloorRequirement(depth, FloorFeeling.DARK,
                anyRooms = listOf("garden", "secret_garden")))).sortedBy { it.depth },
    )
}

internal fun encodeFloors(value: JSONObject, floors: List<FloorRequirement>) {
    if (floors.isEmpty()) return
    value.put("floor_requirements", JSONArray().apply {
        floors.forEach { floor -> put(JSONObject().apply {
            put("depth", floor.depth)
            floor.feeling?.let { put("feeling", it.name.lowercase()) }
            if (floor.rooms.isNotEmpty()) put("rooms", JSONArray(floor.rooms))
            if (floor.anyRooms.isNotEmpty()) put("any_rooms", JSONArray(floor.anyRooms))
        }) }
    })
}

internal fun decodeFloors(value: JSONObject): List<FloorRequirement> {
    if (!value.has("floor_requirements")) return emptyList()
    val floors = value.getJSONArray("floor_requirements")
    fun rooms(floor: JSONObject, key: String): List<String> = if (!floor.has(key)) emptyList()
        else floor.getJSONArray(key).let { list -> List(list.length()) { list.getString(it) } }
    return List(floors.length()) { index ->
        val floor = floors.getJSONObject(index)
        val depth = floor.get("depth")
        require(depth is Number && depth.toDouble() == depth.toInt().toDouble()) { "Invalid floor depth." }
        FloorRequirement(depth.toInt(),
            if (floor.has("feeling")) FloorFeeling.valueOf(floor.getString("feeling").uppercase()) else null,
            rooms(floor, "rooms"), rooms(floor, "any_rooms"))
    }.also { floors ->
        require(floors.map { it.depth }.distinct().size == floors.size) { "Each floor can have only one requirement." }
    }
}
