// Child of chat.rs: emoji pictures on the real chat page (2026-11-20, docs/design/emoji.md rung 3),
// drawn headless with a stand-in for rung 2 (one solid square texture, `widgets::emoji::tests`).
// The row-level checks (a picture on its placeholder, wrapping, nothing changes without a picture)
// are in `widgets/emoji/tests.rs`. Each test was seen failing once on purpose; the break and what it
// said are recorded at the test.

use super::*;
use crate::gui::pages::chat::inline_video::tests::{pictures_of, Chat};
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::widgets::emoji;
use crate::gui::widgets::emoji::tests::{galleys_of, placeholders, stand_in};

/// A REACTION PILL DRAWS ITS EMOJI AS A PICTURE, AND THE PILL IS AS WIDE AS WHAT IT DRAWS: the heart
/// reaction (stored bare, the matcher's one text-by-default exception) is painted as the stand-in over
/// its placeholder in the badge, and the width the row reserved for the pill (`compute_pill_width`)
/// still holds the badge, picture included.
///
/// Seen red 2026-11-20 with `paint_timestamp_pill` and `compute_pill_width` laying the badge out with
/// `layout_no_wrap` (rung 3 not wired into the pills): "one picture, the badge's" (left: 0, right: 1).
#[test]
fn a_reaction_pill_draws_its_emoji_as_a_picture() {
    let mut chat = Chat::new("chat_emoji_pill", &["a heart for this"], 1280.0, 800.0);
    chat.state.chat_messages[0].reactions.insert("\u{2764}".into(), vec!["bob0".into()]);
    let (handle, source) = stand_in(&chat.ctx);
    emoji::install(&chat.ctx, Some(source));
    chat.frame(Vec::new());
    let out = chat.frame(Vec::new());
    let pics: Vec<egui::Rect> = pictures_of(&out).into_iter().filter(|(t, _)| *t == handle.id()).map(|(_, r)| r).collect();
    assert_eq!(pics.len(), 1, "one picture, the badge's");
    let (pos, label) = galleys_of(&out).into_iter().find(|(_, g)| g.job.text == "\u{2764}1").expect("the badge's label is drawn");
    let holes = placeholders(pos, &label);
    assert_eq!(holes.len(), 1, "the heart is a placeholder in the badge");
    let (_, left, right) = holes[0];
    assert!(pics[0].left() >= left - 0.5 && pics[0].right() <= right + 0.5, "the picture {:?} sits on its placeholder {left}..{right}", pics[0]);
    // The reserved width: [6 pad | time | 5 | thorn | 4 | badge: 2 label 2 | 6 pad] from the time's left.
    let (ts_pos, _) = galleys_of(&out).into_iter().find(|(_, g)| g.job.text == "09:20").expect("the time is drawn in the pill");
    let reactions = chat.state.chat_messages[0].reactions.clone();
    let width = compute_pill_width(&chat.ctx, &chat.theme, "09:20", &reactions);
    let pill_right = ts_pos.x - 6.0 + width;
    let badge_right = pos.x + label.size().x + 2.0 + 6.0;
    assert!(badge_right <= pill_right + 1.0, "the pill holds its badge: badge ends at {badge_right}, the pill at {pill_right}");
    assert!(pics[0].right() <= pill_right, "the picture is inside the pill: {:?}, the pill ends at {pill_right}", pics[0]);
}

/// COPYING A MESSAGE GIVES THE REAL EMOJI WHILE PICTURES ARE DRAWN: with the stand-in drawing every
/// emoji, the message's drawn text is still its own text, emoji and all (what selecting it copies),
/// and its right-click "Copy text" copies the message exactly.
///
/// Seen red 2026-11-20 with `with_pictures` writing '?' over each emoji's bytes (a placeholder that
/// is not the emoji's own text): "the drawn text is the message's own".
#[test]
fn copying_a_message_gives_the_real_emoji() {
    let words = "hi \u{1F600} there \u{1F44D}\u{1F3FD} and \u{2764}\u{FE0F}\u{200D}\u{1F525}"; // glyph-exempt: a test message typed with U+FE0F, as people type it; drawn as a picture's placeholder
    let mut chat = Chat::new("chat_emoji_copy", &[words], 1280.0, 800.0);
    let (handle, source) = stand_in(&chat.ctx);
    emoji::install(&chat.ctx, Some(source));
    chat.frame(Vec::new());
    let out = chat.frame(Vec::new());
    assert_eq!(pictures_of(&out).into_iter().filter(|(t, _)| *t == handle.id()).count(), 3, "three pictures in the line");
    assert!(galleys_of(&out).iter().any(|(_, g)| g.job.text.ends_with(words)), "the drawn text is the message's own");
    // Near the line's right end: on the message row, clear of the pill at its start.
    let line = find_text_in_shapes(&out.shapes, "there").expect("the message is drawn").rect;
    let at = egui::pos2(line.right() - 4.0, line.center().y);
    let m = egui::Modifiers::default();
    chat.frame(vec![egui::Event::PointerMoved(at)]);
    chat.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: true, modifiers: m }]);
    chat.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: false, modifiers: m }]);
    let menu = chat.frame(Vec::new());
    let copy = find_text_in_shapes(&menu.shapes, &humanity_core::i18n::t("chat.copy_text")).expect("the menu offers Copy text").rect.center();
    let mut outs = vec![chat.click(copy)];
    outs.push(chat.frame(Vec::new()));
    let copied: Vec<String> = outs
        .iter()
        .flat_map(|o| o.platform_output.commands.iter())
        .filter_map(|c| if let egui::OutputCommand::CopyText(t) = c { Some(t.clone()) } else { None })
        .collect();
    assert_eq!(copied, vec![words.to_string()], "Copy text copies the message exactly");
}
