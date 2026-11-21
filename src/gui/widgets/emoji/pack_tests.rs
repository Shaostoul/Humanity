// Rung 2's tests (2026-11-20, docs/design/emoji.md): the pack's pictures drawn by resvg, measured on
// the drawn pixels; the reuse switch on the kept textures; the reaction button and the credit. Each
// was seen failing once on purpose; the break and what it said are recorded at the test.

use std::collections::HashMap;
use std::sync::Arc;

use egui::mutex::RwLock;
use egui::ColorImage;

use super::button::{credit_parts, CreditPart};
use super::outline::Outline;
use super::pack::{list_due, rasterise, PackPictures, PackStamp, LIST_REUSE_OFF_S};

fn pack_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("assets").join("emoji")
}

fn svg(file: &str) -> Vec<u8> {
    std::fs::read(pack_dir().join("openmoji").join(format!("{file}.svg"))).expect("the pack's picture")
}

/// Every `fill="#RRGGBB"` colour the picture's file names (its own colours, read from the file, so
/// the test never restates them).
fn fills(svg: &[u8]) -> Vec<[u8; 3]> {
    let text = String::from_utf8_lossy(svg);
    text.match_indices("fill=\"#")
        .filter_map(|(at, m)| {
            let hex = text.get(at + m.len()..at + m.len() + 6)?;
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
        })
        .collect()
}

/// The share of the picture's pixels that are drawn (mostly opaque), and its most common drawn
/// colour (un-premultiplied, rounded to steps of 8 so anti-aliased edges do not split the count).
fn measure(image: &ColorImage) -> (f32, [u8; 3]) {
    let mut drawn = 0usize;
    let mut counts: HashMap<[u8; 3], usize> = HashMap::new();
    for p in &image.pixels {
        if p.a() >= 250 {
            drawn += 1;
            let q = |c: u8| (c / 8) * 8;
            *counts.entry([q(p.r()), q(p.g()), q(p.b())]).or_default() += 1;
        }
    }
    let top = counts.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c).unwrap_or([0, 0, 0]);
    (drawn as f32 / image.pixels.len() as f32, top)
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= 10)
}

/// A RASTERISED EMOJI IS THE PICTURE, IN ITS COLOURS: the grinning face drawn 48 pixels square covers
/// about a third of its square (a circle of radius 23 in a 72 box, plus its outline: not blank, not
/// a filled square), and its most common colour is the face's yellow from the file's own fills; the
/// red heart's is its red. Measured on the drawn pixels, not on what the renderer was told.
///
/// Seen red 2026-11-20 with `resvg::render` taken out of `rasterise` (a blank image of the asked
/// size, what a renderer that draws nothing gives): "grinning face: drawn share 0 (want 0.25..0.6)".
#[test]
fn a_rasterised_emoji_has_the_pictures_colours() {
    for (file, what, want) in [("1F600", "grinning face", [0xFC, 0xEA, 0x2B]), ("2764", "red heart", [0xD2, 0x2F, 0x27])] {
        let bytes = svg(file);
        assert!(fills(&bytes).contains(&want), "{what}: the file's own fills hold the colour the test looks for");
        let image = rasterise(&bytes, 48, Outline::NONE).expect("the picture draws");
        assert_eq!(image.size, [48, 48], "{what}: drawn at the asked size");
        let (share, top) = measure(&image);
        let range = if file == "1F600" { 0.25..0.6 } else { 0.15..0.6 };
        assert!(range.contains(&share), "{what}: drawn share {share} (want {range:?})");
        assert!(near(top, want), "{what}: most common colour {top:?}, the file's fill is {want:?}");
    }
}

