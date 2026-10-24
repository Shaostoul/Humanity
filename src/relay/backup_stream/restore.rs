//! Rebuilding a database from the continuous backup (2026-10-22), for three callers: the
//! restore drill (into a scratch file, compared with the live database), the disaster restore
//! (`HumanityOS --restore-backup`, onto a new machine), and the tests.
//!
//! A restore writes a snapshot's chunks into a new file, then replays the segments after it,
//! writing each frame's page at its place and setting the file's length at each commit, exactly
//! as SQLite's own checkpoint would. Segments that make up one transaction are only written once
//! the segment ending it has been read and opened: a damaged or missing segment stops the
//! restore at the last whole transaction, never halfway through one. The file is built under a
//! temporary name and renamed into place only after SQLite's full integrity check passes on it.

use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use rusqlite::Connection;

use super::dest::Destination;
use super::names::{self, Catalog, Target};
use super::seal::{self, Kind, Manifest, Segment};

/// What a restore produced.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Report {
    pub gen: String,
    pub snapshot_ms: u64,
    /// The time of the newest change in the restored file.
    pub restored_to_ms: u64,
    /// The last segment applied (0: only the snapshot).
    pub seq: u64,
    pub segments: u64,
    pub frames: u64,
    pub db_bytes: u64,
    /// Why the restore stopped before the end of what the plan named, if it did.
    pub stopped_early: Option<String>,
}

