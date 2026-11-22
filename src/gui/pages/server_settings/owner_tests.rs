// Child of server_settings/owner.rs (#[path]): the owner's controls, driven through real headless
// egui frames and clicks (the way server_settings/held_tests.rs drives Held copies). Each test was
// seen red once on purpose, recorded at the test.

use super::*;
use crate::engine::owner::tests::{owner_app, sent_frames, serve, Fake};
use crate::gui::screen_surface::find_text_in_shapes;
use crate::net::held::tests::submission_object;
use crate::net::owner::tests::{card_of, key_of, payload_text};
use std::sync::{Arc, Mutex};

/// A page driver: one context, the app's theme, a screen big enough for the whole section.
struct Page {
    ctx: egui::Context,
    theme: Theme,
}

type Draw = fn(&mut egui::Ui, &Theme, &mut GuiState);

impl Page {
    fn new() -> Self {
        crate::config::keep_saves_off_disk();
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        Page { ctx, theme }
    }

    fn run(&self, state: &mut GuiState, events: Vec<egui::Event>, what: Draw) -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 2400.0))), events, ..Default::default() };
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| what(ui, &self.theme, state));
            });
        })
    }

    /// Two frames (the first lays out), then the second's shapes.
    fn draw(&self, state: &mut GuiState, what: Draw) -> egui::FullOutput {
        self.run(state, Vec::new(), what);
        self.run(state, Vec::new(), what)
    }

    /// Click the text `label` (exactly) where it was last drawn.
    fn click(&self, state: &mut GuiState, label: &str, what: Draw) {
        let out = self.draw(state, what);
        let found = find_text_in_shapes(&out.shapes, label).filter(|f| f.text == label).unwrap_or_else(|| panic!("no {label:?} drawn"));
        let at = found.rect.center();
        let m = egui::Modifiers::default();
        self.run(state, vec![egui::Event::PointerMoved(at)], what);
        self.run(state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], what);
        self.run(state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], what);
    }

    /// Draw until `done` (the page polls its jobs as it draws), at most 20 seconds.
    fn draw_until(&self, state: &mut GuiState, what: Draw, done: impl Fn(&GuiState) -> bool) {
        let start = std::time::Instant::now();
        while !done(state) && start.elapsed() < std::time::Duration::from_secs(20) {
            self.run(state, Vec::new(), what);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

fn exact(out: &egui::FullOutput, label: &str) -> bool {
    find_text_in_shapes(&out.shapes, label).is_some_and(|f| f.text == label)
}
fn shows(out: &egui::FullOutput, words: &str) -> bool {
    find_text_in_shapes(&out.shapes, words).is_some()
}

fn claim_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    draw_claim(ui, theme, state);
}
fn owner_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    draw_owner(ui, theme, state);
}
/// A member's label in the picker: the name alone (no fingerprint is drawn beside it: the fifth
/// review's H1; `seed` names whose).
fn member(name: &str, _seed: u8) -> String {
    name.to_string()
}
/// The groups of `key`'s fingerprint found anywhere in the words a frame drew, each as a token of
/// its own (split at anything that is not a hex digit): the owner's page must draw none before the
/// groups typed matched (the fifth review's H1 (b)). The keys are fixed, so a group that happened
/// to equal some other token on the page would fail the same way every run, never by chance.
fn groups_drawn(out: &egui::FullOutput, key: &str) -> Vec<String> {
    let words = crate::gui::pages::settings::own_fingerprint::texts(out);
    let fp = humanity_protocol::key_card::fingerprint(key);
    fp.split(' ')
        .filter(|g| words.iter().any(|w| w.split(|c: char| !c.is_ascii_hexdigit()).any(|tok| tok.eq_ignore_ascii_case(g))))
        .map(str::to_string)
        .collect()
}

