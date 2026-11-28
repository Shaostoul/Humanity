//! The Java the phone app carries, and every call the Rust side makes into it (Android rungs A1
//! and A4, 2026-11-20; android/README.md "The Java in the APK").
//!
//! The app is a NativeActivity, so until now the APK held no Java at all. Two things need some:
//! a foreground service (rung A4, "stay connected while closed") is a Java class Android
//! instantiates itself, and Android's key store is a Java interface (since then the window's
//! insets, `VisibleArea`, and Attach's document picker, `PickFiles`, an activity of its own because
//! only an activity receives a picker's answer, 2026-11-23). The Java is a few small
//! classes under `android/java/`, compiled by android/build-apk.sh with `javac` against the
//! platform's android.jar and turned into `classes.dex` with `d8`, both from the SDK on GitHub's
//! runner (no Gradle). The key store's work is done in Java (`KeyStoreBridge`) rather than as a
//! chain of JNI calls from Rust because `javac` checks every Android class, method and argument
//! against android.jar when the APK is built, where a JNI call names a method in a string that
//! only fails on a phone.
//!
//! What `javac` cannot check is the other direction: that each call Rust makes (a class, a
//! method, a JNI signature, below) names a method the Java declares with those argument types.
//! [`ALL`] lists them, and the test beside it reads the Java sources and fails on any mismatch.

/// The app's Java package (the manifest's `package`).
pub const PACKAGE: &str = "us.united_humanity.humanityos";

/// One static method the Rust side calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaCall {
    /// The class's simple name, in [`PACKAGE`].
    pub class: &'static str,
    pub method: &'static str,
    /// The JNI signature, as `javap -s` prints it.
    pub sig: &'static str,
}

impl JavaCall {
    /// The class's binary name, as `ClassLoader.loadClass` takes it.
    pub fn class_name(&self) -> String {
        format!("{PACKAGE}.{}", self.class)
    }
}

/// Make a fresh sealing key under an alias (replacing any): returns where it lives
/// (`key_wrap::Protection` codes), plus `key_wrap::UNLOCKED_ONLY` when it carries Android's
/// unlocked-only rule (`key_wrap::KeyInfo::from_code`).
pub const KEY_CREATE: JavaCall = JavaCall { class: "KeyStoreBridge", method: "create", sig: "(Ljava/lang/String;)I" };
/// Seal bytes with an alias's key and associated data: returns the nonce, then the sealed bytes.
pub const KEY_SEAL: JavaCall = JavaCall { class: "KeyStoreBridge", method: "seal", sig: "(Ljava/lang/String;[B[B)[B" };
/// Open sealed bytes (alias, nonce, sealed, associated data): null when the key is gone.
pub const KEY_UNSEAL: JavaCall = JavaCall { class: "KeyStoreBridge", method: "unseal", sig: "(Ljava/lang/String;[B[B[B)[B" };
/// Delete an alias's key.
pub const KEY_FORGET: JavaCall = JavaCall { class: "KeyStoreBridge", method: "forget", sig: "(Ljava/lang/String;)V" };
/// Start the stay-connected service (context, channel name, title, text): false when Android
/// refused it.
pub const SERVICE_START: JavaCall = JavaCall {
    class: "ChatService",
    method: "start",
    sig: "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z",
};
/// Stop it.
pub const SERVICE_STOP: JavaCall = JavaCall { class: "ChatService", method: "stop", sig: "(Landroid/content/Context;)V" };
/// Post a notice (context, channel name, tag, title, body, whether the person chose a line on the
/// locked screen, that line's title, that line's body).
pub const NOTICE_SHOW: JavaCall = JavaCall {
    class: "Notices",
    method: "show",
    sig: "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZLjava/lang/String;Ljava/lang/String;)V",
};
/// Take a notice back by its tag.
pub const NOTICE_WITHDRAW: JavaCall = JavaCall { class: "Notices", method: "withdraw", sig: "(Landroid/content/Context;Ljava/lang/String;)V" };
/// Whether Android lets the app show notices.
pub const NOTICES_ALLOWED: JavaCall = JavaCall { class: "Notices", method: "allowed", sig: "(Landroid/content/Context;)Z" };
/// Ask the person to allow notices (Android 13 and later; nothing earlier).
pub const NOTICES_ASK: JavaCall = JavaCall { class: "Notices", method: "ask", sig: "(Landroid/app/Activity;)V" };
/// Start keeping the window's insets: the system bars, a display cutout, the on-screen keyboard
/// (2026-11-22, src/gui/visible_area.rs).
pub const VISIBLE_AREA_LISTEN: JavaCall = JavaCall { class: "VisibleArea", method: "listen", sig: "(Landroid/app/Activity;)V" };
/// The latest insets (left, top, right, bottom, in the window's pixels): null before Android first
/// reported them.
pub const VISIBLE_AREA_INSETS: JavaCall = JavaCall { class: "VisibleArea", method: "insets", sig: "()[I" };

