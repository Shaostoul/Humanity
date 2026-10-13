//! Headless tests of "Report a copyright problem", the admins' copyright reports and the public
//! takedown log on the desktop app (takedown.rs, 2026-10-13). The Media page is drawn into an egui
//! context with no window and clicked the way a person clicks it (ui-system.md's headless pattern);
//! requests go to a closed port on this machine, or to a small stand-in server on the loopback
//! address that answers by route and keeps every request it was sent. Each test was seen to fail
//! once against the break its comment names, with the words it failed with.

use super::*;
use crate::gui::pages::media::item_view::item_tests::{answer_of, click_top, every_exact, frame, rich_item, serve_routes, until};
use crate::gui::pages::media::tests::{ctx_and_theme, settled, shows, SERVER};
use crate::gui::pages::media::{apply_list, item_view, sync_server, MediaTab};
use crate::gui::ChatUser;
use std::time::{Duration, Instant};

/// An app on the server at `base` with the Media page's first answer in (one item, "Dusk", number
/// 7), signed OUT: no upload token, no identity unlocked. Nothing a test does writes config.json.
fn signed_out_on(base: &str) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut state = GuiState::default();
    state.server_url = base.to_string();
    sync_server(&mut state);
    state.media.requested = true;
    apply_list(&mut state.media, answer_of(vec![rich_item(("k2", "Ann"))]), false);
    state
}

/// The seed every signing test unlocks, and the Dilithium key the relay would hold for it.
fn seed_and_key() -> (Vec<u8>, String) {
    let seed = vec![7u8; 32];
    let key = crate::net::identity::derive_pq_identity(&seed).expect("derive").dilithium_hex;
    (seed, key)
}

/// An app signed in to the server at `base` as `role` (its member list says so), with an unlocked
/// identity whose key the relay can check signatures against.
fn signed_in_on(base: &str, role: &str) -> GuiState {
    crate::config::keep_saves_off_disk();
    let (seed, key) = seed_and_key();
    let mut state = GuiState::default();
    state.server_url = base.to_string();
    state.profile_public_key = key.clone();
    sync_server(&mut state);
    state.media.requested = true;
    apply_list(&mut state.media, answer_of(vec![rich_item(("k2", "Ann"))]), false);
    state.upload_token = Some((base.to_string(), "tok".to_string()));
    state.media.token_mark = blake3::hash(b"tok").to_hex()[..16].to_string();
    state.private_key_bytes = Some(seed);
    state.chat_users.push(ChatUser { name: "Me".into(), public_key: key, role: role.into(), status: String::new() });
    // The admins' categories, suggestions and meter count as asked: these tests are about the
    // copyright reports, and nothing else goes to the stand-in server.
    state.media.admin.requested = true;
    state
}

