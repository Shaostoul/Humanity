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
