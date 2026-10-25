//! The continuous backup end to end (2026-10-22): a real relay database, its real write-ahead
//! log, a real destination, and a restore compared with the live database.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rusqlite::params;

use super::dest::{Destination, DestinationConfig, FolderDest};
use super::names::{Catalog, Target};
use super::restore::{logical_digest, restore};
use super::*;

/// A relay database with continuous backup on, shipping to a folder.
struct Rig {
    db: Arc<Storage>,
    rs: RunState,
    data: PathBuf,
    dest: PathBuf,
    _dir: crate::test_temp::TempPath,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let dir = crate::test_temp::dir(tag);
        let dest = dir.join("dest");
        Self::in_dir(dir, dest)
    }

    fn in_dir(dir: crate::test_temp::TempPath, dest: PathBuf) -> Self {
        let data = dir.join("data");
        std::fs::create_dir_all(&data).unwrap();
        if load_config(&data).is_none() {
            let cfg = StreamConfig { enabled: true, destination: Some(DestinationConfig::Folder { path: dest.to_string_lossy().to_string() }), ..Default::default() };
            save_config(&data, &cfg).unwrap();
        }
        let db = Arc::new(Storage::open(&data.join("relay.db")).expect("open"));
        db.backup_stream_install().expect("install");
        Rig { db, rs: RunState::default(), data, dest, _dir: dir }
    }

    async fn pass(&mut self) {
        pass(&self.db, &mut self.rs).await;
    }

    fn folder(&self) -> Destination {
        Destination::Folder(FolderDest::new(self.dest.clone()))
    }

    fn key(&self) -> [u8; 32] {
        object_key_for(&self.db).unwrap()
    }

    fn live_digest(&self) -> String {
        self.db.with_conn(|c| logical_digest(c)).unwrap()
    }

    fn position(&self) -> (String, u64) {
        let g = self.db.stream.0.cap.lock().unwrap();
        let c = g.as_ref().unwrap();
        (c.st.gen.clone(), c.st.seq)
    }

    async fn restore_digest(&self, target: Target<'_>) -> Result<(String, restore::Report), String> {
        let out = self.data.join(format!("restored-{:08x}.db", rand::random::<u32>()));
        let r = restore(&self.folder(), &self.key(), &out, target).await?;
        let c = rusqlite::Connection::open(&out).unwrap();
        let d = logical_digest(&c).unwrap();
        drop(c);
        restore::remove_db_files(&out);
        Ok((d, r))
    }
}

fn write_rows(db: &Storage, from: i64, n: i64) {
    db.with_conn(|c| c.execute_batch("CREATE TABLE IF NOT EXISTS t(id INTEGER PRIMARY KEY, v TEXT, b BLOB)").unwrap());
    for i in from..from + n {
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO t(id, v, b) VALUES (?1, ?2, randomblob(?3))",
                params![i, format!("row {i} SECRET-ROW-MARKER {}", "x".repeat(((i % 7) * 300) as usize)), (i % 5) * 1500],
            )
            .unwrap()
        });
    }
}

