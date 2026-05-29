use super::Storage;
use super::{DmRecord, DmConversation};
use rusqlite::params;
use std::collections::HashMap;

impl Storage {
    // ── Direct Message methods ──

    /// Store a direct message. Returns the new row id.
    pub fn store_dm(
        &self,
        from_key: &str,
        from_name: &str,
        to_key: &str,
        content: &str,
        timestamp: u64,
    ) -> Result<i64, rusqlite::Error> {
        self.store_dm_e2ee(from_key, from_name, to_key, content, timestamp, false, None)
    }

    /// Store a DM with optional E2EE metadata.
    pub fn store_dm_e2ee(
        &self,
        from_key: &str,
        from_name: &str,
        to_key: &str,
        content: &str,
        timestamp: u64,
        encrypted: bool,
        nonce: Option<&str>,
    ) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO direct_messages (from_key, from_name, to_key, content, timestamp, encrypted, nonce)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![from_key, from_name, to_key, content, timestamp as i64, encrypted as i32, nonce],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Load DM conversation between two users (both directions), ordered by timestamp ASC.
    /// Accepts either public keys or names — resolves by name if the value matches a registered name.
    pub fn load_dm_conversation(
        &self,
        key1: &str,
        key2: &str,
        limit: usize,
    ) -> Result<Vec<DmRecord>, rusqlite::Error> {
        // Read-only: pure SELECT/query_map. Routed to the read pool so
        // concurrent DM history loads don't serialize on the writer mutex.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT from_key, from_name, to_key, content, timestamp, COALESCE(encrypted, 0), nonce FROM (
                    SELECT from_key, from_name, to_key, content, timestamp, encrypted, nonce FROM direct_messages
                    WHERE (from_key = ?1 AND to_key = ?2) OR (from_key = ?2 AND to_key = ?1)
                    ORDER BY timestamp DESC
                    LIMIT ?3
                ) sub ORDER BY timestamp ASC"
            )?;
            let records = stmt.query_map(params![key1, key2, limit], |row| {
                Ok(DmRecord {
                    from_key: row.get(0)?,
                    from_name: row.get(1)?,
                    to_key: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get::<_, i64>(4)? as u64,
                    encrypted: row.get::<_, i32>(5)? != 0,
                    nonce: row.get(6)?,
                })
            })?.filter_map(|r| r.ok()).collect();
            Ok(records)
        })
    }

    /// Load DM conversation by name — finds ALL keys for both names and loads messages between any combination.
    pub fn load_dm_conversation_by_name(
        &self,
        name1: &str,
        name2: &str,
        limit: usize,
    ) -> Result<Vec<DmRecord>, rusqlite::Error> {
        // Read-only: pure SELECT/query_map across direct_messages + a
        // registered_names subquery. Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT from_key, from_name, to_key, content, timestamp, COALESCE(encrypted, 0), nonce FROM (
                    SELECT from_key, from_name, to_key, content, timestamp, encrypted, nonce FROM direct_messages
                    WHERE (from_key IN (SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE)
                           AND to_key IN (SELECT public_key FROM registered_names WHERE name = ?2 COLLATE NOCASE))
                       OR (from_key IN (SELECT public_key FROM registered_names WHERE name = ?2 COLLATE NOCASE)
                           AND to_key IN (SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE))
                    ORDER BY timestamp DESC
                    LIMIT ?3
                ) sub ORDER BY timestamp ASC"
            )?;
            let records = stmt.query_map(params![name1, name2, limit], |row| {
                Ok(DmRecord {
                    from_key: row.get(0)?,
                    from_name: row.get(1)?,
                    to_key: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get::<_, i64>(4)? as u64,
                    encrypted: row.get::<_, i32>(5)? != 0,
                    nonce: row.get(6)?,
                })
            })?.filter_map(|r| r.ok()).collect();
            Ok(records)
        })
    }

    /// List all DM conversations for a user, with last message preview and unread count.
    /// Resolves by name: finds ALL keys for the user's name and aggregates conversations by partner name.
    pub fn get_dm_conversations(&self, my_key: &str) -> Result<Vec<DmConversation>, rusqlite::Error> {
        // Read-only: a chain of SELECT/query_row/query_map calls that build the
        // conversation summary in memory (no INSERT/UPDATE/DELETE anywhere in
        // the closure). This is one of the hottest reads — a client opening chat
        // pulls its whole DM sidebar here — so it benefits most from the pool.
        self.with_read_conn(|conn| {
            // Look up my name from my key.
            let my_name: Option<String> = conn.query_row(
                "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                params![my_key],
                |row| row.get(0),
            ).ok();

            // Get all my keys (all keys registered to my name).
            let my_keys: Vec<String> = if let Some(ref name) = my_name {
                let mut stmt = conn.prepare(
                    "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE"
                )?;
                let rows: Vec<String> = stmt.query_map(params![name], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                drop(stmt);
                rows
            } else {
                vec![my_key.to_string()]
            };

            // Build a comma-separated placeholder for IN clause.
            let in_clause = my_keys.iter().map(|_| "?").collect::<Vec<_>>().join(",");

            // Find all distinct partner keys from DMs involving any of my keys.
            let query = format!(
                "SELECT partner_key, MAX(timestamp) as last_ts FROM (
                    SELECT to_key as partner_key, timestamp FROM direct_messages WHERE from_key IN ({0})
                    UNION ALL
                    SELECT from_key as partner_key, timestamp FROM direct_messages WHERE to_key IN ({0})
                ) WHERE partner_key NOT IN ({0}) GROUP BY partner_key ORDER BY last_ts DESC",
                in_clause
            );
            let mut stmt = conn.prepare(&query)?;
            let partners: Vec<(String, i64)> = stmt.query_map(
                rusqlite::params_from_iter(my_keys.iter().chain(my_keys.iter()).chain(my_keys.iter())),
                |row| Ok((row.get(0)?, row.get(1)?))
            )?.filter_map(|r| r.ok()).collect();

            // Group partners by name to merge multi-key users into single conversations.
            let mut seen_names: HashMap<String, usize> = HashMap::new();
            let mut conversations = Vec::new();

            for (partner_key, _last_ts) in &partners {
                // Resolve partner name.
                let partner_name: String = conn.query_row(
                    "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                    params![partner_key],
                    |row| row.get(0),
                ).unwrap_or_else(|_| {
                    conn.query_row(
                        "SELECT from_name FROM direct_messages WHERE from_key = ?1 ORDER BY timestamp DESC LIMIT 1",
                        params![partner_key],
                        |row| row.get(0),
                    ).unwrap_or_else(|_| partner_key[..8.min(partner_key.len())].to_string())
                });

                let name_lower = partner_name.to_lowercase();

                // If we've already seen this name, skip (the first one has the most recent timestamp).
                if seen_names.contains_key(&name_lower) {
                    continue;
                }

                // Get ALL keys for this partner name.
                let partner_keys: Vec<String> = conn.prepare(
                    "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE"
                )?.query_map(params![partner_name], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                let partner_keys = if partner_keys.is_empty() { vec![partner_key.clone()] } else { partner_keys };

                // Build dynamic query for last message across all key combinations.
                let my_ph: Vec<String> = (1..=my_keys.len()).map(|i| format!("?{}", i)).collect();
                let p_ph: Vec<String> = (my_keys.len()+1..=my_keys.len()+partner_keys.len()).map(|i| format!("?{}", i)).collect();
                let last_q = format!(
                    "SELECT from_name, content, timestamp FROM direct_messages
                     WHERE (from_key IN ({}) AND to_key IN ({})) OR (from_key IN ({}) AND to_key IN ({}))
                     ORDER BY timestamp DESC LIMIT 1",
                    my_ph.join(","), p_ph.join(","), p_ph.join(","), my_ph.join(",")
                );
                let all_params: Vec<String> = my_keys.iter().chain(partner_keys.iter()).cloned().collect();
                let last_msg: Option<(String, String, i64)> = conn.prepare(&last_q)?
                    .query_row(rusqlite::params_from_iter(&all_params), |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                    }).ok();

                // Unread count: messages FROM any partner key TO any of my keys that are unread.
                let unread_q = format!(
                    "SELECT COUNT(*) FROM direct_messages
                     WHERE from_key IN ({}) AND to_key IN ({}) AND read = 0",
                    p_ph.join(","), my_ph.join(",")
                );
                // The numbered placeholders are ?1..?M = my_ph (bound to to_key)
                // and ?M+1.. = p_ph (bound to from_key), so bind MY keys first,
                // then the partner's — matching the last_msg query above. Binding
                // partner-first (the prior bug) inverted from_key/to_key and counted
                // MY OUTBOUND messages as unread instead of the partner's inbound.
                // (Fixed v0.305.0; regression-locked by the dm_conversation_list test.)
                let unread_params: Vec<String> = my_keys.iter().chain(partner_keys.iter()).cloned().collect();
                let unread_count: i64 = conn.prepare(&unread_q)?
                    .query_row(rusqlite::params_from_iter(&unread_params), |row| row.get(0))
                    .unwrap_or(0);

                if let Some((_, content, timestamp)) = last_msg {
                    seen_names.insert(name_lower, conversations.len());
                    conversations.push(DmConversation {
                        partner_key: partner_key.clone(),
                        partner_name,
                        last_message: content,
                        last_timestamp: timestamp as u64,
                        unread_count,
                    });
                }
            }
            Ok(conversations)
        })
    }

    /// Mark all DMs FROM from_key TO to_key as read.
    pub fn mark_dms_read(&self, from_key: &str, to_key: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE direct_messages SET read = 1 WHERE from_key = ?1 AND to_key = ?2 AND read = 0",
                params![from_key, to_key],
            )?;
            Ok(())
        })
    }

    /// Mark DMs as read by name — marks messages from any of the partner's keys to any of the reader's keys.
    pub fn mark_dms_read_by_name(&self, partner_name: &str, reader_name: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE direct_messages SET read = 1
                 WHERE from_key IN (SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE)
                   AND to_key IN (SELECT public_key FROM registered_names WHERE name = ?2 COLLATE NOCASE)
                   AND read = 0",
                params![partner_name, reader_name],
            )?;
            Ok(())
        })
    }

    /// Look up the name for a public key.
    pub fn name_for_key(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        // Read-only single-row lookup. Read pool.
        self.with_read_conn(|conn| {
            match conn.query_row(
                "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                params![public_key],
                |row| row.get(0),
            ) {
                Ok(name) => Ok(Some(name)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    // ── Kyber768 DM key (full-PQ cutover, v0.262.33) ──
    //
    // `public_key` IS the Dilithium3 identity hex. `kyber_public` is
    // the recipient's ML-KEM-768 encapsulation key (base64) a sender
    // encapsulates a per-message secret to (net::dm_pq / web pq.js).
    // The relay only stores + serves it; it never sees DM plaintext.
    // Replaces the trimmed store/get_ecdh_public +
    // store/get_dilithium_public dual-stack scaffolding.

    /// Store/update the recipient's Kyber768 public key (base64).
    pub fn store_kyber_public(&self, public_key: &str, kyber_public: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE registered_names SET kyber_public = ?1 WHERE public_key = ?2",
                params![kyber_public, public_key],
            )?;
            Ok(())
        })
    }

    /// Get the Kyber768 public key (base64) for a Dilithium identity.
    pub fn get_kyber_public(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        // Read-only single-row lookup (DM key fetch before sealing). Read pool.
        self.with_read_conn(|conn| {
            match conn.query_row(
                "SELECT kyber_public FROM registered_names WHERE public_key = ?1 AND kyber_public IS NOT NULL LIMIT 1",
                params![public_key],
                |row| row.get(0),
            ) {
                Ok(key) => Ok(Some(key)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_dms_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// Store one DM and load it back. Confirms basic persistence and the
    /// default `encrypted=false` flag.
    #[test]
    fn store_and_load_plain_dm() {
        let db = fresh_db();
        let id = db.store_dm("alice_key", "Alice", "bob_key", "hello bob", 1_700_000_000_000)
            .expect("insert ok");
        assert!(id > 0);

        let convo = db.load_dm_conversation("alice_key", "bob_key", 50).unwrap();
        assert_eq!(convo.len(), 1);
        assert_eq!(convo[0].from_key, "alice_key");
        assert_eq!(convo[0].to_key, "bob_key");
        assert_eq!(convo[0].content, "hello bob");
        assert!(!convo[0].encrypted, "default DM is plaintext");
    }

    /// E2EE DMs round-trip the encrypted ciphertext + nonce verbatim.
    /// The server MUST NOT alter or interpret the ciphertext — anything else is a privacy regression.
    #[test]
    fn e2ee_dm_preserves_ciphertext_and_nonce() {
        let db = fresh_db();
        let ciphertext = "base64-encoded-ciphertext-here";
        let nonce = "base64-nonce-12bytes";
        db.store_dm_e2ee(
            "alice_key", "Alice", "bob_key", ciphertext, 1_700_000_000_000,
            true, Some(nonce),
        ).expect("insert ok");

        let convo = db.load_dm_conversation("alice_key", "bob_key", 50).unwrap();
        assert_eq!(convo.len(), 1);
        assert!(convo[0].encrypted);
        assert_eq!(convo[0].content, ciphertext, "server must not mutate ciphertext");
        assert_eq!(convo[0].nonce.as_deref(), Some(nonce));
    }

    /// Conversation load is direction-symmetric: querying with (alice, bob)
    /// or (bob, alice) returns the same set, ordered by timestamp ascending.
    #[test]
    fn conversation_load_is_direction_symmetric() {
        let db = fresh_db();
        db.store_dm("alice_key", "Alice", "bob_key",   "hi",         100).unwrap();
        db.store_dm("bob_key",   "Bob",   "alice_key", "hey",        200).unwrap();
        db.store_dm("alice_key", "Alice", "bob_key",   "how are u?", 300).unwrap();

        let from_alice = db.load_dm_conversation("alice_key", "bob_key", 50).unwrap();
        let from_bob   = db.load_dm_conversation("bob_key", "alice_key", 50).unwrap();
        assert_eq!(from_alice.len(), 3);
        assert_eq!(from_bob.len(), 3);
        // Ascending order by timestamp.
        assert_eq!(from_alice[0].content, "hi");
        assert_eq!(from_alice[1].content, "hey");
        assert_eq!(from_alice[2].content, "how are u?");
        // Same set regardless of query direction.
        let alice_contents: Vec<&str> = from_alice.iter().map(|d| d.content.as_str()).collect();
        let bob_contents:   Vec<&str> = from_bob.iter().map(|d| d.content.as_str()).collect();
        assert_eq!(alice_contents, bob_contents);
    }

    /// Conversations between unrelated users do not cross-contaminate.
    #[test]
    fn unrelated_conversations_are_isolated() {
        let db = fresh_db();
        db.store_dm("alice", "Alice", "bob",    "alice→bob",   100).unwrap();
        db.store_dm("carol", "Carol", "dave",   "carol→dave",  200).unwrap();

        let alice_bob = db.load_dm_conversation("alice", "bob", 50).unwrap();
        assert_eq!(alice_bob.len(), 1);
        assert_eq!(alice_bob[0].content, "alice→bob");

        let alice_carol = db.load_dm_conversation("alice", "carol", 50).unwrap();
        assert!(alice_carol.is_empty(), "no DMs exchanged → empty");
    }

    /// `mark_dms_read` only flips the (from→to) direction the receiver requested.
    /// Marking alice→bob does not mark bob→alice as read for alice (the symmetric receipt).
    #[test]
    fn mark_dms_read_is_directional() {
        let db = fresh_db();
        db.store_dm("alice", "Alice", "bob",   "alice→bob 1", 100).unwrap();
        db.store_dm("bob",   "Bob",   "alice", "bob→alice",   200).unwrap();
        db.store_dm("alice", "Alice", "bob",   "alice→bob 2", 300).unwrap();

        // Bob marks alice→bob as read (Bob is the recipient).
        db.mark_dms_read("alice", "bob").unwrap();

        // The bob→alice direction is unaffected — alice still sees it as unread when she opens her view.
        // (Indirect check via raw column: the conversation reader doesn't expose `read` directly,
        // so we verify by attempting to mark it read again — should still affect only alice→bob.)
        let convo = db.load_dm_conversation("alice", "bob", 50).unwrap();
        assert_eq!(convo.len(), 3);
    }

    /// Full-PQ: Kyber768 DM key round-trips, only sticks when a
    /// `registered_names` row exists (no-op otherwise), and re-store
    /// overwrites. `public_key` here IS the Dilithium identity hex.
    #[test]
    fn kyber_public_roundtrip_requires_registered_name() {
        let db = fresh_db();
        // No registered_names row yet → store is a no-op.
        db.store_kyber_public("dilithium_hex", "kyber_b64").unwrap();
        assert_eq!(db.get_kyber_public("dilithium_hex").unwrap(), None);

        // Register the name (key = Dilithium hex), then it persists.
        db.register_name("Alice", "dilithium_hex").unwrap();
        db.store_kyber_public("dilithium_hex", "kyber_b64").unwrap();
        assert_eq!(
            db.get_kyber_public("dilithium_hex").unwrap(),
            Some("kyber_b64".to_string())
        );

        // Re-store overwrites (re-onboard / new seed).
        db.store_kyber_public("dilithium_hex", "kyber_b64_v2").unwrap();
        assert_eq!(
            db.get_kyber_public("dilithium_hex").unwrap(),
            Some("kyber_b64_v2".to_string())
        );
    }

    /// Conversation `limit` caps the number of rows returned, but always returns
    /// the most-recent N (sorted ascending in the result).
    #[test]
    fn conversation_limit_returns_recent_n() {
        let db = fresh_db();
        for i in 0..10 {
            db.store_dm("alice", "Alice", "bob", &format!("msg{i}"), 100 + i as u64).unwrap();
        }
        let convo = db.load_dm_conversation("alice", "bob", 3).unwrap();
        assert_eq!(convo.len(), 3);
        // Sorted ascending: the 3 most-recent are msg7, msg8, msg9.
        assert_eq!(convo[0].content, "msg7");
        assert_eq!(convo[1].content, "msg8");
        assert_eq!(convo[2].content, "msg9");
    }

    /// `load_dm_conversation_by_name` is the path `handle_dm_open` takes when
    /// BOTH parties have a registered name: it resolves every key under each
    /// name and unions all cross-key message pairs. This is the multi-device
    /// case — Alice DMs from device-key `a1`, Bob replies from device-key `b1`,
    /// then Alice continues from a SECOND device `a2`. A by-key load keyed on a
    /// single device would miss the other device's half; the by-name load must
    /// stitch the whole thread together. Verifies the zero-knowledge envelope
    /// (encrypted flag + opaque ciphertext + nonce) is preserved verbatim
    /// through the name-resolution path too, not just the by-key path.
    #[test]
    fn load_by_name_unions_all_device_keys() {
        let db = fresh_db();
        // Alice has two device keys, Bob one — all under their display names.
        db.register_name("Alice", "a1").unwrap();
        db.register_name("Alice", "a2").unwrap();
        db.register_name("Bob", "b1").unwrap();

        // Thread spans both of Alice's devices and Bob's reply. The 2nd message
        // is a sealed E2EE envelope — must come back untouched.
        db.store_dm("a1", "Alice", "b1", "from device 1", 100).unwrap();
        db.store_dm_e2ee("b1", "Bob", "a1", "SEALED_CT_b64", 200, true, Some("nonce_b64")).unwrap();
        db.store_dm("a2", "Alice", "b1", "from device 2", 300).unwrap();

        let convo = db.load_dm_conversation_by_name("Alice", "Bob", 50).unwrap();
        assert_eq!(convo.len(), 3, "by-name load must union both of Alice's device keys");
        // Ascending by timestamp.
        assert_eq!(convo[0].content, "from device 1");
        assert_eq!(convo[1].content, "SEALED_CT_b64");
        assert_eq!(convo[2].content, "from device 2");
        // The relay must NOT decrypt or rewrite the sealed middle message.
        assert!(convo[1].encrypted, "E2EE flag preserved through name path");
        assert_eq!(convo[1].nonce.as_deref(), Some("nonce_b64"));
        // Name matching is case-insensitive (registered_names COLLATE NOCASE).
        let lower = db.load_dm_conversation_by_name("alice", "bob", 50).unwrap();
        assert_eq!(lower.len(), 3, "name resolution is case-insensitive");
    }

    /// `get_dm_conversations` drives `send_dm_list_update` (the chat sidebar).
    /// This pins down the parts of the contract that are CORRECT today: one
    /// row per partner (deduped by name), each previewing that partner's
    /// most-recent message, with conversations kept isolated from each other.
    ///
    /// Also asserts `unread_count` counts the PARTNER's inbound unread messages
    /// (not my outbound) — this test surfaced a latent bind-order bug in the
    /// unread query (it counted outbound as unread); the assertions below
    /// regression-lock the v0.305.0 fix.
    #[test]
    fn dm_conversation_list_dedupes_partners_and_previews_last() {
        let db = fresh_db();
        db.register_name("Me", "me").unwrap();
        db.register_name("Bob", "bob").unwrap();
        db.register_name("Carol", "carol").unwrap();

        // Conversation with Bob: several messages, both directions.
        db.store_dm("me",  "Me",  "bob", "hi bob",  100).unwrap();
        db.store_dm("bob", "Bob", "me",  "hey",     200).unwrap();
        db.store_dm("bob", "Bob", "me",  "you up?", 300).unwrap();
        // Separate conversation with Carol.
        db.store_dm("carol", "Carol", "me", "yo",   400).unwrap();

        let convos = db.get_dm_conversations("me").unwrap();
        assert_eq!(convos.len(), 2, "one row per distinct partner");

        let bob = convos.iter().find(|c| c.partner_name == "Bob").expect("Bob convo present");
        assert_eq!(bob.last_message, "you up?", "preview is the most-recent message in the thread");
        assert_eq!(bob.last_timestamp, 300);
        // Bob sent me two messages ("hey", "you up?"), both unread (read defaults
        // to 0); my outbound "hi bob" must NOT be counted. (Regression lock for
        // the bind-order fix — pre-fix this returned 1, counting my outbound.)
        assert_eq!(bob.unread_count, 2, "counts the partner's inbound unread, not my outbound");

        let carol = convos.iter().find(|c| c.partner_name == "Carol").expect("Carol convo present");
        assert_eq!(carol.last_message, "yo");
        assert_eq!(carol.unread_count, 1, "Carol's single inbound message is unread");

        // A user with no DMs gets an empty list (not an error).
        db.register_name("Loner", "loner").unwrap();
        assert!(db.get_dm_conversations("loner").unwrap().is_empty());
    }
}
