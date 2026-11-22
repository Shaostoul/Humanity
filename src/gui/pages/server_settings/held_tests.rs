// Child of server_settings/held.rs (#[path]): hide and keep's screens, driven through real headless
// egui frames and clicks (docs/design/hide-and-keep.md rung 5). Each test was seen red once on
// purpose, recorded at the test.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::net::held::tests::{owner_keys, package, unzip, PDF, PIC, WORDS};

/// A page driver: one context, the app's theme, a screen big enough for the whole section.
struct Page {
    ctx: egui::Context,
    theme: Theme,
}

impl Page {
    fn new() -> Self {
        crate::config::keep_saves_off_disk();
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        Page { ctx, theme }
    }

    fn run(&self, state: &mut GuiState, events: Vec<egui::Event>, what: fn(&mut egui::Ui, &Theme, &mut GuiState)) -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 2000.0))), events, ..Default::default() };
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| what(ui, &self.theme, state));
            });
        })
    }

    /// Two frames (the first lays out), then the second's shapes.
    fn draw(&self, state: &mut GuiState, what: fn(&mut egui::Ui, &Theme, &mut GuiState)) -> egui::FullOutput {
        self.run(state, Vec::new(), what);
        self.run(state, Vec::new(), what)
    }

    /// Click the text `label` (exactly) where it was last drawn.
    fn click(&self, state: &mut GuiState, label: &str, what: fn(&mut egui::Ui, &Theme, &mut GuiState)) {
        let out = self.draw(state, what);
        let found = find_text_in_shapes(&out.shapes, label).filter(|f| f.text == label).unwrap_or_else(|| panic!("no {label:?} drawn"));
        let at = found.rect.center();
        let m = egui::Modifiers::default();
        self.run(state, vec![egui::Event::PointerMoved(at)], what);
        self.run(state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], what);
        self.run(state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], what);
    }
}

fn exact(out: &egui::FullOutput, label: &str) -> bool {
    find_text_in_shapes(&out.shapes, label).is_some_and(|f| f.text == label)
}
fn shows(out: &egui::FullOutput, words: &str) -> bool {
    find_text_in_shapes(&out.shapes, words).is_some()
}

/// Every text drawn this frame, joined: to prove what is NOT on screen.
fn all_drawn(out: &egui::FullOutput) -> String {
    fn walk(s: &egui::Shape, out: &mut String) {
        match s {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut text = String::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut text));
    text
}

const SEED: [u8; 32] = [9u8; 32];
const DID: &str = "did:hum:held-test";

/// An app signed in with `SEED`, on a recording socket, knowing this server's identity.
fn app(tag: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    let mut gs = GuiState::default();
    gs.profile_public_key = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&SEED)).public_key());
    gs.private_key_bytes = Some(SEED.to_vec());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut store = crate::net::dm_store::DmStore::load(&SEED, &gs.profile_public_key, &format!("wss://{tag}-{nanos}.example"));
    store.set_pass_server(DID);
    gs.dm_store = Some(store);
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.reports.reasons_error = Some("not needed here".into());
    (gs, sent)
}

fn sent_frames(rx: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    rx.try_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect()
}

fn reports_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    crate::gui::pages::server_settings::reports::draw(ui, theme, state, true);
}
fn held_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    draw(ui, theme, state);
}

/// An open report about Tom's post, as the relay lists it.
fn post_report(gs: &mut GuiState) {
    gs.reports.list = crate::net::report::parse_reports(&serde_json::json!({ "type": "reports", "state": "open", "items": [{
        "id": 5, "target": "k2", "target_name": "Tom", "context": "post", "reason": "child_danger", "reason_label": "A child in danger", "created_at": 1,
        "evidence": [{ "kind": "post", "from": "k2", "timestamp": 42, "text": "the post", "checked": true }]
    }] }));
    gs.reports.requested = true;
}

const NO_OWNER: &str = "This server has no owner set, so there is nobody a copy could be sealed to. Nothing was kept.";

