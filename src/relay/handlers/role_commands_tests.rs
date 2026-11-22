//! The role and name commands against the server's owner (the owner review's findings 3, 8 and 10,
//! 2026-11-20). Each test's red evidence is in its comment.

use super::*;
use crate::relay::handlers::held::tests::person;
use crate::relay::storage::owner::is_owner_protected;
use crate::relay::storage::Storage;

/// A server with an owner Owen (and a linked laptop key under his name), an admin Adam, and a
/// member Mia.
fn server(tag: &str) -> (RelayState, String, String, String) {
    let st = RelayState::new(Storage::open_temp_dir(&format!("role_cmds_{tag}")));
    let (own, adm, mia) = (person(71).key, person(72).key, person(73).key);
    st.db.register_name("Owen", &own).unwrap();
    st.db.register_name("Adam", &adm).unwrap();
    st.db.register_name("Mia", &mia).unwrap();
    st.db.claim_owner(&own, 0).unwrap();
    st.db.set_role(&adm, "admin").unwrap();
    let code = st.db.create_link_code("Owen", &own).unwrap();
    st.db.redeem_link_code(&code, "owen_laptop").unwrap();
    (st, own, adm, mia)
}

/// `/verify`, `/unverify`, `/donor` and `/name-release` naming the owner are refused WHOLE, with
/// words, and change nothing for any key bound to the owner's name (the laptop included); each
/// works on a member. Seen red 2026-11-20 with the owner check taken out of `name_command_in` (the
/// loop over `set_role` as reviewed): "/verify could not be saved for 1 of Owen's 2 device(s)", the
/// floor keeping the owner's row while the laptop was set to verified.
#[test]
fn the_role_and_name_commands_never_touch_the_owner() {
    let (st, own, adm, mia) = server("names");
    for cmd in ["/verify", "/unverify", "/donor", "/name-release"] {
        let (words, changed) = name_command_in(&st, &adm, cmd, "Owen");
        assert!(words.starts_with("Nobody can") && !changed, "{cmd}: {words}");
        assert_eq!(st.db.get_role(&own).unwrap(), "owner", "{cmd}: the owner's role is kept");
        assert_eq!(st.db.get_role("owen_laptop").unwrap(), "admin", "{cmd}: the owner's laptop is not demoted");
        assert!(st.db.keys_for_name("Owen").unwrap().contains(&own), "{cmd}: the owner's name is kept");
    }
    let (words, changed) = name_command_in(&st, &adm, "/verify", "Mia");
    assert!(changed && words.contains("Mia is now verified"), "{words}");
    assert_eq!(st.db.get_role(&mia).unwrap(), "verified");
    let (words, _) = name_command_in(&st, &mia, "/verify", "Adam");
    assert!(words.starts_with("Only admins"), "{words}");
    let (words, changed) = name_command_in(&st, &adm, "/name-release", "Mia");
    assert!(changed && words.contains("Released name 'Mia'"), "{words}");
}

/// The role picker (`set_user_role`): the owner role in any spelling is never assigned, the owner's
/// own role never changed, each with words for the admin; a member's role is set, stored
/// canonical. Seen red 2026-11-20 with the storage's canonical form taken out (the exact-word guard
/// as reviewed): "\"Owner\": refused by the storage too" failed, "Owner" was stored for Mia (her
/// apps, which trim and lower a role, would show her the owner's controls).
#[test]
fn the_role_picker_never_assigns_or_changes_the_owner() {
    let (st, own, adm, mia) = server("picker");
    for variant in ["owner", "Owner", " owner", "OWNER\t"] {
        let (words, changed) = set_user_role_in(&st, &adm, &mia, variant);
        assert!(!changed && words.as_deref().is_some_and(|w| w.contains("never assigned")), "{variant:?}: {words:?}");
        assert_eq!(st.db.get_role(&mia).unwrap(), "", "{variant:?}: nothing stored");
        assert!(st.db.set_role(&mia, variant).is_err_and(|e| is_owner_protected(&e)), "{variant:?}: refused by the storage too");
    }
    let (words, changed) = set_user_role_in(&st, &adm, &own, "verified");
    assert!(!changed && words.as_deref().is_some_and(|w| w.starts_with("Nobody can")), "{words:?}");
    assert_eq!(st.db.get_role(&own).unwrap(), "owner");
    let (words, changed) = set_user_role_in(&st, &adm, &mia, " Verified ");
    assert!(changed && words.is_some(), "{words:?}");
    assert_eq!(st.db.get_role(&mia).unwrap(), "verified", "stored canonical");
    let (words, _) = set_user_role_in(&st, &mia, &adm, "verified");
    assert_eq!(words.as_deref(), Some("Only admins can assign roles."));
    let mut def = st.db.role_def("verified");
    def.id = " Owner".into();
    def.built_in = false;
    assert!(st.db.upsert_role(&def).is_err_and(|e| is_owner_protected(&e)), "no role definition may be called owner");
}

/// A link code redeemed BY the owner's own key (a browser keeps its key when it links with a code)
/// is REFUSED (the second review, R5): the owner's key is never bound under another name (that name
/// would rank as the owner and refuse every admin action on it), the owner's role never changes,
/// and the code stays unused. Seen red 2026-11-20 with the redeem's raw `ON CONFLICT ... SET role`
/// as first reviewed (BUG-345's redeem side): "the owner survives" failed; and with the owner check
/// taken out of `redeem_link_code_unless_erased`: "refused with words" was Linked("Mia").
#[test]
fn a_link_code_redeemed_by_the_owner_keeps_the_owner() {
    use crate::relay::storage::erased_accounts::LinkRedeemed;
    let (st, own, _adm, mia) = server("redeem");
    st.db.set_role(&mia, "verified").unwrap();
    let code = st.db.create_link_code("Mia", &mia).unwrap();
    assert_eq!(st.db.redeem_link_code_unless_erased(&code, &own).unwrap(), LinkRedeemed::OwnerKey, "refused with words");
    assert!(crate::relay::handlers::owner::OWNER_LINK_REFUSED.contains("owner role"));
    assert!(st.db.redeem_link_code(&code, &own).unwrap().is_none(), "by every path");
    assert_eq!(st.db.get_role(&own).unwrap(), "owner", "the owner survives");
    assert_eq!(st.db.owner_keys().unwrap(), [own.clone()]);
    assert!(st.db.name_for_key(&own).unwrap().as_deref() != Some("Mia"), "never bound under Mia's name");
    assert!(st.db.redeem_link_code_unless_erased(&code, &person(77).key).unwrap() == LinkRedeemed::Linked("Mia".into()), "the code still works for another device");
    // And the owner's own code redeemed by the owner's own key: refused too, still the owner.
    let code = st.db.create_link_code("Owen", &own).unwrap();
    assert!(st.db.redeem_link_code(&code, &own).unwrap().is_none());
    assert_eq!(st.db.get_role(&own).unwrap(), "owner");
    // The account erase's floor.
    assert_eq!(st.db.delete_account(&own, "Owen"), vec![("owner_FAILED".to_string(), 1)]);
    assert_eq!(st.db.get_role(&own).unwrap(), "owner", "the owner's row is never erased");
}
