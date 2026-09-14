// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.CatalogItem
import dev.seedseeker.app.ui.theme.SpdGreen

/** The same compact, icon-only choices in the item list and expanded map. */
@Composable
internal fun TrinketShortcuts(
    offers: List<CatalogItem>,
    selectedTrinket: String?,
    enabled: Boolean,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.CenterHorizontally)) {
        offers.take(4).forEach { offer ->
            val applied = selectedTrinket == offer.id
            Surface(
                selected = applied,
                onClick = { onSelect(if (applied) "none" else offer.id) },
                enabled = enabled,
                modifier = Modifier.size(36.dp).semantics {
                    contentDescription = offer.name
                    stateDescription = if (applied) "Applied +3" else "Not applied"
                },
                shape = MaterialTheme.shapes.small,
                border = BorderStroke(
                    if (applied) 2.dp else 1.dp,
                    if (applied) SpdGreen else MaterialTheme.colorScheme.outlineVariant,
                ),
                color = if (applied) SpdGreen.copy(alpha = 0.14f) else MaterialTheme.colorScheme.surfaceContainerLow,
            ) {
                Box(contentAlignment = Alignment.Center) { ItemSprite(offer, modifier = Modifier.size(22.dp)) }
            }
        }
    }
}
