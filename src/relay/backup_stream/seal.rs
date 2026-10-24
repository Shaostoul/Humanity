//! Sealing what the continuous backup sends (2026-10-22): compressed, then encrypted with a
//! key derived from the relay's existing backup key (`data/backup.key`), so every copy that
//! leaves the machine is unreadable without that key, wherever it is kept.
//!
//! Object format: `HOSB` (4) | version (1) | kind (1) | flags (1) | 0 (1) | nonce (12) |
//! AES-256-GCM ciphertext and tag. The authenticated data binds the object's NAME and these
//! eight header bytes, so a copy renamed at the destination (a later segment swapped in for an
//! earlier one, a chunk of one snapshot put in another's place) fails to open rather than
//! restoring the wrong pages. Compression (raw deflate) happens before encryption, the only
//! order in which it does anything; a body that does not shrink is stored as it is.
//!
//! The bodies inside (a segment of WAL frames, a snapshot's manifest, the destination's owner
//! mark) are this module's own little-endian layouts, versioned by an 8-byte tag.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, Payload},
    AeadCore, Aes256Gcm, Key, Nonce,
};

const MAGIC: &[u8; 4] = b"HOSB";
const VERSION: u8 = 1;
const FLAG_DEFLATE: u8 = 1;
const HEADER_LEN: usize = 8;
const NONCE_LEN: usize = 12;
/// No object ever opens to more than this (a guard against a crafted deflate stream).
pub const MAX_PLAIN: usize = 64 * 1024 * 1024;

/// What an object holds; part of the authenticated header, so one kind cannot be opened as another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Segment = 1,
    SnapChunk = 2,
    SnapManifest = 3,
    Owner = 4,
    Probe = 5,
}

/// The key every continuous-backup object is sealed with: derived from the backup key, so the
/// file an operator already keeps safe is the only one needed, and these objects never share
/// a key with the `.db.enc` snapshots sealed with the backup key itself.
pub fn object_key(backup_key: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key("HumanityOS continuous backup objects v1 2026-10-22", backup_key)
}

/// A short fingerprint of the backup key for people to compare (the Backups panel, the restore
/// command): the same key always shows the same 16 characters, and they say nothing about it.
pub fn key_fingerprint(backup_key: &[u8; 32]) -> String {
    let d = blake3::derive_key("HumanityOS backup key fingerprint v1 2026-10-22", backup_key);
    let h = hex::encode(&d[..8]);
    format!("{} {} {} {}", &h[0..4], &h[4..8], &h[8..12], &h[12..16])
}

fn aad(name: &str, header: &[u8]) -> Vec<u8> {
    let mut a = Vec::with_capacity(32 + name.len());
    a.extend_from_slice(b"hos-backup-object\n");
    a.extend_from_slice(name.as_bytes());
    a.push(b'\n');
    a.extend_from_slice(header);
    a
}

/// Seal `plain` as the object called `name`.
pub fn seal(key: &[u8; 32], name: &str, kind: Kind, plain: &[u8]) -> Vec<u8> {
    let packed = miniz_oxide::deflate::compress_to_vec(plain, 3);
    let (flags, body) = if packed.len() < plain.len() { (FLAG_DEFLATE, packed) } else { (0, plain.to_vec()) };
    let header = [MAGIC[0], MAGIC[1], MAGIC[2], MAGIC[3], VERSION, kind as u8, flags, 0];
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, Payload { msg: &body, aad: &aad(name, &header) })
        .expect("AES-GCM encryption of an in-memory buffer cannot fail");
    let mut out = Vec::with_capacity(HEADER_LEN + NONCE_LEN + ct.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(nonce.as_slice());
    out.extend_from_slice(&ct);
    out
}

/// Open the object called `name`, which must be of `kind`. Err on anything else: not one of
/// ours, another version or kind, the wrong key, a changed byte, a different name, or a body
/// that would open to more than MAX_PLAIN.
pub fn open(key: &[u8; 32], name: &str, kind: Kind, sealed: &[u8]) -> Result<Vec<u8>, String> {
    if sealed.len() < HEADER_LEN + NONCE_LEN + 16 {
        return Err(format!("{name}: too short to be a backup object"));
    }
    let header = &sealed[..HEADER_LEN];
    if &header[..4] != MAGIC {
        return Err(format!("{name}: not a HumanityOS backup object"));
    }
    if header[4] != VERSION {
        return Err(format!("{name}: made by a newer version of HumanityOS (format {})", header[4]));
    }
    if header[5] != kind as u8 {
        return Err(format!("{name}: holds something else than its name says"));
    }
    let nonce = Nonce::from_slice(&sealed[HEADER_LEN..HEADER_LEN + NONCE_LEN]);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let body = cipher
        .decrypt(nonce, Payload { msg: &sealed[HEADER_LEN + NONCE_LEN..], aad: &aad(name, header) })
        .map_err(|_| format!("{name}: does not open with this backup key (another key, or the copy is damaged)"))?;
    if header[6] & FLAG_DEFLATE != 0 {
        miniz_oxide::inflate::decompress_to_vec_with_limit(&body, MAX_PLAIN)
            .map_err(|e| format!("{name}: damaged compressed body ({e:?})"))
    } else {
        Ok(body)
    }
}

