// Child of settings.rs: Settings > Account with the identity on this device locked, missing and
// unlocked (BUG-213). The section is drawn headless (an egui context with no window) and its text
// read back, so each test judges what a person would see, not the state behind it. Each test was
// seen failing once; what it saw red is at the test.

use super::*;
use crate::gui::i18n::t;
use crate::gui::theme::load_theme;

/// Every text drawn.
fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    out.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, &mut found);
    }
    found
}

/// Settings > Account drawn for each frame's events in turn (in a window tall enough that nothing
/// is cut off), then settled over three more; what the last frame drew.
fn account(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, frames: Vec<Vec<egui::Event>>) -> Vec<(String, egui::Rect)> {
    let mut last = None;
    let settle = (0..3).map(|_| Vec::new());
    for events in frames.into_iter().chain(settle) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 12000.0))),
            events,
            ..Default::default()
        };
        last = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_account_content(ui, theme, state));
        }));
    }
    texts(&last.expect("frames ran"))
}

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

fn all_text(t: &[(String, egui::Rect)]) -> String {
    t.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join("\n")
}

/// A press and a release on the text `label`, as a person's click.
fn click_on(t: &[(String, egui::Rect)], label: &str) -> Vec<Vec<egui::Event>> {
    let at = t.iter().find(|(s, _)| s == label).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("{label:?} is drawn"));
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    vec![vec![egui::Event::PointerMoved(at)], vec![button(true)], vec![button(false)]]
}

/// A LOCKED IDENTITY IS NEVER SHOWN AS NONE: with the key encrypted on this device and not
/// unlocked, the section says it is locked and offers Reveal recovery phrase; it offers no Generate
/// New Identity, whose passphrase prompt would encrypt a new key over this one, and its Restore
/// says that a restore replaces it.
///
/// Seen red 2026-10-24 against the section before BUG-213's fix: "a locked identity is offered
/// Generate New Identity".
#[test]
fn a_locked_identity_is_never_shown_as_none() {
    crate::config::keep_saves_off_disk();
    let (ctx, theme) = ctx_and_theme();
    let mut state = GuiState::default();
    state.encrypted_private_key = "an encrypted key".to_string();
    state.key_salt = "its salt".to_string();
    let all = all_text(&account(&ctx, &theme, &mut state, Vec::new()));
    assert!(!all.contains("Generate New Identity"), "a locked identity is offered Generate New Identity:\n{all}");
    assert!(!all.contains("No identity on this device yet"), "a locked identity is called none:\n{all}");
    assert!(all.contains(t("settings.identity_locked")), "it says the identity is locked:\n{all}");
    assert!(all.contains(t("settings.reveal_phrase")), "it offers Reveal recovery phrase:\n{all}");
    assert!(all.contains(t("settings.restore_replaces")), "Restore says it replaces the identity here:\n{all}");
}

/// NO IDENTITY IS STILL OFFERED ONE: with nothing on this device, Generate New Identity is there,
/// with no gate and no word of replacing anything. Seen red 2026-10-24 with the Locked test's
/// branch taken for every identity not in memory: "with no identity, Generate New Identity is
/// offered".
#[test]
fn no_identity_is_still_offered_one() {
    crate::config::keep_saves_off_disk();
    let (ctx, theme) = ctx_and_theme();
    let mut state = GuiState::default();
    let all = all_text(&account(&ctx, &theme, &mut state, Vec::new()));
    assert!(all.contains("Generate New Identity"), "with no identity, Generate New Identity is offered:\n{all}");
    assert!(!all.contains(t("settings.reveal_phrase")), "with no identity, there is nothing to reveal:\n{all}");
    assert!(!all.contains(t("settings.restore_replaces")), "with no identity, a restore replaces nothing:\n{all}");
}

