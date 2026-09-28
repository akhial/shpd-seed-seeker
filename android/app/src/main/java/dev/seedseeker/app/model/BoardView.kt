// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import org.json.JSONArray
import org.json.JSONObject

/**
 * Everything the requirement board draws, as the shared core folds and words
 * the rows ([RequirementEditor]): the board's entries in list order, both
 * sections together, how many entries each section shows, everything wrong
 * with the list, and the Arcane Resin chip. The app lays these out and
 * derives none of them.
 */
data class BoardView(
    val items: List<BoardItemView>,
    /** Entries the ordinary section shows; a cluster or a stack counts once. */
    val ordinaryCount: Int,
    /** Entries the blanket section shows. */
    val blanketCount: Int,
    /** Everything wrong with the list: each row's own, then between rows, then the list's. */
    val problems: List<RequirementProblem>,
    /** The Arcane Resin chip, when the board was asked with the query's resin. */
    val resin: ResinChipView?,
) {
    /** The entry showing the visible row [key]. */
    fun itemOf(key: Long): BoardItemView? = items.firstOrNull { key in it.members }

    companion object {
        /** A board the editor could not answer: nothing to draw, and [reason] as its problem. */
        fun unavailable(reason: String) =
            BoardView(emptyList(), 0, 0, listOf(RequirementProblem(reason, emptyList())), null)

        internal fun decode(answer: JSONObject): BoardView {
            val counts = answer.getJSONObject("counts")
            return BoardView(
                items = answer.getJSONArray("items").objects().map(::decodeItem),
                ordinaryCount = counts.getInt("ordinary"),
                blanketCount = counts.getInt("blanket"),
                problems = answer.getJSONArray("problems").objects().map {
                    RequirementProblem(it.getString("message"), it.getJSONArray("keys").longs())
                },
                resin = answer.objectOrNull("resin")?.let { resin ->
                    ResinChipView(
                        name = resin.getString("name"),
                        tags = resin.getJSONArray("tags").objects().map(::decodeTag),
                        uncursed = resin.getBoolean("uncursed"),
                        description = resin.getString("description"),
                    )
                },
            )
        }

        private fun decodeItem(item: JSONObject): BoardItemView {
            val stack = item.getJSONObject("stack")
            val badges = item.getJSONObject("badges")
            return BoardItemView(
                blanket = item.getBoolean("blanket"),
                cluster = item.intOrNull("cluster"),
                members = item.getJSONArray("members").longs(),
                count = stack.getInt("count"),
                total = stack.intOrNull("total"),
                copyDepth = stack.intOrNull("copy_depth"),
                countBadge = badges.objectOrNull("count")?.let(::decodeBadge),
                totalBadge = badges.objectOrNull("total")?.let(::decodeBadge),
                chips = item.getJSONArray("chips").objects().map(::decodeChip),
            )
        }

        private fun decodeBadge(badge: JSONObject) =
            BadgeView(badge.getString("text"), badge.getString("compact_text"), badge.getString("tooltip"))

        private fun decodeChip(chip: JSONObject) = ChipView(
            key = chip.getLong("key"),
            name = chip.getString("name"),
            item = chip.stringOrNull("item")?.let(ItemCatalog::findById),
            kind = chip.stringOrNull("kind")?.let { kind -> ItemKind.entries.firstOrNull { it.name.lowercase() == kind } },
            tags = chip.getJSONArray("tags").objects().map(::decodeTag),
            trailingTags = chip.getJSONArray("trailing_tags").objects().map(::decodeTag),
            effect = chip.objectOrNull("effect")?.let { effect ->
                EffectView(
                    label = effect.getString("label"),
                    effects = effect.getJSONArray("effects").let { names -> List(names.length(), names::getString) },
                    anyEnchantment = effect.getBoolean("any_enchantment"),
                )
            },
            uncursed = chip.getBoolean("uncursed"),
            description = chip.getString("description"),
            problem = chip.stringOrNull("problem"),
            canDetach = chip.getBoolean("can_detach"),
            join = chip.getJSONArray("join").longs().toSet(),
            refuse = chip.getJSONArray("refuse").objects().associate { it.getLong("key") to it.getString("message") },
        )

        private fun decodeTag(tag: JSONObject) = TagView(tag.getString("text"), upgrade = tag.getString("style") == "upgrade")
    }
}