/// The four groups asked, as the person `key_of(seed)` reads them from their OWN screen (Settings >
/// Account drawn for them; the fifth review's H1 (d)).
fn asked_groups(gs: &GuiState, seed: u8) -> String {
    crate::engine::owner::tests::read_out(gs, &key_of(seed))
}
/// The words of `key` filled with `name` and the four positions asked (1-based).
fn asked_words(gs: &GuiState, key: &'static str, name: &str) -> String {
    let [a, b, c, d] = humanity_protocol::key_card::asked_words(gs.reports.owner.asked.expect("four groups asked"));
    tf(key, &[("name", &name), ("a", &a), ("b", &b), ("c", &c), ("d", &d)])
}
/// The words asking for `name`'s four groups, at the positions asked.
fn fp_needed(gs: &GuiState, name: &str) -> String {
    crate::engine::owner::fp_needed(name, Some(gs.reports.owner.asked.expect("four groups asked")))
}


fn held_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    super::super::held::draw(ui, theme, state);
}

/// A held_items frame: one item, for the owner or an admin, with a successor or none.
pub(crate) fn held_list(owner: bool, successor: Option<(&str, &str)>) -> serde_json::Value {
    serde_json::json!({ "type": "held_items", "can_keep": true, "cannot_keep_why": "", "i_am_owner": owner,
        "items": [{ "id": 1, "report_id": 5, "file_count": 1, "parts": 2, "sealed_bytes": 4096, "held_at": 1_763_600_000_000i64, "held_by_name": "Ada",
                    "delete_after": 1_795_136_000_000i64, "legal_hold_until": 0, "deleted_at": 1_795_136_000_000i64, "exported_count": 0, "sealed_to_owner": true, "sealed_to_successor": true },
                  { "id": 2, "report_id": 6, "file_count": 1, "parts": 2, "sealed_bytes": 4096, "held_at": 1_763_600_000_000i64, "held_by_name": "Ada",
                    "delete_after": 1_795_136_000_000i64, "legal_hold_until": 0, "deleted_at": 1_795_136_000_000i64, "exported_count": 0, "sealed_to_owner": true, "sealed_to_successor": false }],
        "actions": [], "counts": { "held": 2, "exported": 0, "legal_holds": 0, "destroyed": 0, "restored": 0 },
        "successor": successor.map(|(k, n)| serde_json::json!({ "key": k, "name": n })),
        "owner_log": [{ "kind": "owner_successor", "old_name": "Ada", "new_name": "Cy", "reason": "just in case", "at": 1_763_700_000_000i64, "object": "{}" },
                      { "kind": "claim", "old_name": "", "new_name": "Ada", "reason": "", "at": 1_763_500_000_000i64, "object": "" }] })
}

