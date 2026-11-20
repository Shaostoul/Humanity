// Child of msg_handlers.rs (#[path], so `use super::*` reaches everything it has),
// moved out under the file-size ratchet on 2026-10-09 (tests/file_size_ratchet.rs).

use super::*;
use crate::relay::relay::RelayState;

fn fresh_state() -> Arc<RelayState> {
    let db = Storage::open_temp("modact");
    Arc::new(RelayState::new(db))
}

/// Collect the Private/System message bodies a handler broadcast.
fn drain(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(RelayMessage::Private { message, .. }) => out.push(message),
            Ok(RelayMessage::System { message }) => out.push(message),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    out
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A caller with no mod/admin role can't perform any mod action.
#[test]
fn non_mod_cannot_act() {
    let st = fresh_state();
    st.db.register_name("Victim", "victim_key").unwrap();
    st.db.join_server("victim_key", "Victim").unwrap();
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_mod_action(&st, "rando_key", "kick", "victim_key", "Victim"));
    let msgs = drain(&mut rx);
    assert!(
        msgs.iter().any(|m| m.contains("don't have permission")),
        "expected permission denial, got {msgs:?}"
    );
    assert!(
        !st.db.keys_for_name("Victim").unwrap().is_empty(),
        "victim must not have been kicked"
    );
}

/// Ban is admin-only — a plain moderator is refused.
#[test]
fn mod_cannot_ban_admin_only() {
    let st = fresh_state();
    st.db.set_role("mod_key", "mod").unwrap();
    st.db.register_name("Victim", "victim_key").unwrap();
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_mod_action(&st, "mod_key", "ban", "victim_key", "Victim"));
    let msgs = drain(&mut rx);
    assert!(
        msgs.iter().any(|m| m.contains("requires admin")),
        "ban must be admin-only, got {msgs:?}"
    );
    assert!(!st.db.is_banned("victim_key").unwrap());
}

/// Admin ban happy path: persists to banned_keys WITH the display
/// name (the v0.245 irreversible-ban-trap guard, end-to-end through
/// the handler) and confirms to the actor.
#[test]
fn admin_ban_persists_and_captures_name() {
    let st = fresh_state();
    st.db.set_role("admin_key", "admin").unwrap();
    st.db.register_name("Victim", "victim_key").unwrap();
    st.db.join_server("victim_key", "Victim").unwrap();
    st.db.set_role("victim_key", "verified").unwrap();
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_mod_action(&st, "admin_key", "ban", "victim_key", "Victim"));
    let msgs = drain(&mut rx);
    assert!(st.db.is_banned("victim_key").unwrap(), "victim must be banned");
    let banned = st.db.list_banned().unwrap();
    assert_eq!(banned.len(), 1);
    assert_eq!(
        banned[0].name, "Victim",
        "name must be captured for the Banned-users panel"
    );
    assert!(
        msgs.iter().any(|m| m.contains("Banned")),
        "actor should get a Banned confirmation, got {msgs:?}"
    );
}

/// v0.247 SECURITY regression: a non-admin moderator must NOT be
/// able to act on an admin via the NAME-ONLY path (no target key).
/// Before v0.247 the admin-protection check needed a key, so this
/// slipped straight through.
#[test]
fn name_only_kick_of_admin_is_refused() {
    let st = fresh_state();
    st.db.set_role("mod_key", "mod").unwrap();
    st.db.register_name("BigBoss", "boss_key").unwrap();
    st.db.join_server("boss_key", "BigBoss").unwrap();
    st.db.set_role("boss_key", "admin").unwrap();
    let mut rx = st.broadcast_tx.subscribe();
    // target key empty — only the name is supplied.
    block(handle_mod_action(&st, "mod_key", "kick", "", "BigBoss"));
    let msgs = drain(&mut rx);
    assert!(
        msgs.iter()
            .any(|m| m.contains("Only an admin can act on another admin")),
        "name-only kick of an admin must be refused, got {msgs:?}"
    );
    assert!(
        !st.db.keys_for_name("BigBoss").unwrap().is_empty(),
        "the admin must NOT have been kicked via the name-only path"
    );
}

