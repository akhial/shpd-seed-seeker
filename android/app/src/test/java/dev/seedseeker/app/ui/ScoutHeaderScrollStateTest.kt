// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class ScoutHeaderScrollStateTest {
    @Test fun collapseConsumesExactlyTheHeightRemovedFromTheScreen() {
        val state = ScoutHeaderScrollState(96f)
        state.updateMeasurements(input = 180f, summary = 48f)
        assertEquals(-84f, state.consume(-84f), 0.001f)
        assertEquals(0f, state.progress, 0.001f)
        assertEquals(-72f, state.consume(-72f), 0.001f)
        assertEquals(0.5f, state.progress, 0.001f)
        assertEquals(132f, state.inputOffset, 0.001f)
        assertEquals(state.offset, state.inputOffset + 48f * state.progress, 0.001f)
        // An overshooting fling leaves the remaining delta for the floor list.
        assertEquals(-72f, state.consume(-500f), 0.001f)
        assertEquals(1f, state.progress, 0.001f)
        assertEquals(180f, state.inputOffset, 0.001f)
        assertEquals(0f, state.consume(-20f), 0.001f)
    }

    @Test fun reverseScrollRetracesTheTransitionAndStopsAtTheTop() {
        val state = ScoutHeaderScrollState(96f)
        state.updateMeasurements(input = 180f, summary = 48f)
        state.consume(-156f)
        val inputOffset = state.inputOffset
        state.consume(-72f)
        state.consume(72f)
        assertEquals(inputOffset, state.inputOffset, 0.001f)
        assertEquals(156f, state.consume(500f), 0.001f)
        assertEquals(0f, state.offset, 0.001f)
        assertEquals(0f, state.consume(20f), 0.001f)
    }

    @Test fun fontOrWindowSizeChangesKeepACollapsedHeaderCollapsed() {
        val state = ScoutHeaderScrollState(96f)
        state.updateMeasurements(input = 180f, summary = 48f)
        state.consume(-500f)
        state.updateMeasurements(input = 220f, summary = 64f)
        assertEquals(1f, state.progress, 0.001f)
        assertEquals(220f, state.inputOffset, 0.001f)
        state.expand()
        assertEquals(0f, state.offset, 0.001f)
    }
}
