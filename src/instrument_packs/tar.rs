//! Reading a pack archive: plain ustar, as `scripts/lib/instrument-build.js` writes it.
//!
//! A downloaded archive is checked against its published SHA-256 before it is opened, and this
//! reader still trusts nothing in it: only regular files and folders, every path relative, forward
//! slashes, no `..`, no drive letter, no link of any kind, every header's checksum right. Anything
//! else stops the install with nothing written outside the folder being filled.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

const BLOCK: usize = 512;

fn field_str(b: &[u8]) -> &str {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    std::str::from_utf8(&b[..end]).unwrap_or("\u{fffd}")
}

fn octal(b: &[u8]) -> Result<u64, String> {
    let s = field_str(b).trim();
    if s.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(s, 8).map_err(|_| format!("a header holds a bad number: {s:?}"))
}

/// The safe relative path of an archive entry, or why it is refused.
pub fn safe_path(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('\\') || name.contains(':') || name.starts_with('/') {
        return Err(format!("the archive holds a path that is not allowed: {name:?}"));
    }
    let mut out = PathBuf::new();
    for c in Path::new(name).components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return Err(format!("the archive holds a path that is not allowed: {name:?}")),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(format!("the archive holds an empty path: {name:?}"));
    }
    Ok(out)
}

/// Unpack `reader` into `dest` (which must exist). Returns the files written, relative to `dest`.
pub fn extract(reader: &mut impl Read, dest: &Path) -> Result<Vec<PathBuf>, String> {
    let mut written = Vec::new();
    let mut header = [0u8; BLOCK];
    let mut zero_blocks = 0;
    loop {
        if let Err(e) = reader.read_exact(&mut header) {
            return Err(format!("the archive ends early: {e}"));
        }
        if header.iter().all(|&b| b == 0) {
            zero_blocks += 1;
            if zero_blocks == 2 {
                return Ok(written);
            }
            continue;
        }
        zero_blocks = 0;
        let stored_sum = octal(&header[148..156])?;
        let sum: u64 = header
            .iter()
            .enumerate()
            .map(|(i, &b)| if (148..156).contains(&i) { b' ' as u64 } else { b as u64 })
            .sum();
        if sum != stored_sum {
            return Err("an archive header fails its checksum".to_string());
        }
        let mut name = field_str(&header[0..100]).to_string();
        if &header[257..262] == b"ustar" {
            let prefix = field_str(&header[345..500]);
            if !prefix.is_empty() {
                name = format!("{prefix}/{name}");
            }
        }
        let size = octal(&header[124..136])?;
        let kind = header[156];
        let rel = safe_path(name.trim_end_matches('/'))?;
        match kind {
            b'0' | 0 => {
                let path = dest.join(&rel);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                let mut file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let mut limited = (&mut *reader).take(size);
                let copied = std::io::copy(&mut limited, &mut file).map_err(|e| format!("{}: {e}", path.display()))?;
                if copied != size {
                    return Err(format!("the archive ends inside {name}"));
                }
                let pad = (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64;
                if pad > 0 {
                    let mut skip = vec![0u8; pad as usize];
                    reader.read_exact(&mut skip).map_err(|e| format!("the archive ends early: {e}"))?;
                }
                written.push(rel);
            }
            b'5' => {
                let path = dest.join(&rel);
                std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            other => {
                return Err(format!(
                    "the archive holds an entry of a kind that is not allowed ({:?}): {name}",
                    other as char
                ))
            }
        }
    }
}