/// ON A REPORT, HIDE AND KEEP IS OFFERED ONLY WHEN THE SERVER CAN SEAL A COPY, AND OTHERWISE THE
/// PAGE SAYS WHY: with no owner the relay's own sentence, what works instead and what to do today
/// are drawn and no Hide and keep button is; with an owner's card the button is there, and a click
/// sends the decision with my own signed `hide_and_keep` naming the report, for this server.
///
/// Seen red 2026-11-20 with the refusal branch drawing nothing (`draw_refusal` an empty body): "the
/// relay's sentence is shown" failed.
#[test]
fn hide_and_keep_shows_why_not_or_sends_a_signed_decision() {
    let page = Page::new();
    let (mut gs, sent) = app("held_decide");
    post_report(&mut gs);
    gs.reports.held.keep = Some((false, NO_OWNER.to_string()));
    let out = page.draw(&mut gs, reports_page);
    assert!(shows(&out, NO_OWNER), "the relay's sentence is shown");
    assert!(shows(&out, t("held.instead")) && shows(&out, t("held.no_owner_today")), "with what works instead and what to do today");
    assert!(!exact(&out, t("held.decide_label")), "no Hide and keep button when nothing could be kept");
    assert!(exact(&out, "Delete post"), "Delete the post is still there");

    gs.reports.held.keep = Some((true, String::new()));
    let out = page.draw(&mut gs, reports_page);
    assert!(!shows(&out, NO_OWNER));
    let _ = sent_frames(&sent);
    page.click(&mut gs, t("held.decide_label"), reports_page);
    let frames = sent_frames(&sent);
    let decide = frames.iter().find(|f| f["type"] == "report_decide").expect("a report_decide was sent");
    assert_eq!((decide["id"].as_i64(), decide["decision"].as_str()), (Some(5), Some("hide_and_keep")));
    let o = crate::net::held::tests::submission_object(&decide["action"]);
    assert!(o.verify_signature().is_ok(), "the action verifies");
    assert_eq!(hex::encode(&o.author_public_key), gs.profile_public_key, "signed by me");
    assert_eq!(o.space_id.as_deref(), Some(DID), "for this server");
    let payload = format!("{:?}", ciborium::from_reader::<ciborium::Value, _>(&o.payload[..]).unwrap());
    assert!(payload.contains("hide_and_keep") && payload.contains("\"5\"") && payload.contains("report"), "naming report 5: {payload}");
}

/// A held_items frame with one item, for an owner or an admin.
fn list(owner: bool, can_keep: bool) -> serde_json::Value {
    let mut item = serde_json::json!({ "id": 1, "report_id": 5, "file_count": 2, "parts": 3, "sealed_bytes": 4096, "held_at": 1_763_600_000_000i64, "held_by_name": "Ada",
        "delete_after": 1_795_136_000_000i64, "legal_hold_until": 0, "deleted_at": 1_795_136_000_000i64, "exported_count": 0, "sealed_to_owner": true });
    if owner {
        item["package"] = "pkg".into();
    }
    serde_json::json!({ "type": "held_items", "can_keep": can_keep, "cannot_keep_why": if can_keep { "" } else { NO_OWNER }, "i_am_owner": owner,
        "items": [item], "actions": [{ "action": "hide_and_keep", "target": 5, "signer_name": "Ada", "reason": "the policy", "until": 0, "signed_at": 1_763_600_000_000i64 }],
        "counts": { "held": 1, "exported": 0, "legal_holds": 0, "destroyed": 0, "restored": 0 } })
}