// ── Bodies ───────────────────────────────────────────────────────────────────

/// Little-endian writer for the bodies below.
#[derive(Default)]
pub struct W(pub Vec<u8>);
impl W {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.0.extend_from_slice(v);
        self
    }
}

/// Little-endian reader; every read checks the length, so a short body is an error, never a panic.
pub struct R<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> R<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, at: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.at + n > self.b.len() {
            return Err("body ends early".into());
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        let s = self.take(8)?;
        Ok(u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }
    pub fn done(&self) -> bool {
        self.at == self.b.len()
    }
}

/// Where in SQLite's log a segment ends or a snapshot was taken: the pass of the log (its two
/// salts), the last frame, and the checksum chain after it. Kept in every segment and manifest,
/// so the shipper can pick up from a staged object after a crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LogPoint {
    pub salt1: u32,
    pub salt2: u32,
    pub frame: u32,
    pub ck1: u32,
    pub ck2: u32,
}

/// A run of WAL frames, the unit the backup ships. Holds whole transactions, except when one
/// transaction is bigger than an object may be: then it is split, and every part but the last
/// has `ends_at_commit` false. A restore stops only after a part that ends at a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub page_size: u32,
    pub first_frame: u32,
    pub end: LogPoint,
    pub captured_ms: u64,
    /// The database size in pages after the last commit in this segment (0 when it ends mid-transaction).
    pub db_pages: u32,
    pub ends_at_commit: bool,
    /// (page number, commit size, page)
    pub frames: Vec<(u32, u32, Vec<u8>)>,
}

const SEG_TAG: &[u8; 8] = b"HOSSEG01";

impl Segment {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W::default();
        w.bytes(SEG_TAG)
            .u32(self.page_size)
            .u32(self.end.salt1)
            .u32(self.end.salt2)
            .u32(self.first_frame)
            .u32(self.end.frame)
            .u32(self.end.ck1)
            .u32(self.end.ck2)
            .u64(self.captured_ms)
            .u32(self.db_pages)
            .u8(self.ends_at_commit as u8)
            .u32(self.frames.len() as u32);
        for (pgno, commit, data) in &self.frames {
            w.u32(*pgno).u32(*commit).bytes(data);
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let mut r = R::new(b);
        if r.take(8)? != SEG_TAG {
            return Err("not a segment body".into());
        }
        let page_size = r.u32()?;
        if !(512..=65_536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(format!("impossible page size {page_size}"));
        }
        let salt1 = r.u32()?;
        let salt2 = r.u32()?;
        let first_frame = r.u32()?;
        let frame = r.u32()?;
        let ck1 = r.u32()?;
        let ck2 = r.u32()?;
        let captured_ms = r.u64()?;
        let db_pages = r.u32()?;
        let ends_at_commit = r.u8()? != 0;
        let n = r.u32()? as usize;
        if n > MAX_PLAIN / page_size as usize {
            return Err("too many frames".into());
        }
        let mut frames = Vec::with_capacity(n);
        for _ in 0..n {
            let pgno = r.u32()?;
            let commit = r.u32()?;
            if pgno == 0 {
                return Err("page number 0".into());
            }
            frames.push((pgno, commit, r.take(page_size as usize)?.to_vec()));
        }
        if !r.done() {
            return Err("bytes after the last frame".into());
        }
        Ok(Self {
            page_size,
            first_frame,
            end: LogPoint { salt1, salt2, frame, ck1, ck2 },
            captured_ms,
            db_pages,
            ends_at_commit,
            frames,
        })
    }
}

/// What a snapshot is: the database as it was after segment `seq` of its generation, in
/// `chunk_count` chunks of `chunk_size` bytes, each with its BLAKE3 hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub page_size: u32,
    pub db_len: u64,
    pub chunk_size: u32,
    pub taken_ms: u64,
    pub seq: u64,
    /// The log position the snapshot was taken at (None: no log at that moment).
    pub point: Option<LogPoint>,
    pub chunk_hashes: Vec<[u8; 32]>,
}

const SNAP_TAG: &[u8; 8] = b"HOSSNP01";

impl Manifest {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W::default();
        w.bytes(SNAP_TAG)
            .u32(self.page_size)
            .u64(self.db_len)
            .u32(self.chunk_size)
            .u64(self.taken_ms)
            .u64(self.seq);
        match self.point {
            Some(p) => {
                w.u8(1).u32(p.salt1).u32(p.salt2).u32(p.frame).u32(p.ck1).u32(p.ck2);
            }
            None => {
                w.u8(0).u32(0).u32(0).u32(0).u32(0).u32(0);
            }
        }
        w.u32(self.chunk_hashes.len() as u32);
        for h in &self.chunk_hashes {
            w.bytes(h);
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let mut r = R::new(b);
        if r.take(8)? != SNAP_TAG {
            return Err("not a snapshot manifest".into());
        }
        let page_size = r.u32()?;
        let db_len = r.u64()?;
        let chunk_size = r.u32()?;
        let taken_ms = r.u64()?;
        let seq = r.u64()?;
        let has = r.u8()? != 0;
        let p = LogPoint { salt1: r.u32()?, salt2: r.u32()?, frame: r.u32()?, ck1: r.u32()?, ck2: r.u32()? };
        let n = r.u32()? as usize;
        if chunk_size == 0 || n as u64 != db_len.div_ceil(chunk_size as u64) {
            return Err("chunk count does not match the size".into());
        }
        let mut chunk_hashes = Vec::with_capacity(n);
        for _ in 0..n {
            let mut h = [0u8; 32];
            h.copy_from_slice(r.take(32)?);
            chunk_hashes.push(h);
        }
        if !r.done() {
            return Err("bytes after the manifest".into());
        }
        Ok(Self { page_size, db_len, chunk_size, taken_ms, seq, point: has.then_some(p), chunk_hashes })
    }
}

