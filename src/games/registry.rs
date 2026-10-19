//! Reading the Windows registry, and only reading: every key is opened with KEY_READ, and nothing
//! here can create, change or delete a key or a value. Used to find where Steam is
//! (`HKCU\Software\Valve\Steam`), which Steam account is signed in while Steam runs
//! (`...\Steam\ActiveProcess`, `ActiveUser`), and which games GOG installed
//! (`HKLM\...\GOG.com\Games`).

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD, REG_EXPAND_SZ, REG_SZ, REG_VALUE_TYPE};

/// Which of the two roots a key is under.
#[derive(Debug, Clone, Copy)]
pub enum Root {
    CurrentUser,
    LocalMachine,
}

/// An open key, closed when dropped.
pub struct Key(HKEY);

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

/// Open `path` under `under` for reading.
fn open_under(under: HKEY, path: &str) -> Option<Key> {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-ended and outlives the call; `key` is written only on success.
    let r = unsafe { RegOpenKeyExW(under, PCWSTR(path.as_ptr()), None, KEY_READ, &mut key) };
    (r == ERROR_SUCCESS).then_some(Key(key))
}

/// Open `path` under `root` for reading; `None` when it is not there (or cannot be read).
pub fn open(root: Root, path: &str) -> Option<Key> {
    let hkey = match root {
        Root::CurrentUser => HKEY_CURRENT_USER,
        Root::LocalMachine => HKEY_LOCAL_MACHINE,
    };
    open_under(hkey, path)
}

/// The longest value read: a folder's path is far shorter, so anything longer is not one.
const MAX_VALUE_BYTES: u32 = 64 * 1024;

impl Key {
    /// The sub-key `name` of this key, for reading.
    pub fn open(&self, name: &str) -> Option<Key> {
        open_under(self.0, name)
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
