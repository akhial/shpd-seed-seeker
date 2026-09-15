// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.engine

import dev.seedseeker.app.catalog.PackagedCatalog
import java.io.EOFException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class ScoutItemMappingsTest {
    init { PackagedCatalog.install() }

    @Test fun realNativeScoutNamesTheJavaOracleAppearancesAndAgreesWithRingArtwork() {
        val world = JniNativeSeedFinder().scoutSeed("ABC-DEF-GHI")
        val mappings = requireNotNull(world.itemMappings)
        assertEquals(listOf(12, 12, 12), listOf(mappings.scrolls.size, mappings.potions.size, mappings.rings.size))
        assertEquals("Scroll of upgrade", mappings.scrolls[0].name)
        assertEquals("TIWAZ", mappings.scrolls[0].appearance)
        assertEquals(315, mappings.scrolls[0].spriteIndex)
        assertEquals("Potion of healing", mappings.potions[1].name)
        assertEquals("Charcoal", mappings.potions[1].appearance)
        assertEquals(361, mappings.potions[1].spriteIndex)
        assertEquals(world.ringGems.ordinals, mappings.rings.map { it.spriteIndex - 224 })
        assertEquals(mappings, JniNativeSeedFinder().scoutSeed(world.seed, 511).itemMappings)
        assertNotEquals(mappings, JniNativeSeedFinder().scoutSeed("AAA-AAA-AAA").itemMappings)
    }

    @Test fun legacyRequestsKeepTheirResponseAndNewPacketsRejectCorruption() {
        val request = ScoutRequestCodec.encode("ABC-DEF-GHI", 0)
        val packet = JniBindings.scoutSeed(request)
        assertEquals("SSC7", packet.take(4).toByteArray().toString(Charsets.US_ASCII))
        assertNotNull(ScoutResultCodec.decode(packet).itemMappings)
        val legacyRequest = request.clone().also { it[3] = '3'.code.toByte() }
        val legacyPacket = JniBindings.scoutSeed(legacyRequest)
        assertEquals("SSC6", legacyPacket.take(4).toByteArray().toString(Charsets.US_ASCII))
        assertNull(ScoutResultCodec.decode(legacyPacket).itemMappings)
        for (removed in listOf(1, 10, packet.size - legacyPacket.size)) {
            assertThrows(EOFException::class.java) { ScoutResultCodec.decode(packet.dropLast(removed).toByteArray()) }
        }
        val badSprite = packet.clone().also { it[it.lastIndex] = 0 }
        assertThrows(IllegalStateException::class.java) { ScoutResultCodec.decode(badSprite) }
        val emptyName = packet.clone().also { it[legacyPacket.size] = 0; it[legacyPacket.size + 1] = 0 }
        assertThrows(IllegalStateException::class.java) { ScoutResultCodec.decode(emptyName) }
        assertThrows(IllegalStateException::class.java) { ScoutResultCodec.decode(packet + byteArrayOf(0)) }
    }
}