/// CLAIM OWNERSHIP SHOWS ONLY WHILE THE SERVER SAYS IT HAS NO OWNER, AND SENDS THE PASTED CODE:
/// nothing before the server has said, nothing once it has an owner, nothing while not signed in;
/// with none it says where the code comes from (the server's log, data/owner-claim-code.txt on the
/// server machine) and that the key card must have reached the server; Claim with no code sends
/// nothing and says so; with a code it sends `owner_claim` with the code trimmed; the server's
/// answer is shown; once it worked the control is gone and the server's words stay.
///
/// Seen red 2026-11-20 with `draw_claim` not looking at `has_owner`: "no Claim before the server
/// has said" failed (the control was drawn for a server that never said it had no owner).
#[test]
fn claim_shows_only_without_an_owner_and_sends_the_code() {
    let page = Page::new();
    let (mut gs, sent) = owner_app("owner_claim_gui");
    for u in gs.chat_users.iter_mut() {
        u.role = "member".into();
    }
    let out = page.draw(&mut gs, claim_page);
    assert!(!exact(&out, t("owner.claim_button")), "no Claim before the server has said");
    gs.reports.owner.has_owner = Some(true);
    assert!(!exact(&page.draw(&mut gs, claim_page), t("owner.claim_button")), "no Claim on a server with an owner");
    assert!(crate::engine::owner::on_frame(&mut gs, &serde_json::json!({ "type": "owner_state", "has_owner": false })));
    gs.ws_identified = false;
    assert!(!exact(&page.draw(&mut gs, claim_page), t("owner.claim_button")), "no Claim while not signed in");
    gs.ws_identified = true;
    let out = page.draw(&mut gs, claim_page);
    assert!(exact(&out, t("owner.claim_button")), "Claim, for anyone signed in, while there is no owner");
    assert!(shows(&out, t("owner.claim_where")), "where the code comes from");
    let where_words = t("owner.claim_where");
    assert!(where_words.contains("log") && where_words.contains("data/owner-claim-code.txt") && where_words.contains("key card"), "{where_words}");

    let _ = sent_frames(&sent);
    page.click(&mut gs, t("owner.claim_button"), claim_page);
    assert!(sent_frames(&sent).is_empty(), "no code, nothing sent");
    assert!(shows(&page.draw(&mut gs, claim_page), t("owner.claim_code_needed")));
    gs.reports.owner.code = "  0123456789ABCDEF \n".into();
    page.click(&mut gs, t("owner.claim_button"), claim_page);
    let frames = sent_frames(&sent);
    assert_eq!(frames, vec![serde_json::json!({ "type": "owner_claim", "code": "0123456789ABCDEF" })], "the code is sent, trimmed");
    assert!(shows(&page.draw(&mut gs, claim_page), t("owner.claim_sent")));

    crate::engine::owner::on_frame(&mut gs, &serde_json::json!({ "type": "owner_reply", "kind": "claim", "ok": false, "message": "Wrong claim code. Copy it exactly." }));
    let out = page.draw(&mut gs, claim_page);
    assert!(shows(&out, "Wrong claim code. Copy it exactly.") && exact(&out, t("owner.claim_button")), "a refusal is shown and the control stays");
    crate::engine::owner::on_frame(&mut gs, &serde_json::json!({ "type": "owner_reply", "kind": "claim", "ok": true, "message": "You are now this server's owner, and an admin." }));
    let out = page.draw(&mut gs, claim_page);
    assert!(shows(&out, "You are now this server's owner, and an admin."), "the server's words stay");
    assert!(!exact(&out, t("owner.claim_button")), "and the control is gone");
    assert!(gs.reports.owner.code.is_empty());
}

/// The signed action a begin frame carries: its payload's text fields.
fn begun(frames: &[serde_json::Value]) -> std::collections::HashMap<String, String> {
    let b = frames.iter().find(|f| f["type"] == "owner_change_begin").unwrap_or_else(|| panic!("an owner_change_begin was sent: {frames:?}"));
    let o = submission_object(&b["action"]);
    assert!(o.verify_signature().is_ok() && hex::encode(&o.author_public_key) == key_of(9), "signed by me");
    payload_text(&o)
}

