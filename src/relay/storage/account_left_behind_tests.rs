// Child of account.rs (#[path]). BUG-279 (2026-11-07): erasing an account left the person's data
// in a score of tables (pinned copies, task comments, the settings sync and computer details,
// signed objects and P2P groups, votes, bug reports and their votes, reputation, trades, guilds,
// streams, assets, skills, link and invite codes, legacy peer rows), and "Export my data" missed the
// settings sync and the computer details.
//
// Seen red on 2026-11-07 against HEAD's account.rs (this module included in it): "the export holds
// settings_sync: null"; and with the export assertions taken out for a second run, "still holds
// Ann's row: SELECT COUNT(*) FROM user_data WHERE public_key = ?1" (left 1, right 0), the first of
// the tables the old erase never touched.

use super::*;

const ANN: &str = "a1a1a1a1";
const BEA: &str = "b2b2b2b2";

fn fp(key: &str) -> String {
    crate::relay::storage::author_fingerprint(&hex::decode(key).unwrap())
}

fn did(key: &str) -> String {
    crate::relay::storage::plot_owner_id(key)
}

/// One of everything the erase must answer for, for `key` (named `name`); `other` is the other
/// person in the rows two people share.
fn seed(db: &Storage, key: &str, name: &str, other: &str) {
    let (f, d) = (fp(key), did(key));
    let now = 1_800_000_000_000i64;
    // A task of the other person's to comment on (task_comments has a foreign key to it).
    let task = db.create_task("Their task", "", "backlog", "medium", None, other, "").unwrap();
    db.with_conn(|c| {
        let x = |sql: &str, p: &[&dyn rusqlite::types::ToSql]| c.execute(sql, p).map(|_| ()).unwrap_or_else(|e| panic!("{sql}: {e}"));
        x("INSERT INTO user_data (public_key, data, updated_at) VALUES (?1, '{\"v\":1}', 1)", &[&key]);
        x("INSERT INTO system_profiles (public_key, profile, updated_at) VALUES (?1, '{\"cpu\":\"x\"}', 1)", &[&key]);
        x("INSERT INTO pinned_messages (channel, from_key, from_name, content, original_timestamp, pinned_by, pinned_at) VALUES ('general', ?1, ?2, 'my words', 1, ?3, 2)", &[&key, &name, &other]);
        x("INSERT INTO pinned_messages (channel, from_key, from_name, content, original_timestamp, pinned_by, pinned_at) VALUES ('general', ?1, 'Other', 'their words', 3, ?2, 4)", &[&other, &key]);
        x("INSERT INTO task_comments (task_id, author_key, author_name, content, created_at) VALUES (?3, ?1, ?2, 'a comment', 1)", &[&key, &name, &task]);
        // A group they started (with the other person in it), and their place in another's.
        let gid = format!("group-of-{key}");
        x("INSERT INTO p2p_groups (group_id, name, creator_fp, creator_pubkey, created_at) VALUES (?1, 'Book club', ?2, X'00', 1)", &[&gid, &f]);
        x("INSERT OR IGNORE INTO p2p_group_roster (group_id, member_fp, member_pubkey, active, updated_at) VALUES (?1, ?2, X'00', 1, 1)", &[&gid, &fp(other)]);
        x("INSERT INTO p2p_group_invites (invite_id, group_id, secret_hash, expires_at) VALUES (?1, ?2, X'00', 1)", &[&format!("inv-{key}"), &gid]);
        x("INSERT INTO p2p_group_epochs (group_id, epoch, object_id) VALUES (?1, 1, ?2)", &[&gid, &format!("epoch-{key}")]);
        x("INSERT INTO p2p_group_messages (object_id, group_id, author_fp, epoch, created_at) VALUES (?1, ?2, ?3, 1, 1)", &[&format!("othermsg-in-{key}"), &gid, &fp(other)]);
        let theirs = format!("group-of-{other}");
        x("INSERT OR IGNORE INTO p2p_group_roster (group_id, member_fp, member_pubkey, active, updated_at) VALUES (?1, ?2, X'00', 1, 1)", &[&theirs, &f]);
        x("INSERT INTO p2p_group_messages (object_id, group_id, author_fp, epoch, created_at) VALUES (?1, ?2, ?3, 1, 1)", &[&format!("msg-of-{key}"), &theirs, &f]);
        for (oid, ty, refs) in [
            (gid.clone(), "group_v1", "[]".to_string()),
            (format!("profile-of-{key}"), "profile_v1", "[]".to_string()),
            (format!("vote-open-{key}"), "vote_v1", "[]".to_string()),
            (format!("vote-closed-{key}"), "vote_v1", "[]".to_string()),
            (format!("proposal-of-{key}"), "proposal_v1", "[]".to_string()),
        ] {
            x("INSERT INTO signed_objects (object_id, protocol_version, object_type, author_fp, author_pubkey, payload_schema_version, payload_encoding, payload, signature, references_json, received_at) VALUES (?1, 1, ?2, ?3, X'00', 1, 'cbor', X'00', X'00', ?4, 1)", &[&oid, &ty, &f, &refs]);
        }
        // The other person's message object in the group they started.
        x("INSERT INTO signed_objects (object_id, protocol_version, object_type, author_fp, author_pubkey, payload_schema_version, payload_encoding, payload, signature, references_json, received_at) VALUES (?1, 1, 'group_msg_v1', ?2, X'00', 1, 'cbor', X'00', X'00', ?3, 1)", &[&format!("othermsg-obj-{key}"), &fp(other), &format!("[\"{gid}\"]")]);
        // An open decision and a closed one, each voted on.
        x("INSERT INTO proposals (proposal_object_id, proposer_did, proposal_type, scope, opens_at, closes_at, created_at) VALUES (?1, ?2, 'x', 'server', 0, ?3, 0)", &[&format!("open-{key}"), &did(other), &(now * 2)]);
        x("INSERT INTO proposals (proposal_object_id, proposer_did, proposal_type, scope, opens_at, closes_at, created_at) VALUES (?1, ?2, 'x', 'server', 0, 1, 0)", &[&format!("closed-{key}"), &did(other)]);
        x("INSERT INTO votes (vote_object_id, proposal_object_id, voter_did, choice, weight_at_vote, cast_at) VALUES (?1, ?2, ?3, 'yes', 1.0, 1)", &[&format!("vote-open-{key}"), &format!("open-{key}"), &d]);
        x("INSERT INTO votes (vote_object_id, proposal_object_id, voter_did, choice, weight_at_vote, cast_at) VALUES (?1, ?2, ?3, 'yes', 1.0, 1)", &[&format!("vote-closed-{key}"), &format!("closed-{key}"), &d]);
        x("INSERT INTO vc_index (vc_object_id, issuer_did, subject_did, schema_id, issued_at) VALUES (?1, ?2, ?3, 's', 1)", &[&format!("vc-{key}"), &did(other), &d]);
        x("INSERT INTO trust_scores (did, total, sub_scores_json, inputs_json, weights_version, computed_at) VALUES (?1, 1.0, '{}', '{}', 1, 1)", &[&d]);
        x("INSERT INTO ai_status (did, subject_class, last_updated) VALUES (?1, 'human', 1)", &[&d]);
        x("INSERT INTO issuer_trust (observer_server, issuer_did, trust, last_event_at) VALUES ('here', ?1, 1.0, 1)", &[&d]);
        x("INSERT INTO reputation (public_key, score) VALUES (?1, 5)", &[&key]);
        x("INSERT INTO reputation_events (public_key, event_type, points, created_at, source_key) VALUES (?1, 'help', 5, 1, ?2)", &[&key, &other]);
        x("INSERT INTO reputation_events (public_key, event_type, points, created_at, source_key) VALUES (?1, 'help', 5, 1, ?2)", &[&other, &key]);
        x("INSERT INTO bug_reports (title, description, reporter_key, reporter_name, browser_info, page_url, created_at, updated_at) VALUES ('t', 'd', ?1, ?2, 'UA', '/p', 1, 1)", &[&key, &name]);
        x("INSERT INTO trades (id, initiator_key, recipient_key, created_at) VALUES (?1, ?2, ?3, 1)", &[&format!("trade-{key}"), &key, &other]);
        x("INSERT INTO trade_orders (seller_key, item_type, quantity, remaining_qty, price_per_unit, created_at) VALUES (?1, 'wood', 1, 1, 1.0, 1)", &[&key]);
        x("INSERT INTO trade_history (order_id, buyer_key, seller_key, item_type, quantity, price_per_unit, total_price, timestamp) VALUES (1, ?1, ?2, 'wood', 1, 1.0, 1.0, 1)", &[&key, &other]);
        x("INSERT INTO guilds (id, name, owner_key, created_at, member_count) VALUES (?1, 'G', ?2, 'now', 2)", &[&format!("guild-{key}"), &key]);
        x("INSERT OR IGNORE INTO guild_members (guild_id, public_key, joined_at) VALUES (?1, ?2, 'now')", &[&format!("guild-{key}"), &other]);
        x("INSERT OR IGNORE INTO guild_members (guild_id, public_key, joined_at) VALUES (?1, ?2, 'now')", &[&format!("guild-{other}"), &key]);
        x("INSERT INTO guild_invites (id, guild_id, created_by, code, expires_at) VALUES (?1, ?2, ?3, ?4, 1)", &[&format!("gi-{key}"), &format!("guild-{other}"), &key, &format!("code-{key}")]);
        x("INSERT INTO streams (streamer_key, started_at) VALUES (?1, 1)", &[&key]);
        x("INSERT INTO stream_chat (stream_id, content, from_name, timestamp) VALUES (last_insert_rowid(), 'hi', ?1, 1)", &[&name]);
        x("INSERT INTO assets (id, owner_key, filename, file_type, category, url) VALUES (?1, ?2, 'f', 'model', 'c', '/u')", &[&format!("asset-{key}"), &key]);
        x("INSERT INTO projects (id, name, owner_key, created_at) VALUES (?1, 'P', ?2, 'now')", &[&format!("project-{key}"), &key]);
        x("INSERT INTO user_skills (user_key, skill_id) VALUES (?1, 'carpentry')", &[&key]);
        x("INSERT INTO skill_verifications (skill_id, from_key, to_key) VALUES ('carpentry', ?1, ?2)", &[&other, &key]);
        x("INSERT INTO link_codes (code, name, created_by, created_at, expires_at) VALUES (?1, ?2, ?3, 1, 2)", &[&format!("link-{key}"), &name, &key]);
        x("INSERT INTO invite_codes (code, created_by, created_at, expires_at, used_by) VALUES (?1, ?2, 1, 2, NULL)", &[&format!("invite-{key}"), &key]);
        x("INSERT INTO invite_codes (code, created_by, created_at, expires_at, used_by) VALUES (?1, ?2, 1, 2, ?3)", &[&format!("used-{key}"), &other, &key]);
        x("INSERT INTO channels (id, name, created_by, created_at) VALUES (?1, 'room', ?2, 1)", &[&format!("room-{key}"), &key]);
        x("INSERT INTO voice_channels (name, created_by, created_at) VALUES ('voice', ?1, 1)", &[&key]);
        x("INSERT INTO seller_ratings (seller_key) VALUES (?1)", &[&key]);
        x("INSERT INTO peers (public_key, display_name, last_seen) VALUES (?1, ?2, 1)", &[&key, &name]);
        x("INSERT INTO key_rotations (old_key, new_key, sig_by_old, sig_by_new, rotated_at) VALUES (?1, 'n', 's', 's', 1)", &[&key]);
        Ok::<(), rusqlite::Error>(())
    })
    .unwrap();
    // A bug vote through the storage, so its count moves with it.
    let bug = db.create_bug("Theirs", "d", "", "", "", "low", "other", other, "Other", "", "", "").unwrap();
    db.with_conn(|c| {
        c.execute("INSERT INTO bug_votes (bug_id, voter_key, voted_at) VALUES (?1, ?2, 1)", rusqlite::params![bug, key])?;
        c.execute("UPDATE bug_reports SET votes = votes + 1 WHERE id = ?1", rusqlite::params![bug])
    })
    .unwrap();
}

