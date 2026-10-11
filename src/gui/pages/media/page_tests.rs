//! Headless tests of the Media page (media.rs and its children): the page drawn into an egui
//! context with no window and no network, and what it drew read back. Each was seen to fail once
//! before it passed; what it saw red is in its comment.

use super::api::{Item, ListAnswer, Mine, Refusal};
use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::theme::Theme;

/// A server address that answers nothing at once (a closed port on this machine), so a picture a
/// test lets load fails fast instead of reaching anywhere.
pub(crate) const SERVER: &str = "http://127.0.0.1:9";

pub(crate) fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))),
        events,
        ..Default::default()
    };
    ctx.run(input, |ctx| draw(ctx, theme, state))
}

/// Draw a few frames and return the last: a window measures its size on one frame and is placed
/// by it on the next, so a form whose height just changed needs more than two to stand still.
pub(crate) fn settled(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, theme, state, Vec::new());
    }
    frame(ctx, theme, state, Vec::new())
}

pub(crate) fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = settled(ctx, theme, state);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

pub(crate) fn shows(out: &egui::FullOutput, text: &str) -> bool {
    find_text_in_shapes(&out.shapes, text).is_some()
}

fn exact(out: &egui::FullOutput, t: &str) -> bool {
    find_text_in_shapes(&out.shapes, t).is_some_and(|f| f.text == t)
}

/// An app on `SERVER`, the page's first answer `answer` already in (its featured items in the
/// strip), signed in as "me" when `signed_in`. Nothing is asked of the network: the first page
/// counts as asked. Nothing a test does writes the person's config.json.
pub(crate) fn page_state(answer: ListAnswer, signed_in: bool) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut state = GuiState::default();
    state.server_url = SERVER.to_string();
    state.profile_public_key = "me".to_string();
    sync_server(&mut state);
    state.media.requested = true;
    state.media.featured = answer.items.iter().filter(|i| i.featured).cloned().collect();
    apply_list(&mut state.media, answer, false);
    if signed_in {
        state.upload_token = Some((SERVER.to_string(), "tok".to_string()));
        state.media.token_mark = blake3::hash(b"tok").to_hex()[..16].to_string();
    }
    state
}

pub(crate) fn item(id: i64, title: &str, kind: &str, by: (&str, &str)) -> Item {
    Item {
        id,
        url: format!("/uploads/{id}_file"),
        stored: format!("{id}_file"),
        title: title.into(),
        kind: kind.into(),
        visibility: "public".into(),
        size_bytes: 2 * 1024 * 1024,
        uploader_key: by.0.into(),
        uploader_name: by.1.into(),
        category_id: 1,
        category_name: "General".into(),
        ..Default::default()
    }
}

/// The answer the snapshot and these tests draw: a featured video, a model, a labelled picture
/// and a picture from someone who is not a friend.
pub(crate) fn demo_answer() -> ListAnswer {
    ListAnswer {
        items: vec![
            Item { featured: true, picks: 3, description: "Fire staff at dusk".into(), ..item(4, "Contact staff, slow spin", "video", ("k1", "Shaostoul")) },
            Item { picks: 1, ..item(3, "Fibonacci homestead", "model", ("k2", "Ann")) },
            Item { labels: vec!["spoiler".into()], ..item(2, "The ending", "picture", ("k2", "Ann")) },
            item(1, "Garden this morning", "picture", ("k3", "Ben")),
        ],
        members_view: true,
        media_visibility: "public".into(),
        categories: vec![api::Category { id: 1, name: "General".into(), description: String::new() }],
        labels: vec![api::Label { id: "spoiler".into(), label: "Spoiler".into(), desc: "Shows the ending".into() }],
        ..Default::default()
    }
}

/// AN EMPTY PAGE SAYS SO: no server, "Connect to a server first"; asked and not answered,
/// "Loading..."; answered with nothing, the empty line.
///
/// Seen red 2026-10-11 with the filters' line drawn in place of the empty line: "the empty page
/// says so" (no shape held "Nothing is on this server's Media page yet.").
#[test]
fn an_empty_page_says_so() {
    let (ctx, theme) = ctx_and_theme();
    let mut none = GuiState::default();
    none.server_url.clear();
    assert!(shows(&settled(&ctx, &theme, &mut none), "Connect to a server first"), "with no server it says to connect");

    let mut state = page_state(ListAnswer { members_view: true, ..Default::default() }, true);
    state.media.answered = false;
    assert!(shows(&settled(&ctx, &theme, &mut state), "Loading..."), "before the answer it says it is loading");
    state.media.answered = true;
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, EMPTY_LINE), "the empty page says so");
    assert!(shows(&out, "Add to Media"), "a signed-in member is offered Add to Media");
}