/// EACH OWNER CONTROL SENDS ITS FRAME, AND ONLY FOR THE OWNER: an admin sees no Ownership panel;
/// the owner chooses Bea (her key card is fetched from the server and checked here, and the page
/// says it checks out), and Hand over ownership refuses without her exact name, then sends the
/// signed `owner_handover` naming her; while it waits, Cancel sends `owner_change_cancel`; Name as
/// successor sends `owner_successor` naming her; Remove successor sends `owner_successor_remove`
/// naming the successor named now (Cy). The handover's words say I become an admin and that only
/// the new owner can hand it back.
///
/// Seen red 2026-11-20 with Remove successor wired to `Change::Successor`: "Remove successor names
/// the successor being removed" failed (action owner_successor, target Bea).
#[test]
fn each_owner_control_sends_its_frame_and_only_for_the_owner() {
    let page = Page::new();
    let (mut gs, sent) = owner_app("owner_controls_gui");
    let fake = Arc::new(Mutex::new(Fake::default()));
    fake.lock().unwrap().cards.insert(key_of(7), card_of(7, 200));
    gs.server_url = serve(fake);
    gs.reports.held.requested = true;

    gs.chat_users[0].role = "admin".into();
    crate::engine::held::on_frame(&mut gs, &held_list(false, Some((&key_of(8), "Cy"))));
    assert!(!exact(&page.draw(&mut gs, owner_page), t("owner.title")), "an admin sees no Ownership panel");

    gs.chat_users[0].role = "owner".into();
    crate::engine::held::on_frame(&mut gs, &held_list(true, Some((&key_of(8), "Cy"))));
    let out = page.draw(&mut gs, owner_page);
    assert!(exact(&out, t("owner.title")) && shows(&out, &tf("owner.successor_line", &[("name", &"Cy")])), "the owner's panel names the successor");
    // S7 (the third review): beside Remove successor, the words say removal takes nothing back.
    // Seen red 2026-11-21 before the hint was drawn: this assertion.
    assert!(shows(&out, &t("owner.remove_successor_limit")[..60]), "removal is said plainly not to take back what they could open");
    // M2 and H1: each member by name alone, and no member's fingerprint drawn.
    assert!(exact(&out, "Bea") && groups_drawn(&out, &key_of(7)).is_empty() && groups_drawn(&out, &key_of(8)).is_empty(), "the name alone, no fingerprint");
    let warning = t("owner.handover_warning");
    assert!(shows(&out, warning) && warning.contains("admin") && warning.contains("cannot be undone"), "{warning}");

    page.click(&mut gs, &member("Bea", 7), owner_page);
    page.draw_until(&mut gs, owner_page, |gs| gs.reports.owner.card.is_some());
    let out = page.draw(&mut gs, owner_page);
    assert!(shows(&out, &tf("owner.card_ok", &[("name", &"Bea"), ("when", &when(200))])), "her card checks out, checked here");
    assert!(shows(&out, &asked_words(&gs, "owner.fp_compare", "Bea")), "the four positions to ask her for");
    assert_eq!(groups_drawn(&out, &key_of(7)), Vec::<String>::new(), "and none of her groups before they are typed");

    let _ = sent_frames(&sent);
    gs.reports.owner.reason = "moving on".into();
    page.click(&mut gs, t("owner.handover"), owner_page);
    assert!(sent_frames(&sent).is_empty(), "not without her exact name");
    assert_eq!(gs.reports.owner.status, tf("owner.confirm_mismatch", &[("name", &"Bea")]));
    gs.reports.owner.confirm = "Bea".into();
    page.click(&mut gs, t("owner.handover"), owner_page);
    assert!(sent_frames(&sent).is_empty(), "not without the four groups of her fingerprint asked");
    assert_eq!(gs.reports.owner.status, fp_needed(&gs, "Bea"));
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7);
    // Checked: they match, and only now is her whole fingerprint drawn.
    page.click(&mut gs, t("owner.fp_check"), owner_page);
    let matched = tf("owner.fp_matched", &[("name", &"Bea"), ("fp", &humanity_protocol::key_card::fingerprint(&key_of(7)))]);
    assert!(shows(&page.draw(&mut gs, owner_page), &matched), "her whole fingerprint once her groups matched");
    page.click(&mut gs, t("owner.handover"), owner_page);
    let p = begun(&sent_frames(&sent));
    assert_eq!((p["action"].as_str(), p["target"].as_str(), p["reason"].as_str()), ("owner_handover", key_of(7).as_str(), "moving on"));

    page.click(&mut gs, t("owner.cancel"), owner_page);
    assert_eq!(sent_frames(&sent), vec![serde_json::json!({ "type": "owner_change_cancel" })], "Cancel sends the cancel");
    assert!(gs.reports.owner.pending.is_none());

    // Every change asks again (the fourth review's finding 1): the groups typed served the handover
    // alone, so naming her successor asks a new set of four.
    page.click(&mut gs, t("owner.name_successor"), owner_page);
    assert!(sent_frames(&sent).is_empty(), "naming her asks her groups again");
    assert_eq!(gs.reports.owner.status, fp_needed(&gs, "Bea"));
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7);
    page.click(&mut gs, t("owner.name_successor"), owner_page);
    let p = begun(&sent_frames(&sent));
    assert_eq!((p["action"].as_str(), p["target"].as_str()), ("owner_successor", key_of(7).as_str()), "Name as successor names Bea");
    page.click(&mut gs, t("owner.cancel"), owner_page);
    let _ = sent_frames(&sent);

    page.click(&mut gs, t("owner.remove_successor"), owner_page);
    let p = begun(&sent_frames(&sent));
    assert_eq!((p["action"].as_str(), p["target"].as_str()), ("owner_successor_remove", key_of(8).as_str()), "Remove successor names the successor being removed");
}

