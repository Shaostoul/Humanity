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
//! Every notice goes to ONE worker thread through a bounded queue: the frame never waits for COM,
//! D-Bus or a process start, and a burst beyond the queue is dropped rather than piling up. The
//! builders of what each platform is handed ([`toast_xml`], [`linux_args`], [`mac_args`]) are pure
//! and tested on every platform: a name cannot add a button, a link or a script line to a notice.

use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::OnceLock;

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
}

/// How many notices may wait for the worker before more are dropped.
const QUEUE: usize = 8;

/// Hand `toast` to the worker. Never blocks; false when the worker could not be started or is
/// behind (the notice is dropped and the reason logged once).
pub fn show(toast: Toast) -> bool {
    let Some(tx) = worker() else { return false };
    match tx.try_send(toast) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            log::debug!("notifications: the notice queue is full, one notice dropped");
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

fn worker() -> Option<&'static SyncSender<Toast>> {
    static TX: OnceLock<Option<SyncSender<Toast>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = sync_channel::<Toast>(QUEUE);
        let spawned = std::thread::Builder::new().name("hos-notify".into()).spawn(move || {
            let mut backend = Backend::start();
            for t in rx {
                if let Err(e) = backend.show(&t) {
                    log::warn!("notifications: the notice was not shown: {e}");
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
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text>{}</binding></visual><audio silent=\"true\"/></toast>",
        xml_escape(&t.title),
        body
    )
}

/// What `notify-send` is given. `--` ends its options, so a title starting with "-" is a title;
/// the body is escaped because the notification service may read markup in it.
pub fn linux_args(t: &Toast) -> Vec<String> {
    vec![
        format!("--app-name={APP_NAME}"),
        "--expire-time=8000".to_string(),
        "--".to_string(),
        t.title.clone(),
        xml_escape(&t.body),
    ]
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
        // SAFETY: the handle came from a successful RegCreateKeyW and is closed once, here.
        unsafe {
            let _ = RegCloseKey(key);
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
struct Backend;

#[cfg(target_os = "linux")]
impl Backend {
    fn start() -> Backend {
        Backend
    }

    fn show(&mut self, t: &Toast) -> Result<(), String> {
        let status = std::process::Command::new("notify-send")
            .args(linux_args(t))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("notify-send could not be started ({e}); install libnotify's notify-send"))?;
        status.success().then_some(()).ok_or_else(|| format!("notify-send ended with {status}"))
    }
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
}
