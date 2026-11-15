//! The phone build's entry point (Android rung A1, 2026-11-14; docs/design/platforms.md,
//! android/README.md).
//!
//! The operator, 2026-11-14: "It would be nice to have the native app on my one plus nine pro
//! instead of the PWA version." On Android the app is this same library built as a shared object
//! (`cargo rustc --lib --crate-type cdylib` for aarch64-linux-android, in
//! .github/workflows/build-android.yml) and loaded by Android's NativeActivity, which calls
//! [`android_main`] on its own thread with the activity. android-activity (through winit's
//! `android-native-activity` feature) supplies the glue; the APK holds no Java code.
//!
//! WHAT RUNS BEFORE THE APP:
//! 1. **The app's own folder.** A phone app may write only inside its private folder (removed with
//!    the app, readable by no other app). HOME points there before anything else runs, so every
//!    path the desktop builds from HOME (the storage root, the logs, the config with the encrypted
//!    identity: `humanity_core::storage::os_root`) lands inside it, and TMPDIR points at a folder
//!    in it (Android's default, /data/local/tmp, is not an app's to write). The working folder is
//!    the app's storage root, so a relative `data/` resolves to the data written out on first run.
//! 2. **The log to logcat.** stdout and stderr go nowhere on Android; they are piped to the
//!    system log under the tag `HumanityOS`, so `adb logcat -s HumanityOS` shows every log line and
//!    a panic's message (the run.log and crash.log files are written as on a desktop, inside the
//!    app's folder).
//! 3. **The app**, as a desktop runs it (`crate::run_android`), on an event loop built on the
//!    activity.

use std::path::{Path, PathBuf};

use winit::platform::android::activity::AndroidApp;

/// Called by android-activity's NativeActivity glue on the app's main thread.
#[no_mangle]
fn android_main(app: AndroidApp) {
    logcat::pipe_std_streams();
    match app.internal_data_path() {
        Some(home) => {
            let root = use_app_folder(&home);
            // The data files compiled into the app first (and their stamp, which the data
            // folder's refresh keeps current), then the rest of the page files the APK carries.
            humanity_core::storage::extract_data_if_needed();
            unpack_apk_files(&app, &root);
        }
        None => eprintln!("HumanityOS: Android gave no private folder; files will not be saved"),
    }
    crate::run_android(app);
}

/// The page files the APK carries (android/build-apk.sh: `assets/files/<path>`, listed in
/// `assets/files.txt`, made by scripts/lib/release-files.js --phone-list) copied into the app's
/// folder, once per version of the app: `apk-files.stamp` holds the version and the list's size,
/// so an updated APK writes its files again and a start after that writes nothing.
///
/// The data files compiled into the app are skipped: the data folder's refresh owns them
/// (crates/humanity-core/src/storage/refresh.rs), keeping a person's edits across updates, as on a
/// desktop. Everything else is written over on each new version, the way a desktop's new download
/// replaces its folder. A path that would leave the folder is refused.
///
/// On the start's own thread, before the first frame (a few seconds the first time: about 135 MiB
/// on 2026-11-14). Android's input watchdog does not run before the window exists.
fn unpack_apk_files(app: &AndroidApp, root: &Path) {
    use std::io::Read;
    let assets = app.asset_manager();
    let Some(mut list) = assets.open(c"files.txt") else {
        eprintln!("HumanityOS: this APK carries no files.txt; only the data compiled into the app is here");
        return;
    };
    let mut text = String::new();
    if let Err(e) = list.read_to_string(&mut text) {
        eprintln!("HumanityOS: files.txt could not be read: {e}");
        return;
    }
    let stamp_path = root.join("apk-files.stamp");
    let stamp = format!("{} {}\n", env!("CARGO_PKG_VERSION"), text.len());
    if std::fs::read_to_string(&stamp_path).ok().as_deref() == Some(stamp.as_str()) {
        return;
    }
    let compiled_in: std::collections::HashSet<&str> =
        humanity_core::embedded_data::EMBEDDED_KEYS.iter().copied().collect();
    let started = std::time::Instant::now();
    let (mut files, mut bytes, mut failed) = (0u32, 0u64, 0u32);
    for rel in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if rel.starts_with('/') || rel.split('/').any(|part| part == ".." || part.is_empty()) {
            eprintln!("HumanityOS: refused a path that leaves the app's folder: {rel}");
            failed += 1;
            continue;
        }
        if rel.strip_prefix("data/").is_some_and(|d| compiled_in.contains(d)) {
            continue;
        }
        let Ok(name) = std::ffi::CString::new(format!("files/{rel}")) else { continue };
        let Some(mut asset) = assets.open(&name) else {
            eprintln!("HumanityOS: {rel} is listed but not in the APK");
            failed += 1;
            continue;
        };
        let dest = root.join(rel);
        if let Some(dir) = dest.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::File::create(&dest).and_then(|mut out| std::io::copy(&mut asset, &mut out)) {
            Ok(n) => {
                files += 1;
                bytes += n;
            }
            Err(e) => {
                eprintln!("HumanityOS: {rel} could not be written: {e}");
                failed += 1;
            }
        }
    }
    eprintln!(
        "HumanityOS: wrote {files} files ({:.1} MiB) from the APK into {} in {:.1} s{}",
        bytes as f64 / 1_048_576.0,
        root.display(),
        started.elapsed().as_secs_f32(),
        if failed > 0 { format!("; {failed} not written") } else { String::new() }
    );
    // Stamped only when every file was written, so a start after a failure tries again.
    if failed == 0 {
        let _ = std::fs::write(&stamp_path, stamp);
    }
}