/// THE FIFTH REVIEW'S DEMO, AS A TEST (H1): the relay lists its own key R as "Bea" (and the real
/// Bea is not listed at all). Nothing the owner's page draws holds any group of R's fingerprint, or
/// of Bea's, before a match: the member rows, the chosen member, the words asking (which name the
/// four POSITIONS) and the field. So the owner can only type what Bea reads from HER OWN screen
/// (Settings > Account drawn for her key). Checked, those groups do not match R: a warning in words
/// beside the member (not an empty field), the field gone, and R refused for the session: picking it
/// again draws no positions, and Hand over and Name as successor send nothing and say why.
///
/// Seen red 2026-11-21 (scratchpad ownfix8-red-m2.log) with round 5's line drawing the chosen key's
/// whole fingerprint put back under the chosen member: "none of R's groups before a match" failed,
/// left: ["936f", "b02b", "fa0b", "bbb3", "7494", "7c85"] (all six of R's groups).
#[test]
fn the_owners_page_shows_positions_only_and_a_mismatch_warns_and_blocks() {
    let page = Page::new();
    let (mut gs, sent) = owner_app("owner_demo_gui");
    let r = key_of(3);
    gs.chat_users[1].public_key = r.clone(); // "Bea" is the relay's own key
    let fake = Arc::new(Mutex::new(Fake::default()));
    fake.lock().unwrap().cards.insert(r.clone(), card_of(3, 200));
    gs.server_url = serve(fake);
    gs.reports.held.requested = true;
    crate::engine::held::on_frame(&mut gs, &held_list(true, None));

    let out = page.draw(&mut gs, owner_page);
    assert!(groups_drawn(&out, &r).is_empty(), "no member row shows R's fingerprint: {:?}", groups_drawn(&out, &r));
    page.click(&mut gs, &member("Bea", 3), owner_page);
    page.draw_until(&mut gs, owner_page, |gs| gs.reports.owner.card.is_some());
    let out = page.draw(&mut gs, owner_page);
    assert!(shows(&out, &asked_words(&gs, "owner.fp_compare", "Bea")), "the words name the positions");
    assert_eq!(groups_drawn(&out, &r), Vec::<String>::new(), "none of R's groups before a match");
    assert_eq!(groups_drawn(&out, &key_of(7)), Vec::<String>::new(), "nor the real Bea's");

    // Bea reads her groups from her own screen; the owner types them and checks.
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7);
    gs.reports.owner.confirm = "Bea".into();
    gs.reports.owner.reason = "moving on".into();
    let _ = sent_frames(&sent);
    page.click(&mut gs, t("owner.fp_check"), owner_page);
    let warning = crate::engine::owner::mismatch_words("Bea");
    assert!(warning.starts_with(&tf("owner.fp_mismatch", &[("name", &"Bea")])), "{warning}");
    let out = page.draw(&mut gs, owner_page);
    assert!(shows(&out, &warning), "a mismatch is a warning in words");
    assert!(crate::engine::owner::is_refused(&gs, &r) && gs.reports.owner.asked.is_none(), "R is refused on this server");
    assert!(!exact(&out, t("owner.fp_check")), "and the field and its Check button are gone");
    assert!(groups_drawn(&out, &r).is_empty(), "R's fingerprint is still not drawn");
    page.click(&mut gs, &member("Bea", 3), owner_page);
    assert!(gs.reports.owner.asked.is_none(), "picking R again draws no new positions");
    gs.reports.owner.fp_confirm = asked_groups_at([0, 1, 2, 3], 7);
    for control in [t("owner.handover"), t("owner.name_successor")] {
        page.click(&mut gs, control, owner_page);
        assert!(sent_frames(&sent).is_empty(), "nothing is sent ({control})");
        assert_eq!(gs.reports.owner.status, warning, "and it says why ({control})");
    }
}

