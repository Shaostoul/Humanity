//! The operating system's notice: the toast in the corner of the screen, kept in the notification
//! centre (docs/design/notifications.md, section 3).
//!
//! - Windows: the WinRT `ToastNotificationManager`, through the `windows` crate already in the
//!   tree. An unpackaged program is known to Windows by an AppUserModelID, so the first notice
//!   registers ours for this user only (`HKCU\Software\Classes\AppUserModelId\<AUMID>`:
//!   `DisplayName`, `IconUri`), the registration the Windows App SDK makes for unpackaged apps.
//! - Linux: the freedesktop notification service through `notify-send` (libnotify).
//! - macOS: `osascript`'s `display notification`, the words passed as arguments.
//!
//! A click on a notice (rung 2, part 2) hands its token back to the app ([`take_clicks`]), which
//! opens its conversation (`notifications::clicks`): on Windows through the notice activator, a COM
//! class the running app serves (`CustomActivator` in the same per-user registration); on Linux
//! through the notice's default action, when `notify-send` has `--action`. macOS's `osascript` has
//! no click to hand back: rung 5's `UNUserNotificationCenter` brings one.
//!
//! Every notice goes to ONE worker thread through a bounded queue: the frame never waits for COM,
//! D-Bus or a process start, and a burst beyond the queue is dropped rather than piling up. The
//! builders of what each platform is handed ([`toast_xml`], [`linux_args`], [`mac_args`]) are pure
//! and tested on every platform: a name cannot add a button, a link or a script line to a notice.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};

/// The id Windows knows HumanityOS's notices by.
pub const AUMID: &str = "UnitedHumanity.HumanityOS";
/// The app's name as the notice header shows it (a brand: the same in every language).
pub const APP_NAME: &str = "HumanityOS";

/// One notice, its words already chosen and made safe by `decide::words`.
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub title: String,
    pub body: String,
    /// Replaces an earlier notice with the same tag (one per conversation): a hash, never a name.
    pub tag: String,
    /// The kind's id: notices of one kind group together.
    pub group: String,
    /// What a click on it hands back (`clicks::Targets::token`): a token that names nothing. Empty:
    /// a click does nothing of ours.
    pub launch: String,
    /// How long Linux's notification service shows it, in milliseconds ([`EXPIRE_MS`]; a call's
    /// notice lasts its ring, rung 3, since `notify-send` cannot withdraw one it showed).
    pub expire_ms: u32,
}

/// How long a notice stays on Linux unless it says otherwise.
pub const EXPIRE_MS: u32 = 8000;

/// What the worker is asked to do.
enum Job {
    Show(Toast),
    /// Take a notice off the screen and out of the notification centre (rung 3: a call's, when the
    /// ring ends), by its tag and group.
    Withdraw { tag: String, group: String },
}

/// How many notices may wait for the worker before more are dropped.
const QUEUE: usize = 8;

/// Hand `toast` to the worker. Never blocks; false when the worker could not be started or is
/// behind (the notice is dropped and the reason logged once).
pub fn show(toast: Toast) -> bool {
    send(Job::Show(toast))
}

/// Withdraw the notice tagged `tag` in `group` (rung 3). Windows removes it from the screen and the
/// notification centre; Linux's `notify-send` and macOS's `osascript` cannot (rung 5's D-Bus and
/// `UNUserNotificationCenter` can), so there a call's notice lasts only its ring. Never blocks.
pub fn withdraw(tag: &str, group: &str) -> bool {
    send(Job::Withdraw { tag: tag.to_string(), group: group.to_string() })
}

fn send(job: Job) -> bool {
    let Some(tx) = worker() else { return false };
    match tx.try_send(job) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            log::debug!("notifications: the notice queue is full, one dropped");
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

fn worker() -> Option<&'static SyncSender<Job>> {
    static TX: OnceLock<Option<SyncSender<Job>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = sync_channel::<Job>(QUEUE);
        let spawned = std::thread::Builder::new().name("hos-notify".into()).spawn(move || {
            let mut backend = Backend::start();
            for job in rx {
                match job {
                    Job::Show(t) => {
                        if let Err(e) = backend.show(&t) {
                            log::warn!("notifications: the notice was not shown: {e}");
                        }
                    }
                    Job::Withdraw { tag, group } => backend.withdraw(&tag, &group),
                }
            }
        });
        match spawned {
            Ok(_) => Some(tx),
            Err(e) => {
                log::warn!("notifications: could not start the notice thread: {e}");
                None
            }
        }
    })
    .as_ref()
}

