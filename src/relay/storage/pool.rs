//! Read-connection pool for the relay's SQLite store (R3 concurrency work).
//!
//! ## Why this exists
//!
//! `Storage` historically held a SINGLE `Mutex<Connection>`. SQLite in WAL
//! mode (enabled in `Storage::open`) allows **many concurrent readers plus one
//! writer** — but a single shared connection behind a `Mutex` throws that away:
//! every read serializes against every other read AND against writes. On a busy
//! relay that turns independent `SELECT`s into a queue.
//!
//! This module adds a small pool of **read-only** connections (built on
//! `r2d2` + `r2d2_sqlite`, which wrap the same `rusqlite` the rest of the relay
//! already uses). Reads that opt into [`Storage::with_read_conn`] can then run
//! in parallel; the single dedicated writer (`Storage::with_conn` /
//! `with_conn_mut`, still a `Mutex<Connection>`) keeps writes correctly
//! serialized, exactly as WAL requires.
//!
//! ## CRITICAL correctness boundary (read this before using the pool)
//!
//! The pool connections are opened **READ-ONLY** (`SQLITE_OPEN_READ_ONLY`).
//! Any attempt to `INSERT`/`UPDATE`/`DELETE`/`CREATE`/etc. through a pooled
//! connection FAILS at the SQLite layer ("attempt to write a readonly
//! database"). That is deliberate: it makes a mis-routed write a loud, safe
//! error instead of a silent corruption, and it guarantees `last_insert_rowid()`
//! is never read off the wrong connection.
//!
//! Because the existing `with_conn` closures across the 30 storage modules are
//! KNOWN to perform writes (e.g. `dms::store_dm_e2ee` does `INSERT … ;
//! last_insert_rowid()`), `with_conn` and `with_conn_mut` MUST keep going to the
//! writer. Only a closure you are CERTAIN is read-only may use
//! `with_read_conn`. Erring toward the writer is always correct (just less
//! parallel); routing a write to the pool is a bug — and here, a caught one.
//!
//! ## Pragmas
//!
//! Every pooled connection is initialized with the SAME durability/consistency
//! settings as the writer:
//!   * `journal_mode = WAL`     — required so a reader sees a consistent
//!                                snapshot while the writer is mid-transaction.
//!   * `foreign_keys = ON`      — match the writer's referential-integrity view.
//!   * `busy_timeout = 5000`    — a reader waits (up to 5 s) for a transient
//!                                lock rather than erroring out with SQLITE_BUSY
//!                                the instant the writer holds the write lock.
//!   * `query_only = ON`        — belt-and-suspenders on top of READ_ONLY.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OpenFlags;
use std::path::Path;
use std::time::Duration;

/// Number of read connections kept in the pool.
///
/// WAL readers are cheap (each is just a file handle + page cache), and the
/// relay's read workload is bursty (a client connecting pulls history across
/// several tables at once). 8 lets a handful of clients' reads truly overlap
/// without holding open an unbounded number of SQLite handles. Tunable; kept in
/// the 4–8 range the R3 brief specified.
pub(crate) const READ_POOL_SIZE: u32 = 8;

/// How long `with_read_conn` waits to check a connection OUT of the pool before
/// giving up. This is the r2d2-level wait (all 8 connections busy), distinct
/// from the SQLite-level `busy_timeout` (lock contention on a checked-out conn).
const POOL_CHECKOUT_TIMEOUT: Duration = Duration::from_secs(5);

/// SQLite-level busy timeout applied to every pooled read connection (ms).
const READ_BUSY_TIMEOUT_MS: u32 = 5000;

/// The relay's read-only connection pool. `Clone` is cheap (it's an `Arc`
/// internally), but `Storage` just owns one.
pub(crate) type ReadPool = Pool<SqliteConnectionManager>;