/// The groups at `at` as the person `key_of(seed)` reads them from their own screen.
fn asked_groups_at(at: humanity_protocol::key_card::Asked, seed: u8) -> String {
    let groups = crate::gui::pages::settings::own_fingerprint::seen_on_own_screen(&key_of(seed));
    at.iter().map(|p| groups[*p].clone()).collect::<Vec<_>>().join(" ")
}

/// A KEY CARD THAT DOES NOT CHECK OUT REFUSES THE CHANGE, IN WORDS: the server hands this device a
/// card for Bea that is not signed by her key (Cy's card with her key written in); the page says
/// her card does not check out, and Hand over ownership and Name as successor send nothing and say
/// why, though her name is typed and a reason given. A member the server holds no card for is told
/// apart: their card arrives when they sign in.
///
/// Seen red 2026-11-20 with `refusal` not looking at the card check: "nothing is sent" failed (the
/// signed handover went out for a card this device never checked).
#[test]
fn a_card_that_does_not_check_out_refuses_the_change_in_words() {
    let page = Page::new();
    let (mut gs, sent) = owner_app("owner_bad_card_gui");
    let mut swapped = humanity_protocol::key_card::sign_card(&[8; 32], 200).unwrap();
    swapped.key = key_of(7);
    let fake = Arc::new(Mutex::new(Fake::default()));
    fake.lock().unwrap().cards.insert(key_of(7), humanity_protocol::key_card::card_json(&swapped));
    gs.server_url = serve(fake);
    gs.reports.held.requested = true;
    crate::engine::held::on_frame(&mut gs, &held_list(true, None));

    page.click(&mut gs, &member("Bea", 7), owner_page);
    page.draw_until(&mut gs, owner_page, |gs| gs.reports.owner.card.is_some());
    let bad = tf("owner.card_bad", &[("name", &"Bea")]);
    assert!(shows(&page.draw(&mut gs, owner_page), &bad), "the page says her card does not check out");
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7);
    gs.reports.owner.confirm = "Bea".into();
    gs.reports.owner.reason = "moving on".into();
    let _ = sent_frames(&sent);
    for control in [t("owner.handover"), t("owner.name_successor")] {
        page.click(&mut gs, control, owner_page);
        assert!(sent_frames(&sent).is_empty(), "nothing is sent ({control})");
        assert_eq!(gs.reports.owner.status, bad, "and it says why ({control})");
    }
    assert!(!exact(&page.draw(&mut gs, owner_page), t("owner.remove_successor")), "no Remove successor with none named");

    page.click(&mut gs, &member("Cy", 8), owner_page);
    page.draw_until(&mut gs, owner_page, |gs| gs.reports.owner.card.is_some());
    assert!(shows(&page.draw(&mut gs, owner_page), &tf("owner.card_none", &[("name", &"Cy")])), "no card at all, told apart");
}

/// HELD COPIES SHOW EVERY ADMIN THE SUCCESSOR, WHETHER EACH ITEM IS ALSO SEALED TO THEM, AND THE
/// OWNER LOG: "Successor: Cy", each item's words, and the log's lines (who, when, the reason); with
/// no successor named it says so and no item claims to be sealed to one.
///
/// Seen red 2026-11-20 with `item_successor_line` returning before it drew: "each item says whether
/// the successor can open it" failed.
#[test]
fn held_copies_show_the_successor_and_the_owner_log() {
    let page = Page::new();
    let (mut gs, _sent) = owner_app("owner_held_gui");
    gs.chat_users[0].role = "admin".into();
    gs.reports.held.requested = true;
    crate::engine::held::on_frame(&mut gs, &held_list(false, Some((&key_of(8), "Cy"))));
    let out = page.draw(&mut gs, held_page);
    assert!(shows(&out, &tf("owner.successor_line", &[("name", &"Cy")])), "the successor");
    assert!(shows(&out, t("owner.sealed_to_successor")) && shows(&out, t("owner.not_successor_sealed")), "each item says whether the successor can open it");
    assert!(exact(&out, t("owner.log_title")));
    let line = tf("owner.log_owner_successor", &[("when", &when(1_763_700_000_000)), ("old", &"Ada"), ("new", &"Cy"), ("reason", &"just in case")]);
    assert!(shows(&out, &line), "the log: {line}");
    assert!(shows(&out, &tf("owner.log_claim", &[("when", &when(1_763_500_000_000)), ("new", &"Ada")])), "the claim");

    crate::engine::held::on_frame(&mut gs, &held_list(false, None));
    let out = page.draw(&mut gs, held_page);
    assert!(shows(&out, t("owner.no_successor")));
    assert!(!shows(&out, t("owner.sealed_to_successor")) && !shows(&out, t("owner.not_successor_sealed")), "no item speaks of a successor when none is named");
}