/// THE WHOLE PACK DRAWS: every one of the pack's pictures (4,495 in OpenMoji 17.0.0) rasterises at
/// a chat size and draws something (the finding's open question, "resvg 0.50's behaviour on the whole
/// pack"). Also prints the time per picture.
///
/// Seen red 2026-11-20 with `resvg::render` taken out of `rasterise`: "4495 of 4495 pictures did not
/// draw: 0023-FE0F-20E3: blank, 002A-FE0F-20E3: blank, ...". Measured that day (resvg optimised, a
/// debug test build, 24 px): 0.853 ms a picture, reading the file included.
#[test]
fn every_picture_in_the_pack_draws() {
    let index = super::EmojiIndex::load(&pack_dir().join("index.json")).expect("the pack's list");
    let dir = pack_dir().join("openmoji");
    let mut failed = Vec::new();
    let mut n = 0usize;
    let start = std::time::Instant::now();
    for entry in std::fs::read_dir(&dir).expect("the pack's folder") {
        let path = entry.expect("a file").path();
        if path.extension().and_then(|e| e.to_str()) != Some("svg") {
            continue;
        }
        n += 1;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| rasterise(&b, 24, Outline::NONE)) {
            Ok(image) if image.pixels.iter().any(|p| p.a() > 0) => {}
            Ok(_) => failed.push(format!("{name}: blank")),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    let per_ms = start.elapsed().as_secs_f64() * 1000.0 / n.max(1) as f64;
    eprintln!("every_picture_in_the_pack_draws: {n} pictures at 24 px, {per_ms:.3} ms each (read + parse + draw)");
    assert_eq!(n, index.len(), "one file per picture in the list");
    assert!(failed.is_empty(), "{} of {n} pictures did not draw: {}", failed.len(), failed.join(", "));
}

fn textures() -> Arc<RwLock<egui::epaint::TextureManager>> {
    Arc::new(RwLock::new(egui::epaint::TextureManager::default()))
}

/// THE REUSE SWITCH ON THE KEPT PICTURES (CLAUDE.md's no-caches rule): with reuse on, a picture is
/// drawn once per size and the same texture comes back; a new size or a new pack version draws it
/// again; with reuse off ("Turn off all reuse", HUMANITY_NO_REUSE=1) every ask reads and draws the
/// file again, into a new texture. Also prints the measured cost of a first draw and a reused one.
///
/// Seen red 2026-11-20 with the keep decision ignoring `reuse` (`Some(k.version) == Some(version)` in
/// place of `reuse::keeps`): "reuse off: the second ask draws again" (left: 1, right: 2). Measured
/// that day (debug test build, resvg optimised): first draw 3.7 ms (the process's first picture:
/// cold file and code), reused 0.006 ms.
#[test]
fn with_reuse_off_a_second_draw_rasterises_again() {
    let pics = PackPictures::new(pack_dir(), textures());
    let t0 = std::time::Instant::now();
    let a = pics.picture_with("openmoji", "1F600", 24, Outline::NONE, true, 0).expect("drawn");
    let first = t0.elapsed();
    let t1 = std::time::Instant::now();
    let b = pics.picture_with("openmoji", "1F600", 24, Outline::NONE, true, 0).expect("kept");
    let reused = t1.elapsed();
    eprintln!("emoji picture at 24 px: first draw {:.3} ms, reused {:.4} ms", first.as_secs_f64() * 1000.0, reused.as_secs_f64() * 1000.0);
    assert_eq!(pics.drawn(), 1, "reuse on: drawn once");
    assert_eq!(a, b, "reuse on: the same texture comes back");

    pics.picture_with("openmoji", "1F600", 32, Outline::NONE, true, 0).expect("drawn");
    assert_eq!(pics.drawn(), 2, "a new size is drawn");
    pics.picture_with("openmoji", "1F600", 24, Outline::NONE, true, 1).expect("drawn");
    assert_eq!(pics.drawn(), 3, "a new pack version draws again");

    let off = PackPictures::new(pack_dir(), textures());
    let c = off.picture_with("openmoji", "1F600", 24, Outline::NONE, false, 0).expect("drawn");
    let d = off.picture_with("openmoji", "1F600", 24, Outline::NONE, false, 0).expect("drawn again");
    assert_eq!(off.drawn(), 2, "reuse off: the second ask draws again");
    assert_ne!(c, d, "reuse off: into a new texture");
}

/// A PICTURE THAT CANNOT BE DRAWN STAYS TEXT, AND IS NOT READ AGAIN EVERY FRAME: a name the pack does
/// not have gives no texture (the emoji stays the font's glyph); with reuse on it is not tried again;
/// a name that could leave the pack's folder is refused without reading anything.
///
/// Seen red 2026-11-20 with a failed draw not kept: "a missing picture is tried once" (left: 2, right: 1).
#[test]
fn a_missing_picture_stays_text_and_is_tried_once() {
    let pics = PackPictures::new(pack_dir(), textures());
    assert!(pics.picture_with("openmoji", "NOPE", 24, Outline::NONE, true, 0).is_none(), "no texture for a missing picture");
    assert!(pics.picture_with("openmoji", "NOPE", 24, Outline::NONE, true, 0).is_none());
    assert_eq!(pics.drawn(), 1, "a missing picture is tried once");
    assert!(pics.picture_with("..", "1F600", 24, Outline::NONE, true, 0).is_none(), "a set outside the pack is refused");
    assert!(pics.picture_with("openmoji", "../index", 24, Outline::NONE, true, 0).is_none(), "a file outside the set is refused");
}

/// WHEN THE INSTALLED LIST IS READ AGAIN: when the pack changed (the watcher bumped its version),
/// whatever the switch; with reuse off once it is `LIST_REUSE_OFF_S` old; with reuse on never
/// otherwise.
///
/// Seen red 2026-11-20 with `list_due` ignoring `reuse`: "reuse off: an old list is read again".
#[test]
fn the_list_is_read_again_when_the_pack_changes_or_on_the_reuse_off_schedule() {
    let stamp = PackStamp { version: 3, read_at: std::time::Instant::now() };
    assert!(!list_due(&stamp, 3, true, 1.0e6), "reuse on: kept however old while the pack is unchanged");
    assert!(list_due(&stamp, 4, true, 0.0), "the pack changed: read again");
    assert!(!list_due(&stamp, 3, false, LIST_REUSE_OFF_S * 0.5), "reuse off: a fresh list is kept");
    assert!(list_due(&stamp, 3, false, LIST_REUSE_OFF_S), "reuse off: an old list is read again");
}

/// THE CREDIT'S WORDS ARE CUT WHERE THE WEBSITE CUTS THEM: `emoji.credit` gives words, the OpenMoji
/// link, words, the licence link, as `emojiCredit` in web/chat/view/pillPopup.js fills them.
///
/// Seen red 2026-11-20 with `credit_parts` keeping each `{{name}}` as words: "the credit's pieces"
/// (left: [.., Words("{{openmoji}}"), .., Words("{{licence}}")]).
#[test]
fn the_credit_is_cut_at_its_two_links() {
    let parts = credit_parts(humanity_core::i18n::t("emoji.credit"));
    assert_eq!(
        parts,
        vec![
            CreditPart::Words("All emojis designed by "),
            CreditPart::Link("openmoji"),
            CreditPart::Words(" – the open-source emoji and icon project. License: "),
            CreditPart::Link("licence"),
        ],
        "the credit's pieces"
    );
}

/// A REACTION BUTTON DRAWS ITS EMOJI AS THE PACK'S PICTURE: the button lays its emoji out through
/// the installed pictures, so a frame paints one picture inside the button's rectangle; with nothing
/// installed it paints none (the font's glyph).
///
/// Seen red 2026-11-20 with `button` painting its galley but not its pictures: "one picture" (left: 0, right: 1).
#[test]
fn a_reaction_button_draws_its_emoji_as_a_picture() {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let (handle, source) = super::tests::stand_in(&ctx);
    super::install(&ctx, Some(source));
    let mut button_rect = egui::Rect::NOTHING;
    let frame = |ctx: &egui::Context, rect: &mut egui::Rect| {
        ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                *rect = super::button::button(ui, "\u{1F44D}", egui::FontId::proportional(16.0), egui::vec2(26.0, 22.0), None).rect;
            });
        })
    };
    frame(&ctx, &mut button_rect);
    let out = frame(&ctx, &mut button_rect);
    let pics = painted_images(&out, handle.id());
    assert_eq!(pics.len(), 1, "one picture");
    assert!(button_rect.contains_rect(pics[0]), "the picture {:?} is inside the button {button_rect:?}", pics[0]);

    super::install(&ctx, None);
    let out = frame(&ctx, &mut button_rect);
    assert!(painted_images(&out, handle.id()).is_empty(), "nothing installed: no picture");
}