/// Restore from `dest` into `out`, which must not exist yet. `key` is the object key
/// (`seal::object_key`). The integrity check runs before `out` appears.
pub async fn restore(dest: &Destination, key: &[u8; 32], out: &Path, target: Target<'_>) -> Result<Report, String> {
    if out.exists() {
        return Err(format!("{} is already there: restore into a new file", out.display()));
    }
    let list = dest.list().await?;
    let cat = Catalog::from_listing(&list);
    let plan = names::plan_restore(&cat, target).ok_or_else(|| {
        if cat.gens.is_empty() {
            "the destination holds no backup yet".to_string()
        } else {
            "the destination holds no complete snapshot for that point yet (the first full copy may still be on its way)".to_string()
        }
    })?;
    let tmp = out.with_file_name(format!("{}.restoring", out.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
    let _ = std::fs::remove_file(&tmp);
    let res = build(dest, key, &tmp, &plan).await;
    let report = match res {
        Ok(r) => r,
        Err(e) => {
            remove_db_files(&tmp);
            return Err(e);
        }
    };
    let check_path = tmp.clone();
    let check = tokio::task::spawn_blocking(move || integrity(&check_path)).await.map_err(|e| e.to_string())?;
    if let Err(e) = check {
        remove_db_files(&tmp);
        return Err(format!("the rebuilt database fails SQLite's integrity check: {e}"));
    }
    std::fs::rename(&tmp, out).map_err(|e| format!("putting the restored file in place: {e}"))?;
    Ok(report)
}

async fn build(dest: &Destination, key: &[u8; 32], tmp: &Path, plan: &names::Plan) -> Result<Report, String> {
    let dir = plan.snap.dir();
    let n = plan.snap.chunks.unwrap_or(0);
    let mname = names::manifest_name(&dir, n);
    let sealed = dest.get(&mname).await?.ok_or("the snapshot's manifest went missing")?;
    let m = Manifest::decode(&seal::open(key, &mname, Kind::SnapManifest, &sealed)?)?;
    if m.chunk_hashes.len() as u32 != n {
        return Err("the snapshot's manifest does not match its name".into());
    }
    let mut f = std::fs::File::create(tmp).map_err(|e| format!("making {}: {e}", tmp.display()))?;
    for i in 0..n {
        let name = names::chunk_name(&dir, i);
        let sealed = dest.get(&name).await?.ok_or_else(|| format!("chunk {i} of the snapshot went missing"))?;
        let chunk = seal::open(key, &name, Kind::SnapChunk, &sealed)?;
        if blake3::hash(&chunk).as_bytes() != &m.chunk_hashes[i as usize] {
            return Err(format!("chunk {i} of the snapshot is not the one its manifest names"));
        }
        f.write_all(&chunk).map_err(|e| format!("writing the restore: {e}"))?;
    }
    let len = f.metadata().map(|x| x.len()).unwrap_or(0);
    if len != m.db_len {
        return Err(format!("the snapshot is {len} bytes, its manifest says {}", m.db_len));
    }
    let mut report = Report { gen: plan.snap.gen.clone(), snapshot_ms: plan.snap.ms, restored_to_ms: plan.snap.ms, seq: plan.snap.seq, db_bytes: len, ..Default::default() };
    let mut open_txn: Vec<(u64, u64, Segment)> = Vec::new();
    for s in &plan.segs {
        let name = s.name();
        let seg = match dest.get(&name).await {
            Ok(Some(b)) => match seal::open(key, &name, Kind::Segment, &b).and_then(|p| Segment::decode(&p)) {
                Ok(seg) => seg,
                Err(e) => {
                    report.stopped_early = Some(format!("segment {} could not be opened ({e})", s.seq));
                    break;
                }
            },
            Ok(None) => {
                report.stopped_early = Some(format!("segment {} went missing", s.seq));
                break;
            }
            Err(e) => {
                report.stopped_early = Some(format!("segment {} could not be read ({e})", s.seq));
                break;
            }
        };
        if seg.page_size != m.page_size {
            report.stopped_early = Some(format!("segment {} has another page size", s.seq));
            break;
        }
        let ends = seg.ends_at_commit;
        open_txn.push((s.seq, s.ms, seg));
        if ends {
            for (seq, ms, seg) in open_txn.drain(..) {
                apply(&mut f, &seg).map_err(|e| format!("writing the restore: {e}"))?;
                report.frames += seg.frames.len() as u64;
                report.segments += 1;
                report.seq = seq;
                report.restored_to_ms = ms;
            }
        }
    }
    if !open_txn.is_empty() && report.stopped_early.is_none() {
        report.stopped_early = Some("the last transaction's end never arrived".into());
    }
    f.sync_all().map_err(|e| format!("flushing the restore: {e}"))?;
    report.db_bytes = f.metadata().map(|x| x.len()).unwrap_or(0);
    Ok(report)
}

/// Write one segment's pages into the database file, as a checkpoint would.
pub fn apply(f: &mut std::fs::File, seg: &Segment) -> std::io::Result<()> {
    let ps = seg.page_size as u64;
    for (pgno, commit, data) in &seg.frames {
        f.seek(SeekFrom::Start((*pgno as u64 - 1) * ps))?;
        f.write_all(data)?;
        if *commit != 0 {
            f.set_len(*commit as u64 * ps)?;
        }
    }
    Ok(())
}

/// SQLite's full integrity check on a database file.
pub fn integrity(path: &Path) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    let rows: Vec<String> = {
        let mut st = conn.prepare("PRAGMA integrity_check").map_err(|e| e.to_string())?;
        let it = st.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        it.filter_map(|r| r.ok()).collect()
    };
    drop(conn);
    if rows.len() == 1 && rows[0] == "ok" {
        Ok(())
    } else {
        Err(rows.into_iter().take(3).collect::<Vec<_>>().join("; "))
    }
}

/// Delete a database file and the files SQLite keeps beside it.
pub fn remove_db_files(path: &Path) {
    for side in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{side}", path.display()));
    }
}