/// Open Android's document picker over the app, for Attach (2026-11-23, PickFiles.java; the app's
/// own file browser cannot see the phone's pictures under scoped storage).
pub const PICK_FILES_START: JavaCall = JavaCall { class: "PickFiles", method: "start", sig: "(Landroid/app/Activity;)V" };
/// The picker's answer, taken: "picked" then each chosen document's URI, "cancelled", or "failed"
/// then why; null while the person has not answered.
pub const PICK_FILES_TAKE: JavaCall = JavaCall { class: "PickFiles", method: "take", sig: "()[Ljava/lang/String;" };
/// A chosen document's name and size in bytes ("-1" when its provider does not know it).
pub const PICK_FILES_DESCRIBE: JavaCall = JavaCall {
    class: "PickFiles",
    method: "describe",
    sig: "(Landroid/content/Context;Ljava/lang/String;)[Ljava/lang/String;",
};
/// Copy a chosen document, streamed, into a file in the app's folder, at most so many bytes: the
/// bytes copied, or -1 when it holds more.
pub const PICK_FILES_COPY: JavaCall = JavaCall {
    class: "PickFiles",
    method: "copy",
    sig: "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;J)J",
};

/// Hand the downloaded, checked update (its file name in the app's `updates` folder) to Android's
/// installer, which asks the person to confirm it (2026-11-27, UpdateInstall.java, src/updater/phone.rs):
/// "" once the installer was started, otherwise why not.
pub const UPDATE_INSTALL: JavaCall = JavaCall {
    class: "UpdateInstall",
    method: "install",
    sig: "(Landroid/app/Activity;Ljava/lang/String;)Ljava/lang/String;",
};

/// Every call, for the test against the Java sources.
pub const ALL: [JavaCall; 17] = [
    KEY_CREATE,
    KEY_SEAL,
    KEY_UNSEAL,
    KEY_FORGET,
    SERVICE_START,
    SERVICE_STOP,
    NOTICE_SHOW,
    NOTICE_WITHDRAW,
    NOTICES_ALLOWED,
    NOTICES_ASK,
    VISIBLE_AREA_LISTEN,
    VISIBLE_AREA_INSETS,
    PICK_FILES_START,
    PICK_FILES_TAKE,
    PICK_FILES_DESCRIBE,
    PICK_FILES_COPY,
    UPDATE_INSTALL,
];

/// The JNI descriptor of a Java type as the sources write it (the few the calls use).
#[cfg(test)]
fn descriptor(java: &str) -> Option<String> {
    Some(match java {
        "int" => "I".into(),
        "long" => "J".into(),
        "boolean" => "Z".into(),
        "void" => "V".into(),
        "byte[]" => "[B".into(),
        "int[]" => "[I".into(),
        "String" => "Ljava/lang/String;".into(),
        "String[]" => "[Ljava/lang/String;".into(),
        "Context" => "Landroid/content/Context;".into(),
        "Activity" => "Landroid/app/Activity;".into(),
        _ => return None,
    })
}