/// Every change shipped and replayed gives the same database, page for page: through new rows,
/// big rows that spill onto overflow pages, deletes, an index, a dropped table, the relay's own
/// privacy checkpoint (which empties the log) and SQLite starting the log over. Every
/// intermediate point restores to exactly what the live database held then. And it all stays
/// one generation: the shipper never needed a second full copy.
///
/// Seen red 2026-10-22 with the privacy checkpoint no longer capturing first (`wal_truncate`; in the same
/// run the shipper also followed any new pass of the log): the restore was corrupt, "the rebuilt
/// database fails SQLite's integrity check: *** in database main *** Tree 265 page 280:
/// btreeInitPage() returns error code 11; wrong # of entries in index t_v".
#[tokio::test]
async fn every_change_shipped_and_replayed_gives_an_identical_database() {
    let mut rig = Rig::new("bs_replay");
    rig.pass().await;
    let (gen0, _) = rig.position();
    assert!(!gen0.is_empty());
    let mut points: Vec<((String, u64), String)> = Vec::new();

    write_rows(&rig.db, 0, 40);
    rig.pass().await;
    points.push((rig.position(), rig.live_digest()));

    rig.db.with_conn(|c| {
        c.execute_batch("CREATE INDEX t_v ON t(v); DELETE FROM t WHERE id % 3 = 0; UPDATE t SET v = v || ' changed' WHERE id % 4 = 1; CREATE TABLE gone(x); INSERT INTO gone VALUES (1); DROP TABLE gone;")
            .unwrap()
    });
    rig.pass().await;
    points.push((rig.position(), rig.live_digest()));

    // The relay's privacy scrub empties the log (through the capture-first hook), and the next
    // writes start a new pass of it. As in the relay's own bulk deletes, the delete and the
    // scrub happen together, after changes no pass has captured yet.
    write_rows(&rig.db, 50, 5);
    rig.db.with_conn(|c| {
        c.execute("DELETE FROM t WHERE id >= 50", []).unwrap();
        rig.db.wal_truncate(c)
    });
    write_rows(&rig.db, 100, 30);
    rig.pass().await;
    points.push((rig.position(), rig.live_digest()));

    // A long run: past the checkpoint threshold, so the shipper's own checkpoint runs and
    // SQLite starts the log over under it.
    write_rows(&rig.db, 1000, 700);
    rig.pass().await;
    write_rows(&rig.db, 2000, 50);
    rig.pass().await;
    points.push((rig.position(), rig.live_digest()));

    for ((gen, seq), want) in &points {
        assert_eq!(gen, &gen0, "the shipper started another generation; the log was followed without a break");
        let (got, r) = rig.restore_digest(Target::Seq(gen, *seq)).await.unwrap();
        assert_eq!(r.seq, *seq);
        assert_eq!(&got, want, "restoring to change run {seq} did not give the database as it was then");
    }

    // Byte for byte: fold the live log into the file, ship, restore, compare every page.
    rig.db.with_conn(|c| rig.db.wal_truncate(c));
    rig.pass().await;
    let out = rig.data.join("whole.db");
    restore(&rig.folder(), &rig.key(), &out, Target::Latest).await.unwrap();
    let live = std::fs::read(rig.data.join("relay.db")).unwrap();
    let back = std::fs::read(&out).unwrap();
    assert_eq!(live.len(), back.len(), "the restored file is not the same size");
    let ps = u16::from_be_bytes([live[16], live[17]]) as usize;
    for (i, (a, b)) in live.chunks(ps).zip(back.chunks(ps)).enumerate() {
        if i == 0 {
            // Page 1's header: SQLite's online backup (the snapshot) writes its own change
            // counter, schema cookie and version stamp there; everything else must match.
            for (at, (x, y)) in a.iter().zip(b.iter()).enumerate() {
                let header_counter = (24..28).contains(&at) || (40..44).contains(&at) || (92..100).contains(&at);
                assert!(header_counter || x == y, "page 1 differs at byte {at}");
            }
        } else {
            assert!(a == b, "page {} differs", i + 1);
        }
    }
    restore::remove_db_files(&out);
}

/// What reaches the destination is sealed: no plaintext from the database is in any object,
/// and none opens with another backup key.
#[tokio::test]
async fn nothing_at_the_destination_is_readable_without_the_key() {
    let mut rig = Rig::new("bs_sealed");
    rig.pass().await;
    write_rows(&rig.db, 0, 30);
    rig.pass().await;
    let list = rig.folder().list().await.unwrap();
    assert!(list.len() > 3, "objects reached the destination: {list:?}");
    let other = seal::object_key(&[9u8; 32]);
    for l in &list {
        let b = rig.folder().get(&l.name).await.unwrap().unwrap();
        assert!(!b.windows(17).any(|w| w == b"SECRET-ROW-MARKER"), "{} shows the plaintext", l.name);
        assert!(!b.windows(15).any(|w| w == b"SQLite format 3"), "{} is a plain database", l.name);
        for kind in [seal::Kind::Segment, seal::Kind::SnapChunk, seal::Kind::SnapManifest, seal::Kind::Owner] {
            assert!(seal::open(&other, &l.name, kind, &b).is_err(), "{} opened with another key", l.name);
        }
    }
    let (got, _) = rig.restore_digest(Target::Latest).await.unwrap();
    assert_eq!(got, rig.live_digest());
}

