package dev.seedseeker.app.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.util.TimeZone

class DailyRunDateTest {
    @Test fun utcDateDoesNotFollowTheDevicesTimeZone() {
        val previous = TimeZone.getDefault()
        try {
            TimeZone.setDefault(TimeZone.getTimeZone("Pacific/Honolulu"))
            val midnight = requireNotNull(DailyRunDate.parse("2026-09-25"))
            assertEquals("2026-09-24", DailyRunDate.format(midnight - 1))
            assertEquals("2026-09-25", DailyRunDate.format(midnight))
            assertEquals("2026-09-25", DailyRunDate.format(midnight + 86_399_999))
            assertEquals("2026-09-26", DailyRunDate.format(midnight + 86_400_000))
            assertNull(DailyRunDate.parse("2026-02-29"))
            assertEquals("2024-02-29", DailyRunDate.format(requireNotNull(DailyRunDate.parse("2024-02-29"))))
        } finally { TimeZone.setDefault(previous) }
    }
}