// ── Clicks on a notice (rung 2, part 2) ─────────────────────────────────────────────────────────

/// Clicks are taken only after the engine turned them on ([`enable_clicks`]): a person's launch,
/// never an instance a script started (a rig), so a rig never answers a click on the person's own
/// notice.
static CLICKS_ON: AtomicBool = AtomicBool::new(false);
/// A click is waiting: one atomic load per frame when none is.
static CLICKED: AtomicBool = AtomicBool::new(false);
/// The tokens of the notices clicked, oldest first, at most [`CLICKS_KEPT`].
static CLICKS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Wakes the app's event loop, so a click reaches a window waiting hidden in the tray.
static WAKER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
const CLICKS_KEPT: usize = 16;

/// Turn clicks on: from now on a click on one of this run's notices is handed to [`take_clicks`]
/// and `wake` is called. On Windows this registers the notice activator (a COM class this process
/// serves while it runs, `win_activate`) and names it in the per-user registration beside the
/// AppUserModelID; on Linux notices carry a default action when `notify-send` has `--action`.
pub fn enable_clicks(wake: Box<dyn Fn() + Send + Sync>) {
    let _ = WAKER.set(wake);
    CLICKS_ON.store(true, Ordering::SeqCst);
    // The worker registers the activator when it starts: start it now, so a click on the very
    // first notice is answered.
    let _ = worker();
}

/// A notice whose token is `launch` was clicked (called from the platform's thread).
pub(crate) fn clicked(launch: String) {
    if !CLICKS_ON.load(Ordering::SeqCst) {
        return;
    }
    log::info!("notifications: a notice was clicked");
    if let Ok(mut q) = CLICKS.lock() {
        if q.len() >= CLICKS_KEPT {
            q.remove(0);
        }
        q.push(launch);
    }
    CLICKED.store(true, Ordering::SeqCst);
    if let Some(wake) = WAKER.get() {
        wake();
    }
}

/// A click is waiting for [`take_clicks`].
pub fn clicks_waiting() -> bool {
    CLICKED.load(Ordering::SeqCst)
}

