//! Tests of Home > Household (household.rs; ship homes increment 5b, lane 2B): the card drawn and
//! clicked headlessly, the way fleet_ledger_tests.rs drives its panel, over a real DM store and a
//! socket that records what is sent. Each was seen failing once on purpose; the failure text is in
//! its comment.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::net::dm_store::DmStore;
use crate::net::household::{GivenPermit, GivenState, HeldPermit, Judged, PermitKey, PermitPut};
use crate::relay::core::pq_crypto::{build_friend_cert, derive_dilithium_seed, DilithiumKeypair, FRIEND_PASS_DEFAULT_MAY};
use crate::build_permission::Permit;
use serde_json::Value;
use std::sync::mpsc::Receiver;

/// The server every app here is on.
const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const SERIAL: &str = "00112233445566778899aabbccddeeff";

/// The words the card must never say (plan 3.9): whole words, and the stems of the rest (so
/// "revoked" and "verified" count too).
const SECURITY_WORDS: [&str; 4] = ["key", "keys", "token", "tokens"];
const SECURITY_STEMS: [&str; 4] = ["certificat", "signatur", "revok", "verif"];

// ── An app to draw ───────────────────────────────────────────────────────────────────────────────

/// A signed-in app connected to its own store of [`SERVER`] (`tag` names it), with a socket that
/// records what is sent. `seed` and `me` are its identity (a made-up key is enough for drawing; a
/// real one is needed to send).
fn app(seed: &[u8], me: &str, tag: &str) -> (GuiState, Receiver<String>) {
    let mut gs = GuiState::default();
    gs.profile_public_key = me.to_string();
    gs.private_key_bytes = Some(seed.to_vec());
    gs.user_name = "Me".into();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    gs.server_url = format!("https://household-page-{tag}-{nanos}.example");
    let mut store = DmStore::load(seed, me, &crate::gui::pages::chat::norm_server_url(&gs.server_url));
    store.set_pass_server(SERVER);
    gs.dm_store = Some(store);
    gs.block_list = Some(crate::net::block_list::BlockList::in_temp(seed, me, tag));
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.dm_fetch = crate::net::mailbox_fetch::MailboxFetch::already_read();
    (gs, sent)
}

/// [`app`] in this server's shared world on plot p2.
fn on_p2(seed: &[u8], me: &str, tag: &str) -> (GuiState, Receiver<String>) {
    let (mut gs, sent) = app(seed, me, tag);
    gs.copresence_active = true;
    gs.household.my_plot = Some("p2".into());
    gs.household.welcomed = true;
    (gs, sent)
}

fn tidy(gs: &GuiState) {
    if let Some(s) = gs.dm_store.as_ref() {
        s.remove_file_for_test();
    }
    if let Some(l) = gs.block_list.as_ref() {
        l.remove_file_for_test();
    }
}

fn store(gs: &mut GuiState) -> &mut DmStore {
    gs.dm_store.as_mut().expect("a store")
}

/// `key` (called `name`) is a friend here: a mutual follow, on the member list, and, when
/// `sealable`, with a DM key known this run.
fn friend(gs: &mut GuiState, key: &str, name: &str, sealable: bool) {
    store(gs).set_following(key, true);
    store(gs).set_follower(key, true);
    named(gs, key, name);
    if sealable {
        gs.peer_kyber_keys.insert(key.to_string(), "a-dm-key".into());
    }
}

/// `key` is called `name` on the member list.
fn named(gs: &mut GuiState, key: &str, name: &str) {
    gs.chat_users.push(crate::gui::ChatUser { name: name.into(), public_key: key.into(), role: String::new(), status: String::new() });
}

/// I gave `to` a permit for `plot`, named `serial`, given at `at_s` and running out at `expiry`,
/// standing as `state`.
fn gave(gs: &mut GuiState, to: &str, plot: &str, serial: &str, at_s: u64, expiry: u64, state: GivenState) {
    let row = GivenPermit { to: to.into(), plot: plot.into(), serial: serial.into(), expiry, at: at_s * 1000, state };
    store(gs).record_given(row, now_s());
}