/// THE TILES SHOW WHAT THEY ARE: the featured strip, each title, who put it there, the pick
/// count, a model's kind name, and the actions for someone signed in (Pick and Report on other
/// people's, none of Remove, Feature or Keep for a member who is neither admin nor moderator).
///
/// Seen red 2026-10-11 with the featured strip drawn from every item: "an item that is not
/// featured is only in the grid" (left: 2 / right: 1).
#[test]
fn the_tiles_show_what_they_are() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    let out = settled(&ctx, &theme, &mut state);
    for text in ["Featured", "Contact staff, slow spin", "by Shaostoul", "3 picks", "1 pick", "Fibonacci homestead", "3D model", "Pick", "Report"] {
        assert!(shows(&out, text), "the page shows {text:?}");
    }
    let staff = find_text_in_shapes(&out.shapes, "Contact staff, slow spin").unwrap();
    assert_eq!(staff.matches, 2, "only the featured item leads (in the strip and in the grid)");
    let homestead = find_text_in_shapes(&out.shapes, "Fibonacci homestead").unwrap();
    assert_eq!(homestead.matches, 1, "an item that is not featured is only in the grid");
    assert!(!exact(&out, "Feature") && !exact(&out, "Unfeature"), "no Feature button for a member");
    assert!(!shows(&out, "Remove"), "no Remove on other people's items for a member");
    assert!(!shows(&out, tiles::KEEP), "no Keep for a member");

    // An admin sees Remove, Feature and Keep on every tile.
    state.chat_users.push(crate::gui::ChatUser { name: "Me".into(), public_key: "me".into(), role: "admin".into(), status: String::new() });
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Remove") && exact(&out, "Unfeature") && exact(&out, "Feature") && shows(&out, tiles::KEEP), "an admin may remove, feature and keep");
}

/// THE FEATURED STRIP IS EVERY FEATURED ITEM, not just those on the pages loaded (`featured=1`,
/// the relay's A2): an item featured long ago, far past the first page, still leads.
///
/// Seen red 2026-10-11 with the strip drawn from the loaded list's featured items: "an item off
/// the loaded pages is in the strip" (it was not drawn).
#[test]
fn the_featured_strip_is_every_featured_item() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.media.featured.push(Item { featured: true, ..item(90, "Old favourite", "model", ("k5", "Cy")) });
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Old favourite"), "an item off the loaded pages is in the strip");
    // And it is what the featured answer says.
    let generation = state.media.featured_generation;
    on_done(&mut state, Job::Featured { generation, pages: 1 }, Ok(serde_json::json!({ "items": [{"id": 77, "url": "/uploads/77_x.png", "kind": "picture", "visibility": "public", "featured": true}] })));
    assert_eq!(state.media.featured.iter().map(|i| i.id).collect::<Vec<_>>(), vec![77], "the strip is the featured answer");
}

/// A LABELLED TILE SHOWS ITS LABEL UNTIL CLICKED: the label's name and "Click to show." are drawn
/// instead of the picture, nothing is fetched, and the click lets it load. With the protected
/// setup on, a labelled item is left out altogether, even with its pictures rule turned off.
///
/// Seen red 2026-10-11 with the label check after the friend check in `show_of`: "the labelled
/// picture shows its label first" (left: Ask / right: Label([\"Spoiler\"])).
#[test]
fn a_labelled_tile_is_hidden_until_clicked() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    let labelled = state.media.list.items.iter().find(|i| i.id == 2).cloned().unwrap();
    assert_eq!(tiles::show_of(&state, &labelled), tiles::Show::Label(vec!["Spoiler".into()]), "the labelled picture shows its label first");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Spoiler") && shows(&out, tiles::CLICK_TO_SHOW));
    let key = tiles::cache_key(&state, &labelled);
    assert!(matches!(state.image_cache.status(&key), crate::gui::widgets::image_cache::ImageStatus::Idle), "nothing is fetched before the click");

    click(&ctx, &theme, &mut state, "Spoiler");
    assert_eq!(tiles::show_of(&state, &labelled), tiles::Show::Load, "the click lets it load");
    settled(&ctx, &theme, &mut state);
    assert!(!matches!(state.image_cache.status(&key), crate::gui::widgets::image_cache::ImageStatus::Idle), "and it is fetched");

    let mut protected = page_state(demo_answer(), true);
    protected.protected.setup.on = true;
    protected.protected.setup.pictures_hidden = false;
    assert_eq!(tiles::show_of(&protected, &labelled), tiles::Show::Hidden(tiles::LABEL_HIDDEN_LINE.into()), "the protected setup hides labelled items");
    protected.protected.setup.pictures_hidden = true;
    assert!(matches!(tiles::show_of(&protected, &labelled), tiles::Show::Hidden(_)), "and a non-friend's pictures under its pictures rule");
}

