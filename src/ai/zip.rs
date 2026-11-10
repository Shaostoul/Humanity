//! Unpacking the AI engine's zip (rung 2, 2026-11-09), only after its SHA-256 matched the catalog.
//!
//! The engine zip is made by scripts/ai-engine.js with every entry STORED (no compression), sorted, and
//! dated 1980-01-01, so the app needs no decompressor and the same files always make the same bytes.
//! This reader takes exactly that: a stored entry, a plain relative path (a name, or `licenses/<name>`),
//! a size that matches its central directory record and a CRC-32 that matches its bytes. Anything else
//! is refused in words, and nothing is written outside the folder it unpacks into.

use std::path::{Component, Path, PathBuf};

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

fn u16_at(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "the zip ends early".to_string())
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "the zip ends early".to_string())
}

/// CRC-32 (IEEE), the zip format's check of each entry.
pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, slot) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    });
    let mut c = 0xffff_ffffu32;
    for b in data {
        c = table[((c ^ u32::from(*b)) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xffff_ffff
}

/// A relative path that stays inside the folder: plain names joined by `/`, no `..`, no root, no drive.
fn safe_relative(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('\\') || name.contains(':') || name.ends_with('/') {
        return Err(format!("{name:?} is not a plain file path"));
    }
    let p = Path::new(name);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(s) if !s.to_string_lossy().starts_with('.') => out.push(s),
            _ => return Err(format!("{name:?} reaches outside its folder")),
        }
    }
    if out.components().count() > 2 {
        return Err(format!("{name:?} is deeper than the engine's folders"));
    }
    Ok(out)
}

/// One entry: its path in the folder and its bytes.
pub struct Entry<'a> {
    pub path: PathBuf,
    pub data: &'a [u8],
}

/// Read every entry of a stored zip, checking each against its central directory record and CRC-32.
pub fn entries(zip: &[u8]) -> Result<Vec<Entry<'_>>, String> {
    if zip.len() < 22 {
        return Err("too short to be a zip".into());
    }
    // The end record: no comment, so it is the last 22 bytes (scripts/ai-engine.js writes none).
    let end = zip.len() - 22;
    if u32_at(zip, end)? != END {
        return Err("no end record where a zip of the engine has it".into());
    }
    let count = u16_at(zip, end + 10)? as usize;
    let dir_size = u32_at(zip, end + 12)? as usize;
    let dir_at = u32_at(zip, end + 16)? as usize;
    if dir_at.checked_add(dir_size) != Some(end) {
        return Err("its central directory is not where its end record says".into());
    }
    let mut out = Vec::with_capacity(count);
    let mut at = dir_at;
    for _ in 0..count {
        if u32_at(zip, at)? != CENTRAL {
            return Err("a broken central directory".into());
        }
        let method = u16_at(zip, at + 10)?;
        let crc = u32_at(zip, at + 16)?;
        let packed = u32_at(zip, at + 20)? as usize;
        let size = u32_at(zip, at + 24)? as usize;
        let name_len = u16_at(zip, at + 28)? as usize;
        let extra_len = u16_at(zip, at + 30)? as usize;
        let comment_len = u16_at(zip, at + 32)? as usize;
        let local_at = u32_at(zip, at + 42)? as usize;
        let name = std::str::from_utf8(zip.get(at + 46..at + 46 + name_len).ok_or("the zip ends early")?).map_err(|_| "a name that is not UTF-8".to_string())?;
        at += 46 + name_len + extra_len + comment_len;
        if method != 0 || packed != size {
            return Err(format!("{name} is compressed (method {method}); the engine's zip stores every file as it is"));
        }
        let path = safe_relative(name)?;
        if u32_at(zip, local_at)? != LOCAL {
            return Err(format!("{name}: no local header where the directory says"));
        }
        let local_name = u16_at(zip, local_at + 26)? as usize;
        let local_extra = u16_at(zip, local_at + 28)? as usize;
        let start = local_at + 30 + local_name + local_extra;
        let data = zip.get(start..start + size).ok_or_else(|| format!("{name} runs past the end of the zip"))?;
        if crc32(data) != crc {
            return Err(format!("{name} does not match its CRC-32"));
        }
        out.push(Entry { path, data });
    }
    Ok(out)
}

/// Unpack a checked zip into `dir` (made if missing). Refuses the whole zip, writing nothing, if any
/// entry is not one this reader takes.
pub fn unpack(zip: &[u8], dir: &Path) -> Result<usize, String> {
    let all = entries(zip)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in &all {
        let to = dir.join(&e.path);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        std::fs::write(&to, e.data).map_err(|err| format!("{}: {err}", to.display()))?;
    }
    Ok(all.len())
}