/// `from` gave me a permit for `plot` running out at `expiry`; the server said `verdict` of it this
/// session.
fn held(gs: &mut GuiState, from: &str, plot: &str, expiry: u64, verdict: Option<Verdict>) {
    let permit = Permit {
        issuer: from.into(),
        server: SERVER.into(),
        plot: plot.into(),
        grantee: "did:hum:me".into(),
        serial: SERIAL.into(),
        expiry,
        sig: "c2ln".into(),
    };
    let h = HeldPermit { from: from.into(), permit, at: 1 };
    store(gs).keep_permit(h.clone());
    if let Some(verdict) = verdict {
        gs.household.verdicts.insert(PermitKey::of(&h), Judged { verdict, at: 0.0 });
    }
}

/// A ship with these plots (only their ids matter to the card).
fn ship(ids: &[&str]) -> crate::ship::ship_structure::ShipStructure {
    let plots = ids
        .iter()
        .map(|id| {
            let text = format!("(id: \"{id}\", kind: \"homestead\", origin: (0.0, 0.0, 0.0), size: (55.0, 3.0, 89.0), door: (zone: \"street\", lat: 0.0))");
            ron::from_str(&text).expect("a plot")
        })
        .collect();
    crate::ship::ship_structure::ShipStructure { plots, ..Default::default() }
}

/// A hex serial from `n`.
fn serial(n: u32) -> String {
    format!("{n:032x}")
}

const DAY: u64 = 86_400;

/// The card in every state of section 3.9 at once: me (a made-up key) in the shared world on
/// p2, friends whose permits stand each way, and permits held each way. Also the snapshot's.
/// Returns the end dates the words are checked against: (Bo's, Hal's, Ada's p1, Cal's p8).
pub(crate) fn every_row(gs: &mut GuiState) -> (u64, u64, u64, u64) {
    let now = now_s();
    let (bo, hal, ada, cal) = (now + 29 * DAY - 3_600, now - 2 * DAY, now + 29 * DAY - 3_600, now - DAY);
    for (key, name) in [
        ("k-bo", "Bo"),
        ("k-cy", "Cy"),
        ("k-di", "Di"),
        ("k-ed", "Ed"),
        ("k-fay", "Fay"),
        ("k-gil", "Gil"),
        ("k-hal", "Hal"),
        ("k-ivy", "Ivy"),
        ("k-jo", "Jo"),
        ("k-lee", "Lee"),
        ("k-max", "Max"),
    ] {
        friend(gs, key, name, true);
    }
    friend(gs, "k-kim", "Kim", false);
    named(gs, "k-ned", "Ned");
    // Bo: the server took it, given a day ago for 30 days.
    gave(gs, "k-bo", "p2", &serial(1), now - DAY, bo, GivenState::Taken);
    // Cy: on its way this run.
    gave(gs, "k-cy", "p2", &serial(2), now, now + 30 * DAY, GivenState::OnItsWay { reference: "r-cy".into() });
    gs.household.on_their_way.push(PermitPut {
        reference: "r-cy".into(),
        peer: "k-cy".into(),
        plot: "p2".into(),
        serial: serial(2),
        expiry: now + 30 * DAY,
        days: 30,
        self_copy: Value::Null,
        at: std::time::Instant::now(),
    });
    // Di: no answer came.
    gave(gs, "k-di", "p2", &serial(3), now, now + 30 * DAY, GivenState::Unconfirmed);
    // Ed, Fay, Gil: the server did not take it, for each reason.
    for (n, key, reason) in [(4, "k-ed", "reach"), (5, "k-fay", "rate"), (6, "k-gil", "too_large")] {
        gave(gs, key, "p2", &serial(n), now, now + 30 * DAY, GivenState::Refused { reason: reason.into() });
    }
    // Hal: it ran out.
    gave(gs, "k-hal", "p2", &serial(7), now - 9 * DAY, hal, GivenState::Taken);
    // Ivy: taken back, the server has not confirmed it yet.
    gave(gs, "k-ivy", "p2", &serial(8), now - DAY, now + 6 * DAY, GivenState::Taken);
    store(gs).take_back_permits("k-ivy", |_| false);
    // Jo: taken back and confirmed.
    gave(gs, "k-jo", "p2", &serial(9), now - DAY, now + 6 * DAY, GivenState::Taken);
    store(gs).take_back_permits("k-jo", |_| false);
    store(gs).withdrawal_confirmed(&serial(9));
    // Max: a permit for p3, a plot that is no longer mine here.
    gave(gs, "k-max", "p3", &serial(10), now - DAY, now + 20 * DAY, GivenState::Taken);
    // Ned: blocked, which took back his permit.
    gave(gs, "k-ned", "p2", &serial(11), now - DAY, now + 20 * DAY, GivenState::Taken);
    store(gs).take_back_permits("k-ned", |_| false);
    gs.block_list.as_mut().expect("a block list").block("k-ned", now);

    // Permits friends gave me, one for each thing the server can say.
    for (key, name) in [("k-ada", "Ada"), ("k-ben", "Ben"), ("k-cal", "Cal")] {
        named(gs, key, name);
    }
    held(gs, "k-ada", "p1", ada, Some(Verdict::Works));
    held(gs, "k-ada", "p3", ada, Some(Verdict::Checking));
    held(gs, "k-ben", "p4", ada, Some(Verdict::Refused(Why::PermitNotFromHolder)));
    held(gs, "k-ben", "p5", ada, Some(Verdict::Refused(Why::PermitBad)));
    held(gs, "k-ben", "p6", ada, Some(Verdict::Refused(Why::PermitTooLong)));
    held(gs, "k-cal", "p7", ada, Some(Verdict::Refused(Why::PermitWithdrawn)));
    held(gs, "k-cal", "p8", cal, Some(Verdict::Works));
    held(gs, "k-cal", "p99", ada, Some(Verdict::Works));
    gs.ship_structure = Some(ship(&["p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11", "p12"]));
    (bo, hal, ada, cal)
}

