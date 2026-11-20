//! The desktop's emoji (2026-11-20, docs/design/emoji.md): the matcher held to the answers the
//! website gives (scripts/tests/fixtures/emoji-v1.json), and pictures laid into a chat message's
//! line, drawn headless with a stand-in for rung 2 (one solid square texture made here). The chat
//! page's own checks (reaction pills, Copy text) are in `pages/chat/emoji_tests.rs`. Each test was
//! seen failing once on purpose; the break and what it said are recorded at the test.

use super::*;
use std::sync::Arc;

use egui::{Color32, Pos2, Rect, TextureId};

fn repo(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(p)
}

/// The pack's list as it ships (assets/emoji/index.json).
pub(crate) fn shipped() -> EmojiIndex {
    EmojiIndex::load(&repo("assets/emoji/index.json")).expect("the shipped emoji index")
}

/// The fixture's notation: {HEX HEX} stands for those code points, everything else is itself.
fn expand(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = open + rest[open..].find('}').expect("a closing brace");
        for h in rest[open + 1..close].split_whitespace() {
            out.push(char::from_u32(u32::from_str_radix(h, 16).expect("hex")).expect("a code point"));
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Pieces in the fixture's shape: ("img", file name) or ("text", the text).
fn describe(text: &str, pieces: &[Piece]) -> Vec<(String, String)> {
    pieces
        .iter()
        .map(|p| match p {
            Piece::Text(r) => ("text".to_string(), text[r.clone()].to_string()),
            Piece::Picture { file, .. } => ("img".to_string(), file.clone()),
        })
        .collect()
}

/// THE SHARED FIXTURE: every case of scripts/tests/fixtures/emoji-v1.json, the answers the website's
/// web/shared/emoji.js gives too (scripts/tests/emoji-web.test.js, test 2), and the pieces tile the
/// string: one after another, nothing left out, so a picture's text is exactly what it stands for.
///
/// Seen red 2026-11-20 with the bare-heart exception taken out of `first_at`: "a bare red heart is a
/// picture: ..." (left: [("text", "\u{2764}")], right: [("img", "2764")]).
#[test]
fn the_desktop_matcher_gives_every_answer_of_the_shared_fixture() {
    let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(repo("scripts/tests/fixtures/emoji-v1.json")).unwrap()).unwrap();
    let index = shipped();
    let cases = fx["cases"].as_array().expect("the fixture has cases");
    assert!(cases.len() >= 27, "the fixture holds its cases ({})", cases.len());
    for case in cases {
        let text = expand(case["text"].as_str().unwrap());
        let want: Vec<(String, String)> = case["want"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| {
                let (kind, v) = (w[0].as_str().unwrap(), w[1].as_str().unwrap());
                (kind.to_string(), if kind == "text" { expand(v) } else { v.to_string() })
            })
            .collect();
        let got = pieces(&text, &index);
        assert_eq!(describe(&text, &got), want, "{}", case["what"]);
        let mut at = 0;
        for p in &got {
            assert_eq!(p.range().start, at, "{}: the pieces follow one another", case["what"]);
            at = p.range().end;
        }
        assert_eq!(at, text.len(), "{}: the pieces cover the whole text", case["what"]);
    }
}

/// A SET LISTED FIRST DRAWS ITS PICTURES IN PLACE OF A LATER SET'S (the volunteers' set goes ahead of
/// OpenMoji), each picture names the folder it comes from, and every picture the list names is a
/// file on disk.
///
/// Seen red 2026-11-20 with `from_json` letting a later set overwrite an earlier one (`insert` for
/// `entry().or_insert_with`): left: Some(("openmoji", "1F600")), right: Some(("hos", "1F600")).
#[test]
fn the_first_set_that_has_a_picture_draws_it() {
    let shipped_text = std::fs::read_to_string(repo("assets/emoji/index.json")).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&shipped_text).unwrap();
    v["sets"].as_array_mut().unwrap().insert(0, serde_json::json!({ "id": "hos", "dir": "hos", "names": "1F600" }));
    let index = EmojiIndex::from_json(&v.to_string()).unwrap();
    assert_eq!(index.get("1F600"), Some(("hos", "1F600")));
    assert_eq!(index.get("1F44D"), Some(("openmoji", "1F44D")));
    let shipped = shipped();
    assert!(shipped.len() > 4000, "the pack names {} pictures (OpenMoji 17.0.0 has 4,495)", shipped.len());
    for (set, file) in shipped.files.values().map(|(s, f)| (&shipped.sets[*s], f)) {
        assert!(repo(&format!("assets/emoji/{set}/{file}.svg")).is_file(), "{set}/{file}.svg is named but not there");
    }
}

