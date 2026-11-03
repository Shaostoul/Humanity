//! Tests of the Files page's places (places.rs) in the launcher's red, green and blue
//! (2026-11-03). The operator, on the navigation mock-up of the page: "I feel like that first image
//! should be ordered as your private local files files, your publicly shared files, then files by
//! friends/servers? Maintains the RGB styling. R for private, B for servers." Each test reads what
//! was DRAWN, and each was seen failing once on purpose: the break and its failure are in its
//! comment.

use super::super::api::{self, ListAnswer};
use super::super::shared_files::SharedFileRow;
use super::super::tests::{ctx_and_theme, demo_answer, item, page_state, settled};
use super::super::{apply_list, list_filters, list_owner, reload_list, FilesState};
use super::*;
use crate::gui::screen_surface::find_text_in_shapes;

/// Every text shape, recursing into shape lists.
fn visit<'a>(shape: &'a egui::Shape, f: &mut dyn FnMut(&'a egui::epaint::TextShape)) {
    match shape {
        egui::Shape::Text(t) => f(t),
        egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, f)),
        _ => {}
    }
}

/// The colour `text` is drawn in (its first section's, the first time it is drawn), when it is.
fn color_of(out: &egui::FullOutput, text: &str) -> Option<egui::Color32> {
    let mut found = None;
    for cs in &out.shapes {
        visit(&cs.shape, &mut |t| {
            if found.is_none() && t.galley.text() == text {
                found = t.galley.job.sections.first().map(|s| s.format.color);
            }
        });
    }
    found
}

/// Where `text` is drawn, exactly that text.
fn top_of(out: &egui::FullOutput, text: &str) -> f32 {
    let found = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn"));
    assert_eq!(found.text, text, "{text:?} is drawn as itself");
    found.rect.top()
}

/// THE PLACES COME RED, GREEN, BLUE (2026-11-03): on this computer the HumanityOS folder and Your
/// folders; what you share, Yours on <server> and Your shared files; what everyone shares, Shared
/// on <server>, Shared files and, for an admin only, Admin. Admin left set for someone who is not
/// one shows what is shared, and the page still opens on Shared on <server>. Every place has its
/// own id, its name and its hint by key (the words are there, never the key shown).
///
/// The order this pins changed on 2026-11-03 from server first (Shared on, Shared files, Admin,
/// then the computer's two) to the operator's private, yours, everyone's. Seen red 2026-11-03 with
/// `listed` put back to the old order: "red, then green, then blue" (left: [Shared, SharedFiles,
/// AppFolder, YourFolders, Yours, YourSharedFiles], right: [AppFolder, YourFolders, Yours,
/// YourSharedFiles, Shared, SharedFiles]).
#[test]
fn the_places_come_red_then_green_then_blue_and_admin_is_an_admins_alone() {
    assert_eq!(
        listed(false),
        vec![Place::AppFolder, Place::YourFolders, Place::Yours, Place::YourSharedFiles, Place::Shared, Place::SharedFiles],
        "red, then green, then blue"
    );
    assert_eq!(
        listed(true),
        vec![Place::AppFolder, Place::YourFolders, Place::Yours, Place::YourSharedFiles, Place::Shared, Place::SharedFiles, Place::Admin],
        "Admin last, with what everyone shares"
    );
    let groups: Vec<Group> = listed(true).into_iter().map(Group::of).collect();
    let mut seen: Vec<Group> = groups.clone();
    seen.dedup();
    assert_eq!(seen, Group::ALL.to_vec(), "each group's places together, in the groups' order");
    assert_eq!(shown_place(Place::Admin, false), Place::Shared);
    assert_eq!(shown_place(Place::Admin, true), Place::Admin);
    assert_eq!(shown_place(Place::AppFolder, false), Place::AppFolder);
    assert_eq!(Place::default(), Place::Shared, "the page still opens on what everyone shares");
    assert_eq!(FilesState::default().place, Place::Shared);
    let ids: std::collections::HashSet<&str> = listed(true).into_iter().map(id_of).collect();
    assert_eq!(ids.len(), 7, "each place its own id");

    let state = page_state(demo_answer(), true);
    for place in listed(true) {
        let key = hint_key(place);
        assert_ne!(t(key), key, "{place:?}'s hint is in en.json");
        let name = label(&state, place);
        assert!(!name.is_empty() && !name.starts_with("files."), "{place:?}'s name is words: {name:?}");
    }
    for g in Group::ALL {
        assert!(!g.heading().starts_with("files."), "{g:?}'s heading is words");
    }
    assert_eq!(label(&state, Place::Shared), "Shared on 127.0.0.1:9");
    assert_eq!(label(&state, Place::Yours), "Yours on 127.0.0.1:9");
    let mut offline = page_state(demo_answer(), true);
    offline.server_url.clear();
    assert_eq!((label(&offline, Place::Shared), label(&offline, Place::Yours)), ("Shared on a server".to_string(), "Yours on a server".to_string()));
}

