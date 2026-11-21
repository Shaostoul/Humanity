//! The phone app's calls into its own Java (Android rungs A1 and A4, 2026-11-20): Android's key
//! store, the stay-connected service and the notices. Only on Android.
//!
//! Each call attaches the calling thread to the Java VM (cheap when it already is), loads the
//! class through the Application's class loader (a thread the VM did not start sees only
//! Android's own classes through `FindClass`; the Application's loader also sees the app's, so
//! this works from the notices' worker thread as well as the app's main thread), and calls one
//! static method listed in `android_java` (whose test checks every signature against the Java
//! sources). The Java VM and the Application come from ndk-context, which android-activity fills
//! in before `android_main` runs. A Java exception comes back as an `Err`, never a crash: the
//! whole of it, its stack trace included, goes to the log, and the `Err` carries only its class
//! and message (2026-11-20, the review's finding 6). Nothing here is shown on the screen; Settings
//! says a sentence of its own by key (`auto_unlock::RememberError::words_key`).

use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::signature::RuntimeMethodSignature;
use jni::strings::JNIString;
use jni::{Env, JValue, JValueOwned, JavaVM};

use super::android_java::{self as java, JavaCall};

/// Run `f` with this thread attached to the app's Java VM and the Application object. `what`
/// names the call in the log.
fn with_app<T>(what: &str, f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    let ctx = ndk_context::android_context();
    if ctx.vm().is_null() || ctx.context().is_null() {
        return Err("Android has not handed the app its Java VM yet".into());
    }
    // SAFETY: ndk-context's pointers are the process's JavaVM and a global reference to the
    // Application, set once by android-activity and never released.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) };
    let raw: jni::sys::jobject = ctx.context().cast();
    vm.attach_current_thread(|env| -> jni::errors::Result<T> {
        // SAFETY: a valid global reference (above) that outlives this call; the cast does not
        // take ownership, so it is never deleted here.
        let app = unsafe { env.as_cast_raw::<JObject>(&raw) }?;
        f(env, &app)
    })
    .map_err(|e| {
        log::warn!("android: {what}: {e}");
        short(&e)
    })
}

/// A JNI error without a Java stack trace: the exception's class and message.
fn short(e: &jni::errors::Error) -> String {
    match e {
        jni::errors::Error::CaughtJavaException { name, msg, .. } => format!("{name}: {msg}"),
        other => other.to_string().lines().next().unwrap_or_default().to_string(),
    }
}

/// Whether [`bytes_of`] wipes the Java array once copied: yes for a plaintext.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wipe {
    Yes,
    No,
}

/// The app's class `call.class`, through the Application's class loader.
fn app_class<'l>(env: &mut Env<'l>, app: &JObject, call: &JavaCall) -> jni::errors::Result<JClass<'l>> {
    let loader = env
        .call_method(app, JNIString::from("getClassLoader"), RuntimeMethodSignature::from_str("()Ljava/lang/ClassLoader;")?.method_signature(), &[])?
        .l()?;
    let name = env.new_string(call.class_name())?;
    let class = env
        .call_method(
            &loader,
            JNIString::from("loadClass"),
            RuntimeMethodSignature::from_str("(Ljava/lang/String;)Ljava/lang/Class;")?.method_signature(),
            &[JValue::Object(&name)],
        )?
        .l()?;
    env.cast_local::<JClass>(class)
}

/// Call the static method `call` with `args`.
fn call_static<'l>(env: &mut Env<'l>, app: &JObject, call: &JavaCall, args: &[JValue]) -> jni::errors::Result<JValueOwned<'l>> {
    let class = app_class(env, app, call)?;
    let sig = RuntimeMethodSignature::from_str(call.sig)?;
    env.call_static_method(&class, JNIString::from(call.method), sig.method_signature(), args)
}