/// THE PACK'S NEWEST EMOJI ARE PICTURES TOO: OpenMoji 17.0.0 draws Unicode 17.0's new emoji, and the
/// website's engine (Node 24, the browsers) calls them emoji, so the desktop must as well.
///
/// Seen red 2026-11-20 with `presentation()` asking the regex crate's `\p{Emoji_Presentation}`
/// (regex-syntax 0.8.11, Unicode 16.0) instead of props.rs: "landslide, Unicode 17.0" (left:
/// [("text", "🛘")], right: [("img", "1F6D8")]). That is why props.rs is generated.
#[test]
fn unicode_17_emoji_in_the_pack_are_pictures() {
    let index = shipped();
    for (cp, what) in [("1F6D8", "landslide, Unicode 17.0"), ("1FAEA", "distorted face, Unicode 17.0"), ("1FAC8", "hairy creature, Unicode 17.0")] {
        let text = expand(&format!("{{{cp}}}"));
        assert_eq!(describe(&text, &pieces(&text, &index)), vec![("img".to_string(), cp.to_string())], "{what}");
    }
    assert_eq!(unicode_version(), "17.0", "props.rs was made from Unicode 17.0 (node scripts/gen-emoji-props.js)");
}

// ── Rung 3: pictures in a line of text ───────────────────────────────────────────────────────────

/// A stand-in for rung 2: every picture is the one solid square drawn here.
struct StandIn(TextureId);
impl EmojiPictures for StandIn {
    fn picture(&self, _set: &str, _file: &str, _px: u32) -> Option<TextureId> {
        Some(self.0)
    }
}

/// Rung 2 before it can draw anything: no picture for any emoji.
struct NoPictures;
impl EmojiPictures for NoPictures {
    fn picture(&self, _set: &str, _file: &str, _px: u32) -> Option<TextureId> {
        None
    }
}

/// The stand-in texture (kept alive by the handle) and the source that draws every emoji with it.
pub(crate) fn stand_in(ctx: &egui::Context) -> (egui::TextureHandle, Arc<EmojiSource>) {
    let handle = ctx.load_texture("emoji-stand-in", egui::ColorImage::new([16, 16], Color32::WHITE), egui::TextureOptions::LINEAR);
    let source = Arc::new(EmojiSource { index: shipped(), pictures: Box::new(StandIn(handle.id())) });
    (handle, source)
}

/// Every galley one frame drew, with where.
pub(crate) fn galleys_of(out: &egui::FullOutput) -> Vec<(Pos2, Arc<egui::Galley>)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(Pos2, Arc<egui::Galley>)>) {
        match shape {
            egui::Shape::Text(t) => found.push((t.pos, t.galley.clone())),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut found);
    }
    found
}

