//! Organisations leave no trace on the relay (docs/design/organisations.md, section 3): an
//! organisation's log lives on its members' devices, so the relay refuses its objects from a
//! submission and from gossip alike, keeps no organisation table, and holds none of the tables the
//! privacy hardening removed (docs/ai/storage.md, "Removed tables").

use super::*;
use humanity_protocol::org::{build_entry, build_founding, Act, Directory, Founding, Listing, Role};
use humanity_protocol::pq_crypto::DilithiumKeypair;

/// The tables the privacy hardening removed (2026-08-23/24): never re-added.
const REMOVED: [&str; 6] = ["direct_messages", "listing_messages", "groups", "group_members", "group_messages", "follows"];

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
    let e = build_entry(&kp, &org, &Act::Listing { name: "Me".into(), level: Listing::Members }, 1_800_000_001_000).expect("signs");
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