/// A Java `byte[]` as bytes; `None` for null. With [`Wipe::Yes`] the Java array is overwritten
/// with zeros once copied, so the plaintext does not wait in the Java heap for the garbage
/// collector (2026-11-20, the review's finding 5); the caller wipes the `Vec` once used.
fn bytes_of(env: &mut Env, obj: JObject, wipe: Wipe) -> jni::errors::Result<Option<Vec<u8>>> {
    if obj.is_null() {
        return Ok(None);
    }
    let array = env.cast_local::<JByteArray>(obj)?;
    let mut bytes = env.convert_byte_array(&array)?;
    if wipe == Wipe::Yes {
        let zeros = vec![0i8; bytes.len()];
        if let Err(e) = array.set_region(env, 0, &zeros) {
            super::key_wrap::wipe(&mut bytes);
            return Err(e);
        }
    }
    Ok(Some(bytes))
}

// ── Android's key store (rung A1) ───────────────────────────────────────────

/// Make a fresh AES-256-GCM key under `alias` inside Android's key store, replacing any: a
/// separate security chip (StrongBox) when the phone has one, else its secure hardware, else
/// software. Returns where it went (`key_wrap::Protection` codes), plus `key_wrap::UNLOCKED_ONLY`
/// when it carries Android's unlocked-only rule (`key_wrap::KeyInfo::from_code` reads both).
pub fn key_create(alias: &str) -> Result<i32, String> {
    with_app("key_create", |env, app| {
        let alias = env.new_string(alias)?;
        call_static(env, app, &java::KEY_CREATE, &[JValue::Object(&alias)])?.i()
    })
}

/// Seal `plain` with `alias`'s key, binding `aad`: the 12-byte nonce, then the sealed bytes. The
/// Java copy of `plain` is wiped by `KeyStoreBridge.seal` itself; the caller wipes its own.
pub fn key_seal(alias: &str, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    with_app("key_seal", |env, app| {
        let alias = env.new_string(alias)?;
        let plain = env.byte_array_from_slice(plain)?;
        let aad = env.byte_array_from_slice(aad)?;
        let out = call_static(env, app, &java::KEY_SEAL, &[JValue::Object(&alias), JValue::Object(&plain), JValue::Object(&aad)])?.l()?;
        Ok(bytes_of(env, out, Wipe::No)?.unwrap_or_default())
    })
}

/// Open `sealed` with `alias`'s key: `None` when the key store no longer holds that key. The Java
/// array holding the plaintext is overwritten with zeros once copied; the caller wipes the `Vec`.
pub fn key_unseal(alias: &str, nonce: &[u8], sealed: &[u8], aad: &[u8]) -> Result<Option<Vec<u8>>, String> {
    with_app("key_unseal", |env, app| {
        let alias = env.new_string(alias)?;
        let nonce = env.byte_array_from_slice(nonce)?;
        let sealed = env.byte_array_from_slice(sealed)?;
        let aad = env.byte_array_from_slice(aad)?;
        let out = call_static(
            env,
            app,
            &java::KEY_UNSEAL,
            &[JValue::Object(&alias), JValue::Object(&nonce), JValue::Object(&sealed), JValue::Object(&aad)],
        )?
        .l()?;
        bytes_of(env, out, Wipe::Yes)
    })
}

/// Delete `alias`'s key (nothing when there is none).
pub fn key_forget(alias: &str) -> Result<(), String> {
    with_app("key_forget", |env, app| {
        let alias = env.new_string(alias)?;
        call_static(env, app, &java::KEY_FORGET, &[JValue::Object(&alias)])?.v()
    })
}

// ── Staying connected and the notices (rung A4) ─────────────────────────────

/// Start the stay-connected service with its notification's words: false when Android refused
/// it (the reason is in logcat).
pub fn service_start(channel: &str, title: &str, text: &str) -> Result<bool, String> {
    with_app("service_start", |env, app| {
        let channel: JString = env.new_string(channel)?;
        let title = env.new_string(title)?;
        let text = env.new_string(text)?;
        call_static(env, app, &java::SERVICE_START, &[JValue::Object(app), JValue::Object(&channel), JValue::Object(&title), JValue::Object(&text)])?.z()
    })
}