/// Each emoji placeholder of a drawn galley (a transparent section that is not the pill's reserved
/// spaces): which row holds its first glyph, and its run's left and right edge, on screen.
pub(crate) fn placeholders(pos: Pos2, galley: &egui::Galley) -> Vec<(usize, f32, f32)> {
    let job = &galley.job;
    let mut out = Vec::new();
    for (si, sec) in job.sections.iter().enumerate() {
        if sec.format.color != Color32::TRANSPARENT || job.text[sec.byte_range.clone()].trim().is_empty() {
            continue;
        }
        let Some((ri, row)) = galley.rows.iter().enumerate().find(|(_, r)| r.glyphs.iter().any(|g| g.section_index as usize == si)) else { continue };
        let glyphs = row.glyphs.iter().filter(|g| g.section_index as usize == si);
        let (l, r) = glyphs.fold((f32::INFINITY, f32::NEG_INFINITY), |(l, r), g| (l.min(g.pos.x), r.max(g.pos.x + g.advance_width)));
        out.push((ri, pos.x + l, pos.x + r));
    }
    out
}

/// One frame drawing one chat message row (`message_row`, the chat's body painter) `width` wide.
fn draw_row(ctx: &egui::Context, content: &str, width: f32) -> egui::FullOutput {
    let theme = crate::gui::theme::load_theme();
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 400.0))), ..Default::default() };
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            crate::gui::widgets::row::message_row(ui, &theme, 'A', theme.accent(), "Ann", "09:20", content, true, theme.bg_card(), false, 0.0, &[], &[]);
        });
    })
}

/// The drawn galley holding `content`.
fn body_galley(out: &egui::FullOutput, content: &str) -> (Pos2, Arc<egui::Galley>) {
    galleys_of(out).into_iter().find(|(_, g)| g.job.text.contains(content)).unwrap_or_else(|| panic!("{content:?} was drawn"))
}

/// The pictures one frame painted with `texture`.
fn painted(out: &egui::FullOutput, texture: TextureId) -> Vec<Rect> {
    crate::gui::pages::chat::inline_video::tests::pictures_of(out).into_iter().filter(|(t, _)| *t == texture).map(|(_, r)| r).collect()
}

/// A PICTURE SITS ON ITS PLACEHOLDER IN A MESSAGE'S LINE, AND THE TEXT STAYS THE REAL EMOJI: the
/// row lays the emoji out as an invisible run the picture's room wide (1.35 em, the website's 1.2 em
/// picture with its margins), paints the stand-in over that run centred on the row, the next word
/// starts after the picture, and the drawn text is the message's own (what copying gives).
///
/// Seen red 2026-11-20 with `message_row` laying its galley out with `painter.layout_job` (rung 3 not
/// wired): "one picture for the one emoji" (left: 0, right: 1). And with `with_pictures` writing '?'
/// over each emoji's bytes (a placeholder that is not the emoji's own text): "\"hi 😀 there\" was
/// drawn" (the drawn line no longer held the message).
#[test]
fn a_picture_sits_on_its_placeholder_and_the_text_stays_the_emoji() {
    let ctx = egui::Context::default();
    let (handle, source) = stand_in(&ctx);
    install(&ctx, Some(source));
    let content = "hi \u{1F600} there";
    draw_row(&ctx, content, 600.0);
    let out = draw_row(&ctx, content, 600.0);
    let pics = painted(&out, handle.id());
    assert_eq!(pics.len(), 1, "one picture for the one emoji");
    let (pos, galley) = body_galley(&out, content);
    assert!(galley.job.text.ends_with(content), "the drawn text is the message's own: {:?}", galley.job.text);
    let holes = placeholders(pos, &galley);
    assert_eq!(holes.len(), 1, "one placeholder");
    let (row, left, right) = holes[0];
    let em = crate::gui::theme::load_theme().body_size;
    assert!((right - left - 1.35 * em).abs() <= 1.5, "the placeholder is the picture's room wide: {} for {}", right - left, 1.35 * em);
    let pic = pics[0];
    assert!((pic.width() - 1.2 * em).abs() < 0.01 && (pic.height() - 1.2 * em).abs() < 0.01, "the picture is 1.2 em square: {pic:?}");
    assert!(pic.left() >= left - 0.5 && pic.right() <= right + 0.5, "the picture {pic:?} sits on its placeholder {left}..{right}");
    let row_rect = galley.rows[row].rect.translate(pos.to_vec2());
    assert!((pic.center().y - row_rect.center().y).abs() < 0.5, "the picture is centred on its row: {pic:?} in {row_rect:?}");
    // The first glyph after the emoji (the space before "there") starts past the picture.
    let after = galley.job.text.find(" there").unwrap();
    let glyph_after = galley.rows.iter().flat_map(|r| r.glyphs.iter()).nth(galley.job.text[..after].chars().count()).expect("a glyph after the emoji");
    assert!(pos.x + glyph_after.pos.x >= pic.right() - 0.5, "the next word starts after the picture: {} < {}", pos.x + glyph_after.pos.x, pic.right());
}