/// The JNI signature of every `public static` method in a Java source, by name.
#[cfg(test)]
fn java_signatures(src: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for line in src.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("public static ") else { continue };
        let Some(open) = rest.find('(') else { continue };
        let Some(close) = rest.find(')') else { continue };
        let head: Vec<&str> = rest[..open].split_whitespace().collect();
        let [ret, name] = head[..] else { continue };
        let mut sig = String::from("(");
        let mut ok = true;
        for param in rest[open + 1..close].split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let ty = param.split_whitespace().find(|w| *w != "final").unwrap_or("");
            match descriptor(ty) {
                Some(d) => sig.push_str(&d),
                None => ok = false,
            }
        }
        sig.push(')');
        match descriptor(ret) {
            Some(d) if ok => {
                sig.push_str(&d);
                out.insert(name.to_string(), sig);
            }
            _ => {
                out.insert(name.to_string(), format!("<a type the test does not know: {line}>"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Each call Rust makes names a `public static` method its Java class declares with exactly
    /// that JNI signature. Seen red 2026-11-20 with KEY_SEAL's signature written
    /// "(Ljava/lang/String;[B)[B" (the associated data left out): "KeyStoreBridge.seal: Rust calls
    /// (Ljava/lang/String;[B)[B, the Java declares (Ljava/lang/String;[B[B)[B".
    #[test]
    fn every_call_rust_makes_matches_the_java() {
        let dir = Path::new(env!("HUMANITY_REPO_ROOT")).join("android/java").join(PACKAGE.replace('.', "/"));
        let mut problems = Vec::new();
        for call in ALL {
            let path = dir.join(format!("{}.java", call.class));
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(src.contains(&format!("package {PACKAGE};")), "{} is not in package {PACKAGE}", path.display());
            match java_signatures(&src).get(call.method) {
                Some(sig) if sig == call.sig => {}
                Some(sig) => problems.push(format!("{}.{}: Rust calls {}, the Java declares {sig}", call.class, call.method, call.sig)),
                None => problems.push(format!("{}.{}: no public static method of that name", call.class, call.method)),
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    fn java(class: &str) -> String {
        let path = Path::new(env!("HUMANITY_REPO_ROOT")).join("android/java").join(PACKAGE.replace('.', "/")).join(format!("{class}.java"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// The text of the method that starts with `head` (up to its closing brace at the method's own
    /// indentation), with `//` comments taken out.
    fn method<'a>(src: &'a str, head: &str) -> String {
        let start = src.find(head).unwrap_or_else(|| panic!("no method {head:?}"));
        let indent = src[..start].rsplit('\n').next().unwrap_or("").len();
        let close = format!("\n{}}}", " ".repeat(indent));
        let end = src[start..].find(&close).map(|e| start + e + close.len()).unwrap_or(src.len());
        src[start..end].lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n")
    }

    /// A message notice shows NOTHING on a locked phone's SECURE lock screen (a PIN, pattern or
    /// password; Android's own word for VISIBILITY_SECRET is "secure lockscreen", and a swipe-only
    /// lock shows every notice whole) unless the person chose the line (Settings, Notifications,
    /// off by default): VISIBILITY_SECRET. VISIBILITY_PRIVATE shows the
    /// whole notice there on every phone whose "hide sensitive content" setting is off, which is
    /// Android's default, so it is used only on the person's choice; VISIBILITY_PUBLIC never (the
    /// review's finding 1, 2026-11-20). Seen red 2026-11-20 against the Notices.java that set
    /// VISIBILITY_PRIVATE on every notice: "Notices.show sets the visibility without the person's
    /// choice".
    #[test]
    fn message_notices_show_nothing_on_a_locked_screen_by_default() {
        let src = java("Notices");
        assert!(!src.contains("VISIBILITY_PUBLIC"), "a notice is never fully public on a locked screen");
        let show = method(&src, "public static void show(");
        assert!(show.contains("setVisibility(lockScreenVisibility(lockScreenLine))"), "Notices.show sets the visibility without the person's choice:\n{show}");
        assert!(!show.contains("VISIBILITY_PRIVATE") && !show.contains("VISIBILITY_SECRET"), "Notices.show picks a visibility of its own:\n{show}");
        let choice = method(&src, "static int lockScreenVisibility(boolean lineChosen)");
        let default_branch = choice.rsplit_once("if (lineChosen)").map(|(_, rest)| rest).unwrap_or("");
        let after_if = default_branch.split_once('}').map(|(_, rest)| rest).unwrap_or("");
        assert!(after_if.contains("return Notification.VISIBILITY_SECRET;"), "without the person's choice a notice is VISIBILITY_SECRET:\n{choice}");
        assert!(!after_if.contains("VISIBILITY_PRIVATE"), "VISIBILITY_PRIVATE outside the person's choice:\n{choice}");
    }

    /// The setting's words say what a locked screen really shows: nothing only on a phone with a
    /// screen lock (a PIN, pattern or password), and the whole notice on a phone locked only by a
    /// swipe, whatever is chosen, since Android hides VISIBILITY_SECRET and VISIBILITY_PRIVATE only
    /// on a secure lock screen (the second review's D). Seen red 2026-11-20 against the words that
    /// said a locked phone "shows nothing of a message notice" on every phone: "the lock-screen
    /// words must say a screen lock (a PIN, pattern or password) is needed".
    #[test]
    fn the_lock_screen_words_say_a_screen_lock_is_needed() {
        let en: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap()).unwrap();
        let hint = en.pointer("/notifications/lock_screen_line_hint").and_then(|v| v.as_str()).unwrap_or_default();
        assert!(hint.contains("PIN, pattern or password"), "the lock-screen words must say a screen lock (a PIN, pattern or password) is needed: {hint}");
        assert!(hint.contains("swipe") && hint.contains("whole notice whatever you choose"), "the lock-screen words must say a swipe-only lock shows the whole notice: {hint}");
    }

    /// Stopping the stay-connected service right after starting it never crashes the app: Android
    /// kills an app whose service started with startForegroundService ends before calling
    /// startForeground, so `stop` calls stopService only once the service is in the foreground,
    /// and the service stops itself, after startForeground, when it was told to stop before it got
    /// there (the review's finding 7). Seen red 2026-11-20 against the ChatService.java whose stop
    /// called stopService at once: "ChatService.stop must not call stopService before the service
    /// is in the foreground".
    #[test]
    fn stopping_the_service_before_it_started_cannot_crash() {
        let src = java("ChatService");
        let stop = method(&src, "public static void stop(");
        let guard = stop.find("if (!foreground)");
        let call = stop.find("stopService(");
        assert!(guard.is_some_and(|g| call.is_some_and(|c| g < c)), "ChatService.stop must not call stopService before the service is in the foreground:\n{stop}");
        let start = method(&src, "public int onStartCommand(");
        let fg = start.find("startForeground(").expect("onStartCommand calls startForeground");
        let selfstop = start.find("stopSelf(").expect("onStartCommand stops itself when told to stop before it started");
        assert!(fg < selfstop, "the service stops itself only after startForeground:\n{start}");
        assert!(src.contains("public void onDestroy()"), "onDestroy notes the service left the foreground");
    }

    /// Off, on, off before onDestroy has run never stops the new instance before its
    /// startForeground: `foreground` means "the latest start has been answered by
    /// startForeground", so start sets it false (a new one is owed) and stop sets it false as it
    /// stops the service, each under LOCK together with its system call, so a start and a stop
    /// never interleave (the second review's E). Seen red 2026-11-20 against the ChatService.java
    /// whose stop left `foreground` true until onDestroy: "stop must note the service left the
    /// foreground before stopService".
    #[test]
    fn a_stop_then_start_never_stops_the_new_instance_early() {
        let src = java("ChatService");
        let stop = method(&src, "public static void stop(");
        let start = method(&src, "public static boolean start(");
        let before = |m: &str, first: &str, then: &str| m.find(first).is_some_and(|a| m.find(then).is_some_and(|b| a < b));
        assert!(before(&stop, "foreground = false;", "stopService("), "stop must note the service left the foreground before stopService:\n{stop}");
        assert!(before(&start, "foreground = false;", "startForegroundService("), "start must note a new startForeground is owed before startForegroundService:\n{start}");
        for (m, call) in [(&stop, "stopService("), (&start, "startForegroundService(")] {
            assert_eq!(m.matches("synchronized (LOCK)").count(), 1, "one LOCK block:\n{m}");
            assert!(before(m, "synchronized (LOCK)", call), "{call} must run under LOCK, so a start and a stop never interleave:\n{m}");
        }
    }

    /// The plaintext copies on the Java side are wiped: the 32 bytes handed to `seal` once sealed,
    /// and the array `unseal` returns once Rust has copied it (android_jni.rs writes zeros over it)
    /// (the review's finding 5). Seen red 2026-11-20 against the KeyStoreBridge.java without the
    /// wipe: "KeyStoreBridge.seal must wipe the plaintext it was handed".
    #[test]
    fn the_java_plaintext_copies_are_wiped() {
        let seal = method(&java("KeyStoreBridge"), "public static byte[] seal(");
        assert!(seal.contains("finally") && seal.contains("Arrays.fill(plain, (byte) 0)"), "KeyStoreBridge.seal must wipe the plaintext it was handed:\n{seal}");
        let jni = std::fs::read_to_string(Path::new(env!("HUMANITY_REPO_ROOT")).join("crates/humanity-core/src/platform/android_jni.rs")).unwrap();
        let unseal = method(&jni, "pub fn key_unseal(");
        assert!(unseal.contains("bytes_of(env, out, Wipe::Yes)"), "key_unseal must wipe the Java array it copied:\n{unseal}");
        assert!(jni.contains("set_region(env, 0, &zeros)"), "the wipe writes zeros over the Java array");
    }

    /// The reader itself: a known declaration comes out as its JNI signature.
    #[test]
    fn the_java_reader_writes_jni_signatures() {
        let sigs = java_signatures("    public static byte[] seal(String alias, byte[] plain, byte[] aad) throws Exception {\n    public static void ask(final Activity a) {");
        assert_eq!(sigs["seal"], "(Ljava/lang/String;[B[B)[B");
        assert_eq!(sigs["ask"], "(Landroid/app/Activity;)V");
    }
}
