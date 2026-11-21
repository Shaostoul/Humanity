// The phone's notices (Android rung A4, 2026-11-20; docs/design/notifications.md,
// src/notifications/toast.rs). The Rust side decides whether a notice is shown and what it says
// (notifications::decide: who it is from by default, the message only when the person chose
// "Who it is from and the message"); this class only hands it to Android.
//
// On a phone with a screen lock (a PIN, pattern or password), its locked screen shows NOTHING of
// a notice by default (VISIBILITY_SECRET, 2026-11-20). Android hides a notice only on a SECURE
// lock screen: a phone locked only by a swipe is not locked as far as Android is concerned and
// shows every notice whole, whatever this class asks. Only when the person turns on Settings,
// Notifications, "Show a line on the locked screen" is it VISIBILITY_PRIVATE with a public version
// ("Something new arrived"), and even then Android shows that public version only on a phone
// whose own "hide sensitive content" setting is on; with it off (Android's default) a PRIVATE
// notice shows whole on the locked screen. The setting's hint says all of it
// (notifications.lock_screen_line_hint). A tap opens the app. Called from the notices'
// worker thread through JNI
// (crates/humanity-core/src/platform/android_jni.rs); the signatures Rust uses are listed in
// android_java.rs, whose test reads this file.
package us.united_humanity.humanityos;

import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Build;

public final class Notices {
    static final String CHANNEL = "messages";
    private static final int ID = 2;
    private static final String PERMISSION = "android.permission.POST_NOTIFICATIONS";

    private Notices() {}

    // Post a notice; one with the same tag (one per conversation, a hash that names nobody)
    // replaces it. lockScreenLine: the person chose a line on the locked screen (off by default).
    public static void show(Context ctx, String channel, String tag, String title, String body, boolean lockScreenLine, String publicTitle, String publicBody) {
        NotificationManager nm = ctx.getSystemService(NotificationManager.class);
        nm.createNotificationChannel(new NotificationChannel(CHANNEL, channel, NotificationManager.IMPORTANCE_HIGH));
        Notification.Builder b = new Notification.Builder(ctx, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_chat)
            .setContentTitle(title)
            .setVisibility(lockScreenVisibility(lockScreenLine))
            .setAutoCancel(true)
            .setContentIntent(openApp(ctx));
        if (lockScreenLine) {
            b.setPublicVersion(new Notification.Builder(ctx, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_notify_chat)
                .setContentTitle(publicTitle)
                .setContentText(publicBody)
                .build());
        }
        if (body != null && !body.isEmpty()) {
            b.setContentText(body).setStyle(new Notification.BigTextStyle().bigText(body));
        }
        nm.notify(tag, ID, b.build());
    }

    // What a secure locked screen shows of a notice: nothing, unless the person chose the line
    // (a swipe-only lock shows it whole either way: Android's rule, not ours).
    // android_java.rs's test reads this method and fails on VISIBILITY_PRIVATE outside the choice.
    static int lockScreenVisibility(boolean lineChosen) {
        if (lineChosen) {
            return Notification.VISIBILITY_PRIVATE;
        }
        return Notification.VISIBILITY_SECRET;
    }

    // Take the notice tagged tag back.
    public static void withdraw(Context ctx, String tag) {
        ctx.getSystemService(NotificationManager.class).cancel(tag, ID);
    }

    // Whether Android lets the app show notices.
    public static boolean allowed(Context ctx) {
        return ctx.getSystemService(NotificationManager.class).areNotificationsEnabled();
    }

    // Ask the person to allow notices: Android 13 and later only (earlier, they are allowed
    // unless turned off in Android's settings). The dialog is Android's own, on the app's screen.
    public static void ask(final Activity activity) {
        if (Build.VERSION.SDK_INT < 33 || activity.checkSelfPermission(PERMISSION) == PackageManager.PERMISSION_GRANTED) {
            return;
        }
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                activity.requestPermissions(new String[] {PERMISSION}, 1);
            }
        });
    }

    // What a tap on a notice does: bring the app to the front.
    static PendingIntent openApp(Context ctx) {
        Intent open = ctx.getPackageManager().getLaunchIntentForPackage(ctx.getPackageName());
        if (open == null) {
            open = new Intent();
        }
        open.setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED);
        return PendingIntent.getActivity(ctx, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }
}
