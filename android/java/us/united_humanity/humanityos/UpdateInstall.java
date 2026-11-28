// The phone app updates itself (2026-11-27; src/updater/phone.rs, docs/design/platforms.md rung A2,
// android/README.md "Updating"). The operator asked on 2026-11-22 how to update the phone app: until
// then a person found the new APK on the release page, downloaded it and opened it, and Android
// offered Update because both were signed with the project's key. Now the app notices a new release
// itself, downloads its APK inside the app, checks it against the SHA-256 checksum the release
// workflow publishes beside it (src/updater/phone.rs), and hands it here, and this class gives it to
// Android's own installer, which asks the person to confirm. That last question is Android's rule
// for every app outside Google Play, and it stays.
//
// WHY THIS ROUTE (REQUEST_INSTALL_PACKAGES and a content provider of our own) and not opening the
// APK's link in the browser: the browser route would need no permission, but the person would watch
// the browser's own download, find it in the notification shade, and tap it, and the app could not
// check the file against its checksum before Android saw it. In the app, the download shows its
// progress on the page, is checked, and the installer opens by itself. A new-to-tech person sees
// one line, Download, then Install, then Android's question. The first time, Android also asks to
// allow HumanityOS to install apps (the same switch they turned on for their browser to install the
// app at all); android/README.md says so.
//
// WHY A CONTENT PROVIDER: since Android 7 an app may not hand another app a file:// path; it hands a
// content:// address the other app reads through a provider, with a one-time grant to read it
// (FLAG_GRANT_READ_URI_PERMISSION). AndroidX's FileProvider does that, but the APK carries no
// libraries (android/README.md: javac and d8 against android.jar, no Gradle), so this class is the
// provider: it serves exactly one kind of file, a downloaded update in the app's own `updates`
// folder, read only, to whoever was granted it. Not exported (AndroidManifest.xml): nothing else on
// the phone can ask it for anything without that grant.
//
// WHY ACTION_VIEW: it is what a browser or the Files app sends when a person taps a downloaded APK,
// so the phone's own installer opens exactly as it did for the first install.
// ACTION_INSTALL_PACKAGE has been deprecated since Android 10 (API 29) in favour of the
// PackageInstaller session API, which would need a broadcast receiver and its status codes as well;
// ACTION_VIEW needs neither.
//
// The folder's name and the provider's authority are also written in src/updater/phone.rs
// (UPDATE_FOLDER) and AndroidManifest.xml; a test there reads this file and fails when they
// disagree. The signature Rust calls is listed in crates/humanity-core/src/platform/android_java.rs,
// whose test reads this file too.
package us.united_humanity.humanityos;

import android.app.Activity;
import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Intent;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileNotFoundException;

public final class UpdateInstall extends ContentProvider {
    // The provider's authority, as AndroidManifest.xml declares it.
    static final String AUTHORITY = "us.united_humanity.humanityos.update";
    // The folder inside the app's own files folder (getFilesDir(), the HOME src/android.rs sets)
    // where src/updater/phone.rs puts a downloaded, checked update.
    static final String FOLDER = "updates";
    private static final String APK_TYPE = "application/vnd.android.package-archive";

    // Hand the update `name` to Android's installer. Returns "" once the installer was started,
    // otherwise why not, for the app to say. Called on the app's own thread; startActivity is a
    // call into the system and needs no particular thread.
    public static String install(final Activity from, final String name) {
        if (!safeName(name)) {
            return "not an update file: " + name;
        }
        File apk = new File(new File(from.getFilesDir(), FOLDER), name);
        if (!apk.isFile()) {
            return "missing: " + apk;
        }
        Intent view = new Intent(Intent.ACTION_VIEW);
        view.setDataAndType(Uri.parse("content://" + AUTHORITY + "/" + name), APK_TYPE);
        view.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        try {
            from.startActivity(view);
            return "";
        } catch (RuntimeException e) {
            // ActivityNotFoundException (no installer answered) or a SecurityException.
            return String.valueOf(e);
        }
    }

    // A downloaded update's name, as src/updater/phone.rs makes it: HumanityOS-<version>.apk.
    // Anything else (another file, a path with a slash, "..") is refused, so the provider never
    // serves anything but an update.
    static boolean safeName(String name) {
        return name != null && name.matches("HumanityOS-[0-9A-Za-z.\\-]+\\.apk") && !name.contains("..");
    }

    // The file a content:// address names, only when it is one of ours and exists.
    private File fileFor(Uri uri) throws FileNotFoundException {
        String name = uri.getLastPathSegment();
        if (uri.getPathSegments().size() != 1 || !safeName(name) || getContext() == null) {
            throw new FileNotFoundException(String.valueOf(uri));
        }
        File file = new File(new File(getContext().getFilesDir(), FOLDER), name);
        if (!file.isFile()) {
            throw new FileNotFoundException(name);
        }
        return file;
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    // Read only: the installer reads the update; nobody writes through this provider.
    @Override
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        if (!"r".equals(mode)) {
            throw new FileNotFoundException("read only: " + mode);
        }
        return ParcelFileDescriptor.open(fileFor(uri), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    @Override
    public String getType(Uri uri) {
        return APK_TYPE;
    }

    // The update's name and size, which an installer may ask for to show the file.
    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] selectionArgs, String sortOrder) {
        File file;
        try {
            file = fileFor(uri);
        } catch (FileNotFoundException e) {
            return null;
        }
        String[] columns = projection != null ? projection : new String[] { OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE };
        MatrixCursor cursor = new MatrixCursor(columns, 1);
        Object[] row = new Object[columns.length];
        for (int i = 0; i < columns.length; i++) {
            if (OpenableColumns.DISPLAY_NAME.equals(columns[i])) {
                row[i] = file.getName();
            } else if (OpenableColumns.SIZE.equals(columns[i])) {
                row[i] = file.length();
            }
        }
        cursor.addRow(row);
        return cursor;
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        return null;
    }

    @Override
    public int delete(Uri uri, String selection, String[] selectionArgs) {
        return 0;
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) {
        return 0;
    }
}