/// A PICTURE FROM SOMEONE WHO IS NOT A FRIEND WAITS FOR A CLICK, as in the chat; your own loads.
///
/// Seen red 2026-10-11 with the friend check left out of `show_of`: "a stranger's picture asks
/// first" (left: Load / right: Ask).
#[test]
fn a_strangers_picture_asks_first() {
    let mut state = page_state(demo_answer(), true);
    let stranger = state.media.list.items.iter().find(|i| i.id == 1).cloned().unwrap();
    assert_eq!(tiles::show_of(&state, &stranger), tiles::Show::Ask, "a stranger's picture asks first");
    let mine = Item { uploader_key: "me".into(), ..stranger.clone() };
    assert_eq!(tiles::show_of(&state, &mine), tiles::Show::Load, "your own loads");
    state.image_cache.allow(&tiles::cache_key(&state, &stranger));
    assert_eq!(tiles::show_of(&state, &stranger), tiles::Show::Load, "after the click it loads");
}

/// A PICTURE TOO LARGE TO SHOW IN PLACE SAYS SO AND OPENS ITS CARD, where Download takes it
/// whatever its size (the review: pictures over 16 MB could neither be viewed nor downloaded).
///
/// Seen red 2026-10-11 with no size check in `show_of`: "a 20 MB picture says it is too large"
/// (left: Load / right: TooBig).
#[test]
fn a_picture_too_large_to_show_opens_its_card() {
    let (ctx, theme) = ctx_and_theme();
    let big = Item { size_bytes: 20 * 1024 * 1024, ..item(6, "Panorama", "picture", ("me", "Me")) };
    let mut state = page_state(ListAnswer { items: vec![big.clone()], members_view: true, media_visibility: "public".into(), ..Default::default() }, true);
    assert_eq!(tiles::show_of(&state, &big), tiles::Show::TooBig, "a 20 MB picture says it is too large");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "This picture is 20 MB, larger than this app shows in place (16 MB)"));
    click(&ctx, &theme, &mut state, "This picture is 20 MB");
    assert_eq!(state.media.open.as_ref().map(|o| o.item.id), Some(6), "the click opens its card");
    assert!(matches!(state.image_cache.status(&tiles::cache_key(&state, &big)), crate::gui::widgets::image_cache::ImageStatus::Idle), "and nothing is fetched into the image cache");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Download"), "the card offers Download");
}

/// SOMEONE NOT SIGNED IN SEES ONLY PUBLIC ITEMS AND IS TOLD WHY: the members' line when the answer
/// is not the members' view, and none when it is; no Add to Media, Pick or My picks without a
/// sign-in.
///
/// Seen red 2026-10-11 with the line keyed on `members_only_for_you` (true only on a members-only
/// server): "the public view says to sign in" on a public server's public answer.
#[test]
fn someone_not_signed_in_is_told_to_sign_in() {
    let (ctx, theme) = ctx_and_theme();
    let mut public = page_state(ListAnswer { members_view: false, media_visibility: "public".into(), ..demo_answer() }, false);
    let out = settled(&ctx, &theme, &mut public);
    assert!(shows(&out, SIGN_IN_LINE), "the public view says to sign in");
    assert!(!shows(&out, "Add to Media"), "and offers no Add to Media");
    assert!(!shows(&out, "Pick"), "nor Pick");
    let mut member = page_state(demo_answer(), true);
    assert!(!shows(&settled(&ctx, &theme, &mut member), SIGN_IN_LINE), "a member's view has no such line");
}