// ── The sixth review's H1: the check prompt ──

fn check_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    draw_check(ui, theme, state);
}

/// Bea's app (seed 7) on a server whose member list says Ada (seed 9) is the owner.
fn bea_app(tag: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let (mut gs, sent) = owner_app(tag);
    gs.profile_public_key = key_of(7);
    gs.private_key_bytes = Some(vec![7; 32]);
    (gs, sent)
}

/// An `owner_change_check` of `change` naming Bea, its signed action signed with the seed `signer`
/// (None: it carries none), the frame naming the key of seed `owner` as the owner, called
/// `owner_name`.
fn check_frame(change: Change, signer: Option<u8>, owner: u8, owner_name: &str) -> serde_json::Value {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let signed = signer.map(|seed| crate::net::owner::sign(&[seed; 32], "did:hum:owner-test", change, &key_of(7), "a change", now).unwrap());
    serde_json::json!({
        "type": "owner_change_check", "round": "r1", "action": change.action(),
        "grant": signed.as_ref().map(|s| s.object_id.clone()).unwrap_or_else(|| "ab".repeat(32)),
        "object": signed.as_ref().map(|s| serde_json::json!(s.submission)).unwrap_or(serde_json::Value::Null),
        "owner": key_of(owner), "owner_name": owner_name, "target": key_of(7), "target_name": "Bea", "items": [], "until": now + 3_600_000,
    })
}

/// A CHECK PROMPT IS DRAWN ONLY AFTER ITS SIGNED ACTION VERIFIES, SIGNED BY THIS SERVER'S OWNER AS
/// THIS DEVICE KNOWS IT (the sixth review's H1 (c)). A frame with no signed action, one signed by
/// another key than the owner it names, and one signed by the key it names when that key is not
/// the owner the member list shows, each draw NOTHING (no title, no question, no words), keep
/// nothing to draw later, and send nothing. The owner's own signed change is drawn.
///
/// Seen red 2026-11-21 (scratchpad ownfix9-red-native.log) against `on_check` keeping any check it
/// was sent (the signature checked only on accept): "no signed action: nothing is drawn" failed.
#[test]
fn a_check_prompt_is_drawn_only_when_signed_by_the_owner_this_device_knows() {
    let page = Page::new();
    for (what, frame) in [
        ("no signed action", check_frame(Change::Successor, None, 9, "Ada")),
        ("signed by another key than the owner it names", check_frame(Change::Successor, Some(3), 9, "Ada")),
        ("signed by the key it names, not this server's owner", check_frame(Change::Handover, Some(3), 3, "Ada")),
    ] {
        let (mut gs, sent) = bea_app("check_gate");
        assert!(crate::engine::owner::on_frame(&mut gs, &frame), "{what}: an owner frame");
        let out = page.draw(&mut gs, check_page);
        assert!(!shows(&out, t("owner.check_title")) && crate::gui::pages::settings::own_fingerprint::texts(&out).is_empty(), "{what}: nothing is drawn");
        assert!(gs.reports.owner.check.is_none(), "{what}: nothing is kept to draw");
        assert!(sent_frames(&sent).is_empty(), "{what}: nothing is sent");
    }
    let (mut gs, _sent) = bea_app("check_gate_ok");
    crate::engine::owner::on_frame(&mut gs, &check_frame(Change::Successor, Some(9), 9, "Ada"));
    assert!(shows(&page.draw(&mut gs, check_page), t("owner.check_title")), "the owner's own signed change is drawn");
}