/// Stop the stay-connected service (nothing when it is not running).
pub fn service_stop() -> Result<(), String> {
    with_app("service_stop", |env, app| call_static(env, app, &java::SERVICE_STOP, &[JValue::Object(app)])?.v())
}

/// The words of one notice: what it says, and what a locked phone's screen may show instead.
pub struct NoticeWords<'a> {
    pub channel: &'a str,
    pub tag: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    /// The person chose a line on the locked screen (Settings, Notifications, off by default).
    /// Without it a phone's secure lock screen (a PIN, pattern or password) shows nothing of the
    /// notice (`Notices.lockScreenVisibility`); a swipe-only lock shows it whole either way.
    pub lock_screen_line: bool,
    pub public_title: &'a str,
    pub public_body: &'a str,
}

/// Post a notice; one with the same tag replaces it.
pub fn notice_show(w: &NoticeWords) -> Result<(), String> {
    with_app("notice_show", |env, app| {
        let channel = env.new_string(w.channel)?;
        let tag = env.new_string(w.tag)?;
        let title = env.new_string(w.title)?;
        let body = env.new_string(w.body)?;
        let public_title = env.new_string(w.public_title)?;
        let public_body = env.new_string(w.public_body)?;
        let args = [
            JValue::Object(app),
            JValue::Object(&channel),
            JValue::Object(&tag),
            JValue::Object(&title),
            JValue::Object(&body),
            JValue::Bool(w.lock_screen_line.into()),
            JValue::Object(&public_title),
            JValue::Object(&public_body),
        ];
        call_static(env, app, &java::NOTICE_SHOW, &args)?.v()
    })
}

/// Take the notice tagged `tag` back.
pub fn notice_withdraw(tag: &str) -> Result<(), String> {
    with_app("notice_withdraw", |env, app| {
        let tag = env.new_string(tag)?;
        call_static(env, app, &java::NOTICE_WITHDRAW, &[JValue::Object(app), JValue::Object(&tag)])?.v()
    })
}

/// Whether Android lets the app show notices (the person can turn them off in Android's settings,
/// and on Android 13 and later they are off until allowed).
pub fn notices_allowed() -> Result<bool, String> {
    with_app("notices_allowed", |env, app| call_static(env, app, &java::NOTICES_ALLOWED, &[JValue::Object(app)])?.z())
}

/// The running NativeActivity, as a raw `jobject`, for the one call that needs an activity rather
/// than the Application (asking to allow notices). Set by the app's entry point (src/android.rs);
/// null until then.
static ACTIVITY: std::sync::atomic::AtomicPtr<std::ffi::c_void> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

/// Hand over the running NativeActivity (android-activity's `AndroidApp::activity_as_ptr`).
///
/// # Safety
/// `activity` must be null or a global reference to the NativeActivity that stays valid until it
/// is replaced by another call (src/android.rs keeps the `AndroidApp` that owns it alive).
pub unsafe fn set_activity(activity: *mut std::ffi::c_void) {
    ACTIVITY.store(activity, std::sync::atomic::Ordering::SeqCst);
}

/// Ask the person to allow notices (Android 13 and later), on the activity [`set_activity`]
/// handed over.
pub fn notices_ask() -> Result<(), String> {
    let activity = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if activity.is_null() {
        return Err("no activity on screen to ask from".into());
    }
    let raw: jni::sys::jobject = activity.cast();
    with_app("notices_ask", |env, app| {
        // SAFETY: set_activity's contract: a valid global reference until replaced.
        let activity = unsafe { env.as_cast_raw::<JObject>(&raw) }?;
        call_static(env, app, &java::NOTICES_ASK, &[JValue::Object(&activity)])?.v()
    })
}