/// A digest of everything a database holds, independent of how its pages happen to be laid
/// out: the schema, then every row of every table in key order. Two databases with the same
/// digest hold the same data. Used by the restore drill to compare a restored copy with the
/// live database at the same moment.
pub fn logical_digest(conn: &Connection) -> Result<String, String> {
    let e = |x: rusqlite::Error| x.to_string();
    let mut h = blake3::Hasher::new();
    let tables: Vec<(String, String)> = {
        let mut st = conn
            .prepare("SELECT name, COALESCE(sql, '') FROM sqlite_schema WHERE type IN ('table','index','view','trigger') ORDER BY type, name")
            .map_err(e)?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(e)?;
        rows.filter_map(|r| r.ok()).collect()
    };
    for (name, sql) in &tables {
        h.update(b"schema\0");
        h.update(name.as_bytes());
        h.update(b"\0");
        h.update(sql.as_bytes());
        h.update(b"\0");
    }
    let plain: Vec<(String, String)> = {
        let mut st = conn
            .prepare("SELECT name, COALESCE(sql, '') FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_stat%' ORDER BY name")
            .map_err(e)?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(e)?;
        rows.filter_map(|r| r.ok()).collect()
    };
    for (name, sql) in plain {
        if sql.to_ascii_uppercase().starts_with("CREATE VIRTUAL") {
            continue; // its shadow tables are ordinary tables and are read on their own
        }
        let quoted = format!("\"{}\"", name.replace('"', "\"\""));
        let without_rowid = sql.to_ascii_uppercase().contains("WITHOUT ROWID");
        let order = if without_rowid {
            let pk: Vec<String> = {
                let mut st = conn.prepare(&format!("SELECT name FROM pragma_table_info({}) WHERE pk > 0 ORDER BY pk", sql_str(&name))).map_err(e)?;
                let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(e)?;
                rows.filter_map(|r| r.ok()).map(|c| format!("\"{}\"", c.replace('"', "\"\""))).collect()
            };
            if pk.is_empty() { String::new() } else { format!(" ORDER BY {}", pk.join(", ")) }
        } else {
            " ORDER BY rowid".to_string()
        };
        let select = if without_rowid { format!("SELECT * FROM {quoted}{order}") } else { format!("SELECT rowid, * FROM {quoted}{order}") };
        h.update(b"table\0");
        h.update(name.as_bytes());
        let mut st = conn.prepare(&select).map_err(e)?;
        let cols = st.column_count();
        let mut rows = st.query([]).map_err(e)?;
        while let Some(row) = rows.next().map_err(e)? {
            h.update(b"\x1erow");
            for i in 0..cols {
                use rusqlite::types::ValueRef;
                match row.get_ref(i).map_err(e)? {
                    ValueRef::Null => {
                        h.update(b"N");
                    }
                    ValueRef::Integer(v) => {
                        h.update(b"I");
                        h.update(&v.to_le_bytes());
                    }
                    ValueRef::Real(v) => {
                        h.update(b"R");
                        h.update(&v.to_bits().to_le_bytes());
                    }
                    ValueRef::Text(t) => {
                        h.update(b"T");
                        h.update(&(t.len() as u64).to_le_bytes());
                        h.update(t);
                    }
                    ValueRef::Blob(b) => {
                        h.update(b"B");
                        h.update(&(b.len() as u64).to_le_bytes());
                        h.update(b);
                    }
                }
            }
        }
    }
    Ok(h.finalize().to_hex().to_string())
}

fn sql_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Where the command-line tools say what happened: the console, and with `--log <file>` that
/// file too (the release exe on Windows has no console, and a scheduled task has no one
/// watching it).
struct Out {
    log: Option<std::path::PathBuf>,
}

impl Out {
    fn say(&self, line: &str) {
        println!("{line}");
        self.append(line);
    }
    fn err(&self, line: &str) {
        eprintln!("{line}");
        self.append(line);
    }
    fn append(&self, line: &str) {
        if let Some(p) = &self.log {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                let _ = writeln!(f, "{}  {line}", super::utc_text(crate::relay::storage::now_millis()));
            }
        }
    }
}

/// What the command-line tools need from a data folder: the backup key and the destination.
fn open_from_folder(data: &Path, out: &Out) -> Result<(Destination, [u8; 32], String), i32> {
    let key = match std::fs::read(data.join("backup.key")) {
        Ok(k) if k.len() == 32 => {
            let mut a = [0u8; 32];
            a.copy_from_slice(&k);
            a
        }
        _ => {
            out.err(&format!(
                "No usable backup key at {}: the backups cannot be opened without the backup.key they were made with.",
                data.join("backup.key").display()
            ));
            return Err(1);
        }
    };
    let Some(cfg) = super::load_config(data) else {
        out.err(&format!("No {}: copy it from the server (it says where the backups are, and holds the key that may read them).", data.join(super::CONFIG_FILE).display()));
        return Err(1);
    };
    let Some(dcfg) = cfg.destination.clone() else {
        out.err("The configuration names no destination.");
        return Err(1);
    };
    match Destination::from_config(&dcfg, cfg.sender_seed_bytes().as_ref()) {
        Ok(d) => Ok((d, key, dcfg.summary())),
        Err(e) => {
            out.err(&format!("The destination cannot be used: {e}"));
            Err(1)
        }
    }
}

