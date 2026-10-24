//! Reading SQLite's write-ahead log straight from its file (continuous backup, 2026-10-22).
//!
//! The relay runs its database in WAL mode: every committed change is first appended to
//! `relay.db-wal` as whole pages ("frames"), and only later copied into `relay.db` by a
//! checkpoint. Shipping those frames, in order, to another place is how a copy can follow
//! the live database to within seconds: replaying them onto a snapshot rebuilds the database
//! page for page. This is the method Litestream made well known; it is built here in Rust so
//! a relay needs no extra program (docs/admin/continuous-backup.md).
//!
//! The file format is SQLite's own and stable since 3.7.0
//! (https://www.sqlite.org/fileformat2.html#walformat): a 32-byte header, then frames of a
//! 24-byte frame header plus one page. Every frame carries the header's two salts and a
//! running checksum over everything before it, so a reader can tell exactly where the valid
//! log ends: a frame whose salts or checksum do not match is left over from an earlier pass
//! of the log, or is half written. A frame whose "database size after commit" field is not
//! zero ends a transaction; frames after the last such frame belong to a transaction still
//! being written, and are never shipped.

use std::io::{Read, Seek, SeekFrom};

/// Size of the WAL file header.
pub const WAL_HEADER_LEN: u64 = 32;
/// Size of each frame's own header, before its page.
pub const FRAME_HEADER_LEN: u64 = 24;
/// The two magic numbers: the low bit says which byte order the checksums were computed in.
const MAGIC_LE: u32 = 0x377f_0682;
const MAGIC_BE: u32 = 0x377f_0683;
/// The only WAL format version SQLite has ever written.
const WAL_VERSION: u32 = 3_007_000;

/// The WAL header, as far as shipping needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalHeader {
    /// True when the checksums are computed over big-endian words (magic ...0683).
    pub big_endian: bool,
    pub page_size: u32,
    pub checkpoint_seq: u32,
    /// Salt-1 goes up by one each time SQLite starts the log over; salt-2 is new at random.
    pub salt1: u32,
    pub salt2: u32,
    /// The header's own checksum, which is also where the first frame's checksum chain starts.
    pub cksum: (u32, u32),
}

/// One committed (or committing) frame: the page number and the page itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub pgno: u32,
    /// The database size in pages after this commit; 0 for a frame that does not end a transaction.
    pub commit: u32,
    pub data: Vec<u8>,
}