/// How many rows of `sql` (a COUNT) there are for `bind`.
fn count(db: &Storage, sql: &str, bind: &str) -> i64 {
    db.with_conn(|c| c.query_row(sql, rusqlite::params![bind], |r| r.get(0))).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

#[test]
fn an_erase_leaves_nothing_of_the_person_and_the_export_holds_what_it_missed() {
    let db = Storage::open_temp("account_left_behind");
    db.register_name("Ann", ANN).unwrap();
    db.register_name("Bea", BEA).unwrap();
    seed(&db, ANN, "Ann", BEA);
    seed(&db, BEA, "Bea", ANN);

    let export = db.export_account(ANN, "Ann");
    for label in ["settings_sync", "computer_details", "task_comments", "signed_objects", "groups_started", "votes", "trades", "guilds_owned", "skills"] {
        assert!(export[label].as_array().is_some_and(|a| !a.is_empty()), "the export holds {label}: {}", export[label]);
    }

    let receipt = db.delete_account(ANN, "Ann");
    assert!(!receipt.iter().any(|(l, _)| l.ends_with("_FAILED")), "nothing failed: {receipt:?}");

    let (f, d) = (fp(ANN), did(ANN));
    // Gone: each table, by the column that names Ann.
    for (sql, bind) in [
        ("SELECT COUNT(*) FROM user_data WHERE public_key = ?1", ANN),
        ("SELECT COUNT(*) FROM system_profiles WHERE public_key = ?1", ANN),
        ("SELECT COUNT(*) FROM pinned_messages WHERE from_key = ?1 OR pinned_by = ?1", ANN),
        ("SELECT COUNT(*) FROM task_comments WHERE author_key = ?1", ANN),
        ("SELECT COUNT(*) FROM p2p_groups WHERE creator_fp = ?1", &f),
        ("SELECT COUNT(*) FROM p2p_group_roster WHERE member_fp = ?1 OR group_id = 'group-of-a1a1a1a1'", &f),
        ("SELECT COUNT(*) FROM p2p_group_messages WHERE author_fp = ?1 OR group_id = 'group-of-a1a1a1a1'", &f),
        ("SELECT COUNT(*) FROM p2p_group_invites WHERE group_id = 'group-of-a1a1a1a1' AND ?1 = ?1", &f),
        ("SELECT COUNT(*) FROM p2p_group_epochs WHERE group_id = 'group-of-a1a1a1a1' AND ?1 = ?1", &f),
        ("SELECT COUNT(*) FROM signed_objects WHERE (author_fp = ?1 AND object_type NOT IN ('proposal_v1', 'vote_v1')) OR object_id = 'othermsg-obj-a1a1a1a1' OR object_id = 'vote-open-a1a1a1a1'", &f),
        ("SELECT COUNT(*) FROM votes WHERE voter_did = ?1 AND proposal_object_id = 'open-a1a1a1a1'", &d),
        ("SELECT COUNT(*) FROM vc_index WHERE subject_did = ?1 OR issuer_did = ?1", &d),
        ("SELECT COUNT(*) FROM trust_scores WHERE did = ?1", &d),
        ("SELECT COUNT(*) FROM ai_status WHERE did = ?1", &d),
        ("SELECT COUNT(*) FROM issuer_trust WHERE issuer_did = ?1", &d),
        ("SELECT COUNT(*) FROM reputation WHERE public_key = ?1", ANN),
        ("SELECT COUNT(*) FROM reputation_events WHERE public_key = ?1 OR source_key = ?1", ANN),
        ("SELECT COUNT(*) FROM bug_reports WHERE reporter_key = ?1 OR reporter_name = 'Ann'", ANN),
        ("SELECT COUNT(*) FROM bug_votes WHERE voter_key = ?1", ANN),
        ("SELECT COUNT(*) FROM trades WHERE initiator_key = ?1 OR recipient_key = ?1", ANN),
        ("SELECT COUNT(*) FROM trade_orders WHERE seller_key = ?1", ANN),
        ("SELECT COUNT(*) FROM trade_history WHERE buyer_key = ?1 OR seller_key = ?1", ANN),
        ("SELECT COUNT(*) FROM guilds WHERE owner_key = ?1", ANN),
        ("SELECT COUNT(*) FROM guild_members WHERE public_key = ?1 OR guild_id = 'guild-a1a1a1a1'", ANN),
        ("SELECT COUNT(*) FROM guild_invites WHERE created_by = ?1", ANN),
        ("SELECT COUNT(*) FROM streams WHERE streamer_key = ?1", ANN),
        ("SELECT COUNT(*) FROM stream_chat WHERE from_name = 'Ann' AND ?1 = ?1", ANN),
        ("SELECT COUNT(*) FROM assets WHERE owner_key = ?1", ANN),
        ("SELECT COUNT(*) FROM projects WHERE owner_key = ?1", ANN),
        ("SELECT COUNT(*) FROM user_skills WHERE user_key = ?1", ANN),
        ("SELECT COUNT(*) FROM skill_verifications WHERE to_key = ?1 OR from_key = ?1", ANN),
        ("SELECT COUNT(*) FROM link_codes WHERE created_by = ?1", ANN),
        ("SELECT COUNT(*) FROM invite_codes WHERE created_by = ?1 OR used_by = ?1", ANN),
        ("SELECT COUNT(*) FROM channels WHERE created_by = ?1", ANN),
        ("SELECT COUNT(*) FROM voice_channels WHERE created_by = ?1", ANN),
        ("SELECT COUNT(*) FROM seller_ratings WHERE seller_key = ?1", ANN),
        ("SELECT COUNT(*) FROM peers WHERE public_key = ?1", ANN),
        ("SELECT COUNT(*) FROM key_rotations WHERE old_key = ?1 OR new_key = ?1", ANN),
    ] {
        assert_eq!(count(&db, sql, bind), 0, "still holds Ann's row: {sql}");
    }
    // Kept on purpose: a vote on a closed decision, the proposal objects, and other people's things
    // with Ann's name taken out (the pin Ann made of Bea's words, the bug report for the admins,
    // the other side of the trade, the room Ann made).
    assert_eq!(count(&db, "SELECT COUNT(*) FROM votes WHERE voter_did = ?1 AND proposal_object_id = 'closed-a1a1a1a1'", &d), 1, "a vote on a closed decision stays");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM pinned_messages WHERE from_key = ?1 AND pinned_by = ''", BEA), 2, "Bea's words stay pinned, without who pinned them");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM bug_reports WHERE title = ?1 AND reporter_key = '' AND browser_info = ''", "t"), 1, "the report stays for the admins, unnamed");
    assert_eq!(count(&db, "SELECT votes FROM bug_reports WHERE title = 'Theirs' AND reporter_key = ?1", BEA), 0, "Ann's vote is taken off Bea's report");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM channels WHERE id = ?1 AND created_by = ''", "room-a1a1a1a1"), 1, "the room stays, without its maker");
    assert_eq!(count(&db, "SELECT member_count FROM guilds WHERE id = ?1", "guild-b2b2b2b2"), 1, "Bea's guild counts one fewer");
    // Bea's own things are all still there.
    for (sql, bind) in [
        ("SELECT COUNT(*) FROM user_data WHERE public_key = ?1", BEA),
        ("SELECT COUNT(*) FROM p2p_groups WHERE creator_fp = ?1", &fp(BEA)),
        ("SELECT COUNT(*) FROM signed_objects WHERE author_fp = ?1", &fp(BEA)),
        ("SELECT COUNT(*) FROM guilds WHERE owner_key = ?1", BEA),
        ("SELECT COUNT(*) FROM reputation WHERE public_key = ?1", BEA),
    ] {
        assert!(count(&db, sql, bind) >= 1, "Bea's row is kept: {sql}");
    }
    assert!(!db.erase_left_rows(ANN), "the erase reads as finished");
}
