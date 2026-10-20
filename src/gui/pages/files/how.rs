//! The Files page's "How this page works" (2026-10-20, widgets/how_it_works.rs): what the page is
//! and what each place holds, folded behind one small toggle by the page's heading, closed until
//! the person opens it, remembered in config.json `files_how_open`.
//!
//! In the fold, word for word: the page's own line (what people on the server chose to show, and
//! that nothing from a private conversation can be there), and each place's line: Shared files'
//! (public for anyone to download), the HumanityOS folder's (changing one changes how the app
//! works; Save keeps the previous version) and Your folders' (nothing in them leaves this computer
//! until you tick it, add it and upload it).
//!
//! Outside it, where the person acts, stays what they must see before acting: Shared files' line
//! beside Upload a file and Remove (they are public), and the HumanityOS folder's caution beside
//! the file being edited. Your folders' "Click files to tick them..." stays too: it says how to do
//! the thing in front of you.

use super::{app_folder, places, shared_files, your_folders, Place};
use crate::gui::theme::Theme;
use crate::gui::widgets::how_it_works as how;
use crate::gui::GuiState;

/// The page's own line: what is shared on a server's Files page.
pub const PAGE_LINE: &str = "Pictures, videos, 3D models and files the people on this server chose to show. Nothing is here unless the person who uploaded it put it here, and nothing from a private conversation can be.";

/// The toggle, in the heading row: opens or closes the fold, kept in the settings.
pub(crate) fn toggle(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if how::toggle(ui, theme, state.settings.files_how_open) {
        state.settings.files_how_open = !state.settings.files_how_open;
        state.settings_dirty = true;
    }
}

/// The fold, when open: the page's line, then each place's.
pub(crate) fn panel(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    if !state.settings.files_how_open {
        return;
    }
    let (server, computer) = (theme.nav_legacy_red(), theme.nav_tools());
    how::panel(ui, theme, |ui| {
        how::part_heading(ui, theme, &places::label(state, Place::Shared), Some(server));
        how::line(ui, theme, PAGE_LINE);
        how::part_heading(ui, theme, shared_files::PLACE, Some(server));
        how::line(ui, theme, shared_files::HINT);
        how::part_heading(ui, theme, app_folder::PLACE, Some(computer));
        how::line(ui, theme, app_folder::CAUTION);
        how::part_heading(ui, theme, your_folders::TAB, Some(computer));
        how::line(ui, theme, your_folders::HINT);
    });
}

#[cfg(test)]
mod tests {
    use super::super::tests::{click, ctx_and_theme, demo_answer, page_state, settled};
    use super::*;

    fn count(out: &egui::FullOutput, text: &str) -> usize {
        crate::gui::pages::apps::tests::count(out, text)
    }

    /// THE FILES PAGE'S "HOW THIS PAGE WORKS", FOLDED (2026-10-20): closed as the page starts, the
    /// page's line is not drawn, and Your folders' promise is not drawn beside its folders; what
    /// must be seen before acting stays where the person acts: Shared files' line (they are
    /// public) and the HumanityOS folder's caution (editing changes how the app works). Opened,
    /// kept in the settings, the page's line and every place's are there, each once, word for
    /// word; it survives a restart (config.json `files_how_open`), a config without it has it
    /// closed, and pressed again it closes.
    ///
    /// Seen red 2026-10-20 with the fold drawn whatever the setting said (the `files_how_open` check
    /// in `panel` taken out): "closed, the page's line is not drawn" (left: 1, right: 0).
    #[test]
    fn how_this_page_works_is_folded() {
        let (ctx, theme) = ctx_and_theme();
        let mut state = page_state(demo_answer(), true);
        assert!(!state.settings.files_how_open, "closed as the page starts");
        let out = settled(&ctx, &theme, &mut state);
        assert_eq!(count(&out, how::LABEL), 1, "the toggle, by the heading");
        assert_eq!(count(&out, PAGE_LINE), 0, "closed, the page's line is not drawn");

        state.files.place = Place::YourFolders;
        let out = settled(&ctx, &theme, &mut state);
        assert_eq!((count(&out, how::LABEL), count(&out, your_folders::HINT)), (1, 0), "on Your folders, the toggle and not the promise");
        state.files.place = Place::SharedFiles;
        let out = settled(&ctx, &theme, &mut state);
        assert!(count(&out, shared_files::HINT) >= 1, "Shared files' line stays where its files are: they are public");
        state.files.place = Place::AppFolder;
        let out = settled(&ctx, &theme, &mut state);
        assert!(count(&out, app_folder::CAUTION) >= 1, "the HumanityOS folder's caution stays");

        state.files.place = Place::Shared;
        state.settings_dirty = false;
        click(&ctx, &theme, &mut state, how::LABEL);
        assert!(state.settings.files_how_open && state.settings_dirty, "opened, and kept");
        let out = settled(&ctx, &theme, &mut state);
        for line in [PAGE_LINE, shared_files::HINT, app_folder::CAUTION, your_folders::HINT] {
            assert_eq!(count(&out, line), 1, "open, each once, word for word: {line}");
        }

        let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).unwrap();
        let mut fresh = GuiState::default();
        serde_json::from_str::<crate::config::AppConfig>(&json).unwrap().apply_to_gui_state(&mut fresh);
        assert!(fresh.settings.files_how_open, "it survives a restart");
        let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
        old.as_object_mut().unwrap().retain(|k, _| k != "files_how_open");
        assert!(!serde_json::from_value::<crate::config::AppConfig>(old).unwrap().files_how_open, "a config without it has it closed");

        click(&ctx, &theme, &mut state, how::LABEL);
        assert!(!state.settings.files_how_open, "pressed again, it closes");
        assert_eq!(count(&settled(&ctx, &theme, &mut state), PAGE_LINE), 0);
    }
}
