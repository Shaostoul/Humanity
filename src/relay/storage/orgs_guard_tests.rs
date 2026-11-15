//! Organisations leave no trace on the relay (docs/design/organisations.md, section 3): an
//! organisation's log lives on its members' devices, so the relay refuses its objects from a
//! submission and from gossip alike, keeps no organisation table, and holds none of the tables the
//! privacy hardening removed (docs/ai/storage.md, "Removed tables").

use super::*;
use humanity_protocol::org::{build_entry, build_founding, Act, Directory, Founding, Listing, Role};
use humanity_protocol::pq_crypto::DilithiumKeypair;

/// The tables the privacy hardening removed (2026-08-23/24), and the guild tables dropped when
/// guilds retired into organisations (2026-11-15): never re-added.
const REMOVED: [&str; 9] = [
    "direct_messages", "listing_messages", "groups", "group_members", "group_messages", "follows",
    "guilds", "guild_members", "guild_invites",
];

/// Red first, 2026-11-14: before the `is_organisation_object` refusal in `put_signed_object`, the
/// founding was stored: the test failed at its first check, "a server kept an organisation's org_v1".
#[test]
fn organisations_leave_no_trace_on_the_relay() {
    let db = Storage::open_temp("orgs-guard");
    let kp = DilithiumKeypair::from_seed(&[7; 32]);
    let role = |id: &str, rank: u64| Role { id: id.into(), name: String::new(), rank, perms: vec![] };
    let founding = Founding { name: "Cart crew".into(), about: String::new(), directory: Directory::Members, roles: vec![role("owner", 100), role("admin", 50), role("member", 10)] };
    let f = build_founding(&kp, &founding, 1_800_000_000_000).expect("founds");
    let org = f.object_id().unwrap().to_hex();
    let e = build_entry(&kp, &org, &Act::Listing { name: "Me".into(), level: Listing::Members }, &[], 1_800_000_001_000).expect("signs");
    for o in [&f, &e] {
        assert!(db.put_signed_object(o, None).is_err(), "a server kept an organisation's {}", o.object_type);
        assert!(db.put_signed_object(o, Some("peer.example")).is_err(), "gossip put an organisation's {} on a server", o.object_type);
    }
    assert_eq!(db.count_signed_objects(None).unwrap(), 0, "nothing of an organisation is stored");

    let tables: Vec<String> = {
        let conn = db.conn.lock().unwrap();
        let mut q = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table'").unwrap();
        q.query_map([], |r| r.get::<_, String>(0)).unwrap().filter_map(Result::ok).collect()
    };
    for t in &tables {
        assert!(!REMOVED.contains(&t.as_str()), "the removed table `{t}` is back (docs/ai/storage.md, Removed tables)");
        assert!(!t.starts_with("org"), "the relay has an organisation table `{t}`: a log lives on members' devices");
    }
}

/// THE LIVE DATABASE SHEDS THE GUILD TABLES (guilds retired into organisations, 2026-11-15). A fresh
/// database never makes them, which the test above sees; this one opens the database the code
/// before wrote (tests/fixtures/relay/relay_v0_1456.sql, which has all three), with a guild, a
/// member and an invitation in it, through the real upgrade, and none of the three is left.
///
/// Seen red 2026-11-15 with the upgrade's guild DROP step switched off: "the upgrade left `guilds`
/// on the server: a guild's member list in the clear".
#[test]
fn the_live_database_sheds_the_guild_tables() {
    let path = humanity_core::test_temp::db("guilds_retired");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/tests/fixtures/relay/relay_v0_1456.sql"))).expect("the previous code's database loads from its dump");
        conn.execute_batch(
            "INSERT INTO guilds (id, name, owner_key, created_at, member_count) VALUES ('g1', 'Cart crew', 'a1a1', 'now', 1);
             INSERT INTO guild_members (guild_id, public_key, joined_at) VALUES ('g1', 'a1a1', 'now');
             INSERT INTO guild_invites (id, guild_id, created_by, code, expires_at) VALUES ('i1', 'g1', 'a1a1', 'c0de', 1);",
        )
        .unwrap();
    }
    let db = Storage::open(&path).expect("the live database opens");
    let conn = db.conn.lock().unwrap();
    for t in ["guilds", "guild_members", "guild_invites"] {
        assert!(conn.prepare(&format!("SELECT 1 FROM {t} LIMIT 0")).is_err(), "the upgrade left `{t}` on the server: a guild's member list in the clear");
    }
}