/// A FILE OVER THE SHARE IS STOPPED BEFORE IT STARTS, by the server's own count (`/api/media/mine`,
/// what is still arriving included): picked with too little of the share left, it says so with the
/// numbers and nothing is uploaded; in the form, the share line and the stop show and Add to Media
/// stays off. With room, the same press starts the upload in pieces.
///
/// Seen red 2026-10-11 with `picked` not checking the share: "nothing is uploaded" (a form opened
/// for it). Seen red again 2026-10-11 (round 2) with the share counted from this app's own sum of
/// the person's items: the relay's `in_progress_bytes` left out, "an upload still arriving counts"
/// (a form opened).
#[test]
fn a_file_over_the_share_is_stopped_before_it_starts() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.media.mine = Some(Mine { used_bytes: 600 * 1024, in_progress_bytes: 300 * 1024, share_bytes: 1024 * 1024, budget_left_bytes: 1 << 40, ..Default::default() });
    let file = crate::test_temp::file("media_over_share", "png");
    std::fs::write(&file, vec![0u8; 300 * 1024]).unwrap();

    add::picked(&mut state, &file);
    assert!(state.media.form.is_none() && state.media.uploads.is_empty(), "nothing is uploaded: an upload still arriving counts");
    assert!(state.media.status.contains("This file is 0.3 MB, more than the 0.1 MB left of your share"), "it says why: {}", state.media.status);

    // The form opened before the numbers were known; now they are, and too small.
    state.media.form = Some(add::Form::new(add::Source::Picked { path: file.to_path_buf(), name: "big.png".into(), len: 300 * 1024 }));
    state.private_key_bytes = Some(vec![7u8; 32]);
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "You have 0.1 MB of your 1 MB left"), "the share is shown above the form");
    assert!(shows(&out, "more than the 0.1 MB left of your share"), "the form says why it cannot go");
    click(&ctx, &theme, &mut state, "Upload and add to Media");
    assert!(state.media.uploads.is_empty(), "pressing it starts nothing");

    // The control: with room the same press starts the upload, so the press above did land on
    // the button (it goes nowhere here: nothing listens at the test's address).
    state.media.mine.as_mut().unwrap().share_bytes = 10 * 1024 * 1024;
    click(&ctx, &theme, &mut state, "Upload and add to Media");
    assert_eq!(state.media.uploads.len(), 1, "with room, the press starts the upload (form: {:?})", state.media.form);
    assert_eq!(state.media.uploads[0].record.size, 300 * 1024, "carrying what its form said");
    assert!(state.media.form.is_none(), "and the form is done: the upload is in the list");
    uploading::abandon_all(&state.media.uploads);
}

/// PUT ON MEDIA OFFERS PUBLIC ONLY: from a Shared Files row, the form opens on the Media page
/// without the picker, says the file is already public where it was shared, offers no Members
/// only, and publishes as public whatever is set.
///
/// Seen red 2026-10-11 with the visibility choice drawn for every form: "no Members only for a
/// file already public" (the radio was drawn).
#[test]
fn put_on_media_offers_public_only() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    put_on_media(&mut state, "/uploads/1700_homestead.glb", "homestead.glb", 1000);
    assert_eq!(state.active_page, crate::gui::GuiPage::Media, "it opens the Media page");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Put it on Media"), "the form is open");
    assert!(shows(&out, add::ALREADY_PUBLIC_LINE), "it says why");
    assert!(!shows(&out, "Members only"), "no Members only for a file already public");
    let mut form = state.media.form.clone().expect("the form");
    assert_eq!(form.source, add::Source::Existing { filename: "1700_homestead.glb".into(), name: "homestead.glb".into(), len: 1000 });
    form.members_only = true;
    assert_eq!(add::fields_of(&form, "1700_homestead.glb", &[], true).visibility, "public", "published as public whatever is set");

    // A file uploaded for Media may be members only, and is on a members-only server.
    let picked = add::Form { members_only: true, ..add::Form::new(add::Source::Picked { path: "x.png".into(), name: "x.png".into(), len: 1 }) };
    assert_eq!(add::fields_of(&picked, "x.png", &[], false).visibility, "members");
    let waiting = add::Form::new(add::Source::Waiting { filename: "2_x.png".into(), name: "x.png".into(), len: 1 });
    assert_eq!(add::fields_of(&waiting, "2_x.png", &[], true).visibility, "members", "a members-only server's items are members only");
}