/// The launcher's picker (showroom.rs), drawn as a first run opens it with Open Net picked: its
/// three trust models' sections, each titled in its colour.
fn launcher(ctx: &egui::Context, theme: &Theme) -> egui::FullOutput {
    crate::config::keep_saves_off_disk();
    let mut gs = GuiState::default();
    gs.showroom_mode = 0;
    gs.launcher_homes_loaded = true;
    gs.character_name = "Wanderer".into();
    gs.launcher_where_kind = crate::gui::LauncherWhere::Server;
    let raw = |_: ()| egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 3000.0))), ..Default::default() };
    let mut out = ctx.run(raw(()), |ctx| crate::gui::pages::showroom::draw(ctx, theme, &mut gs));
    for _ in 0..2 {
        out = ctx.run(raw(()), |ctx| crate::gui::pages::showroom::draw(ctx, theme, &mut gs));
    }
    out
}

/// EACH GROUP DRAWS IN ITS COLOUR, THE LAUNCHER'S (2026-11-03): the group headings over the
/// places, "Private, on this computer", "What you share" and "What everyone shares", are drawn in
/// `danger`, `success` and `info`, the very colours the character and server picker titles Your
/// Homes, Open Net and Closed Net in (drawn here too and compared), so the same trust model reads
/// the same on both screens and the Settings theme moves them together. The fold of "How this page
/// works" heads each place's line with its group's colour.
///
/// Seen red 2026-11-03 with `Group::color` giving the places' colours before the groups (this
/// computer's `nav_tools`, the server's `nav_legacy_red`): "Private, on this computer is drawn in
/// the launcher's Your Homes colour" (left: Some(#34_97_DB_FF), the computer's old blue; right:
/// Some(#E5_3F_33_FF), the launcher's red).
#[test]
fn the_groups_draw_in_the_launchers_colours() {
    let (ctx, theme) = ctx_and_theme();
    let homes = launcher(&ctx, &theme);
    let pairs = [
        (Group::Private, t("characters.your_homes")),
        (Group::YouShare, t("characters.open_net")),
        (Group::EveryoneShares, t("characters.closed_net")),
    ];
    let (fctx, ftheme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    let out = settled(&fctx, &ftheme, &mut state);
    for (group, title) in pairs {
        let launcher_color = color_of(&homes, title).unwrap_or_else(|| panic!("the launcher draws {title:?}"));
        assert_eq!(color_of(&out, group.heading()), Some(launcher_color), "{} is drawn in the launcher's {title} colour", group.heading());
        assert_eq!(group.color(&theme), launcher_color, "{group:?}'s token is the launcher's");
    }
    assert_eq!(
        (Group::Private.color(&theme), Group::YouShare.color(&theme), Group::EveryoneShares.color(&theme)),
        (theme.danger(), theme.success(), theme.info()),
        "red, green, blue: the theme's own tokens, never a colour written here"
    );
    // Each heading over the first of its places, the groups in order down the left pane.
    let (red, green, blue) = (top_of(&out, Group::Private.heading()), top_of(&out, Group::YouShare.heading()), top_of(&out, Group::EveryoneShares.heading()));
    let at = |p: Place| top_of(&out, &label(&state, p));
    assert!(red < at(Place::AppFolder) && at(Place::YourFolders) < green, "red's places under its heading");
    assert!(green < at(Place::Yours) && at(Place::YourSharedFiles) < blue, "green's under its");
    assert!(blue < at(Place::Shared) && at(Place::Shared) < at(Place::SharedFiles), "blue's under its");

    // The fold heads each place's line in its group's colour.
    state.settings.files_how_open = true;
    let out = settled(&fctx, &ftheme, &mut state);
    for line in [super::super::how::line_of(Place::Yours), super::super::how::line_of(Place::YourSharedFiles)] {
        assert!(find_text_in_shapes(&out.shapes, line).is_some(), "the fold says {line:?}");
    }
}

/// YOURS ON <SERVER> IS THE LIST ASKED FOR WITH THE PERSON FILTER SET TO YOU (2026-11-03): shown,
/// the list is asked for again with `by` = your key and nothing else (whatever the filters picked
/// for everyone's list), and until it answers nobody else's rows are shown under it, nor someone
/// else's item left open; answered, your rows, with no featured shelf and no filters (they are about
/// everyone's). Back on Shared on <server> the list is everyone's again with the filters as they
/// were. YOUR SHARED FILES is Shared files with only your rows, and a stranger's file chosen in
/// Shared files is not shown under it.
///
/// Seen red 2026-11-03 with `list_filters` giving the filters picked whatever the place: "Yours
/// is asked for by your key" (left: Some("k2"), Ann, picked for everyone's list; right:
/// Some("me")); and, built again, with `rows_of` ignoring `mine`: "a stranger's file is not listed
/// under Your shared files, nor shown chosen".
#[test]
fn yours_is_your_own_items_and_your_shared_files_are_your_own_files() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.files.filters.by = Some("k2".into());
    state.files.filters.q = "staff".into();
    // Ann's item open on the right.
    state.files.item = Some(super::super::item_view::ItemPage { id: 3, item: Some(item(3, "Fibonacci homestead", "model", ("k2", "Ann"))), ..Default::default() });
    state.files.place = Place::Yours;
    assert_eq!(list_owner(&state), Place::Yours);
    reload_list(&mut state);
    assert_eq!(state.files.list_place, Place::Yours);
    let f = list_filters(&state);
    assert_eq!(f.by.as_deref(), Some("me"), "Yours is asked for by your key");
    assert_eq!(f, api::Filters { by: Some("me".into()), ..Default::default() }, "and nothing else: the filters picked are everyone's list's");
    assert!(api::list_request(&f, None, api::PAGE, "me", None).path.contains("by=me"), "the server is asked for yours");
    assert!(state.files.list.items.is_empty() && !state.files.answered, "until it answers, none of everyone's rows is shown under Yours");
    assert!(state.files.item.is_none(), "nor someone else's item left open beside it");
    let out = settled(&ctx, &theme, &mut state);
    assert!(find_text_in_shapes(&out.shapes, "Contact staff, slow spin").is_none(), "Shaostoul's item is not under Yours");

    // Your own items come.
    let mine = ListAnswer { items: vec![item(9, "Seedlings, day 12", "picture", ("me", "You"))], members_view: true, media_visibility: "public".into(), ..Default::default() };
    apply_list(&mut state.files, mine, false);
    state.files.jobs.clear();
    let out = settled(&ctx, &theme, &mut state);
    assert!(find_text_in_shapes(&out.shapes, "Seedlings, day 12").is_some(), "your item, as a row");
    assert!(find_text_in_shapes(&out.shapes, "All categories").is_none(), "no filters under Yours");
    assert!(find_text_in_shapes(&out.shapes, "Suggest a category").is_none(), "a new category is about everyone's");
    assert!(find_text_in_shapes(&out.shapes, "Add to Files").is_some(), "Add to Files stays: it adds to yours");

    // Nothing of yours yet: said so.
    apply_list(&mut state.files, ListAnswer { members_view: true, media_visibility: "public".into(), ..Default::default() }, false);
    let out = settled(&ctx, &theme, &mut state);
    assert!(find_text_in_shapes(&out.shapes, t("files.places.yours_empty")).is_some(), "an empty Yours says what puts something there");

    // Back to everyone's: asked again with the filters as they were.
    state.files.place = Place::Shared;
    reload_list(&mut state);
    assert_eq!(list_filters(&state).by.as_deref(), Some("k2"), "everyone's list keeps the person picked");
    assert_eq!(list_filters(&state).q, "staff");
    state.files.jobs.clear();

    // Your shared files: only yours.
    state.files.place = Place::YourSharedFiles;
    let shared = &mut state.files.shared;
    shared.shared_server = crate::gui::pages::chat::norm_server_url(&state.server_url);
    shared.shared_loaded = true;
    let row = |url: &str, name: &str, key: &str, who: &str| SharedFileRow { url: url.into(), name: name.into(), size_bytes: 1000, uploader_key: key.into(), uploader_name: who.into() };
    shared.shared_files = vec![row("/uploads/1_bracket.glb", "bracket.glb", "me", "You"), row("/uploads/2_staff-head.stl", "staff-head.stl", "ann", "Ann")];
    shared.selected = Some("/uploads/2_staff-head.stl".into());
    let out = settled(&ctx, &theme, &mut state);
    assert!(find_text_in_shapes(&out.shapes, "bracket.glb").is_some(), "your file is listed");
    assert!(find_text_in_shapes(&out.shapes, "staff-head.stl").is_none(), "a stranger's file is not listed under Your shared files, nor shown chosen");
    assert!(find_text_in_shapes(&out.shapes, t("files.places.your_shared_files_heading")).is_some(), "under its own heading");
    state.files.place = Place::SharedFiles;
    let out = settled(&ctx, &theme, &mut state);
    assert!(find_text_in_shapes(&out.shapes, "staff-head.stl").is_some(), "Shared files lists everyone's");
    state.files.shared.shared_files.retain(|r| r.uploader_key != "me");
    state.files.place = Place::YourSharedFiles;
    let out = settled(&ctx, &theme, &mut state);
    assert!(find_text_in_shapes(&out.shapes, t("files.places.your_shared_files_empty")).is_some(), "none of yours: said so");
    assert!(takes_drops(Place::YourSharedFiles) && takes_drops(Place::SharedFiles) && !takes_drops(Place::Yours), "files dropped on either shared files place go to Shared Files");
}