fn runtime(out: &Out) -> Option<tokio::runtime::Runtime> {
    match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(r) => Some(r),
        Err(e) => {
            out.err(&format!("could not start: {e}"));
            None
        }
    }
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

/// `HumanityOS --restore-backup <new-file> [--at <time>] [--data <folder>] [--log <file>]`: the
/// disaster restore, for a machine with no running relay. Reads `backup.key` and
/// `backup-destination.secrets.json` from the data folder (default `data`), restores the
/// newest state (or the newest at or before `--at`, given as Unix milliseconds or as
/// `YYYY-MM-DDTHH:MM:SSZ`) into the new file, and prints what it did. Returns the exit code.
pub fn cli_main(args: &[String]) -> i32 {
    let out = Out { log: arg(args, "--log").map(Into::into) };
    let Some(file) = arg(args, "--restore-backup") else {
        out.err("usage: HumanityOS --restore-backup <new-file.db> [--at <unix-ms | YYYY-MM-DDTHH:MM:SSZ>] [--data <folder with backup.key>] [--log <file>]");
        return 2;
    };
    let data = std::path::PathBuf::from(arg(args, "--data").unwrap_or_else(|| "data".into()));
    let target_ms = match arg(args, "--at") {
        None => None,
        Some(s) => match super::parse_time(&s) {
            Some(ms) => Some(ms),
            None => {
                out.err(&format!("--at takes Unix milliseconds or YYYY-MM-DDTHH:MM:SSZ, not {s}"));
                return 2;
            }
        },
    };
    let (dest, key, summary) = match open_from_folder(&data, &out) {
        Ok(x) => x,
        Err(code) => return code,
    };
    out.say(&format!("Restoring from {summary} with backup key {}", seal::key_fingerprint(&key)));
    let Some(rt) = runtime(&out) else { return 1 };
    let target = match target_ms {
        Some(ms) => Target::Time(ms),
        None => Target::Latest,
    };
    match rt.block_on(restore(&dest, &seal::object_key(&key), Path::new(&file), target)) {
        Ok(r) => {
            out.say(&format!(
                "Restored {file} ({}) to {}: the full copy of {}, then {} change runs; SQLite's integrity check is ok.",
                super::human_bytes(r.db_bytes),
                super::utc_text(r.restored_to_ms),
                super::utc_text(r.snapshot_ms),
                r.segments
            ));
            if let Some(why) = r.stopped_early {
                out.say(&format!("Stopped early: {why}. The file is whole, as of the time above."));
            }
            out.say("To use it: stop the relay, move the old data/relay.db (and its -wal and -shm files) aside, put this file in its place as relay.db, and start the relay.");
            0
        }
        Err(e) => {
            out.err(&format!("Restore failed: {e}"));
            1
        }
    }
}

