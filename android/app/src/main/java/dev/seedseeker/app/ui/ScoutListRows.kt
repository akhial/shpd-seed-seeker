// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import dev.seedseeker.app.engine.isMapDepthSupported
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ScoutItem

/** Each card is a lazy item, so a crowded floor does not compose all of its loot at once. */
internal sealed class ScoutListRow(val key: String) {
    data object ArtifactDeck : ScoutListRow("artifact-deck")
    class Heading(val depth: Int, val itemCount: Int) : ScoutListRow("floor-$depth")
    class Map(val depth: Int) : ScoutListRow("floor-map-$depth")
    class Empty(val depth: Int) : ScoutListRow("floor-empty-$depth")
    class Trinkets(val depth: Int, val choices: List<IndexedValue<ScoutItem>>) : ScoutListRow("floor-trinkets-$depth")
    // Keep the engine's index: identical items can have different match and choice marks.
    class Item(val indexedItem: IndexedValue<ScoutItem>) : ScoutListRow("item-${indexedItem.index}")
}

internal fun scoutListRows(
    floors: Map<Int, List<IndexedValue<ScoutItem>>>,
    hasArtifactDeck: Boolean,
    openMapDepth: Int?,
): List<ScoutListRow> = buildList {
    if (hasArtifactDeck) add(ScoutListRow.ArtifactDeck)
    floors.forEach { (depth, items) ->
        add(ScoutListRow.Heading(depth, items.size))
        if (openMapDepth == depth && isMapDepthSupported(depth)) add(ScoutListRow.Map(depth))
        if (items.isEmpty()) add(ScoutListRow.Empty(depth))
        val (trinkets, loot) = items.partition { it.value.item.kind == ItemKind.TRINKET }
        if (trinkets.isNotEmpty()) add(ScoutListRow.Trinkets(depth, trinkets))
        loot.forEach { add(ScoutListRow.Item(it)) }
    }
}
