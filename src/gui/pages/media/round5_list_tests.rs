//! Headless tests of Media round 5's list and Admin tab (lane 1C, docs/design/media-round-5-plan.md
//! 3.9): the heading that names the server, the Admin tab for admins only, Server Settings' line
//! pointing to it, the next page asked for once as the list's end comes into view, Sort, and the
//! picked category's description and shelf. The page is drawn into an egui context with no window
//! (the `headless_run` click pattern of docs/design/ui-system.md) and what it drew is read back.
//!
//! The tests that need to see what was asked use a stand-in server on a loopback port that keeps
//! every request it is sent and never answers ([`Recorder`]): each request stays on its way for
//! the whole test, so what the page asked, and how many times, can be counted exactly.
//!
//! Each was seen to fail once before it passed, against the break named in its comment; what it
//! saw red is there too.

use std::io::Read as _;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::api::{Category, Featured, Item, ListAnswer, Sort};
use super::tests::{ctx_and_theme, demo_answer, item, SERVER};
use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::theme::Theme;
use crate::gui::{GuiPage, GuiState};

/// One frame of a page, drawn by `draw` into a screen of `size`, with `events`.
type Frame = fn(&egui::Context, &Theme, &mut GuiState, Vec<egui::Event>) -> egui::FullOutput;

fn run(ctx: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>, draw_it: impl FnMut(&egui::Context)) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size)), events, ..Default::default() };
    ctx.run(input, draw_it)
}

/// The Media page, in a screen the size of an ordinary window.
fn media_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    run(ctx, egui::vec2(1280.0, 1000.0), events, |ctx| draw(ctx, theme, state))
}

/// The Server Settings page, in a screen tall enough to hold its whole ADMIN section.
fn settings_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    run(ctx, egui::vec2(1400.0, 9000.0), events, |ctx| crate::gui::pages::server_settings::draw(ctx, theme, state))
}

/// A few frames, then the last: a window or popup measures itself on one frame and is placed by
/// it on the next.
fn settle(frame: Frame, ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, theme, state, Vec::new());
    }
    frame(ctx, theme, state, Vec::new())
}

/// Click the drawn text `text` the way a person does: move there, press, release.
fn click(frame: Frame, ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = settle(frame, ctx, theme, state);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

fn shows(out: &egui::FullOutput, text: &str) -> bool {
    find_text_in_shapes(&out.shapes, text).is_some()
}

/// A drawn text that is exactly `text` (not one that only holds it).
fn exact(out: &egui::FullOutput, text: &str) -> bool {
    find_text_in_shapes(&out.shapes, text).is_some_and(|f| f.text == text)
}

/// An app on `server` with `answer` already in (its featured items on the shelf), signed in to
/// it as "me" with an upload token but no unlocked identity (so its reads are the public GETs,
/// whose whole request is in the first line), and an admin with the server's settings in when
/// `admin`. Nothing is asked when it is drawn: the first page and the admins' reads count as
/// asked. Nothing a test does writes the person's config.json.
fn state_on(server: &str, answer: ListAnswer, admin: bool) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut state = GuiState::default();
    state.server_url = server.to_string();
    state.profile_public_key = "me".to_string();
    sync_server(&mut state);
    state.media.requested = true;
    state.media.featured = answer.items.iter().filter(|i| i.featured.is_on()).cloned().collect();
    apply_list(&mut state.media, answer, false);
    state.upload_token = Some((server.to_string(), "tok".to_string()));
    state.media.token_mark = blake3::hash(b"tok").to_hex()[..16].to_string();
    if admin {
        state.chat_users.push(crate::gui::ChatUser { name: "Me".into(), public_key: "me".into(), role: "admin".into(), status: String::new() });
        state.server_settings = Some(crate::relay::storage::ServerSettings::default());
        state.media.admin.requested = true;
    }
    state
}