/// THE CATEGORY SENT IS THE CATEGORY SHOWN (the review: with none chosen the form showed the first
/// category in the list but sent none, so the relay put it in General): none chosen, the form
/// shows General, by the name the server gives it, and sends its number.
///
/// Seen red 2026-10-11 with the old rule, the first category in the list when none was chosen:
/// "with none chosen, General is shown, by its name" (left: (2, "Fire flow arts") / right: (1,
/// "Everything else")).
#[test]
fn the_category_sent_is_the_category_shown() {
    let cats = vec![
        api::Category { id: 2, name: "Fire flow arts".into(), description: String::new() },
        api::Category { id: 1, name: "Everything else".into(), description: String::new() },
    ];
    let mut form = add::Form::new(add::Source::Picked { path: "x.png".into(), name: "x.png".into(), len: 1 });
    assert_eq!(add::shown_category(&form, &cats), (1, "Everything else".to_string()), "with none chosen, General is shown, by its name");
    assert_eq!(add::fields_of(&form, "x.png", &cats, false).category_id, 1, "and its number sent");
    form.category = Some(2);
    assert_eq!(add::shown_category(&form, &cats).1, "Fire flow arts");
    assert_eq!(add::fields_of(&form, "x.png", &cats, false).category_id, 2, "what is shown is sent");
    form.category = Some(9);
    assert_eq!(add::shown_category(&form, &cats).0, 1, "a category the server no longer lists falls back to General, shown and sent");
}

/// A REPORT NAMES THE ITEM: the existing report dialog opens about the person who put it there,
/// in the context `media`, with the item by its number as evidence (the relay's A5), and the
/// signed frame carries exactly that.
///
/// Seen red 2026-10-11 with the report made about the person's profile with a note naming the
/// item (as before the relay had the context): "the report's context is media" (left:
/// Some(Profile)).
#[test]
fn a_report_names_the_item() {
    let mut state = page_state(demo_answer(), true);
    let it = state.media.list.items[1].clone();
    crate::engine::report::open_for_media(&mut state, &it);
    let d = state.reports.dialog.as_ref().expect("the report dialog opens");
    assert_eq!(d.target, "k2");
    assert_eq!(d.context, Some(crate::net::report::ReportContext::Media), "the report's context is media");
    assert_eq!(d.fixed, Some(crate::net::report::Evidence::Media { id: 3 }));
    assert_eq!(d.fixed_text, "Fibonacci homestead", "the dialog names the item");

    let seed = [9u8; 32];
    state.profile_public_key = crate::net::identity::derive_pq_identity(&seed).expect("derive").dilithium_hex;
    state.private_key_bytes = Some(seed.to_vec());
    state.reports.dialog.as_mut().unwrap().reason = "spam".into();
    let (frame, _) = crate::engine::report::prepare_send(&state, 1_791_700_000_000).expect("the report is built");
    let v: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!((v["context"].as_str(), &v["evidence"]), (Some("media"), &serde_json::json!([{"id": 3, "kind": "media"}])));
    assert!(crate::relay::handlers::reports::CONTEXTS.contains(&"media"), "a context the relay takes");
}

/// A STALE TILE IS NEVER ACTED ON (the relay's W5): a 409 to an action on an item reloads the
/// list and says so, instead of showing the relay's refusal as if something else went wrong.
///
/// Seen red 2026-10-11 without the 409 branch in `on_done`: "the page says the item changed"
/// (it showed the relay's own words and nothing was asked again).
#[test]
fn a_stale_tile_reloads_and_says_so() {
    let mut state = page_state(demo_answer(), true);
    let refused = Refusal::new(409, "That is not the Media item you were looking at: the page is out of date. Load it again.");
    on_done(&mut state, Job::Pick { id: 3, on: true }, Err(refused));
    assert_eq!(state.media.status, STALE_LINE, "the page says the item changed");
    assert!(state.media.jobs.iter().any(|(j, _)| matches!(j, Job::List { append: false, .. })), "and loads the list again");
    // Any other refusal is said as the server said it.
    on_done(&mut state, Job::Pick { id: 3, on: true }, Err(Refusal::new(403, "Only members of this server can pick Media items.")));
    assert_eq!(state.media.status, "Only members of this server can pick Media items.");
}

