// Attach on the phone: Android's own document picker (2026-11-23; src/gui/pages/chat/attach_android.rs,
// android/README.md "The Java in the APK"). The operator, 2026-11-23, on his phone: "I can't upload
// images because the windows aren't scaling to the phone." The app's own file browser showed only
// its private folder (/data/user/0/us.united_humanity.humanityos/files), where none of his pictures
// are: Android's scoped storage never lets an app browse the phone's photos that way. The Storage
// Access Framework does: ACTION_OPEN_DOCUMENT opens the system's picker, the person chooses from
// Photos, Downloads, Drive or anything else Android offers, and the app is handed a content:// URI
// for each chosen document, readable through the ContentResolver with no permission of its own.
// (developer.android.com, "Access documents and other files from shared storage", read
// 2026-11-23: "this mechanism doesn't require any system permissions".)
//
// WHY AN ACTIVITY OF ITS OWN. The picker's answer comes back to the activity that asked, in
// onActivityResult, and NativeActivity has none the app can reach: its native callbacks
// (android-activity 0.6.1, native_activity/glue.rs, which sets every one the NDK's
// ANativeActivityCallbacks offers) are onStart, onResume, onPause, onStop, onDestroy,
// onSaveInstanceState, onConfigurationChanged, onLowMemory, onWindowFocusChanged, the native
// window's and input queue's, and onContentRectChanged; there is no result. So this small
// activity, transparent and declared in the manifest, is started on top of the app, asks for the
// documents itself, keeps the answer for the app's thread to take, and closes. A headless
// android.app.Fragment added to the NativeActivity would get the result too, but that class has
// been deprecated since Android 9 (API 28), and an activity of our own depends on nothing inside
// NativeActivity.
//
// The app's thread starts it (start), asks every frame or so whether the person has chosen
// (take), then, on a worker thread, reads each chosen document's name and size (describe) and
// copies it, streamed, into the app's own folder (copy), from where the chat sends it as it sends a
// file chosen on a desktop. The signatures Rust uses are listed in
// crates/humanity-core/src/platform/android_java.rs, whose test reads this file.
package us.united_humanity.humanityos;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;

public final class PickFiles extends Activity {
    private static final int PICK = 1;
    // Android's media documents provider and its Images root (AOSP MediaProvider,
    // MediaDocumentsProvider.java: AUTHORITY and TYPE_IMAGES_ROOT, read 2026-11-23): the picker
    // opens on the person's pictures, and every other place stays one tap away. Android does "its
    // best" with this and starts somewhere of its own when it cannot (DocumentsContract
    // EXTRA_INITIAL_URI, API 26, the app's lowest).
    private static final String MEDIA_DOCUMENTS = "com.android.providers.media.documents";
    private static final String IMAGES_ROOT = "images_root";

    // The latest answer, until the app takes it: "picked" then each chosen document's URI,
    // "cancelled", or "failed" then why. Null while nothing waits.
    private static final Object LOCK = new Object();
    private static String[] outcome;

    // Start the picker over the app. Called on the app's own thread; the activity is started on
    // Android's UI thread.
    public static void start(final Activity from) {
        synchronized (LOCK) {
            outcome = null;
        }
        from.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    from.startActivity(new Intent(from, PickFiles.class));
                } catch (RuntimeException e) {
                    keep(new String[] { "failed", String.valueOf(e) });
                }
            }
        });
    }

    // The answer waiting, taken (null when the person has not answered yet).
    public static String[] take() {
        synchronized (LOCK) {
            String[] now = outcome;
            outcome = null;
            return now;
        }
    }

    static void keep(String[] answer) {
        synchronized (LOCK) {
            outcome = answer;
        }
    }

    @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        // Made again by Android (its process ended while the picker was open): the picker is still
        // open, and its answer still comes to onActivityResult. Asking again would open a second.
        if (saved != null) {
            return;
        }
        Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        // Only documents that can be opened as a stream (Intent.CATEGORY_OPENABLE).
        pick.addCategory(Intent.CATEGORY_OPENABLE);
        pick.setType("*/*");
        pick.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
        pick.putExtra(DocumentsContract.EXTRA_INITIAL_URI, DocumentsContract.buildDocumentUri(MEDIA_DOCUMENTS, IMAGES_ROOT));
        try {
            startActivityForResult(pick, PICK);
        } catch (ActivityNotFoundException e) {
            finishWith(new String[] { "failed", "this phone has no document picker" });
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        if (request != PICK) {
            return;
        }
        if (result != RESULT_OK || data == null) {
            finishWith(new String[] { "cancelled" });
            return;
        }
        // One document comes back in getData; several in getClipData (Intent.ACTION_OPEN_DOCUMENT:
        // "If multiple documents were selected, they are returned in getClipData()").
        ArrayList<String> answer = new ArrayList<String>();
        answer.add("picked");
        ClipData clip = data.getClipData();
        if (clip != null) {
            for (int i = 0; i < clip.getItemCount(); i++) {
                Uri uri = clip.getItemAt(i).getUri();
                if (uri != null) {
                    answer.add(uri.toString());
                }
            }
        } else if (data.getData() != null) {
            answer.add(data.getData().toString());
        }
        finishWith(answer.toArray(new String[0]));
    }

    private void finishWith(String[] answer) {
        keep(answer);
        finish();
    }

    // A chosen document's name as its provider shows it and its size in bytes ("-1" when the
    // provider does not know it, which the guide says happens often for remote files).
    public static String[] describe(Context context, String uri) {
        String name = null;
        long size = -1;
        Cursor cursor = context.getContentResolver().query(Uri.parse(uri), new String[] { OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE }, null, null, null);
        if (cursor != null) {
            try {
                if (cursor.moveToFirst()) {
                    int n = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                    int s = cursor.getColumnIndex(OpenableColumns.SIZE);
                    if (n >= 0 && !cursor.isNull(n)) {
                        name = cursor.getString(n);
                    }
                    if (s >= 0 && !cursor.isNull(s)) {
                        size = cursor.getLong(s);
                    }
                }
            } finally {
                cursor.close();
            }
        }
        return new String[] { name == null ? "" : name, Long.toString(size) };
    }

    // Copy the document into the file at dest, streamed, at most max bytes: the bytes copied, or -1
    // when it holds more than max (the part copied is deleted).
    public static long copy(Context context, String uri, String dest, long max) throws IOException {
        File out = new File(dest);
        InputStream in = context.getContentResolver().openInputStream(Uri.parse(uri));
        if (in == null) {
            throw new IOException("the provider gave no stream");
        }
        long total = 0;
        boolean over = false;
        try {
            OutputStream sink = new FileOutputStream(out);
            try {
                byte[] buffer = new byte[64 * 1024];
                int n;
                while ((n = in.read(buffer)) > 0) {
                    total += n;
                    if (total > max) {
                        over = true;
                        break;
                    }
                    sink.write(buffer, 0, n);
                }
            } finally {
                sink.close();
            }
        } finally {
            in.close();
        }
        if (over) {
            out.delete();
            return -1;
        }
        return total;
    }
}
