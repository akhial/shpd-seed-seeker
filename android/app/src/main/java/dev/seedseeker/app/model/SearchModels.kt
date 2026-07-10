// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

enum class ItemKind(
    val label: String,
    val modifierLabel: String?,
    val maximumSearchUpgrade: Int,
) {
    WEAPON("Weapons", "Enchantment", 3),
    ARMOR("Armor", "Glyph", 3),
    WAND("Wands", null, 3),
    RING("Rings", null, 4),
}

data class CatalogItem(
    val id: String,
    val name: String,
    val kind: ItemKind,
    val spriteIndex: Int,
    val tier: Int? = null,
)

data class ItemRequirement(
    val key: Long,
    val item: CatalogItem,
    val upgrade: Int,
    val modifier: String? = null,
) {
    init {
        require(upgrade in 1..item.kind.maximumSearchUpgrade) {
            "Upgrade requirement must be +1..+${item.kind.maximumSearchUpgrade} for ${item.kind.label}"
        }
        require(item.kind.modifierLabel != null || modifier == null) {
            "${item.kind.label} cannot carry a modifier requirement"
        }
    }

    val description: String
        get() = buildString {
            append("+")
            append(upgrade)
            append(" exactly")
            modifier?.let {
                append(" • ")
                append(it)
            }
        }
}

data class SearchRequest(val requirements: List<ItemRequirement>) {
    init {
        require(requirements.isNotEmpty()) { "At least one requirement is needed" }
    }
}

data class SeedResult(
    val seed: String,
    val matchedRequirements: Int,
)

data class ScoutWorld(
    val seed: String,
    val items: List<ScoutItem>,
)

data class ScoutItem(
    val item: CatalogItem,
    val depth: Int,
    val upgrade: Int,
    val effect: String?,
    val cursed: Boolean,
    val source: ScoutItemSource,
    val accessibility: ScoutAccessibility,
)

enum class ScoutItemSource(val label: String) {
    HEAP("Heap"),
    CHEST("Chest"),
    LOCKED_CHEST("Locked chest"),
    CRYSTAL_CHEST("Crystal chest"),
    TOMB("Tomb"),
    SKELETON("Skeleton"),
    SACRIFICIAL_FIRE("Sacrificial fire"),
    MIMIC("Mimic"),
    GOLDEN_MIMIC("Golden mimic"),
    CRYSTAL_MIMIC("Crystal mimic"),
    STATUE("Statue"),
    ARMORED_STATUE("Armored statue"),
    SHOP("Shop"),
    GHOST_REWARD("Ghost reward"),
    WANDMAKER_REWARD("Wandmaker reward"),
    BLACKSMITH_REWARD("Blacksmith reward"),
    IMP_REWARD("Imp reward"),
}

sealed interface ScoutAccessibility {
    data object Independent : ScoutAccessibility

    data class Choice(
        val group: Int,
        val option: Int,
    ) : ScoutAccessibility

    data class Scenarios(
        val group: Int,
        val mask: ULong,
    ) : ScoutAccessibility
}

enum class SearchState {
    RUNNING,
    COMPLETED,
    CANCELLED,
    FAILED,
}

data class SearchStatus(
    val state: SearchState,
    val scannedSeeds: Long,
    val totalSeeds: Long,
    val errorCode: Long = 0,
)

data class SearchBatch(val results: List<SeedResult>)