/// Frames read from a position up to the last commit.
#[derive(Debug, Default)]
pub struct ReadOut {
    /// Every frame up to and including the last commit frame found, oldest first.
    pub frames: Vec<Frame>,
    /// The frame index (1-based within this pass of the log) of the last commit frame read;
    /// the starting index when there was none.
    pub last_frame: u32,
    /// The checksum chain after `last_frame`.
    pub cksum: (u32, u32),
    /// The database size in pages after that last commit; 0 when nothing new was committed.
    pub db_pages: u32,
    /// Valid frames found after the last commit (a transaction another program is still
    /// writing). Not shipped; read again next time.
    pub uncommitted: u32,
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// SQLite's WAL checksum (walChecksumBytes): two running sums over 32-bit words, read in the
/// byte order the header names. `data` is a multiple of 8 bytes (it always is: 8 header bytes
/// or a page).
pub fn checksum(big_endian: bool, data: &[u8], seed: (u32, u32)) -> (u32, u32) {
    let (mut s1, mut s2) = seed;
    for pair in data.chunks_exact(8) {
        let (a, b) = if big_endian {
            (be32(&pair[0..4]), be32(&pair[4..8]))
        } else {
            (
                u32::from_le_bytes([pair[0], pair[1], pair[2], pair[3]]),
                u32::from_le_bytes([pair[4], pair[5], pair[6], pair[7]]),
            )
        };
        s1 = s1.wrapping_add(a).wrapping_add(s2);
        s2 = s2.wrapping_add(b).wrapping_add(s1);
    }
    (s1, s2)
}

/// Parse and check a WAL header. None when the bytes are not a valid header: too short, the
/// wrong magic or version, an impossible page size, or a checksum that does not match (a
/// header SQLite was in the middle of writing).
pub fn parse_header(b: &[u8]) -> Option<WalHeader> {
    if b.len() < WAL_HEADER_LEN as usize {
        return None;
    }
    let magic = be32(&b[0..4]);
    let big_endian = match magic {
        MAGIC_LE => false,
        MAGIC_BE => true,
        _ => return None,
    };
    if be32(&b[4..8]) != WAL_VERSION {
        return None;
    }
    let page_size = be32(&b[8..12]);
    if !(512..=65_536).contains(&page_size) || !page_size.is_power_of_two() {
        return None;
    }
    let cksum = (be32(&b[24..28]), be32(&b[28..32]));
    if checksum(big_endian, &b[0..24], (0, 0)) != cksum {
        return None;
    }
    Some(WalHeader {
        big_endian,
        page_size,
        checkpoint_seq: be32(&b[12..16]),
        salt1: be32(&b[16..20]),
        salt2: be32(&b[20..24]),
        cksum,
    })
}

/// Read the header of the WAL file at `path`. Ok(None) when the file is missing, empty or
/// shorter than a header (no log yet, or one a checkpoint truncated), or when its header is
/// not valid yet.
pub fn read_header(path: &std::path::Path) -> std::io::Result<Option<WalHeader>> {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut buf = [0u8; WAL_HEADER_LEN as usize];
    let mut got = 0;
    while got < buf.len() {
        let n = f.read(&mut buf[got..])?;
        if n == 0 {
            return Ok(None);
        }
        got += n;
    }
    Ok(parse_header(&buf))
}

/// Read the frames after frame `after` (0 = from the start of this pass of the log), chaining
/// the checksum from `cksum` (the header's checksum when `after` is 0), up to the last commit
/// frame. Stops at the end of the file, at a short frame, and at the first frame whose salts or
/// checksum do not match: that is where the valid log ends.
pub fn read_committed(
    path: &std::path::Path,
    h: &WalHeader,
    after: u32,
    cksum: (u32, u32),
) -> std::io::Result<ReadOut> {
    let mut out = ReadOut { last_frame: after, cksum, ..Default::default() };
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    let ps = h.page_size as u64;
    let frame_len = FRAME_HEADER_LEN + ps;
    f.seek(SeekFrom::Start(WAL_HEADER_LEN + after as u64 * frame_len))?;
    let mut r = std::io::BufReader::with_capacity(256 * 1024, f);
    let mut chain = cksum;
    let mut index = after;
    // Frames of the transaction being read, moved to `out.frames` when its commit frame arrives.
    let mut open: Vec<Frame> = Vec::new();
    let mut fh = [0u8; FRAME_HEADER_LEN as usize];
    loop {
        if !read_full(&mut r, &mut fh)? {
            break;
        }
        let mut page = vec![0u8; ps as usize];
        if !read_full(&mut r, &mut page)? {
            break;
        }
        if be32(&fh[8..12]) != h.salt1 || be32(&fh[12..16]) != h.salt2 {
            break;
        }
        let next = checksum(h.big_endian, &page, checksum(h.big_endian, &fh[0..8], chain));
        if next != (be32(&fh[16..20]), be32(&fh[20..24])) {
            break;
        }
        chain = next;
        index += 1;
        let commit = be32(&fh[4..8]);
        open.push(Frame { pgno: be32(&fh[0..4]), commit, data: page });
        if commit != 0 {
            out.frames.append(&mut open);
            out.last_frame = index;
            out.cksum = chain;
            out.db_pages = commit;
        }
    }
    out.uncommitted = open.len() as u32;
    Ok(out)
}

/// Fill `buf` completely; false at a clean or partial end of file.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<bool> {
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..]) {
            Ok(0) => return Ok(false),
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// A database in WAL mode with SQLite's own automatic checkpoint off, so every frame it
    /// writes stays in the log for the test to read.
    fn wal_db(tag: &str) -> (crate::test_temp::TempPath, Connection) {
        let dir = crate::test_temp::dir(tag);
        let conn = Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT);")
            .unwrap();
        (dir, conn)
    }

    /// The reader agrees with SQLite about where the log ends: every frame SQLite wrote is
    /// read with a matching checksum chain, commits are found, and a byte changed in the
    /// middle of the log ends the valid log at the frame before it. (Its own red run, the
    /// checksum words read in the other byte order, could not be built on 2026-10-22: the
    /// machine ran out of memory for the test build several times. Every integration test in
    /// tests.rs reads the log through this reader.)
    #[test]
    fn the_reader_finds_every_committed_frame_sqlite_wrote() {
        let (dir, conn) = wal_db("wal_read");
        for i in 0..20 {
            conn.execute("INSERT INTO t(v) VALUES (?1)", [format!("row {i} {}", "x".repeat(300))]).unwrap();
        }
        let wal = dir.join("t.db-wal");
        let raw = std::fs::read(&wal).unwrap();
        let h = parse_header(&raw).expect("the header SQLite wrote parses");
        let out = read_committed(&wal, &h, 0, h.cksum).unwrap();
        let frames_in_file = ((raw.len() as u64 - WAL_HEADER_LEN) / (FRAME_HEADER_LEN + h.page_size as u64)) as u32;
        assert_eq!(out.last_frame, frames_in_file, "every frame in the file is part of a committed transaction");
        assert_eq!(out.frames.len() as u32, frames_in_file);
        let commits = out.frames.iter().filter(|f| f.commit != 0).count();
        assert!(commits >= 20, "one commit per insert at least: {commits}");
        assert_eq!(out.uncommitted, 0);
        let pages: u32 = conn.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
        assert_eq!(out.db_pages, pages, "the last commit's size is the database's size");

        // Reading on from the middle continues the same chain to the same end.
        let half = read_committed(&wal, &h, 0, h.cksum).map(|o| o.frames[..5].to_vec()).unwrap();
        let mid_commit = half.iter().rposition(|f| f.commit != 0).map(|i| i as u32 + 1);
        if let Some(mid) = mid_commit {
            let first = read_committed_limited(&wal, &h, mid);
            let rest = read_committed(&wal, &h, mid, first).unwrap();
            assert_eq!(rest.last_frame, frames_in_file, "reading on from frame {mid} reaches the end");
        }

        // One byte changed inside frame 3's page: the valid log now ends before frame 3.
        let mut bad = raw.clone();
        let off = (WAL_HEADER_LEN + 2 * (FRAME_HEADER_LEN + h.page_size as u64) + FRAME_HEADER_LEN + 100) as usize;
        bad[off] ^= 0xff;
        let bad_path = dir.join("bad-wal");
        std::fs::write(&bad_path, &bad).unwrap();
        let cut = read_committed(&bad_path, &h, 0, h.cksum).unwrap();
        assert!(cut.last_frame <= 2, "a damaged frame ends the valid log: read to {}", cut.last_frame);
        drop(conn);
    }

    /// The checksum chain after the first `n` frames, the way a shipper keeps it.
    fn read_committed_limited(path: &std::path::Path, h: &WalHeader, n: u32) -> (u32, u32) {
        let raw = std::fs::read(path).unwrap();
        let ps = h.page_size as usize;
        let mut ck = h.cksum;
        for i in 0..n as usize {
            let at = WAL_HEADER_LEN as usize + i * (FRAME_HEADER_LEN as usize + ps);
            ck = checksum(h.big_endian, &raw[at..at + 8], ck);
            ck = checksum(h.big_endian, &raw[at + 24..at + 24 + ps], ck);
        }
        ck
    }

    /// A header with a bad checksum, magic or page size is not a header.
    #[test]
    fn a_damaged_header_is_refused() {
        let (dir, conn) = wal_db("wal_hdr");
        conn.execute("INSERT INTO t(v) VALUES ('a')", []).unwrap();
        let raw = std::fs::read(dir.join("t.db-wal")).unwrap();
        assert!(parse_header(&raw).is_some());
        for at in [0usize, 5, 10, 17, 30] {
            let mut b = raw[..32].to_vec();
            b[at] ^= 0x01;
            assert!(parse_header(&b).is_none(), "a header changed at byte {at} was accepted");
        }
        assert!(parse_header(&raw[..31]).is_none());
        drop(conn);
    }
}
