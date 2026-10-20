//! Reading the Windows registry, and only reading: every key is opened with KEY_READ, and nothing
//! here can create, change or delete a key or a value. Used to find where Steam is
//! (`HKCU\Software\Valve\Steam`), which Steam account is signed in while Steam runs
//! (`...\Steam\ActiveProcess`, `ActiveUser`), and which games GOG installed
//! (`HKLM\...\GOG.com\Games`). Since 2026-10-19 the Tools page reads Windows' own
//! installed-programs list and App Paths through it too (software/records.rs), in both the 64-bit
//! and the 32-bit view of the registry ([`View`]): still KEY_READ, only the view chosen.

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_DWORD, REG_EXPAND_SZ, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE};

/// Which of the two roots a key is under.
#[derive(Debug, Clone, Copy)]
pub enum Root {
    CurrentUser,
    LocalMachine,
}

/// Which view of the registry a key is read in. A 64-bit Windows keeps a 32-bit program's
/// machine-wide settings apart (under WOW6432Node); a 32-bit installer writes its program there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The view this program reads by default (64-bit, as HumanityOS is).
    Default,
    /// The 64-bit view, asked for by name.
    Bits64,
    /// The 32-bit view (WOW6432Node).
    Bits32,
}

impl View {
    /// What RegOpenKeyExW is asked for: reading, in this view.
    fn access(self) -> REG_SAM_FLAGS {
        match self {
            View::Default => KEY_READ,
            View::Bits64 => KEY_READ | KEY_WOW64_64KEY,
            View::Bits32 => KEY_READ | KEY_WOW64_32KEY,
        }
    }
}

/// An open key, closed when dropped, and the view its sub-keys are opened in.
pub struct Key(HKEY, View);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful RegOpenKeyExW and is closed once, here.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// `s` as a NUL-ended UTF-16 string.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Open `path` under `under` for reading, in `view`.
fn open_under(under: HKEY, path: &str, view: View) -> Option<Key> {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-ended and outlives the call; `key` is written only on success.
    let r = unsafe { RegOpenKeyExW(under, PCWSTR(path.as_ptr()), None, view.access(), &mut key) };
    (r == ERROR_SUCCESS).then_some(Key(key, view))
}

/// Open `path` under `root` for reading; `None` when it is not there (or cannot be read).
pub fn open(root: Root, path: &str) -> Option<Key> {
    open_view(root, path, View::Default)
}

/// Open `path` under `root` for reading in `view` (its sub-keys are opened in the same view).
pub fn open_view(root: Root, path: &str, view: View) -> Option<Key> {
    let hkey = match root {
        Root::CurrentUser => HKEY_CURRENT_USER,
        Root::LocalMachine => HKEY_LOCAL_MACHINE,
    };
    open_under(hkey, path, view)
}

/// The longest value read: a folder's path is far shorter, so anything longer is not one.
const MAX_VALUE_BYTES: u32 = 64 * 1024;

impl Key {
    /// The sub-key `name` of this key, for reading.
    pub fn open(&self, name: &str) -> Option<Key> {
        open_under(self.0, name, self.1)
    }