/// The clicks since the last call, oldest first. One atomic load when there are none.
pub fn take_clicks() -> Vec<String> {
    if !CLICKED.swap(false, Ordering::SeqCst) {
        return Vec::new();
    }
    CLICKS.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

/// A short, stable tag for `conversation` (FNV-1a, 16 hex digits: Windows takes at most 16
/// characters on its oldest supported builds). It names nothing.
pub fn tag_for(conversation: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in conversation.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// `s` with the five XML specials escaped, for text inside an element or an attribute.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// The Windows toast: a generic two-line notice, silent (the app plays its own sound, so the
/// person's sound choice and quiet hours hold). Every value escaped.
pub fn toast_xml(t: &Toast) -> String {
    let body = if t.body.is_empty() {
        String::new()
    } else {
        format!("<text>{}</text>", xml_escape(&t.body))
    };
    let launch = if t.launch.is_empty() { String::new() } else { format!(" launch=\"{}\"", xml_escape(&t.launch)) };
    format!(
        "<toast{}><visual><binding template=\"ToastGeneric\"><text>{}</text>{}</binding></visual><audio silent=\"true\"/></toast>",
        launch,
        xml_escape(&t.title),
        body
    )
}

/// What `notify-send` is given. `--` ends its options, so a title starting with "-" is a title;
/// the body is escaped because the notification service may read markup in it.
pub fn linux_args(t: &Toast) -> Vec<String> {
    vec![
        format!("--app-name={APP_NAME}"),
        format!("--expire-time={}", if t.expire_ms == 0 { EXPIRE_MS } else { t.expire_ms }),
        "--".to_string(),
        t.title.clone(),
        xml_escape(&t.body),
    ]
}

/// What `notify-send` is given for a notice a click can open (rung 2, part 2): [`linux_args`] with
/// the default action (named `default`, the one a click on the notice's body runs, labelled
/// `open`) and `--wait`, so `notify-send` stays until the notice closes and prints `default` when
/// it was clicked. Only with a `notify-send` that has `--action` (libnotify 0.8 and later).
pub fn linux_args_with_click(t: &Toast, open: &str) -> Vec<String> {
    let mut args = linux_args(t);
    let at = args.iter().position(|a| a == "--").unwrap_or(args.len());
    args.insert(at, "--wait".to_string());
    args.insert(at, format!("--action=default={}", open.replace(['\n', '\r'], " ")));
    args
}

/// Did `notify-send`'s output say the notice was clicked? It prints the action's name.
pub fn linux_clicked(stdout: &str) -> bool {
    stdout.lines().any(|l| l.trim() == "default")
}

/// What `osascript` is given: a script that reads its words from its arguments (never pasted into
/// the script), and the words. osascript reads options up to the first argument, so a word
/// starting with "-" gets an invisible word joiner (U+2060) in front and is never taken as one.
pub fn mac_args(t: &Toast) -> Vec<String> {
    let guard = |s: &str| if s.starts_with('-') { format!("\u{2060}{s}") } else { s.to_string() };
    vec![
        "-e".to_string(),
        "on run argv".to_string(),
        "-e".to_string(),
        "display notification (item 2 of argv) with title (item 1 of argv)".to_string(),
        "-e".to_string(),
        "end run".to_string(),
        guard(&t.title),
        guard(&t.body),
    ]
}

// ── The platforms ───────────────────────────────────────────────────────────

#[cfg(windows)]
struct Backend {
    registered: bool,
}

#[cfg(windows)]
impl Backend {
    fn start() -> Backend {
        // WinRT needs COM on this thread; the multithreaded apartment, as the thread has no window.
        // SAFETY: called once, first thing on this thread, with no reserved pointer.
        let hr = unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED) };
        if hr.is_err() {
            log::warn!("notifications: COM did not start on the notice thread ({hr:?})");
        }
        // The notice activator (rung 2, part 2): served from this thread's multithreaded apartment
        // for as long as the app runs, so Windows hands it a click on any of our notices, from the
        // corner or from the notification centre. Only once clicks are on (a person's launch).
        if CLICKS_ON.load(Ordering::SeqCst) {
            win_activate::serve();
        }
        Backend { registered: false }
    }

    fn show(&mut self, t: &Toast) -> Result<(), String> {
        use windows::core::HSTRING;
        use windows::Data::Xml::Dom::XmlDocument;
        use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
        if !self.registered {
            win_register::register();
            self.registered = true;
        }
        let doc = XmlDocument::new().map_err(|e| format!("XmlDocument: {e}"))?;
        doc.LoadXml(&HSTRING::from(toast_xml(t))).map_err(|e| format!("LoadXml: {e}"))?;
        let toast = ToastNotification::CreateToastNotification(&doc).map_err(|e| format!("CreateToastNotification: {e}"))?;
        // Best effort: an older Windows without one of these still shows the notice.
        let _ = toast.SetTag(&HSTRING::from(t.tag.as_str()));
        let _ = toast.SetGroup(&HSTRING::from(t.group.as_str()));
        let _ = toast.SetExpiresOnReboot(true);
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))
            .map_err(|e| format!("CreateToastNotifierWithId: {e}"))?;
        notifier.Show(&toast).map_err(|e| format!("Show: {e}"))
    }

    /// Off the screen and out of the notification centre (rung 3: a call's notice when its ring
    /// ends). Nothing to do before any notice was shown (the registration is what names us).
    fn withdraw(&mut self, tag: &str, group: &str) {
        use windows::core::HSTRING;
        use windows::UI::Notifications::ToastNotificationManager;
        if !self.registered {
            return;
        }
        let removed = ToastNotificationManager::History()
            .and_then(|h| h.RemoveGroupedTagWithId(&HSTRING::from(tag), &HSTRING::from(group), &HSTRING::from(AUMID)));
        if let Err(e) = removed {
            log::debug!("notifications: a notice could not be withdrawn: {e}");
        }
    }
}

/// The per-user registration that lets Windows show an unpackaged program's notices.
#[cfg(windows)]
mod win_register {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{RegCloseKey, RegCreateKeyW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, REG_SZ};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn set_text(key: HKEY, name: &str, value: &str) {
        let name = wide(name);
        let value = wide(value);
        // SAFETY: a u16 slice viewed as its bytes, alive for the call.
        let bytes = unsafe { std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), value.len() * 2) };
        // SAFETY: `key` is open, `name` is NUL-ended and outlives the call.
        let r = unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_SZ, Some(bytes)) };
        if r != ERROR_SUCCESS {
            log::warn!("notifications: could not write the notice registration ({r:?})");
        }
    }

    /// Write `HKCU\Software\Classes\AppUserModelId\<AUMID>`: the name the notice header shows, and
    /// the icon beside it (a copy of the app icon kept beside the person's data folder).
    pub fn register() {
        let path = wide(&format!("Software\\Classes\\AppUserModelId\\{}", super::AUMID));
        let mut key = HKEY::default();
        // SAFETY: `path` is NUL-ended and outlives the call; `key` is written only on success.
        let r = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), &mut key) };
        if r != ERROR_SUCCESS {
            log::warn!("notifications: could not register for notices ({r:?})");
            return;
        }
        set_text(key, "DisplayName", super::APP_NAME);
        if let Some(icon) = super::icon_file() {
            set_text(key, "IconUri", &icon.to_string_lossy());
        }
        // Which COM class takes a click on our notices (rung 2, part 2): the one this running app
        // serves (`win_activate`). The Windows App SDK writes the same value for unpackaged apps.
        // No `LocalServer32` beside it: nothing names this exe's path, so a click on a notice
        // after the app has quit starts nothing (rung 5, with Reply on a notice).
        if super::CLICKS_ON.load(super::Ordering::SeqCst) {
            set_text(key, "CustomActivator", &super::win_activate::clsid_text());
        }
        // SAFETY: the handle came from a successful RegCreateKeyW and is closed once, here.
        unsafe {
            let _ = RegCloseKey(key);
        }
    }
}