/** One board entry: a chip, or an either/or cluster of chips, with the stack behind it. */
data class BoardItemView(
    /** Whether the entry sits in the blanket section. */
    val blanket: Boolean,
    /** The cluster's alternative label; null for a lone chip. */
    val cluster: Int?,
    /** The visible rows' keys, the anchor first. */
    val members: List<Long>,
    /** How many items the entry asks for, its hidden copies included. */
    val count: Int,
    /** The combined level the stack's items reach together, when it counts levels. */
    val total: Int?,
    /** The floor limit the stack's hidden copies keep to. */
    val copyDepth: Int?,
    /** `×3`, or `≤3` while counting levels, when the entry asks for more than one item. */
    val countBadge: BadgeView?,
    /** `Σ ≥ 5`, when the stack counts levels. */
    val totalBadge: BadgeView?,
    /** One chip per member. */
    val chips: List<ChipView>,
) {
    /** The row the badges and the editor act on. */
    val anchor: Long get() = members.first()
}

/** A stack badge: its text, the shorter text for compact chips, and what it means in words. */
data class BadgeView(val text: String, val compactText: String, val tooltip: String)

/** One visible row, as its chip draws it. */
data class ChipView(
    val key: Long,
    /** The short name beside the sprite: the item, or the wildcard (`Any melee`). */
    val name: String,
    /** The item the sprite draws; null for a wildcard. */
    val item: CatalogItem?,
    /** The kind a wildcard's sprite stands for; null only for a row the editor could not read. */
    val kind: ItemKind?,
    /** The qualifiers after the name, in order. */
    val tags: List<TagView>,
    /** The qualifiers after the effect cue (`No resin`). */
    val trailingTags: List<TagView>,
    /** The effect the item must carry; null for any effect. */
    val effect: EffectView?,
    /** Whether cursed items are ruled out. */
    val uncursed: Boolean,
    /** What a screen reader says for the chip: its title, then its details. */
    val description: String,
    /** The first problem the chip carries, its hidden copies' included. */
    val problem: String?,
    /** Whether the chip is a cluster member, which can be taken out on its own. */
    val canDetach: Boolean,
    /** The visible rows this chip may join as an either/or alternative. */
    val join: Set<Long>,
    /** The visible rows a join onto is refused, with the reason in words. */
    val refuse: Map<Long, String>,
)

/** A qualifier beside a chip's name; [upgrade] marks an upgrade's own tint. */
data class TagView(val text: String, val upgrade: Boolean)

/**
 * The effect cue: [label] in words (`any enchantment`, `effect: A/B`), and
 * [effects] in catalog order — every enchantment when [anyEnchantment].
 */
data class EffectView(val label: String, val effects: List<String>, val anyEnchantment: Boolean)

/** One problem with the list, blaming the rows [keys] (none for the list's own). */
data class RequirementProblem(val message: String, val keys: List<Long>)

/** The Arcane Resin chip: its name, tags (`Auto` or `≥N`, `Mage +2`, `F≤N`), and what a screen reader says. */
data class ResinChipView(val name: String, val tags: List<TagView>, val uncursed: Boolean, val description: String)

private fun JSONArray.objects(): List<JSONObject> = List(length(), this::getJSONObject)

private fun JSONArray.longs(): List<Long> = List(length(), this::getLong)

private fun JSONObject.objectOrNull(name: String): JSONObject? = if (isNull(name)) null else getJSONObject(name)

private fun JSONObject.stringOrNull(name: String): String? = if (isNull(name)) null else getString(name)

private fun JSONObject.intOrNull(name: String): Int? = if (isNull(name)) null else getInt(name)
