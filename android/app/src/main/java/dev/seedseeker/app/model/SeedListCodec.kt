// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.util.Locale

const val MAX_SEED_LIST_SIZE = 1_024

class SeedListFormatException(
    message: String,
    val lineNumber: Int? = null,
) : IllegalArgumentException(message)

/** Strict, interoperable one-canonical-seed-per-line UTF-8 documents. */
object SeedListCodec {
    private val seedPattern = Regex("[A-Za-z]{3}-[A-Za-z]{3}-[A-Za-z]{3}")

    fun decode(bytes: ByteArray): List<String> {
        val text = try {
            StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(bytes))
                .toString()
        } catch (_: Exception) {
            throw SeedListFormatException("Seed list is not valid UTF-8.")
        }
        return decode(text)
    }

    fun decode(text: String): List<String> {
        val seeds = LinkedHashSet<String>()
        val lines = text.split('\n')
        lines.forEachIndexed { index, rawLine ->
            val lineNumber = index + 1
            var line = rawLine
            if (lineNumber == 1 && line.startsWith('\uFEFF')) {
                line = line.drop(1)
            }
            if (line.endsWith('\r')) {
                if (index == lines.lastIndex) {
                    throw SeedListFormatException(
                        "Invalid line ending on line $lineNumber: use LF or CRLF.",
                        lineNumber,
                    )
                }
                line = line.dropLast(1)
            }
            if ('\r' in line) {
                throw SeedListFormatException(
                    "Invalid line ending on line $lineNumber: use LF or CRLF.",
                    lineNumber,
                )
            }
            if (line.isBlank()) return@forEachIndexed
            if (!seedPattern.matches(line)) {
                throw SeedListFormatException(
                    "Invalid seed on line $lineNumber: expected XXX-XXX-XXX.",
                    lineNumber,
                )
            }
            val canonical = line.uppercase(Locale.US)
            if (seeds.add(canonical) && seeds.size > MAX_SEED_LIST_SIZE) {
                throw SeedListFormatException(
                    "Seed list exceeds the $MAX_SEED_LIST_SIZE-seed limit on line $lineNumber.",
                    lineNumber,
                )
            }
        }
        return seeds.toList()
    }

    fun encode(seeds: List<String>): ByteArray {
        require(seeds.size <= MAX_SEED_LIST_SIZE) {
            "Seed list cannot contain more than $MAX_SEED_LIST_SIZE seeds"
        }
        val normalized = LinkedHashSet<String>()
        seeds.forEachIndexed { index, seed ->
            require(seedPattern.matches(seed)) {
                "Invalid seed at index $index: expected XXX-XXX-XXX"
            }
            normalized += seed.uppercase(Locale.US)
        }
        return normalized.joinToString(separator = "\n", postfix = if (normalized.isEmpty()) "" else "\n")
            .toByteArray(StandardCharsets.UTF_8)
    }
}