/// `HumanityOS --backup-drill [--data <folder>] [--max-age-minutes N] [--log <file>]`: the
/// restore drill a machine away from the server runs on a schedule (the operator's PC, monthly:
/// docs/admin/continuous-backup.md). Restores the newest state from the destination into a
/// temporary file, which must pass SQLite's integrity check and be no older than
/// `--max-age-minutes` (default 60: a server that writes nothing for an hour still passes,
/// because the copy is then as new as the database), lists what is in it, and deletes it.
/// Exit codes: 0 passed, 1 failed, 2 usage, 7 the newest copy is older than allowed.
pub fn cli_drill(args: &[String]) -> i32 {
    let out = Out { log: arg(args, "--log").map(Into::into) };
    let data = std::path::PathBuf::from(arg(args, "--data").unwrap_or_else(|| "data".into()));
    let max_age_min: u64 = match arg(args, "--max-age-minutes").map(|s| s.parse()) {
        None => 60,
        Some(Ok(n)) => n,
        Some(Err(_)) => {
            out.err("--max-age-minutes takes a whole number");
            return 2;
        }
    };
    let (dest, key, summary) = match open_from_folder(&data, &out) {
        Ok(x) => x,
        Err(code) => return code,
    };
    out.say(&format!("=== restore drill: from {summary}, backup key {} ===", seal::key_fingerprint(&key)));
    let Some(rt) = runtime(&out) else { return 1 };
    let dir = std::env::temp_dir().join(format!("hos-backup-drill-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("restored.db");
    let started = crate::relay::storage::now_millis();
    let res = rt.block_on(restore(&dest, &seal::object_key(&key), &file, Target::Latest));
    let code = match res {
        Ok(r) => {
            let took = crate::relay::storage::now_millis().saturating_sub(started);
            out.say(&format!(
                "Restored {} as of {} (the full copy of {}, then {} change runs) in {:.1} s; SQLite's integrity check is ok.",
                super::human_bytes(r.db_bytes),
                super::utc_text(r.restored_to_ms),
                super::utc_text(r.snapshot_ms),
                r.segments,
                took as f64 / 1000.0
            ));
            if let Some(why) = &r.stopped_early {
                out.err(&format!("Stopped early: {why}."));
            }
            match Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string()).and_then(|c| table_counts(&c)) {
                Ok(counts) => {
                    let line: Vec<String> = counts.iter().filter(|(_, n)| *n > 0).map(|(t, n)| format!("{t} {n}")).collect();
                    out.say(&format!("Rows: {}", line.join(", ")));
                }
                Err(e) => out.err(&format!("Could not count the rows: {e}")),
            }
            let age_min = crate::relay::storage::now_millis().saturating_sub(r.restored_to_ms) / 60_000;
            if r.stopped_early.is_some() {
                1
            } else if age_min > max_age_min {
                out.err(&format!("FAILED: the newest copy is {age_min} min old (limit {max_age_min} min): the server's continuous backup is behind or stopped."));
                7
            } else {
                out.say("=== restore drill PASSED ===");
                0
            }
        }
        Err(e) => {
            out.err(&format!("FAILED: {e}"));
            1
        }
    };
    remove_db_files(&file);
    let _ = std::fs::remove_dir_all(&dir);
    code
}

/// Rows per table, for a drill's report.
pub fn table_counts(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    let names: Vec<String> = {
        let mut st = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND COALESCE(sql, '') NOT LIKE 'CREATE VIRTUAL%' ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };
    let mut out = Vec::new();
    for n in names {
        let c: i64 = conn.query_row(&format!("SELECT count(*) FROM \"{}\"", n.replace('"', "\"\"")), [], |r| r.get(0)).map_err(|e| e.to_string())?;
        out.push((n, c));
    }
    Ok(out)
}

/// `HumanityOS --check-db <file> [--log <file>]`: SQLite's full integrity check and the rows per
/// table of a database file, for a drill of a copy restored some other way (the PC's monthly
/// drill decrypts its newest pulled snapshot and checks it with this). Exit 0 when the check
/// passes, 1 when it does not, 2 for usage.
pub fn cli_check_db(args: &[String]) -> i32 {
    let out = Out { log: arg(args, "--log").map(Into::into) };
    let Some(file) = arg(args, "--check-db") else {
        out.err("usage: HumanityOS --check-db <file.db> [--log <file>]");
        return 2;
    };
    let path = Path::new(&file);
    if !path.is_file() {
        out.err(&format!("No file at {file}"));
        return 1;
    }
    match integrity(path) {
        Ok(()) => {
            let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            out.say(&format!("{file} ({}): SQLite's integrity check is ok.", super::human_bytes(len)));
            if let Ok(counts) = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string()).and_then(|c| table_counts(&c)) {
                let line: Vec<String> = counts.iter().filter(|(_, n)| *n > 0).map(|(t, n)| format!("{t} {n}")).collect();
                out.say(&format!("Rows: {}", line.join(", ")));
            }
            0
        }
        Err(e) => {
            out.err(&format!("FAILED: {file} fails SQLite's integrity check: {e}"));
            1
        }
    }
}
