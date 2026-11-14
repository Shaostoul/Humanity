//! The Organisations page's actions, end to end without drawing (docs/design/organisations.md).

use super::*;
use humanity_protocol::org::Refusal;

fn page_for(seed: &[u8; 32]) -> (OrgsPage, Vec<u8>) {
    let pk = keypair(seed).public_key();
    (OrgsPage::with_store(OrgStore::load_at(seed, None), &hex::encode(&pk)), pk)
}

/// An invitation travels as text and works only when the invitee accepts: the organiser's app
/// makes it, the invitee's app finds it addressed to them, nobody is a member until the invitee
/// signs, and the acceptance text makes them one in the organiser's app.
///
/// Red first, 2026-11-14: with `open_text` looking for invites to anyone (`i.who == me` dropped),
/// a third person's app offered the invitation ("only the invitee is offered it" failed).
#[test]
fn an_invitation_travels_as_text_and_works_only_when_accepted() {
    let (s_owner, s_guest, s_other) = ([1u8; 32], [2u8; 32], [3u8; 32]);
    let (mut a, owner_pk) = page_for(&s_owner);
    let (mut b, guest_pk) = page_for(&s_guest);
    let (mut c, other_pk) = page_for(&s_other);
    let org_id = a.found(&s_owner, "Cart crew", "Saturdays", Directory::Members).expect("founds");
    let (inv, refused) = a.sign(&s_owner, &org_id, Act::Invite { who: guest_pk.clone(), role: org::MEMBER.into(), until: now_ms() + 86_400_000 }).unwrap();
    assert_eq!(refused, None);
    let inv_id = inv.object_id().unwrap().to_hex();
    let text = a.invite_text(&org_id, &inv_id).expect("the invitation's text");

    c.open_text(&text, &other_pk).unwrap();
    assert_eq!(c.pending_invite, None, "only the invitee is offered it");
    let (opened, added) = b.open_text(&format!("Join us! {text}"), &guest_pk).unwrap();
    assert_eq!((opened.as_str(), added), (org_id.as_str(), 2), "the founding and the invitation");
    assert_eq!(b.pending_invite, Some((org_id.clone(), inv_id.clone())));
    assert!(a.state_of(&org_id).unwrap().member(&guest_pk).is_none(), "nobody joins by being invited");

    let (forged, refused) = c.sign(&s_other, &org_id, Act::Accept { invite: inv_id.clone() }).unwrap();
    assert_eq!(refused, Some(Refusal::NotTheInvitee));
    a.open_text(&org::bundle_text(&org_id, &[forged]), &owner_pk).unwrap();
    assert!(a.state_of(&org_id).unwrap().member(&other_pk).is_none());

    let (acc, refused) = b.sign(&s_guest, &org_id, Act::Accept { invite: inv_id }).unwrap();
    assert_eq!(refused, None);
    a.open_text(&org::bundle_text(&org_id, &[acc]), &owner_pk).unwrap();
    let st = a.state_of(&org_id).unwrap();
    assert!(st.member(&guest_pk).is_some(), "the acceptance makes them a member");
    assert_eq!(st.members.len(), 2);

    // The latest, shared back, brings the new member's app up to date.
    let latest = a.latest_text(&org_id).unwrap();
    b.open_text(&latest, &guest_pk).unwrap();
    assert_eq!(b.state_of(&org_id).unwrap().members.len(), 2);
}

/// Every act and every refusal has its words: no line of the record shows a raw key.
#[test]
fn every_line_of_the_record_has_words() {
    let (s_owner, s_guest) = ([1u8; 32], [2u8; 32]);
    let (mut a, owner_pk) = page_for(&s_owner);
    let guest_pk = keypair(&s_guest).public_key();
    let org_id = a.found(&s_owner, "Club", "", Directory::Members).unwrap();
    let acts = [
        Act::Invite { who: guest_pk.clone(), role: org::MEMBER.into(), until: now_ms() + 1000 },
        Act::RoleSet { role: Role { id: "driver".into(), name: "Driver".into(), rank: 20, perms: vec![] } },
        Act::Assign { who: guest_pk.clone(), role: "driver".into() },
        Act::Remove { who: guest_pk.clone() },
        Act::RoleDrop { role: "driver".into() },
        Act::Listing { name: "Me".into(), level: Listing::Members },
        Act::Settings { name: Some("Club two".into()), about: None, directory: None },
        Act::Leave,
        Act::Withdraw { invite: "0".repeat(64) },
        Act::Accept { invite: "0".repeat(64) },
    ];
    for act in acts {
        a.sign(&s_owner, &org_id, act).unwrap();
    }
    let st = a.state_of(&org_id).unwrap().clone();
    for (line, actor, subject) in st.audit_log_for(&owner_pk).unwrap() {
        let words = log_words(&st, &line, &actor, subject.as_ref());
        assert!(!words.contains("orgs."), "a raw key in the record: {words}");
    }
    for r in [
        Refusal::Malformed, Refusal::BeforeFounding, Refusal::NotAMember, Refusal::LacksPermission, Refusal::RankTooLow,
        Refusal::NoSuchRole, Refusal::AlreadyMember, Refusal::NoSuchInvite, Refusal::NotTheInvitee, Refusal::InviteUsed,
        Refusal::InviteWithdrawn, Refusal::InviteExpired, Refusal::InviterCannot, Refusal::RemovedSince, Refusal::OwnerStays,
        Refusal::BuiltInRole,
    ] {
        assert!(humanity_core::i18n::has(&format!("orgs.refused.{}", r.word())), "no words for {}", r.word());
    }
    let book = a.book().unwrap().clone();
    for p in &book.permissions {
        assert!(humanity_core::i18n::has(&format!("orgs.perm.{}", p.replace('.', "_"))), "no words for the permission {p}");
    }
}

#[test]
fn role_ids_and_times() {
    assert_eq!(role_id_from("Treasurer"), "treasurer");
    assert_eq!(role_id_from("Cart Driver 2"), "cart_driver_2");
    assert_eq!(role_id_from("  !! "), "");
    assert_eq!(when(0), "1970-01-01 00:00");
    assert_eq!(when(1_800_000_000_000), "2027-01-15 08:00");
}