/// The notice activator (rung 2, part 2): the COM class Windows calls when one of our notices is
/// clicked, from the corner of the screen or from the notification centre, named by the
/// registration's `CustomActivator`. Windows passes the notice's `launch` (its token); the class
/// hands it to [`super::clicked`]. It is served from the notice thread, a multithreaded apartment,
/// so Windows' call arrives on one of COM's own threads and never waits for the frame.
///
/// Why this and not a `humanityos://` address: a protocol starts a SECOND copy of the program for
/// every click, which must then find the running one and hand the click over, and its registration
/// names an exe path (every worktree's and rig's build would overwrite the person's). The
/// activator reaches the copy that is running, which is the one that knows what the token means.
#[cfg(windows)]
mod win_activate {
    use windows::core::{implement, IUnknown, Interface, Ref, BOOL, GUID, PCWSTR};
    use windows::Win32::Foundation::CLASS_E_NOAGGREGATION;
    use windows::Win32::System::Com::{CoRegisterClassObject, IClassFactory, IClassFactory_Impl, CLSCTX_LOCAL_SERVER, REGCLS_MULTIPLEUSE};
    use windows::Win32::UI::Notifications::{INotificationActivationCallback, INotificationActivationCallback_Impl, NOTIFICATION_USER_INPUT_DATA};

    /// HumanityOS's notice activator. Made once for this app (2026-10-31); never reuse it elsewhere.
    pub const CLSID: GUID = GUID::from_u128(0x2a2508e1_b36c_451d_bc01_2114f1c9a751);

    /// The class id as the registry writes it, `{...}`.
    pub fn clsid_text() -> String {
        format!("{{{:?}}}", CLSID)
    }

    #[implement(INotificationActivationCallback)]
    struct Activator;

    impl INotificationActivationCallback_Impl for Activator_Impl {
        fn Activate(&self, _app: &PCWSTR, invoked: &PCWSTR, _data: *const NOTIFICATION_USER_INPUT_DATA, _count: u32) -> windows::core::Result<()> {
            // SAFETY: Windows passes a NUL-ended string (or null) that lives for the call.
            let launch = if invoked.is_null() { String::new() } else { unsafe { invoked.to_string() }.unwrap_or_default() };
            super::clicked(launch);
            Ok(())
        }
    }

    #[implement(IClassFactory)]
    struct Factory;

    impl IClassFactory_Impl for Factory_Impl {
        fn CreateInstance(&self, outer: Ref<IUnknown>, iid: *const GUID, object: *mut *mut core::ffi::c_void) -> windows::core::Result<()> {
            if object.is_null() {
                return Err(windows::Win32::Foundation::E_POINTER.into());
            }
            // SAFETY: `object` is the caller's out pointer, checked above.
            unsafe { *object = core::ptr::null_mut() };
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let activator: INotificationActivationCallback = Activator.into();
            // SAFETY: `iid` and `object` come from COM's own call, as `query` expects.
            unsafe { activator.query(iid, object).ok() }
        }

        fn LockServer(&self, _lock: BOOL) -> windows::core::Result<()> {
            Ok(())
        }
    }

    /// Serve the activator from this thread's apartment for the rest of the run.
    pub fn serve() {
        let factory: IClassFactory = Factory.into();
        // SAFETY: COM is started on this thread (Backend::start); the class object is kept by COM
        // for the rest of the process, so the cookie is never revoked.
        match unsafe { CoRegisterClassObject(&CLSID, &factory, CLSCTX_LOCAL_SERVER, REGCLS_MULTIPLEUSE) } {
            Ok(_) => log::info!("notifications: a click on a notice opens its conversation (notice activator {})", clsid_text()),
            Err(e) => log::warn!("notifications: clicks on notices will not open anything: the notice activator was refused ({e})"),
        }
    }
}