/// The restore drill restores from the destination into a scratch file, checks it and compares
/// it with the live database, and passes; the live database's files are untouched by it. When
/// the destination's copy is damaged, the drill fails and says so.
///
/// Seen red 2026-10-22 with the drill's two checks (how far it restored, and the comparison) taken out:
/// "a drill over a damaged copy passed: Passed: restored from the destination ... / left:
/// Some(true) / right: Some(false)".
#[tokio::test]
async fn the_restore_drill_passes_on_a_good_copy_and_fails_on_a_damaged_one() {
    let mut rig = Rig::new("bs_drill");
    rig.pass().await;
    write_rows(&rig.db, 0, 60);
    rig.pass().await;
    write_rows(&rig.db, 60, 5); // staged by the drill itself, not by a pass
    let live = rig.data.join("relay.db");
    let before = capture::file_digest(&live).unwrap();
    let d = drill(rig.db.clone()).await;
    assert_eq!(d.ok, Some(true), "{}", d.words);
    assert!(d.words.starts_with("Passed"), "{}", d.words);
    assert_eq!(capture::file_digest(&live).unwrap(), before, "the drill changed the live database file");
    assert!(!work_dir(&live).join("drill").exists(), "the scratch copy was left behind");

    // Damage the newest segment at the destination: the drill must fail.
    write_rows(&rig.db, 200, 5);
    rig.pass().await;
    let list = rig.folder().list().await.unwrap();
    let newest = list.iter().filter(|l| l.name.contains("/seg-")).map(|l| l.name.clone()).max().unwrap();
    let path = newest.split('/').fold(rig.dest.clone(), |p, c| p.join(c));
    let mut b = std::fs::read(&path).unwrap();
    let n = b.len();
    b[n - 5] ^= 0x55;
    std::fs::write(&path, b).unwrap();
    let d = drill(rig.db.clone()).await;
    assert_eq!(d.ok, Some(false), "a drill over a damaged copy passed: {}", d.words);
}

/// A crash in the middle of shipping never leaves the destination unusable:
/// - a write cut off at the destination (a `.part-` file) is never listed or used;
/// - an object sent again after a crash between "sent" and "deleted locally" changes nothing;
/// - a segment staged just before a crash, before the state file recorded it, is picked up at
///   the next start and the run continues without a gap;
/// - a damaged object at the destination stops a restore at the last whole transaction before
///   it, and that restore is exactly the database as it was then.
///
/// Seen red 2026-10-22 with the staged segments not read back at a start (`recover_from_pending`
/// skipped): "the staged segment was not picked up".
#[tokio::test]
async fn a_crash_mid_ship_never_corrupts_the_destination() {
    let mut rig = Rig::new("bs_crash");
    rig.pass().await;
    write_rows(&rig.db, 0, 20);
    rig.pass().await;
    let (gen, seq_a) = rig.position();
    let digest_a = rig.live_digest();

    // A torn write left at the destination.
    std::fs::write(rig.dest.join(&gen).join("seg-9999999999-0000000000000-c.part-deadbeef"), b"half").unwrap();

    // Stage a capture, then put the state file back as it was before it: the crash between
    // staging a segment and recording it.
    let state_file = work_dir(&rig.data.join("relay.db")).join("state.json");
    let saved_state = std::fs::read(&state_file).unwrap();
    write_rows(&rig.db, 20, 20);
    let tick = rig.db.backup_stream_tick();
    assert!(matches!(tick.captured, Some(capture::Captured::Staged { .. })));
    let digest_b = rig.live_digest();
    std::fs::write(&state_file, saved_state).unwrap();
    // The next start re-reads the state: open the capturer again on the same files (with the
    // writer lock held, as at a real start; the key is read before, since reading it takes the lock).
    let key = rig.key();
    rig.db.with_conn(|_| {
        let cap = capture::Capturer::open(&rig.data.join("relay.db"), key).unwrap();
        assert_eq!(cap.needs_new_gen, None, "the run could not continue after the crash");
        assert!(cap.st.seq > seq_a, "the staged segment was not picked up");
        *rig.db.stream.0.cap.lock().unwrap() = Some(cap);
    });
    rig.pass().await;
    let (gen_b, seq_b) = rig.position();
    assert_eq!(gen_b, gen, "a new generation was started");

    // A segment sent twice (as if the local copy was not deleted before a crash).
    let pending = work_dir(&rig.data.join("relay.db")).join("pending");
    let some_seg = rig.folder().list().await.unwrap().into_iter().find(|l| l.name.contains("/seg-")).unwrap().name;
    let again = some_seg.split('/').fold(pending.clone(), |p, c| p.join(c));
    std::fs::create_dir_all(again.parent().unwrap()).unwrap();
    std::fs::write(&again, rig.folder().get(&some_seg).await.unwrap().unwrap()).unwrap();
    rig.pass().await;
    assert_eq!(rig.db.backup_stream_status().last_error, "", "sending a segment again was an error");

    let (got, r) = rig.restore_digest(Target::Seq(&gen, seq_b)).await.unwrap();
    assert_eq!(got, digest_b, "the run after the crash restores to the live database");
    assert_eq!(r.seq, seq_b);

    // Damage at the destination: the newest segment's tail is cut off.
    write_rows(&rig.db, 40, 10);
    rig.pass().await;
    let newest = rig.folder().list().await.unwrap().into_iter().filter(|l| l.name.contains("/seg-")).map(|l| l.name).max().unwrap();
    let path = newest.split('/').fold(rig.dest.clone(), |p, c| p.join(c));
    let len = std::fs::metadata(&path).unwrap().len();
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(len / 2).unwrap();
    let (got, r) = rig.restore_digest(Target::Latest).await.unwrap();
    assert!(r.stopped_early.is_some(), "the damaged segment was used");
    assert_eq!(r.seq, seq_b, "the restore did not stop at the last whole run before the damage");
    assert_eq!(got, digest_b);
    let _ = digest_a;
}

