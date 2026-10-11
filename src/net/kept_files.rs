//! Keeping a copy of the private files a person opened (docs/design/media-showcase.md, "Files on
//! the sender's device and in messages, exact", part 4; the operator's answer 9 of 2026-10-11).
//!
//! WHY: a private file (an encrypted upload sent in a DM or a group) leaves the server with the DM
//! mailbox's own lifetime, 30 days by default. So that nothing already looked at disappears, the
//! app keeps its own copy of every private file the person opened or saved, and opening a file
//! looks here first.
//!
//! HOW IT IS KEPT, ENCRYPTED AT REST: what is kept is the server's ciphertext of the file (sealed
//! with the file's own key, which lives only in the marker inside the encrypted DM history),
//! encrypted again with AES-256 in counter mode under a key derived from the person's identity
//! (`BLAKE3.derive_key("hum/kept-files/v1", seed)`, the same kind of key their DM history is
//! encrypted with). Each file: a random 16-byte counter start, then the encrypted bytes. So a kept
//! file opens only with both the identity and the marker, and it is read and written a megabyte at a
//! time, so a 1 GB file is never held in memory to be kept. A changed byte is caught when the file
//! is opened, by the file's own AES-GCM tag (the outer layer adds no tag of its own: the inner one
//! already covers every byte). A file's NAME on disk is a keyed hash of its address, key and nonce,
//! so the folder listing says nothing about who sent what.
//!
//! SIZE: at most the cap the person chose (Settings > Data, 1 GB by default); the files opened
//! longest ago go first (each open touches its file's time). "Clear kept files" empties the folder.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use aes_gcm::aes::Aes256;
use ctr::cipher::{KeyIvInit, StreamCipher};

type Ctr = ctr::Ctr128BE<Aes256>;

/// The domain of the at-rest key, distinct from every other key made from the seed.
const KEY_DOMAIN: &str = "hum/kept-files/v1";
/// The domain of the key the file names are hashed with.
const NAME_DOMAIN: &str = "hum/kept-files/name/v1";
/// A kept file's extension.
const EXT: &str = "kept";
/// How much is read and encrypted at a time.
const PIECE: usize = 1 << 20;

/// This device's kept private files, for one identity.
#[derive(Clone)]
pub struct KeptFiles {
    dir: PathBuf,
    key: [u8; 32],
    name_key: [u8; 32],
    /// The most the folder may hold, in bytes (0 keeps nothing).
    cap: u64,
}

impl std::fmt::Debug for KeptFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the keys.
        f.debug_struct("KeptFiles").field("dir", &self.dir).field("cap", &self.cap).finish()
    }
}

/// The folder kept files live in: beside the DM history, in the app's own data folder.
pub fn kept_dir() -> PathBuf {
    let dms = crate::net::dm_store::DmStore::store_dir();
    dms.parent().map(|p| p.join("kept-files")).unwrap_or_else(|| PathBuf::from("kept-files"))
}

/// Every kept file in `dir`: (path, size, last opened).
fn listing(dir: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some(EXT))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((e.path(), m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)))
        })
        .collect()
}

/// How many bytes the kept files in `dir` take.
pub fn used(dir: &Path) -> u64 {
    listing(dir).iter().map(|(_, n, _)| n).sum()
}

/// Remove every kept file in `dir` ("Clear kept files"): how many, and how many bytes.
pub fn clear(dir: &Path) -> (usize, u64) {
    let mut gone = (0usize, 0u64);
    for (p, n, _) in listing(dir) {
        if std::fs::remove_file(&p).is_ok() {
            gone.0 += 1;
            gone.1 += n;
        }
    }
    gone
}

/// Remove the files opened longest ago until `dir` holds at most `cap` bytes.
pub fn evict(dir: &Path, cap: u64) {
    let mut files = listing(dir);
    let mut total: u64 = files.iter().map(|(_, n, _)| n).sum();
    files.sort_by_key(|(_, _, t)| *t);
    for (p, n, _) in files {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= n;
        }
    }
}

impl KeptFiles {
    /// The kept files of the identity with this seed, in the app's folder, holding at most `cap`.
    pub fn for_seed(seed: &[u8], cap: u64) -> Self {
        Self::in_dir(kept_dir(), seed, cap)
    }

