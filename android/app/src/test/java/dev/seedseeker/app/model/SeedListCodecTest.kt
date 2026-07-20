// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import java.nio.charset.StandardCharsets
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class SeedListCodecTest {
    @Test
    fun importsBomLfCrLfLowercaseBlanksAndDuplicatesInFirstSeenOrder() {
        val document = byteArrayOf(0xEF.toByte(), 0xBB.toByte(), 0xBF.toByte()) +
            "aaa-aaa-aab\r\n\nBBB-BBB-BBB\n   \r\naAa-aAa-AaB\n".toByteArray(StandardCharsets.UTF_8)

        assertEquals(
            listOf("AAA-AAA-AAB", "BBB-BBB-BBB"),
            SeedListCodec.decode(document),
        )
    }

    @Test
    fun reportsTheInvalidNonblankLineWithoutPartiallyImporting() {
        val failure = assertThrows(SeedListFormatException::class.java) {
            SeedListCodec.decode("AAA-AAA-AAA\nBBB-BBB-BBB\nnot-a-seed\nCCC-CCC-CCC")
        }

        assertEquals(3, failure.lineNumber)
        assertEquals("Invalid seed on line 3: expected XXX-XXX-XXX.", failure.message)
    }

    @Test
    fun rejectsMalformedUtf8AndUnsupportedBareCarriageReturns() {
        assertThrows(SeedListFormatException::class.java) {
            SeedListCodec.decode(byteArrayOf(0xC3.toByte(), 0x28))
        }
        val failure = assertThrows(SeedListFormatException::class.java) {
            SeedListCodec.decode("AAA-AAA-AAA\r")
        }
        assertEquals(1, failure.lineNumber)
    }

    @Test
    fun enforcesTheUniqueSeedLimitAtTheOffendingLine() {
        val accepted = (0 until MAX_SEED_LIST_SIZE).map(::seedCode)
        assertEquals(MAX_SEED_LIST_SIZE, SeedListCodec.decode(accepted.joinToString("\n")).size)

        val failure = assertThrows(SeedListFormatException::class.java) {
            SeedListCodec.decode((accepted + seedCode(MAX_SEED_LIST_SIZE)).joinToString("\n"))
        }
        assertEquals(MAX_SEED_LIST_SIZE + 1, failure.lineNumber)
    }

    @Test
    fun exportsCanonicalDeduplicatedUtf8WithOneTrailingLf() {
        assertArrayEquals(
            "AAA-AAA-AAA\nBBB-BBB-BBB\n".toByteArray(StandardCharsets.UTF_8),
            SeedListCodec.encode(listOf("aaa-aaa-aaa", "BBB-BBB-BBB", "AAA-AAA-AAA")),
        )
        assertArrayEquals(byteArrayOf(), SeedListCodec.encode(emptyList()))
    }

    private fun seedCode(value: Int): String {
        var remaining = value
        val letters = CharArray(9) { 'A' }
        for (index in letters.indices.reversed()) {
            letters[index] = ('A'.code + remaining % 26).toChar()
            remaining /= 26
        }
        return letters.concatToString().chunked(3).joinToString("-")
    }
}