/// HELD COPIES SHOW NUMBERS AND DATES, NEVER A WAY TO VIEW, AND THE ACTIONS ONLY TO THE OWNER: an
/// admin sees the item (its report, who held it and when, when it goes, its size), the log and the
/// counts, the words saying only the owner acts, and no action at all; no button anywhere opens an
/// item. The owner sees Legal hold, Export, Restore and Destroy now; Destroy now asks first, saying
/// it cannot be undone, sends nothing until Yes, refuses without a reason, and then sends my signed
/// `held_destroy`. Export opens a folder chooser whose title says what the folder is for.
///
/// Seen red 2026-11-20 with the owner check in `draw_item` taken out: "an admin gets no action"
/// failed, "(Place the legal hold)": the owner's actions were drawn for Ada.
#[test]
fn held_copies_show_no_contents_and_owner_actions_only_for_the_owner() {
    let page = Page::new();
    let (mut gs, sent) = app("held_list");
    gs.reports.held.requested = true;
    assert!(crate::engine::held::on_frame(&mut gs, &list(false, true)));
    let out = page.draw(&mut gs, held_page);
    assert!(shows(&out, &tf("held.item_line", &[("id", &1), ("report", &5)])), "the item and its report");
    assert!(shows(&out, "Ada"), "who held it");
    assert!(shows(&out, t("held.owner_only")), "only the owner acts");
    assert!(shows(&out, &tf("held.log_line", &[("when", &when(1_763_600_000_000)), ("name", &"Ada"), ("action", &t("held.action_hide_and_keep")), ("target", &5), ("reason", &"the policy")])), "the log");
    for label in [t("held.hold_set"), t("held.export"), t("held.restore"), t("held.destroy")] {
        assert!(!exact(&out, label), "an admin gets no action ({label})");
    }
    for label in ["View", "Open", "Show", "Preview"] {
        assert!(!exact(&out, label), "no button opens an item ({label})");
    }

    assert!(crate::engine::held::on_frame(&mut gs, &list(true, true)));
    let out = page.draw(&mut gs, held_page);
    for label in [t("held.hold_set"), t("held.export"), t("held.restore"), t("held.destroy")] {
        assert!(exact(&out, label), "the owner gets {label}");
    }
    for label in ["View", "Open", "Show", "Preview"] {
        assert!(!exact(&out, label), "no button opens an item, for the owner either ({label})");
    }
    assert!(!all_drawn(&out).contains("pkg"), "the package id is never drawn");

    let _ = sent_frames(&sent);
    page.click(&mut gs, t("held.destroy"), held_page);
    assert!(sent_frames(&sent).is_empty(), "Destroy now sends nothing before it is confirmed");
    let out = page.draw(&mut gs, held_page);
    let question = tf("held.destroy_confirm", &[("id", &1)]);
    assert!(shows(&out, &question) && question.contains("cannot be undone"), "it asks first, saying it cannot be undone: {question}");
    page.click(&mut gs, t("held.destroy_yes"), held_page);
    assert!(sent_frames(&sent).is_empty(), "no reason, nothing signed");
    assert_eq!(gs.reports.held.status, t("held.reason_needed"));
    gs.reports.held.reasons.insert(1, "a request from law enforcement".into());
    crate::engine::held::ask(&mut gs, 1, Confirm::Destroy);
    page.click(&mut gs, t("held.destroy_yes"), held_page);
    let frames = sent_frames(&sent);
    let act = frames.iter().find(|f| f["type"] == "held_act").expect("held_act sent");
    let o = crate::net::held::tests::submission_object(&act["action"]);
    let payload = format!("{:?}", ciborium::from_reader::<ciborium::Value, _>(&o.payload[..]).unwrap());
    assert!(o.verify_signature().is_ok() && payload.contains("held_destroy") && payload.contains("law enforcement"), "{payload}");
    assert!(gs.reports.held.confirm.is_none());

    page.click(&mut gs, t("held.export"), held_page);
    let out = page.draw(&mut gs, held_page);
    let title = tf("held.export_picker_title", &[("id", &1)]);
    assert!(shows(&out, &title), "the folder chooser says what it is for: {title}");
    assert!(title.contains(".zip") && title.contains("Nothing is shown"), "{title}");
}