    /// The same in `dir` (the tests' own folders).
    pub fn in_dir(dir: PathBuf, seed: &[u8], cap: u64) -> Self {
        KeptFiles { dir, key: blake3::derive_key(KEY_DOMAIN, seed), name_key: blake3::derive_key(NAME_DOMAIN, seed), cap }
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the file at `url` with key `k` and nonce `n` is kept, if it is.
    fn path_of(&self, url: &str, k: &str, n: &str) -> PathBuf {
        let h = blake3::keyed_hash(&self.name_key, format!("{url}\n{k}\n{n}").as_bytes());
        self.dir.join(format!("{}.{EXT}", &h.to_hex()[..40]))
    }

    /// Whether a copy of this file is kept.
    pub fn has(&self, url: &str, k: &str, n: &str) -> bool {
        self.path_of(url, k, n).is_file()
    }

    /// Open the kept copy for reading: its keystream and the reader past the counter start. Touches
    /// its time, so it counts as opened now.
    fn open(&self, path: &Path) -> Option<(Ctr, File, u64)> {
        let mut f = File::open(path).ok()?;
        let len = f.metadata().ok()?.len();
        let mut iv = [0u8; 16];
        f.read_exact(&mut iv).ok()?;
        if let Ok(w) = File::options().write(true).open(path) {
            let _ = w.set_modified(std::time::SystemTime::now());
        }
        Some((Ctr::new(&self.key.into(), &iv.into()), f, len.saturating_sub(16)))
    }

    /// The server's ciphertext of this file from its kept copy, when one is kept and is no bigger
    /// than `max` (a picture shown in place).
    pub fn get(&self, url: &str, k: &str, n: &str, max: u64) -> Option<Vec<u8>> {
        let (mut ctr, mut f, len) = self.open(&self.path_of(url, k, n))?;
        if len > max {
            return None;
        }
        let mut out = Vec::with_capacity(len as usize);
        f.read_to_end(&mut out).ok()?;
        ctr.apply_keystream(&mut out);
        Some(out)
    }

    /// The server's ciphertext of this file from its kept copy, written into `out` a piece at a time
    /// (Save). False when none is kept or it could not be read.
    pub fn get_into(&self, url: &str, k: &str, n: &str, out: &mut dyn Write) -> bool {
        let Some((mut ctr, mut f, _)) = self.open(&self.path_of(url, k, n)) else { return false };
        let mut buf = vec![0u8; PIECE];
        loop {
            let got = match f.read(&mut buf) {
                Ok(0) => return true,
                Ok(g) => g,
                Err(_) => return false,
            };
            ctr.apply_keystream(&mut buf[..got]);
            if out.write_all(&buf[..got]).is_err() {
                return false;
            }
        }
    }

    /// Forget the kept copy of this file (one that would not open).
    pub fn forget(&self, url: &str, k: &str, n: &str) {
        let _ = std::fs::remove_file(self.path_of(url, k, n));
    }

    /// Keep the server's ciphertext of this file, read from `src` a piece at a time, encrypted under
    /// the identity's key, then make room under the cap (the files opened longest ago go). False,
    /// with nothing written, when the file alone is over the cap (0 keeps nothing).
    pub fn put_from(&self, url: &str, k: &str, n: &str, src: &mut dyn Read, len: u64) -> std::io::Result<bool> {
        if len + 16 > self.cap {
            return Ok(false);
        }
        std::fs::create_dir_all(&self.dir)?;
        let to = self.path_of(url, k, n);
        let tmp = to.with_extension("tmp");
        let mut iv = [0u8; 16];
        getrandom::getrandom(&mut iv).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let mut ctr = Ctr::new(&self.key.into(), &iv.into());
        let written = (|| -> std::io::Result<()> {
            let mut out = File::create(&tmp)?;
            out.write_all(&iv)?;
            let mut buf = vec![0u8; PIECE];
            loop {
                let got = src.read(&mut buf)?;
                if got == 0 {
                    break;
                }
                ctr.apply_keystream(&mut buf[..got]);
                out.write_all(&buf[..got])?;
            }
            out.sync_all()
        })();
        if let Err(e) = written.and_then(|_| std::fs::rename(&tmp, &to)) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        evict(&self.dir, self.cap);
        Ok(true)
    }

    /// [`Self::put_from`] for ciphertext held in memory (a picture shown in place).
    pub fn put(&self, url: &str, k: &str, n: &str, ciphertext: &[u8]) -> std::io::Result<bool> {
        self.put_from(url, k, n, &mut std::io::Cursor::new(ciphertext), ciphertext.len() as u64)
    }
}

#[cfg(test)]
#[path = "kept_files_tests.rs"]
mod tests;