/// Self-protection also covers the name-only path (v0.247 widened
/// the self-guard from `target == my_key` to the resolved key set).
#[test]
fn cannot_self_ban_by_name() {
    let st = fresh_state();
    st.db.set_role("me_key", "admin").unwrap();
    st.db.register_name("Me", "me_key").unwrap();
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_mod_action(&st, "me_key", "ban", "", "Me"));
    let msgs = drain(&mut rx);
    assert!(
        msgs.iter().any(|m| m.contains("can't ban yourself")),
        "self-ban by name must be blocked, got {msgs:?}"
    );
    assert!(!st.db.is_banned("me_key").unwrap());
}

/// v0.246 invariant proven through the handler: mute must not touch
/// the user's role, and unmute leaves it intact (a Donor stays a
/// Donor across mute → unmute).
#[test]
fn mute_preserves_role_through_handler() {
    let st = fresh_state();
    st.db.set_role("mod_key", "mod").unwrap();
    st.db.register_name("Donor", "donor_key").unwrap();
    st.db.set_role("donor_key", "donor").unwrap();

    block(handle_mod_action(&st, "mod_key", "mute", "donor_key", "Donor"));
    assert!(st.db.is_muted("donor_key").unwrap(), "should be muted");
    assert_eq!(
        st.db.get_role("donor_key").unwrap(),
        "donor",
        "mute must not clobber the role"
    );

    block(handle_mod_action(&st, "mod_key", "unmute", "donor_key", "Donor"));
    assert!(!st.db.is_muted("donor_key").unwrap(), "should be unmuted");
    assert_eq!(
        st.db.get_role("donor_key").unwrap(),
        "donor",
        "role still intact after unmute"
    );
}

/// A member verified mid-session is told their new line limit at once (2026-10-24), so the
/// website stops refusing, and both apps stop flagging, lines the server now takes. Before this
/// the limit was told only on sign-in, and a page held the old 280 until it was reloaded.
#[test]
fn a_member_verified_mid_session_is_told_their_new_line_limit() {
    let st = fresh_state();
    st.db.set_role("admin_key", "admin").unwrap();
    st.db.register_name("Newcomer", "new_key").unwrap();
    let before = st.db.role_def(&st.db.get_role("new_key").unwrap_or_default()).max_chars;
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_mod_action(&st, "admin_key", "verify", "new_key", "Newcomer"));
    let mut told = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let RelayMessage::MyLimits { to, max_chars } = msg {
            told.push((to, max_chars));
        }
    }
    let verified = st.db.role_def("verified").max_chars;
    assert!(verified > before, "the test needs a verified limit above a member's ({verified} vs {before})");
    assert_eq!(told, vec![("new_key".to_string(), verified)], "only the verified member, with the verified limit");
}