/// When something other than the backup checkpoints the database and the log starts over (a
/// person with the sqlite3 tool, say), the shipper cannot prove the run unbroken: it starts a
/// new generation from a fresh full copy, and the newest restore is still exactly the live
/// database.
///
/// Seen red 2026-10-22 with any new pass of the log followed, whatever came before it: "assertion
/// `left != right` failed: the run was continued across a checkpoint the backup did not make".
#[tokio::test]
async fn a_checkpoint_the_backup_did_not_make_starts_a_new_generation() {
    let mut rig = Rig::new("bs_outside");
    rig.pass().await;
    write_rows(&rig.db, 0, 20);
    rig.pass().await;
    let (gen1, _) = rig.position();
    write_rows(&rig.db, 20, 20); // not captured yet
    let outside = rusqlite::Connection::open(rig.data.join("relay.db")).unwrap();
    outside.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); INSERT INTO t(id, v) VALUES (5000, 'from outside');").unwrap();
    drop(outside);
    write_rows(&rig.db, 40, 5);
    rig.pass().await;
    rig.pass().await;
    let (gen2, _) = rig.position();
    assert_ne!(gen2, gen1, "the run was continued across a checkpoint the backup did not make");
    let (got, _) = rig.restore_digest(Target::Latest).await.unwrap();
    assert_eq!(got, rig.live_digest());
}

