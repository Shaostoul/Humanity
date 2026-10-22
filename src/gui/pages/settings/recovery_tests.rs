// Child of settings/recovery.rs: Recovery through trusted people, in Settings > Account. The
// section is drawn headless (an egui context with no window) and its text read back; the work
// behind it (recovery_net.rs) runs against the REAL relay router on a loopback port. Each test was
// seen failing once; what it saw red is at the test.

use super::*;
use crate::gui::theme::load_theme;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

/// Every text drawn, with where.
fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    out.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, &mut found);
    }
    found
}

/// Draw `body` in a window tall enough that nothing is cut off, settled over three frames.
fn drawn(state: &mut GuiState, body: impl Fn(&mut egui::Ui, &Theme, &mut GuiState)) -> Vec<(String, egui::Rect)> {
    let (ctx, theme) = ctx_and_theme();
    let mut last = None;
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 12000.0))),
            ..Default::default()
        };
        last = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| body(ui, &theme, state));
        }));
    }
    texts(&last.expect("three frames ran"))
}

fn top_of(t: &[(String, egui::Rect)], text: &str) -> Option<f32> {
    t.iter().find(|(s, _)| s == text).map(|(_, r)| r.top())
}

fn all_text(t: &[(String, egui::Rect)]) -> String {
    t.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join("\n")
}

/// SETTINGS > ACCOUNT HAS RECOVERY, RIGHT AFTER SECURITY: below the Security subsection (which
/// follows Identity, where the recovery phrase is) and above Donation addresses.
///
/// Seen red 2026-10-20, before Settings drew it: "Recovery through trusted people is drawn in
/// Settings > Account".
#[test]
fn settings_account_has_recovery_right_after_security() {
    let mut state = GuiState::default();
    let t = drawn(&mut state, |ui, theme, state| crate::gui::pages::settings::draw_account_content(ui, theme, state));
    let identity = top_of(&t, "Identity").expect("Identity is drawn");
    let security = top_of(&t, "Security").expect("Security is drawn");
    let recovery = top_of(&t, TITLE).expect("Recovery through trusted people is drawn in Settings > Account");
    assert!(identity < security && security < recovery, "Recovery comes after Identity and Security: {identity} {security} {recovery}");
    if let Some(donation) = top_of(&t, "Donation addresses") {
        assert!(recovery < donation, "Recovery comes before Donation addresses: {recovery} {donation}");
    }
}

/// THE WORDS ARE FOR PEOPLE, AND THE THREE PARTS ARE THERE: none of the developer words (the
/// algorithm and object names, the routes, "DID", "guardian"), no "not finished" any more, the
/// three parts by their names, and on a device not signed in the part a new device needs.
///
/// Seen red 2026-10-21 against the 2026-10-20 section ("Not finished yet." and "Are you a
/// guardian?"): "it no longer says it is not finished".
#[test]
fn the_words_are_for_people_and_the_three_parts_are_there() {
    let mut state = GuiState::default();
    let t = drawn(&mut state, |ui, theme, state| draw(ui, theme, state, theme.info()));
    let all = all_text(&t);
    for word in [
        "Shamir", "Kyber", "DID", "signed_object", "signed object", "key_rotation", "recovery_share", "guardian",
        "Dilithium", "pubkey", "/api/", "ciphertext", "threshold", "seed",
    ] {
        assert!(!all.contains(word), "the section says {word}:\n{all}");
    }
    assert!(!all.contains("Not finished"), "it no longer says it is not finished:\n{all}");
    for part in ["Your trusted people", "Help someone get back in", "Lost your device? Ask your trusted people for help"] {
        assert!(all.contains(part), "the section has its part {part:?}:\n{all}");
    }
    assert!(all.contains("This device is not signed in to an account yet"), "not signed in, it says there is nothing to protect yet");
    assert!(all.contains("Find my account"), "a new device can look its account up");
}

/// TRUSTED PEOPLE ARE CHOSEN FROM FRIENDS: only people I follow who follow me back are offered,
/// each with the name the app knows them by and whether their key is known.
///
/// Seen red 2026-10-21 with `friends` taking everyone followed (`is_follower` left out): "someone
/// who does not follow back is not offered".
#[test]
fn trusted_people_are_chosen_from_friends() {
    let mut state = GuiState::default();
    let seed = [0x5au8; 32];
    let me = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key());
    let mut store = crate::net::dm_store::DmStore::load(&seed, &me, "recovery-friends-test");
    let key = |b: u8| hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[b; 32])).public_key());
    let (ann, ben, cy) = (key(1), key(2), key(3));
    for k in [&ann, &ben, &cy] {
        store.set_following(k, true);
    }
    store.set_follower(&ann, true);
    store.set_follower(&cy, true);
    state.dm_store = Some(store);
    state.peer_kyber_keys.insert(ann.clone(), "a-key".into());
    let list = friends(&state);
    let keys: Vec<&str> = list.iter().map(|f| f.key.as_str()).collect();
    assert!(keys.contains(&ann.as_str()) && keys.contains(&cy.as_str()));
    assert!(!keys.contains(&ben.as_str()), "someone who does not follow back is not offered");
    let a = list.iter().find(|f| f.key == ann).unwrap();
    assert_eq!(a.kyber_b64.as_deref(), Some("a-key"));
    assert_eq!(a.did, crate::relay::core::did::did_for_pubkey(&hex::decode(&ann).unwrap()));
    assert!(list.iter().find(|f| f.key == cy).unwrap().kyber_b64.is_none());
}