/// A stand-in server on a loopback port: it keeps every request it is sent (its first line, a
/// line break, and its body) and never answers, so each request stays on its way until the app
/// gives up on it long after the test.
struct Recorder {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Recorder {
    fn start() -> Recorder {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let keep = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let keep = keep.clone();
                std::thread::spawn(move || {
                    let mut got = Vec::new();
                    let mut buf = [0u8; 8192];
                    let mut kept = false;
                    while let Ok(n) = stream.read(&mut buf) {
                        if n == 0 {
                            return;
                        }
                        got.extend_from_slice(&buf[..n]);
                        if kept {
                            continue;
                        }
                        // The head, then as much body as its Content-Length says.
                        let Some(end) = got.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                        let head = String::from_utf8_lossy(&got[..end]).to_string();
                        let len = head
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                            .unwrap_or(0);
                        if got.len() < end + 4 + len {
                            continue;
                        }
                        let body = String::from_utf8_lossy(&got[end + 4..end + 4 + len]).to_string();
                        keep.lock().unwrap().push(format!("{}\n{body}", head.lines().next().unwrap_or("")));
                        kept = true;
                        // Read on (and answer nothing) until the app closes the connection.
                    }
                });
            }
        });
        Recorder { url, seen }
    }

    /// The requests whose first line holds `part`, waiting up to five seconds for at least `n`
    /// of them (the app sends from worker threads).
    fn wait_for(&self, part: &str, n: usize) -> Vec<String> {
        let start = Instant::now();
        loop {
            let got: Vec<String> = self.seen.lock().unwrap().iter().filter(|r| r.lines().next().unwrap_or("").contains(part)).cloned().collect();
            if got.len() >= n || start.elapsed() > Duration::from_secs(5) {
                return got;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// A first page of 48 models, newest (48) first, with a cursor for the next: far longer than
/// the window, so its end is out of view at the top.
fn long_list() -> ListAnswer {
    let items: Vec<Item> = (1..=48).rev().map(|id| item(id, &format!("Model {id}"), "model", ("k2", "Ann"))).collect();
    ListAnswer { items, next_cursor: Some("1.1".into()), members_view: true, media_visibility: "public".into(), ..Default::default() }
}

/// Scroll the Media page to its end with the mouse wheel over the list (egui eases a wheel's
/// scroll over a few frames, so it is turned for sixty).
fn scroll_to_the_end(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    media_frame(ctx, theme, state, vec![egui::Event::PointerMoved(egui::pos2(640.0, 600.0))]);
    for _ in 0..60 {
        let wheel = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -400.0), modifiers: egui::Modifiers::default() };
        media_frame(ctx, theme, state, vec![wheel]);
    }
}

/// How many pages of the list are on their way, of the kind `append` (a next page, or a first).
fn list_jobs(state: &GuiState, next: bool) -> usize {
    state.media.jobs.iter().filter(|(j, _)| matches!(j, Job::List { append, .. } if *append == next)).count()
}

/// THE HEADING NAMES THE SERVER (3.9): "Media on <host>", its port included when it has one (as
/// the website's `location.host` writes it), whatever form the app holds the address in, and
/// always the host the server strip names.
///
/// Seen red 2026-10-12 with the heading drawn as plain "Media" again: "the heading names the
/// server it is for" (no shape was exactly "Media on 127.0.0.1:9").
#[test]
fn the_heading_names_the_server() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = state_on(SERVER, demo_answer(), false);
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(exact(&out, "Media on 127.0.0.1:9"), "the heading names the server it is for");

    // Whatever form the address is held in, the heading names the host the server strip under
    // the top menu names (lane 1E's `host_of`, over the same connected server).
    for (address, words) in [
        ("wss://united-humanity.us/ws", "Media on united-humanity.us"),
        ("https://public.guide/", "Media on public.guide"),
        ("http://localhost:8099", "Media on localhost:8099"),
        ("united-humanity.us", "Media on united-humanity.us"),
        ("", "Media"),
    ] {
        state.server_url = address.to_string();
        assert_eq!(heading(&state), words, "{address:?}");
        let strip = crate::gui::pages::server_strip::host_of(&state.active_server());
        assert!(strip.is_empty() || heading(&state) == format!("Media on {strip}"), "the strip and the heading name one host: {address:?}");
    }
}