/// Build the read-only connection pool for the database at `path`.
///
/// Call this AFTER the writer has created/migrated the schema, so a freshly
/// opened reader never observes a database with no tables. Each connection in
/// the pool is opened read-only and initialized with the WAL/foreign-keys/
/// busy-timeout pragmas described in the module docs.
///
/// Returns the same `rusqlite::Error` family the rest of `Storage::open` uses,
/// so the boot path can propagate it with `?` without a new error type.
pub(crate) fn build_read_pool(path: &Path) -> Result<ReadPool, rusqlite::Error> {
    // READ_ONLY: a pooled connection physically cannot write — a mis-routed
    // write becomes a clean error, never silent corruption.
    // NO_MUTEX: each connection is used by exactly one thread at a time (r2d2
    // hands it out exclusively), so SQLite's per-connection mutex is needless
    // overhead. Matches the single-thread-per-checkout invariant of r2d2.
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;

    let manager = SqliteConnectionManager::file(path)
        .with_flags(flags)
        .with_init(|conn| {
            // Same consistency settings as the writer. WAL is set on the
            // database file by the writer; a read-only connection cannot
            // *change* the journal mode, but issuing the PRAGMA is harmless and
            // documents intent. foreign_keys + busy_timeout are per-connection
            // and DO matter here.
            conn.busy_timeout(Duration::from_millis(READ_BUSY_TIMEOUT_MS as u64))?;
            conn.pragma_update(None, "foreign_keys", true)?;
            // query_only is a second guard: even a hypothetical writable handle
            // would refuse writes. Cheap, defensive.
            conn.pragma_update(None, "query_only", true)?;
            Ok(())
        });

    Pool::builder()
        .max_size(READ_POOL_SIZE)
        .connection_timeout(POOL_CHECKOUT_TIMEOUT)
        // Validate a connection on checkout (r2d2_sqlite's `is-valid` feature
        // runs a trivial `SELECT 1`). Cheap insurance against handing out a
        // connection whose underlying file handle went bad.
        .test_on_check_out(true)
        .build(manager)
        .map_err(|e| {
            // r2d2::Error isn't a rusqlite::Error; fold it into the SQLite
            // error channel the boot path already threads through `?`.
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                Some(format!("failed to build SQLite read pool: {e}")),
            )
        })
}

#[cfg(test)]
mod tests {
    //! These tests lock in the two correctness invariants the entire R3
    //! connection split rests on:
    //!   1. A read taken from the pool (`with_read_conn`) sees rows the writer
    //!      (`with_conn`) committed — i.e. the pool really points at the same
    //!      database file with a consistent WAL snapshot, not a stale/empty one.
    //!   2. A WRITE attempted through `with_read_conn` FAILS — the pooled
    //!      connections are read-only, so a future accidental mis-route is a
    //!      loud error, never silent corruption. This is the guard that makes
    //!      "erring toward the writer is always correct" enforceable.
    use crate::relay::storage::Storage;
    use std::path::PathBuf;

    fn tmp_db(tag: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("hum_pool_{tag}_{pid}_{nanos}.db"))
    }

    /// Invariant 1: data written on the writer is visible through the read pool.
    #[test]
    fn read_pool_sees_writer_committed_rows() {
        let db = Storage::open(&tmp_db("read_after_write")).expect("open");

        // Write via the writer path (this is how all current call sites work).
        let id = db
            .store_dm("alice", "Alice", "bob", "hello via writer", 1_700_000_000_000)
            .expect("writer insert");
        assert!(id > 0, "writer INSERT returns a rowid via last_insert_rowid()");

        // Read it back THROUGH THE READ POOL — proves the pooled connection
        // observes the writer's committed WAL frames against the same file.
        let content: String = db
            .with_read_conn(|conn| {
                conn.query_row(
                    "SELECT content FROM direct_messages WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get::<_, String>(0),
                )
            })
            .expect("read via pool");
        assert_eq!(content, "hello via writer");

        // A second concurrent-style checkout also works (pool hands out a
        // distinct connection; both are valid readers under WAL).
        let count: i64 = db
            .with_read_conn(|conn| {
                conn.query_row("SELECT COUNT(*) FROM direct_messages", [], |r| r.get(0))
            })
            .expect("second pool read");
        assert_eq!(count, 1);
    }

    /// Invariant 2: a write through the read pool is REJECTED by SQLite.
    /// This is the safety net behind the conservative routing rule — if anyone
    /// ever sends write SQL to `with_read_conn`, it fails loudly here instead of
    /// silently hitting a read replica.
    #[test]
    fn read_pool_rejects_writes() {
        let db = Storage::open(&tmp_db("reject_writes")).expect("open");

        let result: Result<usize, rusqlite::Error> = db.with_read_conn(|conn| {
            conn.execute(
                "INSERT INTO direct_messages (from_key, from_name, to_key, content, timestamp)
                 VALUES ('x','X','y','should not persist', 1)",
                [],
            )
        });
        assert!(
            result.is_err(),
            "writing through a read-only pooled connection must fail"
        );

        // And nothing was written — the writer-side view confirms the table is
        // still empty, so the rejected write had zero side effects.
        let count: i64 = db
            .with_conn(|c| c.query_row("SELECT COUNT(*) FROM direct_messages", [], |r| r.get(0)))
            .expect("count on writer");
        assert_eq!(count, 0, "rejected write must not have persisted any row");
    }
}