/// MY PICKS ARE THE ITEMS' OWN `picked_by_me` (the relay's A3): a picked item, members-only or
/// not, says Unpick; a pick updates every copy of the item on the page.
///
/// Seen red 2026-10-11 with the picks read from a separate public list of 100 (as before): "a
/// picked members-only item says Unpick" (it said Pick).
#[test]
fn my_picks_come_from_the_items_themselves() {
    let (ctx, theme) = ctx_and_theme();
    let mut answer = demo_answer();
    answer.items[1].picked_by_me = true;
    answer.items[1].visibility = "members".into();
    let mut state = page_state(answer, true);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Unpick"), "a picked members-only item says Unpick");
    on_done(&mut state, Job::Pick { id: 4, on: true }, Ok(serde_json::json!({ "ok": true, "id": 4, "picked": true, "picks": 4 })));
    let both: Vec<(bool, i64)> = state.media.list.items.iter().chain(state.media.featured.iter()).filter(|i| i.id == 4).map(|i| (i.picked_by_me, i.picks)).collect();
    assert_eq!(both, vec![(true, 4), (true, 4)], "the grid's copy and the strip's");
}

/// THE PAGE BELONGS TO ONE PERSON (the review: a restore from a recovery phrase kept the old
/// identity's list, picks and share): a new identity forgets it all, as a new server does, but an
/// unfinished upload's record stays (it says whose it is).
///
/// Seen red 2026-10-11 with `sync_server` comparing the server alone: "another identity is a
/// change" (it said nothing had changed, and kept the page).
#[test]
fn another_identity_forgets_the_page() {
    let mut state = page_state(demo_answer(), true);
    state.media.mine = Some(Mine::default());
    state.media.unfinished = vec![resumable::Unfinished { id: "abc".into(), server: state.media.server.clone(), key: "me".into(), ..Default::default() }];
    assert!(!sync_server(&mut state), "the same server and person keep everything");
    state.profile_public_key = "someone else".into();
    assert!(sync_server(&mut state), "another identity is a change");
    assert!(state.media.list.items.is_empty() && state.media.mine.is_none() && state.media.featured.is_empty(), "another person's page is forgotten");
    assert_eq!(state.media.unfinished.iter().map(|u| u.key.as_str()).collect::<Vec<_>>(), vec!["me"], "the unfinished upload's record stays");
    assert!(uploading::offered(&state).is_empty(), "but is not offered to someone else");
}

/// A MEMBERS-ONLY PICTURE REFUSED UNDER ONE TOKEN IS ASKED FOR AGAIN UNDER THE NEXT (the review: a
/// 403 after a reconnect was kept for the whole session): when the upload token changes, the
/// refusal is forgotten; and a failed picture says so and tries again on a click.
///
/// Seen red 2026-10-11 without `note_token`: "the refusal is forgotten with the old token" (it was
/// still Failed).
#[test]
fn a_refused_members_only_picture_is_tried_again_with_a_new_token() {
    let (ctx, theme) = ctx_and_theme();
    let members = Item { visibility: "members".into(), url: "/api/media/file/8".into(), ..item(8, "Night", "picture", ("me", "Me")) };
    let mut state = page_state(ListAnswer { items: vec![members.clone()], members_view: true, media_visibility: "public".into(), ..Default::default() }, true);
    let key = tiles::cache_key(&state, &members);
    state.image_cache.fail_for_test(&key, "GET .../api/media/file/8: HTTP 403");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, tiles::COULD_NOT_LOAD), "a failed picture says so");
    state.upload_token = Some((SERVER.to_string(), "tok-after-reconnect".to_string()));
    poll_upload(&ctx, &mut state);
    assert!(!matches!(state.image_cache.status(&key), crate::gui::widgets::image_cache::ImageStatus::Failed(_)), "the refusal is forgotten with the old token");

    state.image_cache.fail_for_test(&key, "again");
    click(&ctx, &theme, &mut state, "This picture could not be loaded");
    assert!(!matches!(state.image_cache.status(&key), crate::gui::widgets::image_cache::ImageStatus::Failed(_)), "a click tries again");
}