/// The snapshot's app (ui_snapshots.rs `snapshot_household`): [`every_row`], with a made-up
/// identity, nothing written to disk.
pub(crate) fn snapshot_state(gs: &mut GuiState) {
    let (mut fresh, _sent) = on_p2(&[230; 32], "me-key", "snapshot");
    every_row(&mut fresh);
    // The recording socket's receiver is dropped: what a snapshot sends goes nowhere.
    *gs = fresh;
}

// ── Drawing headlessly ───────────────────────────────────────────────────────────────────────────

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

fn screen(w: f32, h: f32, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))), events, ..Default::default() }
}

/// One frame of the card's body, uncapped, in a tall panel (so every row is on screen).
fn body_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    let lengths = hh::load_lengths(std::path::Path::new("data"));
    ctx.run(screen(1000.0, 4000.0, Vec::new()), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw_body(ui, theme, state, &lengths));
    })
}

/// One frame of the Home page as the app draws it, with `events`.
fn home_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(screen(1000.0, 1100.0, events), |ctx| crate::gui::pages::homes::draw(ctx, theme, state))
}

/// Every text drawn, whole (a wrapped label is one text).
fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn visit(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut all);
    }
    all
}

/// The sentences in `want` that `out` does not draw exactly.
fn missing(out: &egui::FullOutput, want: &[String]) -> Vec<String> {
    let drawn = texts(out);
    want.iter().filter(|w| !drawn.contains(w)).cloned().collect()
}