/// The mark a destination's owner leaves at its root: which server keeps its backups here.
/// Sealed like everything else, so only the same backup key can read it, and a destination
/// already holding another server's (or another key's) backups is refused before anything is
/// mixed into it.
pub fn owner_body(server_fingerprint: &str, created_ms: u64) -> Vec<u8> {
    let mut w = W::default();
    w.bytes(b"HOSOWN01").u64(created_ms).bytes(server_fingerprint.as_bytes());
    w.0
}

/// (server fingerprint, created) from an owner mark's body.
pub fn owner_decode(b: &[u8]) -> Result<(String, u64), String> {
    let mut r = R::new(b);
    if r.take(8)? != b"HOSOWN01" {
        return Err("not an owner mark".into());
    }
    let created = r.u64()?;
    let rest = r.take(b.len() - 16)?;
    Ok((String::from_utf8_lossy(rest).to_string(), created))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> [u8; 32] {
        object_key(&[n; 32])
    }

    /// Sealing round-trips; the sealed bytes do not contain the plaintext; and the object does
    /// not open with another key, with one byte changed, under another name, or as another
    /// kind. Compression is used when it helps and skipped when it does not.
    ///
    /// Seen red 2026-10-22 with the name left out of the authenticated data: "another name
    /// opened it".
    #[test]
    fn sealed_objects_open_only_with_their_key_and_name() {
        let plain = b"a page of the relay database with SECRET-WORDS in it ".repeat(100);
        let k = key(1);
        let sealed = seal(&k, "g1/seg-0000000001-0000000000001-c", Kind::Segment, &plain);
        assert_eq!(open(&k, "g1/seg-0000000001-0000000000001-c", Kind::Segment, &sealed).unwrap(), plain);
        assert!(!sealed.windows(12).any(|w| w == b"SECRET-WORDS"), "the plaintext shows through");
        assert!(sealed.len() < plain.len() / 4, "repetitive text was not compressed: {} of {}", sealed.len(), plain.len());

        assert!(open(&key(2), "g1/seg-0000000001-0000000000001-c", Kind::Segment, &sealed).is_err(), "another key opened it");
        assert!(open(&k, "g1/seg-0000000002-0000000000001-c", Kind::Segment, &sealed).is_err(), "another name opened it");
        assert!(open(&k, "g1/seg-0000000001-0000000000001-c", Kind::SnapChunk, &sealed).is_err(), "another kind opened it");
        for at in [6usize, 9, 30, sealed.len() - 1] {
            let mut bad = sealed.clone();
            bad[at] ^= 0x10;
            assert!(open(&k, "g1/seg-0000000001-0000000000001-c", Kind::Segment, &bad).is_err(), "a byte changed at {at} still opened");
        }
        // Random bytes do not shrink: stored as they are, and still round-trip.
        let noise: Vec<u8> = (0..4096).map(|_| rand::random::<u8>()).collect();
        let s = seal(&k, "x", Kind::Probe, &noise);
        assert_eq!(s[6] & FLAG_DEFLATE, 0);
        assert_eq!(open(&k, "x", Kind::Probe, &s).unwrap(), noise);
    }

    #[test]
    fn segment_and_manifest_bodies_round_trip() {
        let seg = Segment {
            page_size: 512,
            first_frame: 4,
            end: LogPoint { salt1: 7, salt2: 9, frame: 5, ck1: 11, ck2: 13 },
            captured_ms: 1_790_000_000_000,
            db_pages: 3,
            ends_at_commit: true,
            frames: vec![(1, 0, vec![1u8; 512]), (3, 3, vec![2u8; 512])],
        };
        assert_eq!(Segment::decode(&seg.encode()).unwrap(), seg);
        let mut short = seg.encode();
        short.pop();
        assert!(Segment::decode(&short).is_err());
        let m = Manifest {
            page_size: 4096,
            db_len: 9000,
            chunk_size: 4096,
            taken_ms: 5,
            seq: 12,
            point: Some(seg.end),
            chunk_hashes: vec![[1; 32], [2; 32], [3; 32]],
        };
        assert_eq!(Manifest::decode(&m.encode()).unwrap(), m);
        let (fp, at) = owner_decode(&owner_body("abcd", 99)).unwrap();
        assert_eq!((fp.as_str(), at), ("abcd", 99));
    }
}
