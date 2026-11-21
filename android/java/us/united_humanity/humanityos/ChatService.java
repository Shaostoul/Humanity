// Staying connected while the app is closed (Android rung A4, 2026-11-20; android/README.md,
// docs/design/platforms.md "Staying connected: the foreground service and UnifiedPush").
//
// Android stops an app that is not on screen, and with it the chat connection. A foreground
// service keeps the process (and the Rust side's connection, which runs on its own thread) alive,
// at the price Android sets: a notification stays in the bar for as long as it runs. Only when the
// person turns on Settings, Notifications, "Stay connected while HumanityOS is closed" (off by
// default). Nothing goes through Google or any other push service: the connection is the app's
// own, so messages stay sealed end to end.
//
// The service does no work itself; it only exists so Android keeps the process. Its type is
// specialUse (the manifest says why in words): dataSync stops after 6 hours a day on Android 15,
// and remoteMessaging is described as moving messages between a person's own devices.
// START_NOT_STICKY: if Android ends the process anyway, the service is not restarted without the
// app (it would hold a notification for a connection nobody runs). The manifest's
// stopWithTask="true" ends it when the person swipes HumanityOS away in the recent-apps list.
package us.united_humanity.humanityos;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
import android.util.Log;

public final class ChatService extends Service {
    static final String CHANNEL = "connection";
    private static final int ID = 1;

    // The words the Rust side handed over (data/i18n/en.json, notifications.android_*): Android
    // builds the notification in onStartCommand, after start() has returned.
    private static volatile String channelName = "HumanityOS";
    private static volatile String title = "HumanityOS";
    private static volatile String text = "";

    // Stopping right after starting (2026-11-20): a service started with startForegroundService
    // MUST call startForeground, and Android kills the app if it ends before it has. So `stop`
    // calls stopService only once the service is in the foreground; told to stop before that, it
    // leaves `wanted` false and the service stops itself in onStartCommand, right after its
    // startForeground. `foreground` means "the latest start has been answered by startForeground":
    // every start owes a new one, so start sets it false, and stop sets it false as it stops the
    // service, so a start that follows before onDestroy has run is never taken for a service
    // already in the foreground (the second review's E: off, on, off called stopService on the new
    // instance before its startForeground). Guarded by LOCK, the system calls included, so a start
    // and a stop never interleave: start, stop and the service's callbacks run on different threads
    // (the notices' worker, the app's, Android's main thread). Neither system call waits for the
    // service's callbacks, which Android posts to the main thread, so holding LOCK cannot deadlock.
    private static final Object LOCK = new Object();
    private static boolean wanted;
    private static boolean foreground;

    // Start (or refresh) the service. Called while the app is on screen: Android 12 and later
    // refuse to start a foreground service from the background. False when Android refused.
    public static boolean start(Context ctx, String channel, String heading, String body) {
        channelName = channel;
        title = heading;
        text = body;
        synchronized (LOCK) {
            boolean was = foreground;
            wanted = true;
            foreground = false;
            try {
                ctx.startForegroundService(new Intent(ctx, ChatService.class));
                return true;
            } catch (RuntimeException refused) {
                // ForegroundServiceStartNotAllowedException (an IllegalStateException) and
                // friends: nothing new is owed, so a service already running stays stoppable.
                wanted = false;
                foreground = was;
                Log.w("HumanityOS", "the stay-connected service was refused: " + refused);
                return false;
            }
        }
    }

    public static void stop(Context ctx) {
        synchronized (LOCK) {
            wanted = false;
            if (!foreground) {
                // Not started, or a start not yet answered by startForeground: onStartCommand stops it.
                return;
            }
            foreground = false;
            ctx.stopService(new Intent(ctx, ChatService.class));
        }
    }

    @Override
    public void onDestroy() {
        synchronized (LOCK) {
            foreground = false;
        }
        super.onDestroy();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        NotificationManager nm = getSystemService(NotificationManager.class);
        NotificationChannel ch = new NotificationChannel(CHANNEL, channelName, NotificationManager.IMPORTANCE_LOW);
        ch.setShowBadge(false);
        nm.createNotificationChannel(ch);
        Notification n = new Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_chat)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(new Notification.BigTextStyle().bigText(text))
            .setOngoing(true)
            .setShowWhen(false)
            .setContentIntent(Notices.openApp(this))
            .build();
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
        } else {
            startForeground(ID, n);
        }
        boolean stopNow;
        synchronized (LOCK) {
            foreground = true;
            stopNow = !wanted;
        }
        if (stopNow) {
            // Told to stop before it got here: now that Android's rule is met, stop.
            stopSelf();
        }
        return START_NOT_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