/// The requests the stand-in server was sent that start with `prefix` ("POST /path "), once at
/// least one has come (ten seconds at most), drawing frames meanwhile so answers are read.
fn wait_for(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, seen: &std::sync::Arc<std::sync::Mutex<Vec<String>>>, prefix: &str) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got: Vec<String> = seen.lock().unwrap().iter().filter(|r| r.starts_with(prefix)).cloned().collect();
        if !got.is_empty() {
            return got;
        }
        assert!(Instant::now() < deadline, "the stand-in server was sent {prefix:?}: {:?}", seen.lock().unwrap());
        frame(ctx, theme, state, Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// THE FORM SENDS THE EXACT BODY, AND SEND WAITS FOR THE BOX: "Report a copyright problem" on an
/// item's page opens the form about that item, for someone signed OUT; Send does nothing at all
/// until the good-faith box (the relay's own words beside it) is ticked; then it sends exactly
/// `{item, name, contact, statement, good_faith: true}`, the words trimmed, unsigned, which the
/// relay's own reader takes; and the relay's words are shown once it went.
///
/// Seen red 2026-10-13 with Send left on before the box was ticked (`.disabled(f.sending)` alone):
/// "Send is off until the box is ticked: a press does nothing at all" (left: "Tick the box first."
/// / right: "").
#[test]
fn the_form_sends_the_exact_body_and_send_waits_for_the_box() {
    let (ctx, theme) = ctx_and_theme();
    let (base, seen) = serve_routes(vec![("/api/media/takedown", r#"{"ok":true,"id":3,"words":"Received from the stand-in."}"#)]);
    let mut state = signed_out_on(&base);
    assert!(!crate::gui::pages::media::signed_in_here(&state) && state.private_key_bytes.is_none(), "signed out, no identity");
    let item = state.media.list.items[0].clone();
    item_view::open(&mut state, &item);

    click_top(&ctx, &theme, &mut state, BUTTON);
    let f = state.media.takedown.form.clone().expect("the form opens");
    assert_eq!((f.item, f.fixed), (Some(7), true), "about the item whose page it was opened on");
    let out = settled(&ctx, &theme, &mut state);
    for text in [NEXT, GOOD_FAITH_LINE, "About: Item 7, \"Dusk\"", NAME_LABEL, CONTACT_LABEL, STATEMENT_LABEL] {
        assert!(shows(&out, text), "the form shows {text:?}");
    }
    assert_eq!(GOOD_FAITH_LINE, crate::relay::media_takedown::GOOD_FAITH_LINE, "the relay's own words beside the box");

    // What a person typed (the ends trimmed when sent).
    if let Some(f) = state.media.takedown.form.as_mut() {
        f.name = "  Ann Example ".into();
        f.contact = " ann@example.org".into();
        f.statement = "I filmed it at the shore in June and first posted it on my own site.\n".into();
    }
    click_top(&ctx, &theme, &mut state, SEND);
    assert!(!state.media.jobs.iter().any(|(j, _)| matches!(j, Job::TakedownSend { .. })), "nothing is sent before the box is ticked");
    assert_eq!(state.media.takedown.form.as_ref().map(|f| f.error.as_str()), Some(""), "Send is off until the box is ticked: a press does nothing at all");

    click_top(&ctx, &theme, &mut state, GOOD_FAITH_LINE);
    assert!(state.media.takedown.form.as_ref().is_some_and(|f| f.good_faith), "the box is ticked");
    click_top(&ctx, &theme, &mut state, SEND);
    until(&ctx, &theme, &mut state, "the report went", |s| s.media.takedown.form.as_ref().is_some_and(|f| !f.done.is_empty()));

    let sent = wait_for(&ctx, &theme, &mut state, &seen, "POST /api/media/takedown ");
    assert_eq!(sent.len(), 1, "one report: {sent:?}");
    let text = sent[0].strip_prefix("POST /api/media/takedown ").unwrap();
    let body: Value = serde_json::from_str(text).expect("the body is JSON");
    assert_eq!(
        body,
        json!({
            "item": 7,
            "name": "Ann Example",
            "contact": "ann@example.org",
            "statement": "I filmed it at the shore in June and first posted it on my own site.",
            "good_faith": true,
        }),
        "exactly the form's body: no key, no signature, no token"
    );
    let read = crate::relay::media_takedown::read_form(text).expect("the relay's own reader takes it");
    assert_eq!((read.item, read.name.as_str()), (7, "Ann Example"));
    assert!(shows(&settled(&ctx, &theme, &mut state), "Received from the stand-in."), "the relay's words are shown");
}

/// AN ADMIN'S REMOVE SENDS THE SIGNED DECIDE WITH THE RIGHT PURPOSE: the Admin tab reads the open
/// reports (signed `media_takedowns`) and shows what the person wrote; Remove item asks first; Yes,
/// remove sends `{key, timestamp, sig, id, state: "removed", note}` whose signature the relay's own
/// check accepts over its own purpose, `media_takedown:5:removed`, and which its body type reads.
///
/// Seen red 2026-10-13 with `decide_purpose` signing `media_takedowns:<id>:<removed|kept>` (the
/// open list's word): "the relay accepts the decision's signature over media_takedown:5:removed".
#[test]
fn an_admins_remove_sends_the_signed_decide_with_the_right_purpose() {
    let (ctx, theme) = ctx_and_theme();
    let open = r#"{"requests":[{"id":5,"item_id":12,"item_title":"Dusk","item_public":true,"item_on_page":true,"created_at":1760313600000,"name":"Ann Example","contact":"ann@example.org","statement":"I filmed it at the shore.","state":"open"}]}"#;
    let decided = r#"{"ok":true,"id":5,"outcome":"removed","item_id":12,"item_removed":true,"item_already_gone":false}"#;
    let (base, seen) = serve_routes(vec![("/api/media/takedowns/open", open), ("/api/media/takedown/decide", decided)]);
    let mut state = signed_in_on(&base, "admin");
    state.media.tab = MediaTab::Admin;

    until(&ctx, &theme, &mut state, "the open reports arrive", |s| s.media.takedown.open.as_ref().is_some_and(|o| o.len() == 1));
    let out = settled(&ctx, &theme, &mut state);
    for text in [ADMIN_HEADING, "Report #5 about Item 12, \"Dusk\"", "Received 2025-10-13", "Name: Ann Example", "Contact: ann@example.org", "I filmed it at the shore."] {
        assert!(shows(&out, text), "the Admin tab shows {text:?}");
    }
    // The read was signed over the relay's purpose for it.
    let reads = wait_for(&ctx, &theme, &mut state, &seen, "POST /api/media/takedowns/open ");
    let read: Value = serde_json::from_str(reads[0].strip_prefix("POST /api/media/takedowns/open ").unwrap()).unwrap();
    assert!(
        crate::relay::handlers::broadcast::verify_dilithium_signature(
            read["key"].as_str().unwrap_or(""),
            crate::relay::media_takedown::OPEN_PURPOSE,
            read["timestamp"].as_u64().unwrap_or(0),
            read["sig"].as_str().unwrap_or("")
        ),
        "the relay accepts the open list's signature"
    );

    state.media.takedown.notes.insert(5, "  The uploader agreed it was not theirs. ".into());
    click_top(&ctx, &theme, &mut state, REMOVE_ITEM);
    assert!(!state.media.jobs.iter().any(|(j, _)| matches!(j, Job::TakedownDecide { .. })), "Remove item asks first");
    assert!(shows(&settled(&ctx, &theme, &mut state), REMOVE_CONFIRM), "it asks");
    click_top(&ctx, &theme, &mut state, YES_REMOVE);

    let sent = wait_for(&ctx, &theme, &mut state, &seen, "POST /api/media/takedown/decide ");
    let text = sent[0].strip_prefix("POST /api/media/takedown/decide ").unwrap();
    let body: Value = serde_json::from_str(text).expect("the body is JSON");
    assert_eq!((body["id"].clone(), body["state"].clone(), body["note"].clone()), (json!(5), json!("removed"), json!("The uploader agreed it was not theirs.")));
    let relay_purpose = crate::relay::media_takedown::decide_purpose(5, crate::relay::media_takedown::Outcome::Removed);
    assert_eq!(relay_purpose, "media_takedown:5:removed");
    assert!(
        crate::relay::handlers::broadcast::verify_dilithium_signature(
            body["key"].as_str().unwrap_or(""),
            &relay_purpose,
            body["timestamp"].as_u64().unwrap_or(0),
            body["sig"].as_str().unwrap_or("")
        ),
        "the relay accepts the decision's signature over media_takedown:5:removed"
    );
    let parsed: crate::relay::media_takedown::DecideBody = serde_json::from_str(text).expect("the relay's body type reads it");
    assert_eq!((parsed.id, parsed.state.as_str()), (5, "removed"));
    until(&ctx, &theme, &mut state, "the decision is said", |s| s.media.takedown.status.contains("Report #5: the item was removed."));
}

/// THE LOG WINDOW NEVER SHOWS A NAME, A CONTACT OR A STATEMENT: the Media page's foot opens the
/// public log for anyone, signed out; it shows each decision's outcome, item, dates and note, and
/// even when a server sends who reported (the real route never does), none of it is drawn.
///
/// Seen red 2026-10-13 with `parse_log` putting the reporter's `name` into the row's note: "the log
/// never shows \"Secret Name\"". (Its first run timed out instead: the foot's log link was an `egui`
/// link, whose text in a wrapping row is laid out from the row's start, so the middle of its text's
/// box lay over the button beside it and the test's click opened the form; it is a ghost button
/// now, as the house's inline links are, with a hit area of its own.)
#[test]
fn the_log_window_never_shows_a_name_contact_or_statement() {
    let (ctx, theme) = ctx_and_theme();
    let log = r#"{"takedowns":[{"id":3,"item_id":12,"item_title":"Dusk","created_at":1760313600000,"decided_at":1760400000000,"outcome":"removed","note":"The uploader agreed it was not theirs.","name":"Secret Name","contact":"secret@example.org","statement":"Secret statement","decided_by":"adminkey"},{"id":2,"item_id":9,"item_title":"","created_at":1760313600000,"decided_at":1760313700000,"outcome":"kept","note":""}]}"#;
    let (base, _) = serve_routes(vec![("/api/media/takedowns", log)]);
    let mut state = signed_out_on(&base);

    click_top(&ctx, &theme, &mut state, LOG_LINK);
    assert!(state.media.takedown.log.is_some(), "the foot's link opens the log window");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.media.takedown.log.as_ref().is_some_and(|l| l.rows.is_some()) {
        assert!(Instant::now() < deadline, "the log arrives (log: {:?}, jobs: {:?})", state.media.takedown.log, state.media.jobs.iter().map(|(j, _)| j).collect::<Vec<_>>());
        frame(&ctx, &theme, &mut state, Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
    let out = settled(&ctx, &theme, &mut state);
    for text in [LOG_LEAD, "Item 12, \"Dusk\"", "Reported 2025-10-13, decided 2025-10-14", "The uploader agreed it was not theirs.", "Item 9"] {
        assert!(shows(&out, text), "the log shows {text:?}");
    }
    assert!(!every_exact(&out, REMOVED).is_empty() && !every_exact(&out, KEPT).is_empty(), "each decision's outcome");
    for text in ["Secret Name", "secret@example.org", "Secret statement", "adminkey", "Name:", "Contact:"] {
        assert!(!shows(&out, text), "the log never shows {text:?}");
    }
}

/// THE ADMIN TAB'S COPYRIGHT SECTION IS FOR ADMINS ONLY: a member never sees the reports or asks
/// for them, whether through the page (which gives them no Admin tab) or the section itself; an
/// admin sees the section and it asks for the open reports.
///
/// Seen red 2026-10-13 with the section's own admin check taken out of `draw_admin_section`: "the
/// section draws nothing for a member".
#[test]
fn the_admin_tabs_copyright_section_is_for_admins_only() {
    let (ctx, theme) = ctx_and_theme();
    let draw_section = |ctx: &egui::Context, state: &mut GuiState| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1100.0, 1600.0))), ..Default::default() };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| crate::gui::pages::media::admin::draw_admin(ui, &theme, state));
        })
    };

    // A member, signed in with an identity (so a section that forgot to check could ask).
    let mut member = signed_in_on(SERVER, "member");
    member.media.tab = MediaTab::Admin;
    let out = settled(&ctx, &theme, &mut member);
    assert!(!shows(&out, ADMIN_HEADING), "the page gives a member no copyright reports");
    let out = draw_section(&ctx, &mut member);
    assert!(!shows(&out, ADMIN_HEADING), "the section draws nothing for a member");
    assert!(!member.media.takedown.requested && !member.media.jobs.iter().any(|(j, _)| *j == Job::TakedownOpen), "a member's section asks for nothing");

    // An admin: the section, and the open reports asked for.
    let mut admin = signed_in_on(SERVER, "admin");
    let out = draw_section(&ctx, &mut admin);
    assert!(shows(&out, ADMIN_HEADING) && shows(&out, ADMIN_LEAD), "an admin sees the section");
    assert!(admin.media.takedown.requested, "and it asks for the open reports");
}

/// THE FOOT'S FORM READS THE ITEM'S NUMBER FROM WHAT WAS TYPED, as the website's does: "12", "#12",
/// "/media/12" or a whole address ending in /media/12, with a query or fragment after it.
///
/// Seen red 2026-10-13 with the query and fragment left on the address: "an address with a query"
/// (left: None / right: Some(12)).
#[test]
fn the_item_number_is_read_from_what_was_typed() {
    assert_eq!(item_number_from("12"), Some(12));
    assert_eq!(item_number_from(" #12 "), Some(12));
    assert_eq!(item_number_from("/media/12"), Some(12));
    assert_eq!(item_number_from("https://united-humanity.us/media/12/"), Some(12));
    assert_eq!(item_number_from("united-humanity.us/media/12?from=chat"), Some(12), "an address with a query");
    assert_eq!(item_number_from("united-humanity.us/media/12#top"), Some(12));
    for nothing in ["", "0", "twelve", "/media/", "/media/12x", "1234567890123456"] {
        assert_eq!(item_number_from(nothing), None, "{nothing:?} names no item");
    }
}