/// The rectangles a frame painted with `texture`.
fn painted_images(out: &egui::FullOutput, texture: egui::TextureId) -> Vec<egui::Rect> {
    crate::gui::pages::chat::inline_video::tests::pictures_of(out).into_iter().filter(|(t, _)| *t == texture).map(|(_, r)| r).collect()
}

/// THE PACK'S PICTURES ON A REAL CONTEXT: `install_pack`'s source draws a real emoji through the
/// context's own textures, so the frame's texture delta uploads an image of the asked size.
///
/// Seen red 2026-11-20 with `rasterise` drawing at least 24 px whatever it was asked: "uploaded at
/// the asked size" (left: [24, 24], right: [20, 20]).
#[test]
fn the_installed_pack_uploads_a_drawn_picture() {
    let ctx = egui::Context::default();
    let source = super::pack::pack_source(&ctx, &pack_dir()).expect("the pack installs");
    assert!(source.pack.is_some(), "a source from the pack is stamped, so it is read again when the pack changes");
    let id = source.pictures.picture("openmoji", "1F600", 20, Outline::NONE).expect("drawn");
    let manager = ctx.tex_manager();
    let meta = manager.read().meta(id).cloned().expect("the texture is the context's");
    assert_eq!(meta.size, [20, 20], "uploaded at the asked size");
}