/// After a clean shutdown the next start continues the same generation (no new full copy),
/// and after a crash (the log still on disk) it continues from the log.
///
/// Seen red 2026-10-22 with the clean-shutdown marker ignored: "a clean shutdown was not continued /
/// left: Some(\"the relay stopped without a clean shutdown and its log no longer reaches the
/// last copy\") / right: None".
#[tokio::test]
async fn a_restart_continues_the_same_generation() {
    let dir = crate::test_temp::dir("bs_restart");
    let dest = dir.join("dest");
    let data = dir.join("data");
    let (gen, digest) = {
        let mut rig = Rig::in_dir(crate::test_temp::dir("bs_restart_a"), dest.clone());
        rig.pass().await;
        write_rows(&rig.db, 0, 30);
        rig.pass().await;
        // A crash: copy the database, its log and the shipper's files as they are this moment.
        std::fs::create_dir_all(&data).unwrap();
        for f in ["relay.db", "relay.db-wal", super::CONFIG_FILE, "backup.key"] {
            std::fs::copy(rig.data.join(f), data.join(f)).unwrap();
        }
        copy_dir(&work_dir(&rig.data.join("relay.db")), &work_dir(&data.join("relay.db")));
        (rig.position().0, rig.live_digest())
    };
    // The crashed copy starts: same generation, picked up from the log.
    let mut rig = Rig { db: Arc::new(Storage::open(&data.join("relay.db")).unwrap()), rs: RunState::default(), data: data.clone(), dest: dest.clone(), _dir: dir };
    rig.db.backup_stream_install().unwrap();
    assert_eq!(rig.db.stream.0.cap.lock().unwrap().as_ref().unwrap().needs_new_gen, None, "the crash could not be continued");
    assert_eq!(rig.live_digest(), digest);
    write_rows(&rig.db, 30, 10);
    rig.pass().await;
    assert_eq!(rig.position().0, gen);
    let (got, _) = rig.restore_digest(Target::Latest).await.unwrap();
    assert_eq!(got, rig.live_digest());

    // A clean shutdown, then a start: same generation again.
    rig.db.backup_stream_close();
    let digest = rig.live_digest();
    let Rig { db, data, dest, _dir, .. } = rig;
    drop(db);
    let mut rig = Rig { db: Arc::new(Storage::open(&data.join("relay.db")).unwrap()), rs: RunState::default(), data, dest, _dir };
    rig.db.backup_stream_install().unwrap();
    assert_eq!(rig.db.stream.0.cap.lock().unwrap().as_ref().unwrap().needs_new_gen, None, "a clean shutdown was not continued");
    assert_eq!(rig.live_digest(), digest);
    write_rows(&rig.db, 50, 10);
    rig.pass().await;
    assert_eq!(rig.position().0, gen, "a clean restart sent a new full copy");
    let (got, _) = rig.restore_digest(Target::Latest).await.unwrap();
    assert_eq!(got, rig.live_digest());
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

/// Lag is reported from what the destination can restore: while the destination is down it
/// grows, past the threshold the state is "behind" and ONE alert goes out (with no folder,
/// bucket or address in it), and once the destination is back the lag is zero, the state is up
/// to date and one "caught up" follows.
///
/// Seen red 2026-10-22 with an alert sent on every check while behind: "one alert per episode: [two
/// warn alerts] / left: 2 / right: 1".
#[tokio::test]
async fn lag_is_reported_and_alerts_fire_once_each_way() {
    let mut rig = Rig::new("bs_lag");
    let alerts = Arc::new(std::sync::Mutex::new(Vec::<(String, String)>::new()));
    let a2 = alerts.clone();
    *rig.db.stream.0.alert_sink.lock().unwrap() = Some(Box::new(move |m: &str, s: &str| a2.lock().unwrap().push((m.to_string(), s.to_string()))));
    let down = Arc::new(AtomicBool::new(false));
    rig.rs.version = rig.db.stream.0.ctl.lock().unwrap().version;
    rig.rs.dest = Some(Arc::new(Destination::Flaky(FolderDest::new(rig.dest.clone()), down.clone())));
    rig.pass().await;
    write_rows(&rig.db, 0, 10);
    rig.pass().await;
    let st = rig.db.backup_stream_status();
    assert_eq!((st.state.as_str(), st.lag_secs), ("up_to_date", 0), "{}", st.words);

    down.store(true, Ordering::SeqCst);
    write_rows(&rig.db, 10, 10);
    rig.pass().await;
    let st = rig.db.backup_stream_status();
    assert_eq!(st.state, "failing", "{}", st.words);
    assert!(st.pending_objects > 0);
    // Ten minutes later, still down.
    let cfg = rig.db.stream.config();
    let later = capture::now_ms() + 10 * 60 * 1000;
    refresh_status(&rig.db, &rig.db.stream, &cfg, later, later);
    let st = rig.db.backup_stream_status();
    assert_eq!(st.state, "behind", "{}", st.words);
    assert!(st.lag_secs >= 600, "lag {}", st.lag_secs);
    alert_check(&rig.db.stream, &cfg);
    alert_check(&rig.db.stream, &cfg);
    {
        let a = alerts.lock().unwrap();
        assert_eq!(a.len(), 1, "one alert per episode: {a:?}");
        assert_eq!(a[0].1, "warn");
        assert!(!a[0].0.contains(&rig.dest.to_string_lossy().to_string()), "the alert names the folder: {}", a[0].0);
    }

    down.store(false, Ordering::SeqCst);
    rig.rs.retry_at = 0;
    rig.pass().await;
    let st = rig.db.backup_stream_status();
    assert_eq!((st.state.as_str(), st.lag_secs, st.pending_objects), ("up_to_date", 0, 0), "{}", st.words);
    let a = alerts.lock().unwrap();
    assert_eq!(a.len(), 2, "{a:?}");
    assert_eq!(a[1].1, "info");
    assert!(a[1].0.contains("caught up"));
}

/// Retention at a real destination: after the window has passed, older snapshots, the segments
/// they made unnecessary and whole older generations are deleted, while the newest state and
/// the window's base still restore.
///
/// Seen red 2026-10-22 with retention deleting nothing: "the older full copy was kept / left: 2 / right: 1".
#[tokio::test]
async fn retention_prunes_a_real_destination_and_keeps_it_restorable() {
    let mut rig = Rig::new("bs_retain");
    rig.pass().await;
    write_rows(&rig.db, 0, 20);
    rig.pass().await;
    // A second full copy in the same generation, then more changes.
    rig.db.stream.0.ctl.lock().unwrap().force_snapshot = true;
    rig.pass().await;
    write_rows(&rig.db, 20, 20);
    rig.pass().await;
    let before = Catalog::from_listing(&rig.folder().list().await.unwrap());
    assert_eq!(before.complete_snaps().len(), 2);
    // Three days on, with the default 72 h window plus an hour.
    let cfg = rig.db.stream.config();
    let future = capture::now_ms() + 73 * 3_600_000;
    *rig.db.stream.0.known.lock().unwrap() = Some(rig.folder().list().await.unwrap());
    prune(&rig.folder(), &rig.db.stream, &cfg, future).await.unwrap();
    let after = Catalog::from_listing(&rig.folder().list().await.unwrap());
    assert_eq!(after.complete_snaps().len(), 1, "the older full copy was kept");
    let first_seq = after.gens.values().next().unwrap().segs.keys().next().copied().unwrap();
    assert!(first_seq > after.complete_snaps()[0].seq, "segments before the kept full copy were kept");
    assert!(after.owner, "the owner mark was deleted");
    let (got, _) = rig.restore_digest(Target::Latest).await.unwrap();
    assert_eq!(got, rig.live_digest());
}

/// A destination already holding another server's backups is refused before anything is
/// written there, both by the test button and by turning backup on.
///
/// Seen red 2026-10-22 with the test button not reading the destination's owner mark: `tested.is_err()`
/// failed (the other server's destination tested as fine).
#[tokio::test]
async fn a_destination_holding_another_servers_backups_is_refused() {
    let mut a = Rig::new("bs_owner_a");
    a.pass().await;
    assert!(a.dest.join(names::OWNER).exists());
    let b = Rig::new("bs_owner_b");
    let cfg = DestinationConfig::Folder { path: a.dest.to_string_lossy().to_string() };
    let tested = test_destination(&b.db, &cfg, None).await;
    assert!(tested.is_err(), "{tested:?}");
    let before = a.folder().list().await.unwrap().len();
    let set = apply_settings(&b.db, SetRequest { enabled: true, destination: Some(cfg), retention_hours: 72, snapshot_every_hours: 24, alert_after_minutes: 5 }).await;
    assert!(set.is_err(), "{set:?}");
    assert_eq!(a.folder().list().await.unwrap().len(), before, "something was written into the other server's backups");
}

/// The operator's setup: one server keeps another's backups. The sending server signs every
/// request with its backup sending key; the keeping server accepts only listed keys, only
/// fresh unreplayed signatures over exactly that request, only within the space it gives, and
/// stores ciphertext it cannot open. The sender's database restores from it exactly.
///
/// Seen red 2026-10-22 with the keeping server not checking for a reused request: "a replay was
/// accepted / left: 204 / right: 401".
#[tokio::test]
async fn another_humanityos_server_keeps_the_backups_and_lets_only_the_sender_in() {
    // The keeping server, serving on loopback.
    let keeper = Storage::open_temp_dir("bs_keeper");
    let keeper_data = live_data_dir(&keeper);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let state = crate::relay::serve_relay(keeper, listener).await;

    // The sending server, with the keeping server as its destination.
    let dir = crate::test_temp::dir("bs_sender");
    let data = dir.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let mut cfg = config_with_sender(&data);
    cfg.enabled = true;
    cfg.destination = Some(DestinationConfig::Server { url: url.clone() });
    save_config(&data, &cfg).unwrap();
    let sending_key = cfg.sending_key().unwrap();

    // Not listed yet: refused.
    let dest = Destination::from_config(cfg.destination.as_ref().unwrap(), cfg.sender_seed_bytes().as_ref()).unwrap();
    let e = dest.put("probe-1", vec![1, 2, 3]).await.unwrap_err();
    assert!(e.contains("does not keep backups for this server"), "{e}");

    host::allow(&keeper_data, "united-humanity.us", &sending_key, 50_000_000, capture::now_ms()).unwrap();
    let mut rig = Rig { db: Arc::new(Storage::open(&data.join("relay.db")).unwrap()), rs: RunState::default(), data: data.clone(), dest: PathBuf::new(), _dir: dir };
    rig.db.backup_stream_install().unwrap();
    rig.pass().await;
    write_rows(&rig.db, 0, 25);
    rig.pass().await;
    let st = rig.db.backup_stream_status();
    assert_eq!(st.state, "up_to_date", "{} / {}", st.words, st.last_error);

    // What the keeper holds is ciphertext, in the sender's own folder.
    let fp = peer::fingerprint(&hex::decode(&sending_key).unwrap());
    let kept = host::store_dir(&keeper_data, &fp);
    let mut n = 0;
    for l in FolderDest::new(kept.clone()).list().await.unwrap() {
        let b = std::fs::read(l.name.split('/').fold(kept.clone(), |p, c| p.join(c))).unwrap();
        assert!(!b.windows(17).any(|w| w == b"SECRET-ROW-MARKER"));
        n += 1;
    }
    assert!(n > 3);
    let out = data.join("from-keeper.db");
    restore(&dest, &rig.key(), &out, Target::Latest).await.unwrap();
    let c = rusqlite::Connection::open(&out).unwrap();
    assert_eq!(logical_digest(&c).unwrap(), rig.live_digest(), "the restore from the other server is not the live database");
    drop(c);
    restore::remove_db_files(&out);

    // Another server's key, a replayed request, a changed body, a name that climbs out, and a
    // request past the space given: all refused.
    let stranger = Destination::from_config(&DestinationConfig::Server { url: url.clone() }, Some(&[7u8; 32])).unwrap();
    assert!(stranger.list().await.is_err());
    let client = reqwest::Client::new();
    let ms = capture::now_ms();
    let kp = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&crate::relay::core::pq_crypto::derive_dilithium_seed(&cfg.sender_seed_bytes().unwrap()));
    let sig = hex::encode(kp.sign(peer::preimage("put", "replayed", b"abc", ms).as_bytes()));
    let send = |body: &'static [u8], name: &str| {
        client.put(format!("{url}/api/backup-host/o/{name}")).header(peer::H_FROM, &fp).header(peer::H_TIME, ms.to_string()).header(peer::H_SIG, &sig).body(body).send()
    };
    assert_eq!(send(b"abc", "replayed").await.unwrap().status().as_u16(), 204);
    assert_eq!(send(b"abc", "replayed").await.unwrap().status().as_u16(), 401, "a replay was accepted");
    assert_eq!(send(b"abd", "replayed").await.unwrap().status().as_u16(), 401, "a changed body was accepted");
    assert_eq!(send(b"abc", "..%2Fescape").await.unwrap().status().as_u16(), 400, "a name outside the folder was accepted");
    host::allow(&keeper_data, "united-humanity.us", &sending_key, 1_000, capture::now_ms()).unwrap();
    let e = dest.put("big", vec![0u8; 5_000]).await.unwrap_err();
    assert!(e.contains("no more room"), "{e}");
    drop(state);
}

/// A 413 IS NOT A FULL SERVER (2026-10-25): nginx's 1 MB default in front of both servers turned
/// every piece away with 413, and the Backups panel said "no more room", which sent the operator to
/// raise a space limit that was never the problem. 413 now names the piece size and the web server's
/// limit; 507 alone means out of room. Seen red against the old wording, which sent 413 and 507 to
/// one "no more room" line: "a 413 was called a full server".
#[test]
fn a_413_is_a_piece_too_big_and_only_a_507_is_a_full_server() {
    let too_big = super::peer::refusal_words(413, "");
    assert!(!too_big.contains("no more room"), "a 413 was called a full server: {too_big}");
    assert!(too_big.contains("too big") && too_big.contains("client_max_body_size"), "{too_big}");
    let full = super::peer::refusal_words(507, "Over the limit this server keeps for you.");
    assert!(full.contains("no more room") && full.contains("Over the limit"), "{full}");
}