/// A CHECK THAT COMES BEFORE THE MEMBER LIST WAITS FOR IT (the sixth review's H1 (c)): nothing is
/// drawn while this device does not know who the owner is; once the list comes (engine/owner.rs
/// `on_users`, which the full user list calls), the owner's own signed change is drawn.
/// Seen red 2026-11-21 by mutation (scratchpad ownfix9-red-native2.log): with `gate_check` drawing a
/// check whose owner is not known yet, "nothing is drawn before this device knows the owner" failed.
#[test]
fn a_check_that_comes_before_the_member_list_waits_for_it() {
    let page = Page::new();
    let (mut gs, _sent) = bea_app("check_wait");
    let users = std::mem::take(&mut gs.chat_users);
    crate::engine::owner::on_frame(&mut gs, &check_frame(Change::Successor, Some(9), 9, "Ada"));
    assert!(!shows(&page.draw(&mut gs, check_page), t("owner.check_title")), "nothing is drawn before this device knows the owner");
    gs.chat_users = users;
    crate::engine::owner::on_users(&mut gs);
    assert!(shows(&page.draw(&mut gs, check_page), t("owner.check_title")), "drawn once the member list says who the owner is");
}

/// A NAME IN A CHECK PROMPT THAT AN HONEST RELAY COULD NOT LIST IS A REFUSED NAME (the sixth
/// review's H1 (c)): the relay's owner_name carrying "Your key's fingerprint: <R's>..." on a change
/// the owner really signed shows as the refused name in words, and none of the relay's text and
/// none of R's groups is drawn.
///
/// Seen red 2026-11-21 (scratchpad ownfix9-red-native.log) against the prompt drawing owner_name as
/// sent: "the owner's name shows as a refused name" failed.
#[test]
fn a_name_in_a_check_prompt_an_honest_relay_could_not_list_is_refused() {
    let page = Page::new();
    let (mut gs, _sent) = bea_app("check_name");
    let forged = format!("Ada. {} Read them from here", tf("owner.my_fp", &[("fp", &humanity_protocol::key_card::fingerprint(&key_of(3)))]));
    crate::engine::owner::on_frame(&mut gs, &check_frame(Change::Successor, Some(9), 9, &forged));
    let out = page.draw(&mut gs, check_page);
    assert!(shows(&out, &tf("owner.check_successor", &[("owner", &t("owner.name_refused"))])), "the owner's name shows as a refused name");
    assert!(groups_drawn(&out, &key_of(3)).is_empty(), "none of R's groups: {:?}", groups_drawn(&out, &key_of(3)));
    assert!(!shows(&out, "Read them from here"), "none of the relay's text");
}

/// THE CHECK PROMPT SHOWS NO FINGERPRINT, AND SAYS WHERE YOUR OWN IS (the sixth review's H1 (a),
/// (b)): Bea, handed the role or named successor, sees none of her own fingerprint's groups on the
/// prompt (a person's own fingerprint is shown only in Settings > Account), and the words telling
/// her to read the groups from Settings > Account on her own device.
///
/// Seen red 2026-11-21 (scratchpad ownfix9-red-native.log) against the round-6 prompt, which drew
/// "Your fingerprint is ..." beside the question: "none of Bea's own groups on the prompt" failed.
#[test]
fn the_check_prompt_shows_no_fingerprint_and_says_where_yours_is() {
    let page = Page::new();
    for change in [Change::Successor, Change::Handover] {
        let (mut gs, _sent) = bea_app("check_no_fp");
        crate::engine::owner::on_frame(&mut gs, &check_frame(change, Some(9), 9, "Ada"));
        let out = page.draw(&mut gs, check_page);
        assert!(shows(&out, t("owner.check_title")), "{change:?}: the prompt is drawn");
        assert!(groups_drawn(&out, &key_of(7)).is_empty(), "{change:?}: none of Bea's own groups on the prompt: {:?}", groups_drawn(&out, &key_of(7)));
        assert!(shows(&out, t("owner.check_where_fp")), "{change:?}: it says where her own fingerprint is");
    }
}