/// A web server on 127.0.0.1 answering `GET /api/held/1/<part>?grant=<grant>` with `parts`, and
/// 404 for anything else; its address.
fn serve_parts(parts: Vec<Vec<u8>>, grant: String) -> String {
    use std::io::{BufRead, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten().take(16) {
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("");
            let body = path
                .strip_prefix("/api/held/1/")
                .and_then(|rest| rest.split_once("?grant="))
                .filter(|(_, g)| *g == grant)
                .and_then(|(p, _)| p.parse::<usize>().ok())
                .and_then(|p| parts.get(p).cloned());
            let mut s = stream;
            match body {
                Some(b) => {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len());
                    let _ = s.write_all(&b);
                }
                None => {
                    let _ = write!(s, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        }
    });
    format!("http://{addr}")
}

/// EXPORT OPENS THE HELD COPY ON THIS DEVICE STRAIGHT INTO THE FILE CHOSEN, AND NEVER ONTO THE
/// SCREEN: the signed `held_export` goes first; once the relay's list arrives (the grant) the
/// device fetches the sealed parts from the server, opens them with my own key into one .zip in the
/// folder I chose (the only file written there), and the page then says where it went, with none
/// of the post's words or file bytes drawn anywhere.
///
/// Seen red 2026-11-20 with the job's answer carrying what it opened ("opened: " and the file's
/// bytes, the post's words among them): "none of the post's words are drawn (HELD-WORDS-91c2 the
/// post's words)".
#[test]
fn export_writes_only_to_the_chosen_file_and_never_to_screen() {
    let page = Page::new();
    let (mut gs, sent) = app("held_export_gui");
    let kp = owner_keys(9);
    assert_eq!(kp.public_key(), humanity_protocol::pq_crypto::KyberKeypair::from_seed(&humanity_protocol::pq_crypto::derive_kyber_seed(&SEED)).unwrap().public_key());
    gs.reports.held.requested = true;
    crate::engine::held::on_frame(&mut gs, &list(true, true));
    gs.reports.held.reasons.insert(1, "a CyberTipline report".into());
    let folder = humanity_core::test_temp::dir("held_export_gui");
    // The item as the list shows the test package (net/held_tests.rs `package`: report 4, author
    // "aa", post time 42): what opens must be the item chosen (the third review's W7, the fourth
    // review's finding 8). Round 4 left this list at report 5, so this test failed from W7 on.
    let mut item = gs.reports.held.list.as_ref().unwrap().items[0].clone();
    (item.report_id, item.author, item.stamp) = (4, "aa".into(), 42);
    crate::engine::held::export(&mut gs, &item, folder.path());
    let frames = sent_frames(&sent);
    let act = frames.iter().find(|f| f["type"] == "held_act").expect("the signed export is sent first");
    let grant = crate::net::held::tests::submission_object(&act["action"]).object_id().unwrap().to_hex();
    gs.server_url = serve_parts(package(&kp, "pkg"), grant);
    crate::engine::held::on_frame(&mut gs, &list(true, true)); // the relay's answer: granted
    let start = std::time::Instant::now();
    while gs.reports.held.running.is_some() && start.elapsed() < std::time::Duration::from_secs(20) {
        crate::engine::held::poll(&mut gs);
        std::thread::yield_now();
    }
    let file = folder.path().join("held-item-1.zip");
    assert!(file.is_file(), "the zip is in the chosen folder: {}", gs.reports.held.status);
    let in_folder: Vec<_> = std::fs::read_dir(folder.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(in_folder.len(), 1, "and nothing else was written there");
    let zip = unzip(&std::fs::read(&file).unwrap());
    assert_eq!(zip[1].1, WORDS.as_bytes());
    assert_eq!((zip[2].1.as_slice(), zip[3].1.as_slice()), (PIC, PDF));
    assert!(gs.reports.held.status.contains("held-item-1.zip"), "the page says where it went: {}", gs.reports.held.status);
    let out = page.draw(&mut gs, held_page);
    let drawn = all_drawn(&out);
    assert!(drawn.contains("held-item-1.zip"));
    for secret in [WORDS, std::str::from_utf8(PIC).unwrap(), std::str::from_utf8(PDF).unwrap(), "HELD-"] {
        assert!(!drawn.contains(secret) && !gs.reports.held.status.contains(secret), "none of the post's words are drawn ({secret})");
    }
}
