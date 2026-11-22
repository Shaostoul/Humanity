//! The writer connection's lock (the owner branch's sixth review, L3, 2026-11-21).
//!
//! `Storage::with_conn` and `with_conn_mut` used to take the writer connection with
//! `lock().unwrap()`: a panic inside any closure (a bug in one handler's SQL glue, say) poisoned the
//! mutex, and every later write on the whole relay panicked in turn until it restarted. Now the lock
//! recovers: it takes the inner guard, rolls back a transaction the panicking closure left open (so
//! the next writer never commits half of it), logs it, and clears the poison.

use std::sync::MutexGuard;

use rusqlite::Connection;

use super::Storage;

impl Storage {
    /// The writer connection, locked; recovered if a closure panicked while holding it (this
    /// module's header).
    pub(crate) fn lock_conn(&self) -> MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let guard = poisoned.into_inner();
                if !guard.is_autocommit() {
                    match guard.execute_batch("ROLLBACK") {
                        Ok(()) => tracing::error!("Storage: a panic while the database was in use left a transaction open; it was rolled back"),
                        Err(e) => tracing::error!("Storage: a panic while the database was in use left a transaction open, and rolling it back failed: {e}"),
                    }
                } else {
                    tracing::error!("Storage: a panic while the database was in use; the connection is used again");
                }
                self.conn.clear_poison();
                guard
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::relay::storage::Storage;

    /// THE STORAGE LOCK RECOVERS AFTER A PANIC INSIDE THE CLOSURE (the sixth review's L3): a bug that
    /// panics inside `with_conn_mut` in the middle of a transaction it opened by hand leaves the
    /// connection usable: the next read and write work, and what the panicking closure wrote is
    /// rolled back, not committed by the next writer.
    ///
    /// Seen red 2026-11-21 (scratchpad ownfix9-red-relay.log) against `lock().unwrap()`: the next
    /// `with_conn` panicked "called `Result::unwrap()` on an `Err` value: PoisonError".
    #[test]
    fn the_storage_lock_recovers_after_a_panic_inside_the_closure() {
        let db = Storage::open_temp("conn_lock_panic");
        db.with_conn(|c| c.execute_batch("CREATE TABLE lock_test (n INTEGER)")).unwrap();
        let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            db.with_conn_mut(|c| {
                c.execute_batch("BEGIN; INSERT INTO lock_test (n) VALUES (1);").unwrap();
                panic!("a bug inside the closure, mid-transaction");
            })
        }));
        assert!(hit.is_err(), "the closure panicked");
        let n: i64 = db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM lock_test", [], |r| r.get(0))).unwrap();
        assert_eq!(n, 0, "what the panicking closure wrote was rolled back");
        db.with_conn_mut(|c| c.execute("INSERT INTO lock_test (n) VALUES (2)", [])).unwrap();
        let n: i64 = db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM lock_test WHERE n = 2", [], |r| r.get(0))).unwrap();
        assert_eq!(n, 1, "and the next write works");
        assert!(db.with_conn(|c| c.is_autocommit()), "no transaction is left open");
    }
}
