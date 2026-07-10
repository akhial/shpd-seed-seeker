// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.engine

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.SearchRequest
import org.junit.Assert.assertArrayEquals
import org.junit.Test

class QueryCodecTest {
    @Test
    fun encodesStableSsf1PacketWithExactUpgrade() {
        val sword = ItemCatalog.weapons.first { it.id == "sword" }
        val request = SearchRequest(
            listOf(ItemRequirement(key = 9, item = sword, upgrade = 2, modifier = "Lucky")),
        )

        assertArrayEquals(
            byteArrayOf(
                0x53, 0x53, 0x46, 0x31, // SSF1
                0x00, 0x01, // one requirement
                0x00, 0x05, 0x73, 0x77, 0x6F, 0x72, 0x64, // sword
                0x02, // exactly +2
                0x00, 0x05, 0x4C, 0x75, 0x63, 0x6B, 0x79, // Lucky
            ),
            QueryCodec.encode(request),
        )
    }
}
