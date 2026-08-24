use super::Storage;
use rand::Rng;
use rusqlite::{params, OptionalExtension};

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Storage {
    // ── Follow/Friend System ──

    /// Add a follow relationship. Returns true if newly created.
    pub fn add_follow(&self, follower_key: &str, followed_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let now = now_millis().to_string();
            let rows = conn.execute(
                "INSERT OR IGNORE INTO follows (follower_key, followed_key, created_at) VALUES (?1, ?2, ?3)",
                params![follower_key, followed_key, now],
            )?;
            Ok(rows > 0)
        })
    }

    /// Remove a follow relationship. Returns true if actually removed.
    pub fn remove_follow(&self, follower_key: &str, followed_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM follows WHERE follower_key = ?1 AND followed_key = ?2",
                params![follower_key, followed_key],
            )?;
            Ok(rows > 0)
        })
    }

    /// Get list of keys that `user_key` is following.
    pub fn get_following(&self, user_key: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT followed_key FROM follows WHERE follower_key = ?1")?;
            let keys: Vec<String> = stmt.query_map(params![user_key], |row| row.get(0))?
                .filter_map(|r| r.ok()).collect();
            Ok(keys)
        })
    }

    /// Get list of keys that follow `user_key`.
    pub fn get_followers(&self, user_key: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT follower_key FROM follows WHERE followed_key = ?1")?;
            let keys: Vec<String> = stmt.query_map(params![user_key], |row| row.get(0))?
                .filter_map(|r| r.ok()).collect();
            Ok(keys)
        })
    }

    /// Check if two users are mutual followers (friends).
    pub fn are_friends(&self, key_a: &str, key_b: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM follows WHERE
                 (follower_key = ?1 AND followed_key = ?2) OR
                 (follower_key = ?2 AND followed_key = ?1)",
                params![key_a, key_b],
                |row| row.get(0),
            )?;
            Ok(count >= 2)
        })
    }

    // ── Group System ──

    // (Legacy group storage removed 2026-08-23: plaintext groups died
    // with the P2P E2EE group cutover; tables dropped by migration.)

}

#[cfg(test)]
mod follow_friend_tests {
    //! The follow graph + the `are_friends` mutual-follow predicate. This is
    //! the EXACT gate `handle_dm` (src/relay/handlers/msg_handlers.rs) consults
    //! before letting a non-privileged user send a DM ("you must be friends to
    //! DM <name>"). Friendship == a follow edge in BOTH directions. The bug
    //! class this guards: a one-sided follow being mistaken for friendship
    //! (it must NOT be) — i.e. that the `>= 2` edge count can't be reached by
    //! a single user following the same person twice.
    use super::*;

    fn fresh_db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_social_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// A one-directional follow is NOT friendship; the reciprocal follow makes
    /// them friends; dropping either edge ends the friendship. This is the core
    /// state machine the DM gate relies on.
    #[test]
    fn mutual_follow_required_for_friendship() {
        let db = fresh_db();
        // No edges yet.
        assert!(!db.are_friends("alice", "bob").unwrap());

        // Alice follows Bob — one-sided, NOT friends (the security-relevant
        // case: Alice can't DM Bob just by following him).
        assert!(db.add_follow("alice", "bob").unwrap(), "new edge created");
        assert!(!db.are_friends("alice", "bob").unwrap(), "one-way follow is not friendship");

        // Bob follows back — now mutual, so friends (symmetric).
        assert!(db.add_follow("bob", "alice").unwrap());
        assert!(db.are_friends("alice", "bob").unwrap());
        assert!(db.are_friends("bob", "alice").unwrap(), "are_friends is order-independent");

        // Bob unfollows — friendship ends, but Alice's follow edge survives.
        assert!(db.remove_follow("bob", "alice").unwrap());
        assert!(!db.are_friends("alice", "bob").unwrap(), "removing one edge ends friendship");
        assert_eq!(db.get_following("alice").unwrap(), vec!["bob".to_string()]);
    }

    /// `add_follow` is idempotent (UNIQUE constraint): following the same
    /// person twice returns false the second time AND does not fabricate the
    /// 2-edge count `are_friends` looks for. Without this, a user could "befriend
    /// themselves into" a target by double-following — a DM-gate bypass.
    #[test]
    fn double_follow_does_not_forge_friendship() {
        let db = fresh_db();
        assert!(db.add_follow("alice", "bob").unwrap(), "first follow is new");
        assert!(!db.add_follow("alice", "bob").unwrap(), "duplicate follow is a no-op");
        // Only ONE real edge exists despite two add_follow calls.
        assert_eq!(db.get_following("alice").unwrap().len(), 1);
        assert!(!db.are_friends("alice", "bob").unwrap(), "a doubled one-way follow is still not friendship");
    }

    /// followers/following are direction-correct and don't leak across users.
    #[test]
    fn following_and_followers_are_directional() {
        let db = fresh_db();
        db.add_follow("alice", "bob").unwrap();
        db.add_follow("carol", "bob").unwrap();
        db.add_follow("bob", "alice").unwrap();

        // Bob is followed by alice + carol; Bob follows only alice.
        let mut followers = db.get_followers("bob").unwrap();
        followers.sort();
        assert_eq!(followers, vec!["alice".to_string(), "carol".to_string()]);
        assert_eq!(db.get_following("bob").unwrap(), vec!["alice".to_string()]);

        // Carol↔Bob is one-way → not friends; Alice↔Bob is mutual → friends.
        assert!(!db.are_friends("carol", "bob").unwrap());
        assert!(db.are_friends("alice", "bob").unwrap());
    }
}