/// A PICTURE WRAPS WITH ITS LINE: in a narrow row the emoji falls onto a later line, and its picture
/// is painted there, on its placeholder in that row, while the text is still the message's own.
///
/// Seen red 2026-11-20 with rung 3 not wired into `message_row`: "one picture" (left: 0, right: 1);
/// with '?' written over each emoji's bytes, the message was no longer found in the drawn text.
#[test]
fn a_picture_wraps_onto_the_line_its_emoji_wraps_onto() {
    let ctx = egui::Context::default();
    let (handle, source) = stand_in(&ctx);
    install(&ctx, Some(source));
    let content = "words that fill a narrow line before the picture \u{1F44D} and after it";
    draw_row(&ctx, content, 260.0);
    let out = draw_row(&ctx, content, 260.0);
    let pics = painted(&out, handle.id());
    assert_eq!(pics.len(), 1, "one picture");
    let (pos, galley) = body_galley(&out, content);
    assert!(galley.job.text.ends_with(content), "the drawn text is the message's own");
    let holes = placeholders(pos, &galley);
    assert_eq!(holes.len(), 1, "one placeholder");
    let (row, left, right) = holes[0];
    assert!(row >= 1, "the emoji wrapped onto a later line (row {row} of {})", galley.rows.len());
    let row_rect = galley.rows[row].rect.translate(pos.to_vec2());
    let pic = pics[0];
    assert!(pic.left() >= left - 0.5 && pic.right() <= right + 0.5, "the picture {pic:?} sits on its placeholder {left}..{right}");
    assert!((pic.center().y - row_rect.center().y).abs() < 0.5, "the picture is on the emoji's line: {pic:?} in {row_rect:?}");
}

/// WITHOUT A PICTURE NOTHING CHANGES: with nothing installed, and with a source that has no picture
/// for any emoji (rung 2 before it can draw), the row draws exactly the text job it drew before
/// rung 3 (the system emoji font's glyph, no placeholder, no picture).
///
/// Seen red 2026-11-20 with `with_pictures` making a placeholder for every matched emoji, picture or
/// not (in the same run as the '?' break of the test above, which then rewrote those placeholders):
/// "\"hi 😀 there ❤\" was drawn", the line drawn without any picture was no longer the message.
#[test]
fn without_a_picture_the_line_is_drawn_as_before() {
    let content = "hi \u{1F600} there \u{2764}";
    let plain = egui::Context::default();
    draw_row(&plain, content, 600.0);
    let before = draw_row(&plain, content, 600.0);
    let (_, plain_galley) = body_galley(&before, content);
    let none = egui::Context::default();
    install(&none, Some(Arc::new(EmojiSource { index: shipped(), pictures: Box::new(NoPictures) })));
    draw_row(&none, content, 600.0);
    let after = draw_row(&none, content, 600.0);
    let (pos, galley) = body_galley(&after, content);
    assert_eq!(placeholders(pos, &galley).len(), 0, "no placeholder without a picture");
    assert_eq!(galley.job.sections, plain_galley.job.sections, "the same sections as with nothing installed");
    assert_eq!(galley.job.text, plain_galley.job.text);
    assert!(crate::gui::pages::chat::inline_video::tests::pictures_of(&after).is_empty(), "no picture painted");
}