/// Point HOME, TMPDIR and the working folder at the app's private folder (the module docs), and
/// return the app's storage root (`humanity_core::storage::os_root`, under HOME).
/// Runs before any other thread exists, so setting the process's environment here is sound.
fn use_app_folder(home: &Path) -> PathBuf {
    let tmp = home.join("tmp");
    let _ = std::fs::create_dir_all(&tmp);
    std::env::set_var("HOME", home);
    std::env::set_var("TMPDIR", &tmp);
    let root: PathBuf = humanity_core::storage::os_root().unwrap_or_else(|| home.to_path_buf());
    let _ = std::fs::create_dir_all(&root);
    if let Err(e) = std::env::set_current_dir(&root) {
        eprintln!("HumanityOS: could not work in {}: {e}", root.display());
    }
    eprintln!("HumanityOS: the app's folder is {}", root.display());
    root
}

/// stdout and stderr into the system log (the module docs).
mod logcat {
    use std::ffi::CString;
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;
    use std::os::raw::{c_char, c_int};

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    /// android/log.h's ANDROID_LOG_INFO.
    const INFO: c_int = 4;

    /// Replace stdout and stderr with a pipe whose lines a thread writes to logcat. Nothing
    /// happens when the pipe cannot be made (the app runs on; its log files are still written).
    pub fn pipe_std_streams() {
        let mut fds = [0 as libc::c_int; 2];
        // SAFETY: pipe fills two descriptors on success; dup2 and close take descriptors we own.
        unsafe {
            if libc::pipe(fds.as_mut_ptr()) != 0 {
                return;
            }
            libc::dup2(fds[1], libc::STDOUT_FILENO);
            libc::dup2(fds[1], libc::STDERR_FILENO);
            libc::close(fds[1]);
        }
        let read_end = fds[0];
        let _ = std::thread::Builder::new().name("logcat".into()).spawn(move || {
            // SAFETY: the read end is ours alone from here on.
            let file = unsafe { std::fs::File::from_raw_fd(read_end) };
            let tag = CString::new("HumanityOS").expect("no NUL in the tag");
            for line in BufReader::new(file).split(b'\n') {
                let Ok(mut bytes) = line else { break };
                bytes.retain(|b| *b != 0);
                if let Ok(text) = CString::new(bytes) {
                    // SAFETY: both pointers are NUL-terminated strings that outlive the call.
                    unsafe {
                        __android_log_write(INFO, tag.as_ptr(), text.as_ptr());
                    }
                }
            }
        });
    }
}
