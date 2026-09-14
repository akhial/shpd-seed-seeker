// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Keeps the application-owned search running after the activity or screen goes away. */
class SearchService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val controller get() = (application as SeedSeekerApplication).searchController
    private var wakeLock: PowerManager.WakeLock? = null
    private var observing = false
    private var foreground = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Promotion must precede checkpoint loading, JNI calls, and notification permission UI.
        try {
            createChannel()
            val notification = notification()
            if (Build.VERSION.SDK_INT >= 34) {
                startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
            } else {
                startForeground(NOTIFICATION_ID, notification)
            }
            foreground = true
        } catch (failure: Exception) {
            controller.serviceUnavailable(failure)
            stopSelf()
            return START_NOT_STICKY
        }
        if (intent?.action == ACTION_STOP) {
            controller.stop()
        } else {
            controller.runPending()
        }
        if (!observing) {
            observing = true
            scope.launch {
                var ticks = 0
                while (!controller.ready || controller.isSearching) {
                    // Bound each acquisition even if the process gets stuck. Renew only while
                    // serving a live search; no screen wake lock or battery exemption is forced.
                    if (ticks++ % 60 == 0) renewWakeLock()
                    getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification())
                    delay(1_000)
                }
                releaseWakeLock()
                removeForeground()
                stopSelf()
            }
        }
        return START_STICKY
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        controller.interrupt()
        releaseWakeLock()
        removeForeground()
        stopSelf()
    }

    override fun onDestroy() {
        // Activity recreation never gets here. Unexpected service destruction drains and saves
        // cooperatively; abrupt process death instead recovers the previous atomic checkpoint.
        if (controller.isSearching) controller.interrupt()
        scope.cancel()
        releaseWakeLock()
        removeForeground()
        super.onDestroy()
    }

    private fun renewWakeLock() {
        val lock = wakeLock ?: getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "SeedSeeker:Search")
            .apply { setReferenceCounted(false) }.also { wakeLock = it }
        lock.acquire(10 * 60 * 1_000L)
    }

    private fun releaseWakeLock() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
    }

    @Suppress("DEPRECATION")
    private fun removeForeground() {
        if (!foreground) return
        if (Build.VERSION.SDK_INT >= 24) stopForeground(STOP_FOREGROUND_REMOVE) else stopForeground(true)
        foreground = false
    }

    private fun createChannel() {
        if (Build.VERSION.SDK_INT >= 26) {
            getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Seed searches", NotificationManager.IMPORTANCE_LOW).apply {
                    description = "Progress and stop control for searches running in the background"
                },
            )
        }
    }

    private fun notification(): Notification {
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getService(this, 1, Intent(this, SearchService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val snapshot = controller.snapshot
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_search_notification)
            .setContentTitle(if (controller.refinePhase != null) "Refining seeds" else "Searching for seeds")
            .setContentText("${snapshot.results.size} found · ${snapshot.status?.scannedSeeds ?: 0} checked")
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .addAction(0, "Stop", stop)
            .build()
    }

    companion object {
        internal const val CHANNEL_ID = "seed_search"
        internal const val NOTIFICATION_ID = 1
        internal const val ACTION_STOP = "dev.seedseeker.app.STOP_SEARCH"

        internal fun start(context: Context) {
            ContextCompat.startForegroundService(context, Intent(context, SearchService::class.java))
        }
    }
}
