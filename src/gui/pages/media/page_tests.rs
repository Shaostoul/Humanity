//! Headless tests of the Media page (media.rs and its children): the page drawn into an egui
//! context with no window and no network, and what it drew read back. Each was seen to fail once
//! before it passed; what it saw red is in its comment.

use super::api::{Item, ListAnswer};
use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::theme::Theme;

/// A server address that answers nothing at once (a closed port on this machine), so a picture a
/// test lets load fails fast instead of reaching anywhere.
const SERVER: &str = "http://127.0.0.1:9";

fn ctx_and_theme() -> (egui::Context, Theme) {
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
fn settled(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, theme, state, Vec::new());
    }
    frame(ctx, theme, state, Vec::new())
}

fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = settled(ctx, theme, state);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

fn shows(out: &egui::FullOutput, text: &str) -> bool {
    find_text_in_shapes(&out.shapes, text).is_some()
}

/// An app on `SERVER`, the page's first answer `answer` already in, signed in as "me" when
/// `signed_in`. Nothing is asked of the network: the first page counts as asked.
pub(crate) fn page_state(answer: ListAnswer, signed_in: bool) -> GuiState {
    let mut state = GuiState::default();
    state.server_url = SERVER.to_string();
    state.profile_public_key = "me".to_string();
    sync_server(&mut state);
    state.media.requested = true;
    apply_list(&mut state.media, answer, false);
    if signed_in {
        state.upload_token = Some((SERVER.to_string(), "tok".to_string()));
    }
    state
}

pub(crate) fn item(id: i64, title: &str, kind: &str, by: (&str, &str)) -> Item {
    Item {
        id,
        url: format!("/uploads/{id}_file"),
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
/// people's, none of Remove or Feature for a member who is not an admin).
///
/// Seen red 2026-10-11 with the featured strip drawn from every item: "an item that is not
/// featured is only in the grid" (left: 2 / right: 1). (The first version counted only the
/// featured title, which a strip of everything also shows twice, and stayed green; so this check.)
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
    let exact = |out: &egui::FullOutput, t: &str| find_text_in_shapes(&out.shapes, t).is_some_and(|f| f.text == t);
    assert!(!exact(&out, "Feature") && !exact(&out, "Unfeature"), "no Feature button for a member");
    assert!(!shows(&out, "Remove"), "no Remove on other people's items for a member");

    // An admin sees Remove and Feature on every tile.
    state.chat_users.push(crate::gui::ChatUser { name: "Me".into(), public_key: "me".into(), role: "admin".into(), status: String::new() });
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Remove") && exact(&out, "Unfeature") && exact(&out, "Feature"), "an admin may remove and feature");
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

    // The picture area is the box above the title; the label's name is drawn in it.
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

/// SOMEONE NOT SIGNED IN SEES ONLY PUBLIC ITEMS AND IS TOLD WHY: the members' line when the answer
/// is not the members' view, and none when it is; no Add to Media without a sign-in.
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

/// A FILE OVER THE SHARE IS STOPPED BEFORE IT STARTS: picked with too little of the share left,
/// it says so with the numbers and nothing is uploaded; in the form, once the share is known to
/// be too small, the line shows and Add to Media stays off.
///
/// Seen red 2026-10-11 with `picked` not checking the share: "nothing is uploaded" (a form opened
/// for it), and with the form's button enabled and its send unguarded: "pressing it starts
/// nothing" (a Publish job was spawned).
#[test]
fn a_file_over_the_share_is_stopped_before_it_starts() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    let mut settings = crate::relay::storage::ServerSettings::default();
    settings.media_per_person_mb = 1;
    state.server_settings = Some(settings);
    state.media.my_use = Some(900 * 1024);
    let file = crate::test_temp::file("media_over_share", "png");
    std::fs::write(&file, vec![0u8; 300 * 1024]).unwrap();

    add::picked(&mut state, &file);
    assert!(state.media.form.is_none() && state.media.jobs.is_empty(), "nothing is uploaded");
    assert!(state.media.status.contains("This file is 0.3 MB, more than the 0.1 MB left of your share"), "it says why: {}", state.media.status);

    // The form opened before the share was known; now it is, and too small.
    state.media.form = Some(add::Form::new(add::Source::Picked { path: file.to_path_buf(), name: "big.png".into(), len: 300 * 1024 }));
    state.private_key_bytes = Some(vec![7u8; 32]);
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "You have 0.1 MB of your 1 MB left"), "the share is shown above the form");
    assert!(shows(&out, "more than the 0.1 MB left of your share"), "the form says why it cannot go");
    click(&ctx, &theme, &mut state, "Upload and add to Media");
    assert!(state.media.jobs.iter().all(|(j, _)| !matches!(j, Job::Publish { .. })), "pressing it starts nothing");

    // The control: with room in the share the same press starts the upload, so the press above
    // did land on the button (it fails at once here: nothing listens at the test's address).
    state.server_settings.as_mut().unwrap().media_per_person_mb = 10;
    click(&ctx, &theme, &mut state, "Upload and add to Media");
    assert!(
        state.media.jobs.iter().any(|(j, _)| matches!(j, Job::Publish { uploaded: true })),
        "with room, the press starts the upload (form: {:?}; notices: {:?})",
        state.media.form,
        state.pending_notices
    );
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
    assert_eq!(add::fields_of(&form, "1700_homestead.glb", true).visibility, "public", "published as public whatever is set");

    // A file picked here may be members only, and is on a members-only server.
    let picked = add::Form { members_only: true, ..add::Form::new(add::Source::Picked { path: "x.png".into(), name: "x.png".into(), len: 1 }) };
    assert_eq!(add::fields_of(&picked, "x.png", false).visibility, "members");
    let plain = add::Form::new(add::Source::Picked { path: "x.png".into(), name: "x.png".into(), len: 1 });
    assert_eq!(add::fields_of(&plain, "x.png", true).visibility, "members", "a members-only server's items are members only");
}

/// A REPORT NAMES THE ITEM: the existing report dialog opens about the person who put it there,
/// its note naming the item.
///
/// Seen red 2026-10-11 with the note not set: "the note names the item" (it was empty).
#[test]
fn a_report_names_the_item() {
    let mut state = page_state(demo_answer(), true);
    let it = state.media.list.items[1].clone();
    tiles::report(&mut state, &it);
    let d = state.reports.dialog.as_ref().expect("the report dialog opens");
    assert_eq!(d.target, "k2");
    assert!(d.note.contains("\"Fibonacci homestead\" (number 3"), "the note names the item: {}", d.note);
}