/// THE ADMIN TAB IS FOR ADMINS AND HOLDS THE SETTINGS (3.9): an admin has two tabs, Media and
/// Admin; Admin shows the section that was in Server Settings (who sees the page, its space, its
/// categories, the suggestions, the meter) in place of the list, and Media goes back. A member
/// has no tabs and always sees the list, even with the tab left set.
///
/// Seen red 2026-10-12 with the tabs drawn for everyone: "a member has no Admin tab".
#[test]
fn the_admin_tab_is_for_admins_and_holds_the_settings() {
    let (ctx, theme) = ctx_and_theme();
    let mut member = state_on(SERVER, demo_answer(), false);
    member.media.tab = MediaTab::Admin;
    let out = settle(media_frame, &ctx, &theme, &mut member);
    assert!(!exact(&out, "Admin"), "a member has no Admin tab");
    assert!(shows(&out, "Contact staff, slow spin"), "a member sees the list");
    assert!(!shows(&out, "Who sees the Media page"), "and never the settings");

    let mut state = state_on(SERVER, demo_answer(), true);
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(exact(&out, "Media") && exact(&out, "Admin"), "an admin has the two tabs");
    assert!(shows(&out, "Contact staff, slow spin") && !shows(&out, "Who sees the Media page"), "the list first");

    click(media_frame, &ctx, &theme, &mut state, "Admin");
    assert_eq!(state.media.tab, MediaTab::Admin, "Admin is picked");
    let out = settle(media_frame, &ctx, &theme, &mut state);
    for text in ["Who sees the Media page", "Media space (MB)", "Each person's share (MB)", "Categories", "Category suggestions", "Save Media settings"] {
        assert!(shows(&out, text), "the Admin tab shows {text:?}");
    }
    assert!(!shows(&out, "Contact staff, slow spin"), "in place of the list");
    assert!(!shows(&out, "Add to Media"), "the list's own actions stay on the Media tab");
    assert!(exact(&out, "Media on 127.0.0.1:9"), "under the same heading");

    click(media_frame, &ctx, &theme, &mut state, "Media");
    assert_eq!(state.media.tab, MediaTab::Media, "Media goes back");
    assert!(shows(&settle(media_frame, &ctx, &theme, &mut state), "Contact staff, slow spin"), "to the list");
}

/// SERVER SETTINGS POINTS TO THE ADMIN TAB (3.9): its ADMIN section keeps one line saying where
/// the Media settings went, with "Open them", which lands on the Media page with Admin selected.
///
/// Seen red 2026-10-12 with `open_admin_tab` leaving the tab as it was: "with Admin selected"
/// (left: Media / right: Admin).
#[test]
fn server_settings_points_to_the_admin_tab() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = state_on(SERVER, demo_answer(), true);
    state.active_page = GuiPage::ServerSettings;
    let out = settle(settings_frame, &ctx, &theme, &mut state);
    assert!(shows(&out, admin::POINTER_LINE), "Server Settings says where the Media settings are");
    assert!(!shows(&out, "Each person's share (MB)"), "and no longer holds them");

    click(settings_frame, &ctx, &theme, &mut state, "Open them");
    assert_eq!(state.active_page, GuiPage::Media, "Open them goes to the Media page");
    assert_eq!(state.media.tab, MediaTab::Admin, "with Admin selected");
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(shows(&out, "Each person's share (MB)"), "where the Media settings are");
}