#[test]
fn the_suggestion_is_more_than_half_and_a_date_reads() {
    assert_eq!((2..=10).map(suggested_needed).collect::<Vec<_>>(), vec![2, 2, 3, 3, 4, 4, 5, 5, 6]);
    assert_eq!(day(0), "1 January 1970");
    assert_eq!(day(1_761_000_000_000), "20 October 2025");
}

/// A loopback relay on its own runtime: the real router, a temp database. Returns the web address
/// and the runtime (kept alive by the caller).
fn loopback_relay(tag: &str) -> (String, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let db = crate::relay::storage::Storage::open_temp(&format!("recovery_desktop_{tag}"));
    let state = std::sync::Arc::new(crate::relay::relay::RelayState::new(db));
    let app = crate::relay::build_router(state);
    let port = rt.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        port
    });
    (format!("http://127.0.0.1:{port}"), rt)
}

fn friend_of(secret: &[u8; 32], name: &str) -> Friend {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(secret));
    let kyber = crate::relay::core::pq_crypto::KyberKeypair::from_seed(&crate::relay::core::pq_crypto::derive_kyber_seed(secret)).unwrap();
    use base64::Engine;
    Friend {
        key: hex::encode(dil.public_key()),
        did: crate::relay::core::did::did_for_pubkey(&dil.public_key()),
        name: name.into(),
        kyber_b64: Some(base64::engine::general_purpose::STANDARD.encode(kyber.public_key())),
    }
}

/// THE DESKTOP APP, END TO END, AGAINST THE REAL ROUTES: a person sends pieces to three friends
/// and sees who keeps them; on a new device they find their account and ask; two trusted people
/// see who is asking, look up the code as read aloud, and send their pieces; the new device
/// rebuilds the account and restores it through "Restore from recovery phrase", and it is the
/// same account. Then "that was not me" closes a request, and taking the pieces back empties the
/// setup.
///
/// Seen red 2026-10-21 with `poll_ask` opening answers under the slot after their own (an
/// off-by-one in the slot it passed): the request never rebuilt, "the new device rebuilds the
/// account".
#[test]
fn the_desktop_app_end_to_end_against_the_real_routes() {
    let (base, _rt) = loopback_relay("e2e");
    let holder = [0x61u8; 32];
    let (ann, ben, cy) = ([0x62u8; 32], [0x63u8; 32], [0x64u8; 32]);
    let people = vec![friend_of(&ann, "Ann"), friend_of(&ben, "Ben"), friend_of(&cy, "Cy")];
    let holder_did = proto::account_of(&holder);

    let said = net::send_setup(&base, &holder, people.clone(), 2).expect("the pieces are sent");
    assert!(said.contains("Ann, Ben and Cy each keep a piece"), "{said}");
    let mine = net::load_my_setup(&base, &holder).unwrap();
    assert_eq!(mine.needed, 2);
    assert_eq!(mine.keepers.len(), 3);
    assert!(mine.keepers[0].starts_with("Ann (did:hum:"), "the notes to self name who keeps each piece: {:?}", mine.keepers);
    assert_eq!(mine.asked_since, None);

    // A new device asks.
    let found = net::find_account(&base, &holder_did).unwrap();
    assert_eq!((found[0].needed, found[0].total), (2, 3));
    let live = std::sync::Arc::new(net::open_ask(&base, found[0].clone()).unwrap());
    assert_eq!(net::poll_ask(&base, &live).unwrap(), Poll::Waiting { arrived: 0, needed: 2 });
    assert!(net::load_my_setup(&base, &holder).unwrap().asked_since.is_some(), "the person's own devices see the request");

    // Two trusted people help.
    let names: HashMap<String, String> = [(holder_did.clone(), "Holder".to_string())].into();
    let asking = net::who_is_asking(&base, &ann, &names).unwrap();
    assert_eq!(asking.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Holder"], "Ann sees who is asking");
    assert!(net::who_is_asking(&base, &[0x70u8; 32], &names).unwrap().is_empty(), "someone with no piece sees nobody");
    let read_aloud = proto::show_code(&live.code).to_lowercase().replace('0', "o");
    for helper in [&ann, &cy] {
        let l = net::look_up_code(&base, helper, &read_aloud, &names).expect("the code as read aloud");
        assert_eq!(l.name, "Holder");
        net::send_piece(&base, &l).expect("the piece is sent");
    }
    assert!(net::look_up_code(&base, &[0x70u8; 32], &read_aloud, &names).is_err(), "no piece, nothing to send");
    let rebuilt = match net::poll_ask(&base, &live).unwrap() {
        Poll::Rebuilt(secret) => secret,
        other => panic!("the new device rebuilds the account: {other:?}"),
    };
    assert_eq!(rebuilt, holder);

    // Restored through the recovery phrase path: the same account.
    let mut device = GuiState::default();
    let phrase = crate::net::identity::mnemonic_from_seed(&rebuilt).unwrap();
    crate::gui::pages::settings::restore_from_phrase(&mut device, &phrase).unwrap();
    let want = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&holder)).public_key());
    assert_eq!(device.profile_public_key, want, "restoring gives back the same account");
    net::close_ask(&base, &live);
    assert_eq!(net::poll_ask(&base, &live).unwrap(), Poll::Ended);

    // "That was not me", then taking the pieces back.
    let other = std::sync::Arc::new(net::open_ask(&base, found[0].clone()).unwrap());
    net::not_me(&base, &holder).unwrap();
    assert_eq!(net::poll_ask(&base, &other).unwrap(), Poll::Ended, "that was not me closes it");
    net::withdraw(&base, &holder).unwrap();
    assert!(net::load_my_setup(&base, &holder).unwrap().keepers.is_empty(), "taken back");
    assert!(net::find_account(&base, &holder_did).is_err(), "nothing to ask for any more");
}
