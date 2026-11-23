// The part of the window a person can see (2026-11-22; src/gui/visible_area.rs, android/README.md
// "What the code does differently on a phone"). The app draws its pages into the whole window, and
// Android lays other things over it: the status bar and any camera cutout at the top, the
// navigation bar with its back, home and recent-apps buttons at the bottom (or the side, turned),
// and the on-screen keyboard while a text box is being typed in. The operator, 2026-11-22, on his
// phone: the keyboard covered the recovery phrase box he was typing in, and the chat's text box sat
// under the phone's home button, so the attach button could not be pressed.
//
// This class tells the app how far in from each edge those reach, as Android's WindowInsets say:
// a listener on the window's root view keeps the latest values, and the app's own thread reads
// them (insets) through JNI (crates/humanity-core/src/platform/android_jni.rs; the signatures Rust
// uses are listed in android_java.rs, whose test reads this file). The insets, and not the
// NativeActivity's content rectangle alone, because the content rectangle shrinks for the bars and
// the keyboard only while Android fits the window's content inside them; on a window laid out edge
// to edge (Android 15 does that for every app targeting it) the content rectangle stays the whole
// window and only the insets say what is covered. The app takes the smaller of the two areas.
package us.united_humanity.humanityos;

import android.app.Activity;
import android.os.Build;
import android.view.DisplayCutout;
import android.view.View;
import android.view.WindowInsets;
import java.lang.ref.WeakReference;

public final class VisibleArea {
    // The latest insets in the window's pixels: left, top, right, bottom. Null until Android first
    // reported them. Written on the UI thread, read on the app's thread: a fresh array each time,
    // so a reader never sees half of one update.
    private static volatile int[] latest;
    // The root view listened to, so a second call for the same window adds nothing; weak, so a
    // closed activity's window is not kept alive by it.
    private static WeakReference<View> listened = new WeakReference<View>(null);

    private VisibleArea() {}

    // Start keeping the insets of this activity's window (called once each time the app starts;
    // the work runs on Android's UI thread, where views may be touched).
    public static void listen(final Activity activity) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                View root = activity.getWindow().getDecorView();
                if (listened.get() == root) {
                    return;
                }
                listened = new WeakReference<View>(root);
                root.setOnApplyWindowInsetsListener(new View.OnApplyWindowInsetsListener() {
                    @Override
                    public WindowInsets onApplyWindowInsets(View v, WindowInsets insets) {
                        store(insets);
                        // The window's own handling of the insets carries on as before.
                        return v.onApplyWindowInsets(insets);
                    }
                });
                WindowInsets now = root.getRootWindowInsets();
                if (now != null) {
                    store(now);
                }
                root.requestApplyInsets();
            }
        });
    }

    // The latest insets (left, top, right, bottom, in the window's pixels), or null before Android
    // first reported them.
    public static int[] insets() {
        int[] now = latest;
        return now == null ? null : now.clone();
    }

    // The system bars, a display cutout and the keyboard, whichever reaches furthest in from each
    // edge.
    static void store(WindowInsets insets) {
        int[] out;
        if (Build.VERSION.SDK_INT >= 30) {
            android.graphics.Insets i = insets.getInsets(
                WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
            out = new int[] { i.left, i.top, i.right, i.bottom };
        } else {
            // Before Android 11 the system window insets hold the bars, and the keyboard too for a
            // window that asks to be resized for it (the manifest's adjustResize).
            out = new int[] {
                insets.getSystemWindowInsetLeft(),
                insets.getSystemWindowInsetTop(),
                insets.getSystemWindowInsetRight(),
                insets.getSystemWindowInsetBottom(),
            };
            if (Build.VERSION.SDK_INT >= 28) {
                DisplayCutout cutout = insets.getDisplayCutout();
                if (cutout != null) {
                    out[0] = Math.max(out[0], cutout.getSafeInsetLeft());
                    out[1] = Math.max(out[1], cutout.getSafeInsetTop());
                    out[2] = Math.max(out[2], cutout.getSafeInsetRight());
                    out[3] = Math.max(out[3], cutout.getSafeInsetBottom());
                }
            }
        }
        latest = out;
    }
}