/// THE END OF THE LIST COMING INTO VIEW ASKS FOR ONE MORE PAGE (3.9, decision 13): at the top of
/// a long list nothing is asked; scrolled to its end, the next page is asked for once, with the
/// last answer's cursor, however many frames the end stays in view while it is on its way. More
/// stays under the grid for keyboards.
///
/// Seen red 2026-10-12 with `more` asking without looking for a page on its way: "the next page
/// is asked for once, not once a frame (asked 93 times)" (left: 93 / right: 1).
#[test]
fn the_end_of_the_list_coming_into_view_asks_for_one_more_page() {
    let (ctx, theme) = ctx_and_theme();
    let rec = Recorder::start();
    let mut state = state_on(&rec.url, long_list(), false);

    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(!exact(&out, "Model 1"), "the list's end is out of view at the top");
    assert_eq!(list_jobs(&state, true), 0, "and nothing is asked while it is");

    // Scrolled down with the wheel over the list, then the end left in view for forty frames.
    scroll_to_the_end(&ctx, &theme, &mut state);
    for _ in 0..40 {
        media_frame(&ctx, &theme, &mut state, Vec::new());
    }
    let out = media_frame(&ctx, &theme, &mut state, Vec::new());
    assert!(exact(&out, "Model 1"), "scrolled to the end of the list");
    let asked = list_jobs(&state, true);
    assert_eq!(asked, 1, "the next page is asked for once, not once a frame (asked {asked} times)");
    assert!(shows(&out, "Loading more..."), "and the page says it is coming");
    let seen = rec.wait_for("/api/media?", 1);
    assert_eq!(seen.len(), 1, "one request reached the server: {seen:?}");
    assert!(seen[0].contains("cursor=1.1"), "with the last answer's cursor: {seen:?}");

    // A next page that fails is not asked again by itself (the end is still in view): More asks.
    let generation = state.media.generation;
    state.media.jobs.clear();
    on_done(&mut state, Job::List { append: true, generation }, Err(api::Refusal::new(0, "The server could not be reached.")));
    // (Its words above the list push the end down a line: scrolled to it again.)
    scroll_to_the_end(&ctx, &theme, &mut state);
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(exact(&out, "More"), "the end is in view, with More");
    assert_eq!(list_jobs(&state, true), 0, "a failed next page waits for More");
    click(media_frame, &ctx, &theme, &mut state, "More");
    assert_eq!(list_jobs(&state, true), 1, "More asks again");
}

/// A NEW SORT ASKS AFRESH WITH ITS SORT (3.3, 3.9): picking "Most picked" asks for the first page
/// again (no cursor: the old one belongs to the old order) with `sort=picks`, and its answer
/// replaces the list.
///
/// Seen red 2026-10-12 with the sort left out of the public list request: "with its sort" (the
/// request was "GET /api/media?limit=48 HTTP/1.1").
#[test]
fn a_new_sort_asks_afresh_with_its_sort() {
    let (ctx, theme) = ctx_and_theme();
    let rec = Recorder::start();
    // A long list, so its end (which would ask for the next page by itself) is out of view.
    let mut state = state_on(&rec.url, long_list(), false);
    assert!(exact(&settle(media_frame, &ctx, &theme, &mut state), "Newest"), "the list is newest first to begin with");

    click(media_frame, &ctx, &theme, &mut state, "Newest");
    click(media_frame, &ctx, &theme, &mut state, "Most picked");
    assert_eq!(state.media.filters.sort, Sort::Picks, "Most picked is picked");
    assert_eq!((list_jobs(&state, false), list_jobs(&state, true)), (1, 0), "a first page is asked for, not a next one");
    let seen = rec.wait_for("/api/media?", 1);
    let req = seen.first().unwrap_or_else(|| panic!("the list was asked for: {seen:?}"));
    assert!(req.contains("sort=picks"), "with its sort: {req}");
    assert!(!req.contains("cursor="), "from the first page, with no cursor: {req}");

    // Its answer replaces the list, with its own cursor.
    let generation = state.media.generation;
    let v = serde_json::json!({ "items": [{"id": 9, "url": "/uploads/9_x.glb", "filename": "9_x.glb", "kind": "model", "visibility": "public", "featured": "none", "title": "Most picked of all", "picks": 30}], "next_cursor": "30.9", "members_view": true });
    on_done(&mut state, Job::List { append: false, generation }, Ok(v));
    assert_eq!(state.media.list.items.iter().map(|i| i.id).collect::<Vec<_>>(), vec![9], "the answer replaces the list");
    assert_eq!(state.media.list.next_cursor.as_deref(), Some("30.9"), "and brings its own cursor");
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(exact(&out, "Most picked") && shows(&out, "Most picked of all"), "the order shown is the one picked");
}

