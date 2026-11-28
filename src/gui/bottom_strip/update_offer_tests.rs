//! The update line (gui/bottom_strip/update_offer.rs, 2026-11-27): drawn for real under a headless
//! context, it takes room at the bottom of a page when there is a new version to offer and none
//! otherwise; Not now hides that offer; and its words are the ones Settings > Updates shows.

use super::*;
use crate::gui::bottom_strip::draw_for_version;
use crate::gui::GuiPage;

/// A 1.0 version, so the unfinished-software warning takes no room and the line is measured alone.
const FINISHED: &str = "1.0.0";

/// Where the page's room ends after the bottom strip drew, with the updater as `set` leaves it.
fn page_ends_at(set: impl Fn(&mut GuiState)) -> f32 {
    let theme = crate::gui::theme::load_theme();
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(412.0, 900.0));
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    theme.apply_to_egui(&ctx);
    let mut state = GuiState::default();
    state.active_page = GuiPage::Humanity;
    state.chat_footer.mode = crate::gui::bottom_strip::chat_footer::Mode::Off;
    set(&mut state);
    let mut bottom = 0.0;
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            draw_for_version(ctx, &theme, &mut state, FINISHED);
            bottom = ctx.available_rect().bottom();
        });
    }
    bottom
}

fn offer(state: &mut GuiState) {
    state.updater.state = UpdateState::Available {
        version: "v0.1580.0".into(),
        download_url: "https://example.invalid/HumanityOS-android-arm64.apk".into(),
        release_notes: String::new(),
        asset_size: 91_234_567,
    };
}

/// THE LINE OFFERS A NEW VERSION ON THE PAGE, measured where the page's room ends (a phone's width):
/// an offer takes a line or two at the bottom; nothing to offer, a failed check, or an offer the
/// person said Not now to takes none; a download they started shows again.
///
/// Seen red 2026-11-27 with bottom_strip::draw_at not drawing the line: "an offer took no room on
/// the page: it ends at 900".
#[test]
fn the_line_takes_room_only_when_there_is_something_to_offer() {
    let screen_bottom = 900.0;
    let with_offer = page_ends_at(offer);
    assert!(with_offer < screen_bottom - 20.0, "an offer took no room on the page: it ends at {with_offer}");
    assert!(with_offer > screen_bottom - 200.0, "the line is taller than a few lines: the page ends at {with_offer}");
    assert_eq!(page_ends_at(|_| {}), screen_bottom, "the line drew with nothing to offer");
    assert_eq!(page_ends_at(|s| s.updater.state = UpdateState::UpToDate), screen_bottom, "the line drew for an up-to-date app");
    assert_eq!(page_ends_at(|s| s.updater.state = UpdateState::Error("offline".into())), screen_bottom, "a failed check showed on the page");
    let hidden = page_ends_at(|s| {
        offer(s);
        s.updater.hide_offer();
    });
    assert_eq!(hidden, screen_bottom, "Not now did not hide the offer");
    let downloading = page_ends_at(|s| {
        offer(s);
        s.updater.hide_offer();
        s.updater.state = UpdateState::Downloading { version: "v0.1580.0".into(), progress: 0.4 };
    });
    assert!(downloading < screen_bottom - 20.0, "a download the person started did not show: the page ends at {downloading}");
}

/// The line says the version and its size, and each step's sentence comes from data/i18n/en.json
/// (a missing key would show as itself).
#[test]
fn the_words_name_the_version_and_its_size() {
    let mut state = GuiState::default();
    offer(&mut state);
    let words = sentence(&state.updater);
    assert!(words.contains("v0.1580.0") && words.contains("87.0 MB"), "the offer does not say the version and its size: {words}");
    assert!(!words.contains("app_update."), "a key showed instead of its words: {words}");
    for (target, st) in [
        (UpdateTarget::Phone, UpdateState::Ready { version: "v0.1580.0".into() }),
        (UpdateTarget::Desktop, UpdateState::Ready { version: "v0.1580.0".into() }),
        (UpdateTarget::Phone, UpdateState::Downloading { version: "v0.1580.0".into(), progress: 0.5 }),
        (UpdateTarget::Phone, UpdateState::Failed { version: "v0.1580.0".into(), key: "app_update.checksum_mismatch", detail: "x".into() }),
        (UpdateTarget::Desktop, UpdateState::Idle),
        (UpdateTarget::Desktop, UpdateState::Checking),
        (UpdateTarget::Desktop, UpdateState::UpToDate),
        (UpdateTarget::Desktop, UpdateState::Error("e".into())),
    ] {
        state.updater.target = target;
        state.updater.state = st.clone();
        let words = sentence(&state.updater);
        assert!(!words.is_empty() && !words.contains("app_update."), "{target:?} {st:?} says no words of its own: {words:?}");
    }
}