/// Click the text `label` on the Home page: the canonical three frames (move, press, release).
fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, label: &str) {
    let out = home_frame(ctx, theme, state, Vec::new());
    let found = find_text_in_shapes(&out.shapes, label).unwrap_or_else(|| panic!("{label:?} is drawn: {:?}", texts(&out)));
    assert_eq!(found.text, label, "{label:?} is drawn as itself");
    let pos = found.rect.center();
    let m = egui::Modifiers::default();
    home_frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    home_frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    home_frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

/// Everything sent since the last look.
fn frames(sent: &Receiver<String>) -> Vec<Value> {
    sent.try_iter().map(|f| serde_json::from_str(&f).expect("a JSON frame")).collect()
}

/// The seed and key (Dilithium3, hex) of the person whose seed is `[n; 32]`.
fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

/// `key` (seed `seed`) is a real friend of `me`: a mutual follow, their DM key known, the pass
/// they gave me held (so a permit can be sealed and sent to them).
fn real_friend(gs: &mut GuiState, seed: &[u8], key: &str, name: &str, me: &str) {
    friend(gs, key, name, false);
    gs.peer_kyber_keys.insert(key.to_string(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap().public_base64());
    let pass = build_friend_cert(seed, SERVER, key, me, SERIAL, &FRIEND_PASS_DEFAULT_MAY).unwrap();
    store(gs).store_cert_from(key, &pass);
}

// ── The tests ────────────────────────────────────────────────────────────────────────────────────

/// THE CARD IN WORDS (plan 3.9 as amended by section 8): every row the card can show, each
/// exactly as written there. In the shared world on p2: the intro (a permit can be taken back at
/// any time), "Your plot on this server: p2.", a permit that stands with Renew for its length and
/// Take back, one on its way, one never answered (Give again), refused for each reason, one that
/// ran out (Give a new one), one being taken back, one taken back, a friend not online this run,
/// a friend with the lengths and Give a permit, a permit for a plot no longer mine, and a blocked
/// person; then the permits I hold, one for each thing the server can say, and one for a plot not
/// on this ship. Outside the shared world nothing is offered for giving, but a standing permit can
/// still be taken back. Locked, not connected, a guest, and nobody to list each say so.
/// Seen red 2026-10-12 with " You can take a permit back at any time." removed from INTRO: "the
/// card in the shared world draws every row of 3.9; missing:\nA household permit lets a friend
/// build on your plot on this server, ... You can take a permit back at any time." failed.
#[test]
fn the_household_card_in_words() {
    let (ctx, theme) = ctx_and_theme();
    let (mut gs, _sent) = on_p2(&[231; 32], "me-key", "words");
    let (bo, hal, ada, cal) = every_row(&mut gs);
    let date = hh::date_words;
    let out = body_frame(&ctx, &theme, &mut gs);
    let want: Vec<String> = [
        "A household permit lets a friend build on your plot on this server, and walk into your home, until the date you choose. Permits are kept on your devices; the server checks each one when it is used. You can take a permit back at any time.".to_string(),
        "Your plot".into(),
        "Your plot on this server: p2.".into(),
        format!("Can build on your plot until {} (29 days left).", date(bo)),
        "Renew for 30 days".into(),
        "Take back".into(),
        "Sending the permit...".into(),
        "The server did not say whether Di got the permit. Give it again to be sure.".into(),
        "Give again".into(),
        "Ed's settings do not take messages from you right now.".into(),
        "The server is busy; try again in a moment.".into(),
        "The server did not take it; try again.".into(),
        format!("Their permit ran out on {}.", date(hal)),
        "Give a new one".into(),
        "Taking it back...".into(),
        "You took their permit back.".into(),
        "Kim has not been online on this server since you started the app, so a permit cannot be sealed for them yet.".into(),
        "7 days".into(),
        "30 days".into(),
        "90 days".into(),
        "Give a permit".into(),
        "For plot p3, which is no longer yours here: it does not work any more.".into(),
        "You blocked Ned: that takes back every permit you gave them, and none can be given until you unblock them. What Ned built stays; you can take it down.".into(),
        "Plots you may build on".into(),
        format!("Ada's plot p1, until {} (29 days left).", date(ada)),
        "The server says it works: plot p1's door opens for you, and you can build there and take down what you build.".into(),
        "Checking it with the server...".into(),
        "It no longer works: Ben does not hold plot p4 any more.".into(),
        "It does not check out. Ask Ben for a new one.".into(),
        "Cal took this permit back.".into(),
        "Cal's plot p8.".into(),
        format!("Ran out on {}. Ask Cal for a new one.", date(cal)),
        "Plot p99 is not on this ship any more.".into(),
    ]
    .into_iter()
    .collect();
    let gone = missing(&out, &want);
    assert!(gone.is_empty(), "the card in the shared world draws every row of 3.9; missing:\n{}", gone.join("\n"));
    let drawn = texts(&out);
    let count = |t: &str| drawn.iter().filter(|d| d.as_str() == t).count();
    assert_eq!(count("Take back"), 3, "Take back beside each permit that might stand (Bo's, Cy's, Di's): {drawn:?}");
    assert_eq!(count("Give a permit"), 5, "Lee and Max have none, Ed, Fay and Gil were refused; Kim cannot be sealed for: {drawn:?}");

    // Outside the shared world: nothing offered for giving, a standing permit can still be taken back.
    gs.copresence_active = false;
    gs.household.my_plot = None;
    gs.household.verdicts.clear();
    let out = body_frame(&ctx, &theme, &mut gs);
    let want: Vec<String> = vec![
        "Step into this server's shared world to give permits: the server says which plot is yours when you arrive.".into(),
        "The server checks it when you step into its shared world.".into(),
        "Take back".into(),
    ];
    let gone = missing(&out, &want);
    assert!(gone.is_empty(), "outside the shared world; missing: {gone:?}");
    let drawn = texts(&out);
    for offered in ["Give a permit", "Renew for 30 days", "Give again", "Give a new one", "30 days"] {
        assert!(!drawn.iter().any(|d| d == offered), "outside the shared world {offered:?} is not offered");
    }
    assert!(!drawn.iter().any(|d| d == "Lee"), "a friend with nothing given is not listed outside a giving session");

    // Joining: in the shared world, the welcome not here yet. The card says so, not that the
    // player is a guest (it used to flash the guest line on every join). Seen red 2026-10-12 with
    // the JOINING arm taken out of draw_your_plot: "joining; missing: [Joining this server's
    // shared world...]".
    gs.copresence_active = true;
    gs.household.welcomed = false;
    let out = body_frame(&ctx, &theme, &mut gs);
    let gone = missing(&out, &["Joining this server's shared world...".into()]);
    assert!(gone.is_empty(), "joining; missing: {gone:?}");
    assert!(
        !texts(&out).iter().any(|t| t.starts_with("You have no plot")),
        "no guest line before the welcome"
    );

    // A guest: in the shared world with no plot of their own, the welcome in.
    gs.household.welcomed = true;
    let out = body_frame(&ctx, &theme, &mut gs);
    let gone = missing(&out, &["You have no plot on this ship (the server had none free), so there is nothing to give a permit for.".into()]);
    assert!(gone.is_empty(), "a guest; missing: {gone:?}");
    tidy(&gs);

    // Nobody to list, nothing held.
    let (mut empty, _sent) = on_p2(&[232; 32], "me-key", "words-empty");
    let out = body_frame(&ctx, &theme, &mut empty);
    let gone = missing(
        &out,
        &[
            "No friends here yet: people you follow who follow you back on this server are listed here.".into(),
            "Nobody has given you a permit on this server.".into(),
        ],
    );
    assert!(gone.is_empty(), "nobody to list; missing: {gone:?}");
    tidy(&empty);

    // Not connected, then locked.
    empty.ws_client = None;
    let out = body_frame(&ctx, &theme, &mut empty);
    assert!(missing(&out, &["Connect to a server to see its household permits.".into()]).is_empty(), "not connected");
    let mut locked = GuiState::default();
    let out = body_frame(&ctx, &theme, &mut locked);
    assert!(missing(&out, &["Unlock your identity to see your household permits.".into()]).is_empty(), "locked");
}

/// NO SECURITY WORDS ON THE CARD (plan 3.9): in every state the card can be in, both questions
/// open included, no word it draws is certificate, signature, key, token, revoke or verify (or a
/// form of one).
/// Seen red 2026-10-12 with "certificate" added to INTRO ("Permits are kept on your devices as a
/// certificate"): "the card says \"certificate\" in \"A household permit lets a friend build on
/// your plot on this server, ... Permits are kept on your devices as a certificate; ...\"" failed.
#[test]
fn no_security_words_on_the_card() {
    let (ctx, theme) = ctx_and_theme();
    let (mut gs, _sent) = on_p2(&[233; 32], "me-key", "words-security");
    every_row(&mut gs);
    let mut all = texts(&body_frame(&ctx, &theme, &mut gs));
    with_draft(|d| d.confirm = Some(Confirm::Give { key: "k-lee".into(), days: 30 }));
    let asked = texts(&body_frame(&ctx, &theme, &mut gs));
    assert!(asked.iter().any(|t| t.starts_with("Give Lee a household permit")), "the give question is on the card");
    all.extend(asked);
    with_draft(|d| d.confirm = Some(Confirm::TakeBack { key: "k-bo".into(), plot: "p2".into() }));
    let asked = texts(&body_frame(&ctx, &theme, &mut gs));
    assert!(asked.iter().any(|t| t.starts_with("Take back Bo's household permit")), "the take-back question is on the card");
    all.extend(asked);
    with_draft(|d| d.confirm = None);
    gs.copresence_active = false;
    all.extend(texts(&body_frame(&ctx, &theme, &mut gs)));
    gs.copresence_active = true;
    gs.household.my_plot = None;
    all.extend(texts(&body_frame(&ctx, &theme, &mut gs)));
    gs.ws_client = None;
    all.extend(texts(&body_frame(&ctx, &theme, &mut gs)));
    all.extend(texts(&body_frame(&ctx, &theme, &mut GuiState::default())));
    tidy(&gs);
    let mut said = Vec::new();
    for text in &all {
        for word in text.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase) {
            let bad = SECURITY_WORDS.contains(&word.as_str()) || SECURITY_STEMS.iter().any(|s| word.starts_with(s));
            if bad {
                said.push(format!("the card says {word:?} in {text:?}"));
            }
        }
    }
    assert!(said.is_empty(), "{}", said.join("\n"));
}

/// GIVE ASKS FIRST, AND THE QUESTION'S GIVE SENDS EXACTLY ONE PERMIT (plan 3.9, through the Home
/// page as the app draws it). Ada is on p2 with her friend Bo. A click on "Give a permit" sends
/// nothing and asks "Give Bo a household permit for your plot p2 until <30 days from now>? ..."
/// (the default length from the data file); its Give sends exactly one `dm_put`, to Bo, and the
/// row then says the permit is on its way. Nothing more goes on later frames.
/// Seen red 2026-10-12 with "Give a permit" giving at once (`Act::Give` instead of
/// `Act::AskGive`): "a click on Give a permit only asks: it sent [Object {\"content\":
/// String(\"{\\\"ct_b64\\\": ..." (the permit's `dm_put`) failed.
#[test]
fn give_asks_first_and_the_confirm_gives() {
    crate::config::keep_saves_off_disk();
    let (ctx, theme) = ctx_and_theme();
    let (ada_seed, ada) = identity(234);
    let (bo_seed, bo) = identity(235);
    let (mut gs, sent) = on_p2(&ada_seed, &ada, "give-asks");
    real_friend(&mut gs, &bo_seed, &bo, "Bo", &ada);

    click(&ctx, &theme, &mut gs, "Give a permit");
    let out = home_frame(&ctx, &theme, &mut gs, Vec::new());
    let early = frames(&sent);
    assert!(early.is_empty(), "a click on Give a permit only asks: it sent {early:?}");
    let now = now_s();
    let asked: Vec<String> = [now.saturating_sub(5), now + 5]
        .iter()
        .map(|t| format!("Give Bo a household permit for your plot p2 until {}? Bo can build there and take down only what Bo builds. You can take down anything on your plot. You can take it back at any time.", hh::date_words(t + 30 * DAY)))
        .collect();
    let drawn = texts(&out);
    assert!(asked.iter().any(|q| drawn.contains(q)), "the dated question is asked: {drawn:?}");

    click(&ctx, &theme, &mut gs, "Give");
    home_frame(&ctx, &theme, &mut gs, Vec::new());
    let out = home_frame(&ctx, &theme, &mut gs, Vec::new());
    let sent_now = frames(&sent);
    assert_eq!(sent_now.len(), 1, "exactly one thing sent: {sent_now:?}");
    assert_eq!((sent_now[0]["type"].as_str(), sent_now[0]["to"].as_str()), (Some("dm_put"), Some(bo.as_str())), "a permit, to Bo");
    assert!(texts(&out).iter().any(|t| t == "Sending the permit..."), "the row says it is on its way");
    assert!(!texts(&out).iter().any(|t| t.starts_with("Give Bo a household permit")), "the question is closed");
    tidy(&gs);
}

/// TAKE BACK ASKS FIRST (amendment A1). Ada gave Bo a permit for p2 that the server took. A click on
/// the row's Take back sends nothing and changes nothing; it asks "Take back Bo's household permit
/// for plot p2? Bo can no longer build there or walk in. What Bo built stays; you can take it
/// down.". Cancel closes the question, still sending nothing. Take back, then the question's Take
/// back, calls the engine once: exactly one `cert_revoke` naming the permit, one note to Ada's
/// own devices, the row taken back, and the row says "Taking it back..." until the server confirms.
/// Seen red 2026-10-12 with the row's Take back taking back at once (`Act::TakeBack` instead of
/// `Act::AskTakeBack`): "a click on Take back only asks: it sent [Object {\"content\":
/// String(\"{\\\"ct_b64\\\": ..." (the note to Ada's own devices, then the `cert_revoke`) failed.
#[test]
fn take_back_asks_first() {
    crate::config::keep_saves_off_disk();
    let (ctx, theme) = ctx_and_theme();
    let (ada_seed, ada) = identity(236);
    let (bo_seed, bo) = identity(237);
    let (mut gs, sent) = on_p2(&ada_seed, &ada, "take-back-asks");
    real_friend(&mut gs, &bo_seed, &bo, "Bo", &ada);
    let now = now_s();
    gave(&mut gs, &bo, "p2", SERIAL, now - DAY, now + 29 * DAY, GivenState::Taken);
    let state = |gs: &GuiState| gs.dm_store.as_ref().unwrap().permits_given()[0].state.clone();
    let question = "Take back Bo's household permit for plot p2? Bo can no longer build there or walk in. What Bo built stays; you can take it down.";

    click(&ctx, &theme, &mut gs, "Take back");
    let out = home_frame(&ctx, &theme, &mut gs, Vec::new());
    let early = frames(&sent);
    assert!(early.is_empty(), "a click on Take back only asks: it sent {early:?}");
    assert_eq!(state(&gs), GivenState::Taken, "and takes nothing back");
    assert!(texts(&out).iter().any(|t| t == question), "it asks: {:?}", texts(&out));

    click(&ctx, &theme, &mut gs, "Cancel");
    let out = home_frame(&ctx, &theme, &mut gs, Vec::new());
    assert!(frames(&sent).is_empty(), "Cancel sends nothing");
    assert_eq!(state(&gs), GivenState::Taken, "Cancel takes nothing back");
    assert!(!texts(&out).iter().any(|t| t == question), "Cancel closes the question");

    click(&ctx, &theme, &mut gs, "Take back");
    click(&ctx, &theme, &mut gs, "Take back");
    home_frame(&ctx, &theme, &mut gs, Vec::new());
    let out = home_frame(&ctx, &theme, &mut gs, Vec::new());
    let sent_now = frames(&sent);
    let revokes: Vec<&Value> = sent_now.iter().filter(|v| v["type"] == "cert_revoke").collect();
    let notes: Vec<&Value> = sent_now.iter().filter(|v| v["type"] == "dm_put" && v["to"] == ada.as_str()).collect();
    assert_eq!((revokes.len(), notes.len(), sent_now.len()), (1, 1, 2), "the engine was called once: {sent_now:?}");
    assert_eq!(revokes[0]["serial"], SERIAL, "naming the permit");
    assert_eq!(state(&gs), GivenState::TakenBack);
    assert!(texts(&out).iter().any(|t| t == "Taking it back..."), "the row says it is being taken back: {:?}", texts(&out));
    assert!(!texts(&out).iter().any(|t| t == question), "the question is closed");
    tidy(&gs);
}

/// THE LENGTHS COME FROM THE DATA FILE (plan 3.2 and 3.9): with a data folder whose
/// ship/household.ron offers 3 and 14 days, 14 picked first, the card offers exactly those two,
/// and Give a permit asks about a permit running 14 days from now.
/// Seen red 2026-10-12 with `draw_card_from` reading the lengths from `crate::data_dir()` instead of
/// its own folder: "the card offers the data file's lengths: missing [\"3 days\", \"14 days\"]"
/// failed.
#[test]
fn the_lengths_come_from_the_data_file() {
    let (ctx, theme) = ctx_and_theme();
    let dir = crate::test_temp::dir("household_page_lengths");
    std::fs::create_dir_all(dir.join("ship")).unwrap();
    std::fs::write(dir.join(hh::LENGTHS_FILE), "(lengths_days: [3, 14], default_days: 14, ended_kept_days: 30)").unwrap();
    let (mut gs, sent) = on_p2(&[238; 32], "me-key", "lengths");
    friend(&mut gs, "k-lee", "Lee", true);
    let card = |ctx: &egui::Context, gs: &mut GuiState, events: Vec<egui::Event>| {
        ctx.run(screen(1000.0, 1100.0, events), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_card_from(ui, &theme, gs, &dir));
        })
    };
    let out = card(&ctx, &mut gs, Vec::new());
    let gone = missing(&out, &["3 days".into(), "14 days".into()]);
    assert!(gone.is_empty(), "the card offers the data file's lengths: missing {gone:?}");
    let drawn = texts(&out);
    for not in ["7 days", "30 days", "90 days"] {
        assert!(!drawn.iter().any(|d| d == not), "and nothing else: {not:?} is offered");
    }
    let pos = find_text_in_shapes(&out.shapes, "Give a permit").expect("Give a permit").rect.center();
    let m = egui::Modifiers::default();
    card(&ctx, &mut gs, vec![egui::Event::PointerMoved(pos)]);
    card(&ctx, &mut gs, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    card(&ctx, &mut gs, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    let out = card(&ctx, &mut gs, Vec::new());
    let now = now_s();
    let starts: Vec<String> = [now.saturating_sub(5), now + 5].iter().map(|t| format!("Give Lee a household permit for your plot p2 until {}?", hh::date_words(t + 14 * DAY))).collect();
    assert!(
        texts(&out).iter().any(|t| starts.iter().any(|s| t.starts_with(s.as_str()))),
        "the question is about 14 days, the file's default: {:?}",
        texts(&out)
    );
    assert!(frames(&sent).is_empty(), "nothing sent before the question is answered");
    tidy(&gs);
}

/// THE CARD HEADS THE HOME PAGE, WITH OR WITHOUT A DESIGN LOADED (plan 3.9): the Household card is
/// drawn above the page's title both when no homestead blueprint is loaded and when one is.
/// Seen red 2026-10-12 with the call moved below homes.rs's no-design return: "with no design
/// loaded the card is drawn: [\"Home\", \"No homestead blueprint loaded (expected
/// data/blueprints/fibonacci_homestead.ron).\"]" failed.
#[test]
fn the_card_heads_the_home_page_with_or_without_a_design() {
    let (ctx, theme) = ctx_and_theme();
    let mut gs = GuiState::default();
    let out = home_frame(&ctx, &theme, &mut gs, Vec::new());
    let card = find_text_in_shapes(&out.shapes, "Household");
    assert!(card.is_some(), "with no design loaded the card is drawn: {:?}", texts(&out));
    let title = find_text_in_shapes(&out.shapes, "Home").expect("the page's title");
    assert!(card.unwrap().rect.top() < title.rect.top(), "above the title");
    assert!(texts(&out).iter().any(|t| t == "Unlock your identity to see your household permits."), "and says what it needs");

    gs.homestead_design = crate::gui::load_homestead_design(std::path::Path::new("data"));
    assert!(gs.homestead_design.is_some(), "the design loads");
    let out = home_frame(&ctx, &theme, &mut gs, Vec::new());
    let card = find_text_in_shapes(&out.shapes, "Household").expect("with a design loaded the card is drawn");
    let title = find_text_in_shapes(&out.shapes, "Your Home").expect("the design's title");
    assert!(card.rect.top() < title.rect.top(), "above the design");
}

/// TWO LINES FROM THE 2026-10-12 REVIEW. A permit held for the plot that is now my own here is
/// never asked about (engine/household.rs `tick` skips my own plot), so it read "Checking it with
/// the server..." for ever: it says it is my own plot now. And under the protected setup the card
/// offers no Give at all (it offered Give and refused only after the press): it says why instead,
/// and Take back stays.
/// Seen red 2026-10-12 with the own-plot arm taken out of `held_line`: "own plot; missing: [That
/// plot is your own on this ship now, so you need no permit for it.]"; and with `giving` ignoring
/// the setup: "protected: Give a permit is offered".
#[test]
fn a_permit_for_my_own_plot_and_the_protected_setup_in_words() {
    let (ctx, theme) = ctx_and_theme();
    let (mut gs, _sent) = on_p2(&[234; 32], "me-key", "own-and-protected");
    held(&mut gs, "k-ada", "p2", 4_000_000_000, None);
    named(&mut gs, "k-ada", "Ada");
    let out = body_frame(&ctx, &theme, &mut gs);
    let gone = missing(&out, &[YOUR_OWN_PLOT.into()]);
    assert!(gone.is_empty(), "own plot; missing: {gone:?}");
    assert!(!texts(&out).iter().any(|t| t == CHECKING), "no Checking line for my own plot");

    friend(&mut gs, "k-lee", "Lee", true);
    assert!(texts(&body_frame(&ctx, &theme, &mut gs)).iter().any(|t| t == "Give a permit"), "Give is offered before the setup");
    gs.protected.setup.on = true;
    let out = body_frame(&ctx, &theme, &mut gs);
    let drawn = texts(&out);
    assert!(!drawn.iter().any(|t| t == "Give a permit"), "protected: Give a permit is offered");
    assert!(missing(&out, &[crate::engine::household::PROTECTED.into()]).is_empty(), "protected: the reason is on the card");
    tidy(&gs);
}