/// A PICKED CATEGORY SHOWS ITS DESCRIPTION AND ITS SHELF (3.9, 3.3): with a category picked, its
/// description shows under the filters, the shelf is asked for with that category
/// (`featured=1&category=2`), and once it arrives its heading reads "Featured in Fire flow arts".
///
/// Seen red 2026-10-12 with `featured_page` asking without the category: "the shelf is asked for
/// with its category" (the request was "GET /api/media?limit=48&featured=1 HTTP/1.1").
#[test]
fn a_picked_category_shows_its_description_and_its_shelf() {
    let (ctx, theme) = ctx_and_theme();
    let rec = Recorder::start();
    let mut answer = demo_answer();
    answer.categories.push(Category { id: 2, name: "Fire flow arts".into(), description: "Staff, poi, darts and the rest of the flow arts.".into() });
    let mut state = state_on(&rec.url, answer, false);
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(exact(&out, "Featured"), "the main shelf to begin with");
    assert!(!shows(&out, "Staff, poi, darts"), "and no category's description");

    click(media_frame, &ctx, &theme, &mut state, "All categories");
    click(media_frame, &ctx, &theme, &mut state, "Fire flow arts");
    assert_eq!(state.media.filters.category, Some(2), "Fire flow arts is picked");
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(shows(&out, "Staff, poi, darts and the rest of the flow arts."), "its description shows under the filters");

    let shelf = rec.wait_for("featured=1", 1);
    let req = shelf.first().unwrap_or_else(|| panic!("the shelf was asked for: {shelf:?}"));
    assert!(req.contains("category=2"), "the shelf is asked for with its category: {req}");
    assert!(rec.wait_for("/api/media?", 2).iter().any(|r| r.contains("category=2") && !r.contains("featured=1")), "and so is the list");

    // The shelf's answer: that category's featured items, under its name.
    let generation = state.media.featured_generation;
    let v = serde_json::json!({ "items": [{"id": 12, "url": "/uploads/12_x.glb", "filename": "12_x.glb", "kind": "model", "visibility": "public", "featured": "category", "title": "Poi at night", "category": {"id": 2, "name": "Fire flow arts"}}] });
    on_done(&mut state, Job::Featured { generation, pages: 1 }, Ok(v));
    let out = settle(media_frame, &ctx, &theme, &mut state);
    assert!(exact(&out, "Featured in Fire flow arts"), "the shelf's heading names its category");
    assert!(shows(&out, "Poi at night"), "and holds its items");
}

/// THE FEATURE CHOICE SENDS THE PLACE PICKED (3.4, 3.9; the function lane 1D's tile calls): an
/// admin picks "Featured in its category" and the feature request goes with `featured:
/// "category"`, signed over `media_feature:<id>:category`; a member is offered no choice.
///
/// Seen red 2026-10-12 with the choice sending the main shelf whatever was picked: "the place
/// picked is sent" (left: Main / right: Category).
#[test]
fn the_feature_choice_sends_the_place_picked() {
    fn choice_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let it = state.media.list.items[0].clone();
        run(ctx, egui::vec2(800.0, 600.0), events, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                filters::draw_feature_choice(ui, theme, state, &it);
            });
        })
    }
    let (ctx, theme) = ctx_and_theme();
    let rec = Recorder::start();
    let one = ListAnswer { items: vec![item(5, "Poi at night", "model", ("k2", "Ann"))], members_view: true, ..Default::default() };

    let mut member = state_on(&rec.url, one.clone(), false);
    assert!(!shows(&settle(choice_frame, &ctx, &theme, &mut member), "Not featured"), "a member is offered no choice");

    let mut state = state_on(&rec.url, one, true);
    state.private_key_bytes = Some(vec![7u8; 32]);
    click(choice_frame, &ctx, &theme, &mut state, "Not featured");
    click(choice_frame, &ctx, &theme, &mut state, "Featured in its category");
    let to = state.media.jobs.iter().find_map(|(j, _)| match j {
        Job::Feature { id: 5, to } => Some(*to),
        _ => None,
    });
    assert_eq!(to, Some(Featured::Category), "the place picked is sent");
    let sent = rec.wait_for("/api/media/feature", 1);
    let req = sent.first().unwrap_or_else(|| panic!("the feature request went: {sent:?}"));
    assert!(req.contains("\"featured\":\"category\"") && req.contains("\"filename\":\"5_file\""), "naming the place and the item's file: {req}");
}