/// ONE PASSPHRASE UNLOCKS IT AND SHOWS THE COVER: the right passphrase typed in the locked
/// section's gate puts the identity in memory (the vault left as it was) and the next frames draw
/// the recovery phrase's cover at once, with no second passphrase; a wrong one unlocks nothing.
///
/// Seen red 2026-10-24 with the unlocked section's gate keyed apart from the locked one's: "after
/// the passphrase the words' cover is drawn" (the gate asked again).
#[test]
fn one_passphrase_unlocks_it_and_shows_the_cover() {
    crate::config::keep_saves_off_disk();
    let (ctx, theme) = ctx_and_theme();
    let seed = vec![7u8; 32];
    let (enc, salt, iters) = crate::config::encrypt_private_key(&seed, "correct horse").expect("encrypts");
    let locked = || {
        let mut state = GuiState::default();
        state.encrypted_private_key = enc.clone();
        state.key_salt = salt.clone();
        state.key_iterations = iters;
        state
    };
    // Typed as a person types it: a click in the box (found by its hint), the words, a click on
    // Unlock. Nothing reaches into the gate's state by name.
    let type_and_unlock = |ctx: &egui::Context, state: &mut GuiState, pass: &str| {
        let first = account(ctx, &theme, state, Vec::new());
        let mut frames = click_on(&first, "passphrase");
        frames.push(vec![egui::Event::Text(pass.to_string())]);
        frames.extend(click_on(&first, "Unlock"));
        all_text(&account(ctx, &theme, state, frames))
    };

    // A wrong passphrase: still locked.
    let mut state = locked();
    let after_wrong = type_and_unlock(&ctx, &mut state, "wrong horse");
    assert!(state.private_key_bytes.is_none(), "a wrong passphrase unlocks nothing");
    assert!(after_wrong.contains(t("settings.identity_locked")), "still locked after a wrong passphrase:\n{after_wrong}");

    // The right one: unlocked once, and the cover drawn.
    let (ctx, _) = ctx_and_theme();
    let mut state = locked();
    let after = type_and_unlock(&ctx, &mut state, "correct horse");
    assert_eq!(state.private_key_bytes.as_deref(), Some(&seed[..]), "the right passphrase unlocks this identity");
    assert_eq!(state.encrypted_private_key, enc, "the vault is left as it was");
    assert!(after.contains(crate::gui::widgets::phrase_cover::SHOW), "after the passphrase the words' cover is drawn:\n{after}");
    assert!(!after.contains(t("settings.identity_locked")), "it no longer says locked:\n{after}");
}

/// THE WORDS' COPY IS THERE ONLY WHILE THEY ARE SHOWN (BUG-214): covered, the recovery phrase box
/// draws no Copy, so a stray click cannot put the words on the clipboard; held for its 3 seconds,
/// the words show and Copy with them (one more Copy than before: the public key keeps its own).
///
/// Seen red 2026-10-24 against the box before the fix: "covered, the phrase box draws no Copy".
#[test]
fn the_words_copy_is_there_only_while_they_are_shown() {
    crate::config::keep_saves_off_disk();
    let (ctx, theme) = ctx_and_theme();
    let mut state = GuiState::default();
    // A just-made identity: in memory, no passphrase yet, so the box is drawn with no gate.
    state.private_key_bytes = Some(vec![7u8; 32]);
    let copies = |t: &[(String, egui::Rect)]| t.iter().filter(|(s, _)| s == "Copy").count();
    let covered = account(&ctx, &theme, &mut state, Vec::new());
    assert!(all_text(&covered).contains(crate::gui::widgets::phrase_cover::SHOW), "the words are covered");
    // Held for the cover's 3 seconds and a little more, at egui's 1/60 s a pass.
    let at = covered.iter().find(|(s, _)| s == crate::gui::widgets::phrase_cover::SHOW).map(|(_, r)| r.center()).expect("the show button");
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let mut frames = vec![vec![egui::Event::PointerMoved(at), button(true)]];
    frames.extend((0..((crate::gui::widgets::phrase_cover::HOLD_SECONDS + 0.3) * 60.0).ceil() as usize).map(|_| Vec::new()));
    frames.push(vec![button(false)]);
    let shown = account(&ctx, &theme, &mut state, frames);
    assert!(all_text(&shown).contains(crate::gui::widgets::phrase_cover::HIDE), "held, the words are shown:\n{}", all_text(&shown));
    assert_eq!(copies(&shown), copies(&covered) + 1, "covered, the phrase box draws no Copy; shown, it does");
}