/// THE PERSON'S OWN WAITING UPLOADS ARE LISTED WITH PUT IT ON MEDIA AND REMOVE (from
/// `/api/media/mine`'s `waiting`): Remove asks first, then removes it by its file name; Put it on
/// Media opens the form for it, with the visibility choice (it was uploaded for Media).
///
/// Seen red 2026-10-11 with the list not drawn: "the waiting upload is listed" (no shape held
/// "Clip from the beach.mp4, 40 MB").
#[test]
fn the_waiting_uploads_are_listed_with_their_actions() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.media.mine = Some(Mine {
        share_bytes: 512 << 20,
        budget_left_bytes: 1 << 40,
        waiting: vec![api::Waiting { filename: "2_wait.mp4".into(), original_name: "Clip from the beach.mp4".into(), size_bytes: 40 << 20, ..Default::default() }],
        ..Default::default()
    });
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Clip from the beach.mp4, 40 MB"), "the waiting upload is listed");
    click(&ctx, &theme, &mut state, "Put it on Media");
    assert_eq!(state.media.form.as_ref().map(|f| f.source.clone()), Some(add::Source::Waiting { filename: "2_wait.mp4".into(), name: "Clip from the beach.mp4".into(), len: 40 << 20 }));
    state.media.form = None;

    // Remove: the "Remove" of the waiting row is the last on the page (the tiles' Remove are for
    // admins only, and this person is a member).
    click(&ctx, &theme, &mut state, "Remove");
    assert!(state.media.confirm_waiting.is_some(), "Remove asks first");
    assert!(state.media.jobs.iter().all(|(j, _)| !matches!(j, Job::Remove { .. })), "and sends nothing yet");
    let out = settled(&ctx, &theme, &mut state);
    let buttons = find_text_in_shapes(&out.shapes, "Remove").map(|f| f.matches).unwrap_or(0);
    assert!(buttons >= 2, "the confirmation has its own Remove");
    let dialog_remove = {
        // The dialog's button is drawn last (on top).
        let mut last = None;
        for cs in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &cs.shape {
                if t.galley.text() == "Remove" {
                    last = Some(t.pos + t.galley.size() / 2.0);
                }
            }
        }
        last
    };
    if let Some(pos) = dialog_remove {
        let m = egui::Modifiers::default();
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(pos)]);
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }
    assert!(state.media.jobs.iter().any(|(j, _)| *j == Job::Remove { id: None, from_admin: false }), "the confirmation removes it by its file name");
}

/// KEPT ITEMS SAY SO, AND A MODERATOR MAY KEEP OR STOP KEEPING ONE (the relay's A7): the badge on
/// a kept tile, and the action for an admin or a moderator, never for a member.
///
/// Seen red 2026-10-11 with `may_keep` asking for an admin only: "a moderator may keep" (no Keep
/// on the screen).
#[test]
fn kept_items_say_so_and_moderators_may_keep() {
    let (ctx, theme) = ctx_and_theme();
    let mut answer = demo_answer();
    answer.items[1].kept = true;
    let mut state = page_state(answer, true);
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, tiles::KEPT_BADGE), "a kept tile says so");
    assert!(!shows(&out, tiles::KEEP) && !shows(&out, tiles::STOP_KEEPING), "a member may not keep");
    state.chat_users.push(crate::gui::ChatUser { name: "Me".into(), public_key: "me".into(), role: "mod".into(), status: String::new() });
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, tiles::KEEP), "a moderator may keep");
    assert!(shows(&out, tiles::STOP_KEEPING), "and stop keeping a kept one");
    on_done(&mut state, Job::Keep { id: 4, on: true, from_admin: false }, Ok(serde_json::json!({ "ok": true, "id": 4, "kept": true })));
    assert!(state.media.list.items.iter().find(|i| i.id == 4).unwrap().kept, "the answer lands on the tile");
}