/// The app icon as a file a notice can point at: written once beside the person's data folder
/// (`None` before a storage place is chosen, or when it cannot be written).
#[cfg(windows)]
fn icon_file() -> Option<std::path::PathBuf> {
    const ICON: &[u8] = include_bytes!("../../assets/icon.png");
    let root = crate::storage::writable_data_dir()?.parent()?.to_path_buf();
    let path = root.join("notice-icon.png");
    let same = std::fs::read(&path).map(|b| b == ICON).unwrap_or(false);
    if !same && std::fs::write(&path, ICON).is_err() {
        return None;
    }
    Some(path)
}

#[cfg(target_os = "linux")]
struct Backend {
    /// This `notify-send` takes `--action` (libnotify 0.8 and later), so a notice can be clicked.
    actions: bool,
    /// Notices waiting on a click right now, each with its `notify-send` still running.
    waiting: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// At most this many notices wait on a click at once (each holds one `notify-send` and one small
/// thread until it closes); past it a notice is shown without one.
#[cfg(target_os = "linux")]
const LINUX_WAITING: usize = 3;

#[cfg(target_os = "linux")]
impl Backend {
    fn start() -> Backend {
        let help = std::process::Command::new("notify-send")
            .arg("--help")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let actions = help.contains("--action");
        if CLICKS_ON.load(Ordering::SeqCst) && !actions {
            log::info!("notifications: this notify-send has no --action, so a click on a notice opens nothing (libnotify 0.8 brings it)");
        }
        Backend { actions, waiting: Default::default() }
    }

    fn show(&mut self, t: &Toast) -> Result<(), String> {
        use std::sync::atomic::Ordering as O;
        if self.actions && CLICKS_ON.load(Ordering::SeqCst) && !t.launch.is_empty() && self.waiting.load(O::SeqCst) < LINUX_WAITING {
            let child = std::process::Command::new("notify-send")
                .args(linux_args_with_click(t, crate::i18n::t("notifications.open_action")))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("notify-send could not be started ({e}); install libnotify's notify-send"))?;
            let waiting = self.waiting.clone();
            waiting.fetch_add(1, O::SeqCst);
            let launch = t.launch.clone();
            let spawned = std::thread::Builder::new().name("hos-notify-wait".into()).spawn(move || {
                if let Ok(out) = child.wait_with_output() {
                    if linux_clicked(&String::from_utf8_lossy(&out.stdout)) {
                        clicked(launch);
                    }
                }
                waiting.fetch_sub(1, O::SeqCst);
            });
            if spawned.is_err() {
                self.waiting.fetch_sub(1, O::SeqCst);
            }
            return Ok(());
        }
        let status = std::process::Command::new("notify-send")
            .args(linux_args(t))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("notify-send could not be started ({e}); install libnotify's notify-send"))?;
        status.success().then_some(()).ok_or_else(|| format!("notify-send ended with {status}"))
    }

    /// `notify-send` and `osascript` cannot withdraw a notice they showed (rung 5's D-Bus and
    /// `UNUserNotificationCenter` can): a call's notice expires after its ring instead (`expire_ms`).
    fn withdraw(&mut self, _tag: &str, _group: &str) {}
}

#[cfg(target_os = "macos")]
struct Backend;

#[cfg(target_os = "macos")]
impl Backend {
    fn start() -> Backend {
        Backend
    }

    fn show(&mut self, t: &Toast) -> Result<(), String> {
        let status = std::process::Command::new("osascript")
            .args(mac_args(t))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("osascript could not be started ({e})"))?;
        status.success().then_some(()).ok_or_else(|| format!("osascript ended with {status}"))
    }

    /// `notify-send` and `osascript` cannot withdraw a notice they showed (rung 5's D-Bus and
    /// `UNUserNotificationCenter` can): a call's notice expires after its ring instead (`expire_ms`).
    fn withdraw(&mut self, _tag: &str, _group: &str) {}
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
struct Backend;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
impl Backend {
    fn start() -> Backend {
        Backend
    }

    fn show(&mut self, _t: &Toast) -> Result<(), String> {
        Err("this platform has no notices yet".into())
    }

    /// No notices here, so none to withdraw.
    fn withdraw(&mut self, _tag: &str, _group: &str) {}
}
