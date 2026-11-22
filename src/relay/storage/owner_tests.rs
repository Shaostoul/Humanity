//! The owner role's storage (2026-11-20, storage/owner.rs).

use super::*;

/// A claim makes its key the owner only while no key holds the role, in one step with its log
/// row; the guard then refuses every other way to grant, change or duplicate the role.
///
/// Seen red 2026-11-20 with `claim_owner`'s owner count left out: "a second claim is refused" (a
/// second key became an owner, and keeping would have stopped with two owners).
#[test]
fn the_owner_is_claimed_once_and_guarded_after() {
    let db = Storage::open_temp("owner_claim_once");
    assert!(!db.has_owner());
    assert!(db.claim_owner("aa", 5).unwrap(), "the first claim");
    assert!(!db.claim_owner("bb", 6).unwrap(), "a second claim is refused");
    assert_eq!(db.owner_keys().unwrap(), ["aa".to_string()]);
    assert!(db.has_owner() && db.is_owner("aa") && !db.is_owner("bb") && !db.is_owner(""));
    let log = db.owner_log(10).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!((log[0].kind.as_str(), log[0].new_key.as_str(), log[0].at), ("claim", "aa", 5));
    assert!(is_owner_protected(&db.set_role("aa", "member").unwrap_err()));
    assert!(is_owner_protected(&db.set_role("bb", OWNER_ROLE).unwrap_err()));
    db.set_role("bb", "admin").unwrap();
    assert_eq!(db.get_role("bb").unwrap(), "admin", "everyone else's role still changes");
    assert!(!db.owner_handover_used("never").unwrap());
}
