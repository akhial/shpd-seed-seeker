// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.setValue

/** Scroll distance includes both the departing form and the summary's lost height. */
@Stable
internal class ScoutHeaderScrollState(private val collapseWindow: Float) {
    var inputHeight by mutableFloatStateOf(0f)
        private set
    var summaryCollapseDistance by mutableFloatStateOf(0f)
        private set
    var offset by mutableFloatStateOf(0f)
        private set

    val maxOffset: Float get() = inputHeight + summaryCollapseDistance
    private val window: Float get() = minOf(collapseWindow, inputHeight)
    val progress: Float
        get() = if (window + summaryCollapseDistance > 0f) {
            ((offset - inputHeight + window) / (window + summaryCollapseDistance)).coerceIn(0f, 1f)
        } else 0f
    val inputOffset: Float get() = offset - summaryCollapseDistance * progress

    fun updateMeasurements(input: Float = inputHeight, summary: Float = summaryCollapseDistance) {
        val wasCollapsed = maxOffset > 0f && offset >= maxOffset
        inputHeight = input
        summaryCollapseDistance = summary
        offset = if (wasCollapsed) maxOffset else offset.coerceAtMost(maxOffset)
    }

    /** Compose scroll deltas are negative when content moves toward the app bar. */
    fun consume(delta: Float): Float {
        val previous = offset
        offset = (offset - delta).coerceIn(0f, maxOffset)
        return previous - offset
    }

    fun expand() { offset = 0f }
}