/// THE OWNER IS AN ADMIN EVERYWHERE THE RELAY CHECKS FOR ONE (hide and keep rung 5, 2026-11-20;
/// BUG-330 fixed `mod_action_refusal` alone). The owner edits and deletes another person's
/// marketplace listing, deletes another person's review, and edits and deletes another person's
/// project, as an admin does; a member who is not the author is still refused each one; the board
/// and the knock budget's checks take the owner as well.
///
/// Seen red 2026-11-20 against the checks as they were (`user_role == "admin" || user_role ==
/// "mod"` and `== "admin"`): "the owner edits another seller's listing" failed first (the title
/// stayed "Lamp").
#[test]
fn the_owner_counts_as_an_admin_for_listings_reviews_and_projects() {
    let st = fresh_state();
    st.db.set_role("owner_key", "owner").unwrap();
    let listing = |id: &str| {
        st.db.create_listing(id, "seller_key", "Sam", "Lamp", "a lamp", "home", "used", "5", "cash", "here").unwrap();
    };
    let edit = |who: &str, id: &str| {
        block(handle_listing_update(&st, who, id.into(), "Taken down".into(), "".into(), "home".into(), "used".into(), "5".into(), "cash".into(), "here".into(), None));
    };
    let title = |id: &str| st.db.get_listing_by_id(id).unwrap().map(|l| l.title);

    listing("l1");
    edit("rando_key", "l1");
    assert_eq!(title("l1").as_deref(), Some("Lamp"), "a member who is not the seller cannot edit it");
    edit("owner_key", "l1");
    assert_eq!(title("l1").as_deref(), Some("Taken down"), "the owner edits another seller's listing");
    block(handle_listing_delete(&st, "rando_key", "l1".into()));
    assert!(title("l1").is_some(), "a member who is not the seller cannot delete it");
    block(handle_listing_delete(&st, "owner_key", "l1".into()));
    assert!(title("l1").is_none(), "the owner deletes another seller's listing");

    listing("l2");
    let review = st.db.create_review("l2", "buyer_key", "Bea", 1, "bad").unwrap();
    block(handle_review_delete(&st, "rando_key", "l2".into(), review));
    assert!(st.db.get_review_by_id(review).unwrap().is_some(), "a member who did not write it cannot delete a review");
    block(handle_review_delete(&st, "owner_key", "l2".into(), review));
    assert!(st.db.get_review_by_id(review).unwrap().is_none(), "the owner deletes another person's review");

    st.db.create_project("p1", "Garden", "", "maker_key", "public", "#000000", "").unwrap();
    let project = || st.db.get_projects_visible_to("maker_key").unwrap().into_iter().find(|(p, _)| p.id == "p1").map(|(p, _)| p.name);
    block(handle_project_update(&st, "rando_key", "p1".into(), "Mine now".into(), "".into(), "public".into(), "#000000".into(), "".into()));
    assert_eq!(project().as_deref(), Some("Garden"), "a member who did not make it cannot edit a project");
    block(handle_project_update(&st, "owner_key", "p1".into(), "Renamed".into(), "".into(), "public".into(), "#000000".into(), "".into()));
    assert_eq!(project().as_deref(), Some("Renamed"), "the owner edits another person's project");
    block(handle_project_delete(&st, "rando_key", "p1".into()));
    assert!(project().is_some(), "a member who did not make it cannot delete a project");
    block(handle_project_delete(&st, "owner_key", "p1".into()));
    assert!(project().is_none(), "the owner deletes another person's project");

    assert!(may_edit_tasks("owner") && may_comment_on_tasks("owner"), "the board takes the owner as an admin");
    assert!(is_admin_role("owner") && is_staff_role("owner") && !is_admin_role("mod") && is_staff_role("moderator"));
}

/// THE OWNER ALSO LISTS THINGS FOR SALE AND RUNS THE VOICE CHANNELS, AS AN ADMIN DOES (BUG-332,
/// found merging it: the listing gate and the voice channels' create, rename and delete still read
/// "admin" or "mod" alone). A member with no role is still refused each one.
///
/// Seen red 2026-11-20 against the checks as they were (`user_role != "verified" && ... != "admin"`
/// and `user_role != "admin" && user_role != "mod"`): "the owner lists something for sale" failed.
#[test]
fn the_owner_lists_for_sale_and_runs_voice_channels_as_an_admin_does() {
    let st = fresh_state();
    st.db.set_role("owner_key", "owner").unwrap();
    let create = |who: &str, id: &str| {
        block(handle_listing_create(&st, who, id.into(), "Lamp".into(), "a lamp".into(), "home".into(), "used".into(), "5".into(), "cash".into(), "here".into()));
    };
    let listed = |id: &str| st.db.get_listing_by_id(id).unwrap().is_some();
    create("rando_key", "l1");
    assert!(!listed("l1"), "a member who is not verified cannot list");
    create("owner_key", "l2");
    assert!(listed("l2"), "the owner lists something for sale");

    let room = |name: &str| st.db.list_voice_channels().unwrap().into_iter().find(|c| c.name == name).map(|c| c.id);
    let voice = |who: &str, action: &str, id: Option<i64>, name: Option<&str>| {
        block(handle_voice_room(&st, who, 1, action.into(), id.map(|i| i.to_string()), name.map(str::to_string)));
    };
    voice("rando_key", "create", None, Some("Members"));
    assert!(room("Members").is_none(), "a member cannot make a voice channel");
    voice("owner_key", "create", None, Some("Lobby"));
    let id = room("Lobby").expect("the owner makes a voice channel");
    voice("rando_key", "rename", Some(id), Some("Mine"));
    assert_eq!(room("Lobby"), Some(id), "a member cannot rename it");
    voice("owner_key", "rename", Some(id), Some("Hall"));
    assert_eq!(room("Hall"), Some(id), "the owner renames it");
    voice("rando_key", "delete", Some(id), None);
    assert_eq!(room("Hall"), Some(id), "a member cannot delete it");
    voice("owner_key", "delete", Some(id), None);
    assert!(room("Hall").is_none(), "the owner deletes it");
}
