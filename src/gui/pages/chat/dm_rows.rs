//! HOW MUCH OF A MESSAGE THE DM LIST SHOWS (2026-11-03). The operator: "Also we should add an
//! option to hide or show varying amounts of people's replies in the DMs section. I like seeing
//! the snippet but, some people might only want the usernames to display."
//!
//! Under each person's name the Direct Messages rows show the start of the conversation's last
//! message. How much is the person's choice, from the levels in `data/gui/dm_preview.json`:
//!   * Names only (0 lines): nothing of any message is on the screen, which is also the private
//!     choice: a preview can be read by anyone who can see the screen (a shared room, a stream, a
//!     screenshot), and the words of each level say so;
//!   * Name and one line (1, the default): the look since v0.715, which the operator likes;
//!   * Name and two lines (2): more of a long message, for someone who reads the list rather than
//!     opening each conversation.
//! Three or more would make each row as tall as a short conversation, so at dozens of
//! conversations the list would stop being a list; a level for it is one line of the data file.
//!
//! The choice is kept as the number of lines (`config.json` `dm_preview_lines`), chosen in
//! Settings > Privacy and in the DMs header's cog (CLAUDE.md, GUI-first: the cog is where a person
//! looks). The website's DM list shows names only already (web/chat/chat-dms.js `renderDmList`),
//! so it has nothing to choose.

use std::sync::{Arc, OnceLock};

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, Galley, RichText};

use humanity_core::i18n::t_dyn;
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint as _;
use crate::gui::GuiState;

/// The lines a row shows until the person chooses: one, the look the operator likes.
pub(crate) const DEFAULT_LINES: u8 = 1;

/// One choice: how many lines of the last message, and its words. (Each level in the file also has
/// an `id`, for people reading it; the choice is kept as its number of lines.)
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct Level {
    pub lines: u8,
    /// Keys in `data/i18n/`.
    pub label: String,
    pub hint: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub(crate) struct Levels {
    pub levels: Vec<Level>,
}

impl Levels {
    fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("it is not the levels file ({e})"))
    }

    /// The most lines any level shows (a hand-edited config asking for more gets this many).
    fn most_lines(&self) -> u8 {
        self.levels.iter().map(|l| l.lines).max().unwrap_or(DEFAULT_LINES)
    }
}

/// The levels, read once for the run: the data folder's file, else the copy built into the exe.
pub(crate) fn levels() -> &'static Levels {
    static FILE: OnceLock<Levels> = OnceLock::new();
    FILE.get_or_init(|| {
        humanity_core::embedded_data::load_text_or_embedded(&humanity_core::storage::data_dir(), "gui/dm_preview.json", Levels::parse)
            .unwrap_or_else(|e| {
                log::warn!("{e}: the DM list offers no preview choices this run");
                Levels::default()
            })
    })
}

/// How many lines of the last message a DM row shows: the person's choice, never more than the
/// largest level offers.
pub(crate) fn preview_lines(state: &GuiState) -> u8 {
    state.settings.dm_preview_lines.min(levels().most_lines())
}

/// The choices, one radio each with its hint, the person's marked. Returns the lines picked.
/// Drawn in Settings > Privacy and in the DMs header's cog; each caller keeps the pick.
pub(crate) fn picker(ui: &mut egui::Ui, theme: &Theme, current: u8) -> Option<u8> {
    let mut picked = None;
    for level in &levels().levels {
        let words = RichText::new(t_dyn(&level.label)).color(theme.text_primary());
        if ui.radio(current == level.lines, words).hint_dyn(&level.hint).clicked() {
            picked = Some(level.lines);
        }
    }
    picked
}

/// A conversation's last message laid out for its row: on one paragraph (line breaks become
/// spaces), cut to `lines` rows at `width`, ending with an ellipsis when cut. `None` when the row
/// shows no preview: names only, or nothing to show.
pub(crate) fn preview(ui: &egui::Ui, theme: &Theme, text: &str, width: f32, lines: u8, color: Color32) -> Option<Arc<Galley>> {
    if lines == 0 {
        return None;
    }
    let flat: String = text.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
    let flat = flat.trim();
    if flat.is_empty() {
        return None;
    }
    let mut job = LayoutJob::single_section(
        flat.to_owned(),
        TextFormat { font_id: egui::FontId::proportional(theme.font_size_small), color, ..Default::default() },
    );
    job.wrap = TextWrapping { max_width: width.max(1.0), max_rows: lines as usize, break_anywhere: false, overflow_character: Some('\u{2026}') };
    Some(ui.fonts(|f| f.layout_job(job)))
}

#[cfg(test)]
#[path = "dm_rows_tests.rs"]
mod tests;