/// CANCEL STOPS THE UPLOAD (the review: Cancel during "Uploading..." let the worker publish
/// anyway): before the file is up, the server is told and nothing is published. (Cancel once its
/// publish is on its way: uploads_tests.rs.)
///
/// Seen red 2026-10-11 with `uploading::cancel` leaving the upload running: "the upload stops"
/// (the run finished and a publish was sent).
#[test]
fn cancel_stops_the_upload() {
    let (ctx, _theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.private_key_bytes = Some(vec![7u8; 32]);
    let file = crate::test_temp::file("media_cancel", "webm");
    std::fs::write(&file, vec![5u8; 40_000]).unwrap();
    let form = add::Form::new(add::Source::Picked { path: file.to_path_buf(), name: "clip.webm".into(), len: 40_000 });
    let fake = resumable::tests::Fake::new(8192);
    state.upload_token = Some((SERVER.to_string(), resumable::tests::TOKEN.to_string()));
    // Every piece waits for an allowance an hour away, so the upload sits started and unfinished.
    fake.s().allowance_forever = Some((upload_waits::now_ms() + 3_600_000, Some(3600.0)));
    uploading::start_with(&mut state, &form, fake.clone()).expect("it starts");
    let local = state.media.uploads[0].local;
    // Wait for it to start and wait, then Cancel.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.media.uploads.iter().any(|e| !matches!(e.run.phase, resumable::Phase::Waiting { .. }) || e.record.id.is_empty()) {
        uploading::poll(&ctx, &mut state);
        assert!(std::time::Instant::now() < deadline, "the upload started and waits");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(state.media.unfinished.iter().any(|u| !u.id.is_empty()), "its record was kept once the server named it");
    uploading::cancel(&mut state, local);
    while !state.media.uploads.is_empty() {
        uploading::poll(&ctx, &mut state);
        assert!(std::time::Instant::now() < deadline, "the upload stops");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(fake.s().cancelled, "the server was told");
    assert!(state.media.jobs.iter().all(|(j, _)| !matches!(j, Job::Publish { .. })), "nothing is published");
    assert!(state.media.unfinished.is_empty(), "and its record is forgotten");
    assert_eq!(state.media.status, uploading::CANCELLED_LINE);
}

/// AN UPLOAD A RESTART CUT OFF IS OFFERED AGAIN, AND CARRIES ON WITH ONLY THE REST: the page
/// shows it with Carry on, which puts it back in the uploads list with what its form said and
/// sends what the server does not have; a file changed since is not carried on.
///
/// Seen red 2026-10-11 with the offer drawn only while an upload runs: "the page offers to carry
/// it on" (no shape held "An upload did not finish").
#[test]
fn an_upload_a_restart_cut_off_is_offered_again() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    let file = crate::test_temp::file("media_resume_page", "webm");
    let bytes: Vec<u8> = (0..20_000u32).map(|i| (i % 253) as u8).collect();
    std::fs::write(&file, &bytes).unwrap();
    state.upload_token = Some((SERVER.to_string(), resumable::tests::TOKEN.to_string()));
    let record = resumable::Unfinished {
        server: state.media.server.clone(),
        key: "me".into(),
        id: "0123456789abcdef0123456789abcdef".into(),
        path: file.to_path_buf(),
        name: "clip.webm".into(),
        size: 20_000,
        modified_ms: resumable::modified_ms(&file),
        title: "Dusk".into(),
        category_id: 1,
        ..Default::default()
    };
    state.media.unfinished = vec![record.clone()];
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "An upload did not finish: clip.webm"), "the page offers to carry it on");
    assert!(shows(&out, "Carry on") && shows(&out, "Cancel it"));

    let fake = resumable::tests::Fake::new(8192);
    {
        let mut s = fake.s();
        s.id = Some(record.id.clone());
        s.size = 20_000;
        s.blake3 = blake3::hash(&bytes).to_hex().to_string();
        s.data = bytes[..8192].to_vec();
    }
    uploading::resume_with(&mut state, &record.id, fake.clone());
    let entry = state.media.uploads.first().expect("it carries on, in the list");
    assert_eq!(entry.record.title, "Dusk", "with what its form said");
    assert_eq!(entry.run.id.as_deref(), Some(record.id.as_str()));
    assert!(uploading::offered(&state).is_empty(), "and is no longer offered while it runs");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !fake.s().log.iter().any(|l| l.starts_with("piece 8192")) {
        assert!(std::time::Instant::now() < deadline, "it sends the rest: {:?}", fake.s().log);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!fake.s().log.iter().any(|l| l == "start" || l.starts_with("piece 0+")), "nothing new started, nothing sent twice");
    uploading::abandon_all(&state.media.uploads);
    state.media.uploads.clear();

    // A file changed since is not carried on while the server lacks pieces of it: the server is
    // told to drop what arrived, the record goes, and the person is told. Round 4 (D4-1) changed
    // how: round 3 dropped it here at once, before asking the server anything; now the upload
    // asks where the server got first, since with every piece there it finishes from the server's
    // copy (round4_tests.rs `carry_on_finishes_from_the_servers_copy_whatever_the_local_file`).
    std::fs::write(&file, b"something else").unwrap();
    let fresh = resumable::tests::Fake::new(8192);
    {
        let mut s = fresh.s();
        s.id = Some(record.id.clone());
        s.size = 20_000;
        s.data = bytes[..8192].to_vec();
    }
    uploading::resume_with(&mut state, &record.id, fresh.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !state.media.uploads.is_empty() {
        uploading::poll(&ctx, &mut state);
        assert!(std::time::Instant::now() < deadline, "the upload ends");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(state.media.unfinished.is_empty(), "a changed file is not carried on");
    assert!(fresh.s().cancelled, "and the server is told to drop what arrived");
    assert_eq!(state.media.status, format!("clip.webm: {} {}", resumable::FILE_CHANGED, resumable::NOTHING_STAYS));
}
