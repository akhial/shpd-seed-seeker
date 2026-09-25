package dev.seedseeker.app.model

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/** Calendar presentation only; the Rust engine resolves dates to daily seeds. */
object DailyRunDate {
    private fun formatter() = SimpleDateFormat("yyyy-MM-dd", Locale.ROOT).apply {
        timeZone = TimeZone.getTimeZone("UTC")
        isLenient = false
    }

    fun today(): String = format(System.currentTimeMillis())
    fun format(millis: Long): String = formatter().format(Date(millis))
    fun parse(date: String): Long? = runCatching {
        formatter().parse(date)?.time?.takeIf { format(it) == date }
    }.getOrNull()
}