    /// The text value `name` (REG_SZ, or REG_EXPAND_SZ read as written), when it is there.
    pub fn string(&self, name: &str) -> Option<String> {
        let name = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut size: u32 = 0;
        // SAFETY: a size query (no buffer); `name` is NUL-ended and outlives the call.
        let r = unsafe { RegQueryValueExW(self.0, PCWSTR(name.as_ptr()), None, Some(&mut kind), None, Some(&mut size)) };
        if r != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) || size > MAX_VALUE_BYTES {
            return None;
        }
        // One spare u16 so a value stored without its NUL still ends in one.
        let mut buf = vec![0u16; size as usize / 2 + 1];
        let mut got = (buf.len() * 2) as u32;
        // SAFETY: `buf` holds `got` bytes and outlives the call.
        let r = unsafe { RegQueryValueExW(self.0, PCWSTR(name.as_ptr()), None, Some(&mut kind), Some(buf.as_mut_ptr() as *mut u8), Some(&mut got)) };
        if r != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) {
            return None;
        }
        let units = &buf[..(got as usize / 2).min(buf.len())];
        let end = units.iter().position(|c| *c == 0).unwrap_or(units.len());
        Some(String::from_utf16_lossy(&units[..end]))
    }

    /// The number value `name` (REG_DWORD), when it is there.
    pub fn dword(&self, name: &str) -> Option<u32> {
        let name = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut value: u32 = 0;
        let mut size: u32 = 4;
        // SAFETY: `value` holds the 4 bytes `size` says and outlives the call; `name` is NUL-ended.
        let r = unsafe { RegQueryValueExW(self.0, PCWSTR(name.as_ptr()), None, Some(&mut kind), Some(&mut value as *mut u32 as *mut u8), Some(&mut size)) };
        (r == ERROR_SUCCESS && kind == REG_DWORD && size == 4).then_some(value)
    }

    /// The names of this key's sub-keys (at most 10,000).
    pub fn subkeys(&self) -> Vec<String> {
        let mut out = Vec::new();
        for index in 0..10_000u32 {
            // A key's name is at most 255 characters.
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            // SAFETY: `name` holds `len` characters and outlives the call.
            let r = unsafe { RegEnumKeyExW(self.0, index, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None) };
            if r != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&name[..(len as usize).min(name.len())]));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// READING A KEY EVERY WINDOWS HAS: Windows' own version key holds the text value
    /// "SystemRoot", which must read exactly as the SystemRoot environment variable Windows gives
    /// every program (letter case aside), and "ProductName"; SOFTWARE's sub-keys are listed; a key
    /// that is not there, and a value that is not there, read as nothing. Only reads (KEY_READ),
    /// like everything here.
    ///
    /// A first version only checked that ProductName started with "Windows", and it stayed GREEN
    /// with `string` keeping half the characters it read (`got as usize / 4`): "Windows 10 Home"
    /// cut to "Windows " still starts with "Windows". Against the variable it went red, 2026-10-18:
    /// "SystemRoot reads whole: \"C:\\\\Wi\" against \"C:\\\\Windows\"".
    #[test]
    fn reading_a_key_every_windows_has() {
        let key = open(Root::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion").expect("Windows' version key is there");
        let root = key.string("SystemRoot").expect("it has a SystemRoot");
        let env = std::env::var("SystemRoot").expect("Windows sets SystemRoot");
        assert!(root.eq_ignore_ascii_case(&env), "SystemRoot reads whole: {root:?} against {env:?}");
        let product = key.string("ProductName").expect("it has a ProductName");
        assert!(product.starts_with("Windows"), "ProductName starts with Windows: {product:?}");
        assert!(key.string("NoSuchValueHumanityOS").is_none(), "a value that is not there is nothing");
        let hklm_software = open(Root::LocalMachine, "SOFTWARE").expect("SOFTWARE is there");
        assert!(hklm_software.subkeys().iter().any(|k| k.eq_ignore_ascii_case("Microsoft")), "SOFTWARE's sub-keys include Microsoft");
        assert!(open(Root::CurrentUser, r"Software\NoSuchKeyHumanityOS\Games").is_none(), "a key that is not there is nothing");
    }

    /// THE TWO VIEWS (2026-10-19, for the Tools page): HKLM\SOFTWARE in the 64-bit view holds
    /// WOW6432Node (the 32-bit programs' part), and in the 32-bit view it IS that part, so it does
    /// not hold it again; a sub-key opened from a key in the 32-bit view stays in it (its Uninstall
    /// list is read where 32-bit installers write).
    ///
    /// Seen red 2026-10-19 with every view read as the default (`access` always KEY_READ): "the
    /// 32-bit view is WOW6432Node itself" failed.
    #[test]
    fn the_two_views() {
        let has_wow = |view: View| open_view(Root::LocalMachine, "SOFTWARE", view).expect("SOFTWARE is there").subkeys().iter().any(|k| k.eq_ignore_ascii_case("WOW6432Node"));
        assert!(has_wow(View::Bits64) && has_wow(View::Default), "the 64-bit view holds WOW6432Node");
        assert!(!has_wow(View::Bits32), "the 32-bit view is WOW6432Node itself");
        let microsoft = open_view(Root::LocalMachine, "SOFTWARE", View::Bits32).and_then(|k| k.open("Microsoft")).expect("its Microsoft key");
        assert!(microsoft.open("Windows").is_some_and(|w| w.open("CurrentVersion").is_some()), "a sub-key stays in its view and opens");
    }

    /// A NUMBER VALUE (2026-10-19, for Steam's ActiveUser): Windows' own CurrentMajorVersionNumber
    /// reads as the version Windows says it is (10 on Windows 10 and 11); a text value, or one not
    /// there, is not a number.
    ///
    /// Seen red 2026-10-19 with `dword` taking text for a number (`kind == REG_SZ`): "Windows 10
    /// and 11 both say 10" (left: None, right: Some(10)).
    #[test]
    fn a_number_value() {
        let key = open(Root::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion").expect("Windows' version key is there");
        assert_eq!(key.dword("CurrentMajorVersionNumber"), Some(10), "Windows 10 and 11 both say 10");
        assert_eq!(key.dword("SystemRoot"), None, "a text value is not a number");
        assert_eq!(key.dword("NoSuchValueHumanityOS"), None);
    }
}
