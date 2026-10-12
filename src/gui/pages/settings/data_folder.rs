//! Settings > Data > "Data folder" (BUG-163 part 2, 2026-10-12; docs/design/data-folder-updates.md
//! section 6). Child of settings.rs.
//!
//! The data folder holds the game's data files (items, plants, recipes, the ship, the home
//! designs) where a player or a mod can edit them, and since 2026-10-12 every update brings the
//! files nobody edited up to date and keeps every edited one (src/storage/refresh.rs). This block
//! says so in the app, so nobody needs the log or a terminal to learn it (the GUI-first rule):
//! - which version's files the folder holds, or why the game leaves it alone (a source tree, a
//!   rig's linked folder);
//! - what the last update brought up to date, and which files it could not write;
//! - each file it KEPT because it was edited, with "Use the game's copy" (moves the player's file
//!   aside as `<name>.yours-<version>`, never deletes it) and "Show the file";
//! - each file this run did not use because this version cannot read it, and why (BUG-163 part
//!   1's `[built-in data copy]` line), so a player whose edit did nothing can see why.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::HintDisplay;
use crate::storage::refresh::{self, View};

/// What a press in the block asked for (acted on after drawing).
#[derive(Debug, Clone, PartialEq)]
enum Asked {
    UseGamesCopy(String),
    Show(String),
}

/// The block, drawn inside Settings > Data's card. Nothing when there is no data folder yet.
pub(super) fn draw_data_folder(ui: &mut egui::Ui, theme: &Theme, hint: HintDisplay) {
    let Some(view) = refresh::view() else { return };
    let not_used = crate::embedded_data::builtin_copies_this_run();
    let result_id = egui::Id::new("data_folder_result");
    ui.add_space(theme.spacing_lg);
    match draw_view(ui, theme, hint, &view, &not_used) {
        Some(Asked::UseGamesCopy(rel)) => {
            let line = match refresh::use_the_games_copy(&rel) {
                Ok(done) => done,
                Err(e) => format!("Nothing was changed: {e}"),
            };
            ui.data_mut(|d| d.insert_temp(result_id, line));
        }
        // Joined part by part: a shipped path is written with `/`, which Explorer's /select does
        // not take.
        Some(Asked::Show(rel)) => show_file(&rel.split('/').fold(view.dir.clone(), |p, part| p.join(part))),
        None => {}
    }
    if let Some(line) = ui.data_mut(|d| d.get_temp::<String>(result_id)) {
        if !line.is_empty() {
            ui.add_space(theme.spacing_xs);
            let failed = line.starts_with("Nothing was changed");
            small(ui, theme, &line, if failed { theme.danger() } else { theme.success() });
        }
    }
}

fn small(ui: &mut egui::Ui, theme: &Theme, text: &str, color: egui::Color32) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(color));
}

/// Draw `view` (and the files this run did not use) and return what was pressed. Pure apart from
/// the drawing, so a test can draw any state of the folder.
fn draw_view(ui: &mut egui::Ui, theme: &Theme, hint: HintDisplay, view: &View, not_used: &[(String, String)]) -> Option<Asked> {
    let mut asked = None;
    ui.label(RichText::new("Data folder").color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "The game's data files (items, plants, recipes, the ship, your home's design) are kept in the \
         Game data folder above, where you or a mod can edit them. When the game updates, every file \
         nobody edited is brought up to date, and every file that was edited is kept as it is.",
    );

    if let Some(why) = &view.left_alone {
        small(ui, theme, &format!("The game does not update this folder: {why}."), theme.text_muted());
    } else {
        let holds = match (&view.version, view.stamped) {
            (Some(v), _) => format!("Holds the data files of version {v}."),
            (None, true) => "The last update could not bring every file up to date. The game tries again at the next start.".to_string(),
            (None, false) => "Written by a version from before the game kept a record of its data files.".to_string(),
        };
        small(ui, theme, &holds, theme.text_primary());
        if let Some(r) = &view.last_update {
            let when = crate::gui::pages::chat::format_full_timestamp(r.at.saturating_mul(1000));
            // No `from`: the folder was written before the game kept a record of its files.
            let from = r.from.as_deref().map(|f| format!("from {f} ")).unwrap_or_else(|| "from an earlier version ".to_string());
            let mut line = format!(
                "Last update, {from}to {} ({when}): {} brought up to date",
                r.to,
                count(r.updated.len(), "file", "files")
            );
            if !r.added.is_empty() {
                line.push_str(&format!(", {} added", count(r.added.len(), "new file", "new files")));
            }
            line.push('.');
            small(ui, theme, &line, theme.text_secondary());
            for f in &r.failed {
                small(ui, theme, &format!("Not brought up to date: {}: {}. The game tries again at the next start.", f.file, f.why), theme.warning());
            }
            if !r.kept.is_empty() {
                ui.add_space(theme.spacing_xs);
                small(
                    ui,
                    theme,
                    "Kept as you left them, because they differ from the copy the game wrote there. \
                     \"Use the game's copy\" moves yours aside (it is renamed, never deleted) and puts this \
                     version's copy in its place:",
                    theme.text_secondary(),
                );
                for rel in &r.kept {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(rel).size(theme.font_size_small).strong().color(theme.text_primary()));
                        if widgets::compact_button(ui, theme, "Use the game's copy", widgets::ButtonVariant::Secondary) {
                            asked = Some(Asked::UseGamesCopy(rel.clone()));
                        }
                        if widgets::compact_button(ui, theme, "Show the file", widgets::ButtonVariant::Secondary) {
                            asked = Some(Asked::Show(rel.clone()));
                        }
                    });
                }
            }
        }
    }

    if !not_used.is_empty() {
        ui.add_space(theme.spacing_xs);
        small(
            ui,
            theme,
            "Not used this run, because this version cannot read them (the game used its own copy instead):",
            theme.text_secondary(),
        );
        for (rel, why) in not_used {
            small(ui, theme, &format!("{rel}: {why}"), theme.warning());
        }
    }
    asked
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Show a file in the system's file manager, selected where it can be (Windows), else its folder.
fn show_file(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        // Explorer reads its own command line, and wants `/select,"<path>"` exactly as written,
        // which the usual argument quoting (the whole argument in quotes) does not give.
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    super::open_in_file_manager(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::refresh::{NotWritten, Report};

    /// The words drawn for `view`, and what a press would ask for (none: nothing is pressed).
    fn drawn(view: &View, not_used: &[(String, String)]) -> Vec<String> {
        let theme = crate::gui::theme::load_theme();
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw_view(ui, &theme, HintDisplay::Full, view, not_used);
            });
        });
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    /// The person sees which files the last update brought up to date and which it KEPT because
    /// they edited them, each kept one with "Use the game's copy" and "Show the file", and every
    /// file this run did not use, with why (design section 6).
    ///
    /// Seen red 2026-10-12 with the kept rows not drawn: "each kept file is named: [\"Data
    /// folder\", ...]"; and with a folder from before stamps given no `from`: the texts drawn,
    /// "Last update, to 0.1490.0 (...)" among them, where "from an earlier version" belongs.
    #[test]
    fn the_block_names_each_kept_file_with_its_two_buttons_and_each_file_not_used() {
        let view = View {
            dir: std::path::PathBuf::from("data"),
            left_alone: None,
            version: Some("0.1490.0".into()),
            stamped: true,
            last_update: Some(Report {
                from: Some("0.1489.0".into()),
                to: "0.1490.0".into(),
                at: 1_760_000_000,
                updated: vec!["items.csv".into(), "blueprints/ship_structure.ron".into()],
                added: vec![],
                kept: vec!["plants.csv".into()],
                failed: vec![NotWritten { file: "recipes.csv".into(), why: "it could not be written (in use)".into() }],
            }),
        };
        let not_used = vec![("status_effects.csv".to_string(), "this version cannot read line 21".to_string())];
        let texts = drawn(&view, &not_used);
        let has = |want: &str| texts.iter().any(|t| t.contains(want));
        assert!(has("Holds the data files of version 0.1490.0."), "{texts:?}");
        assert!(has("Last update, from 0.1489.0 to 0.1490.0") && has("2 files brought up to date"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "plants.csv"), "each kept file is named: {texts:?}");
        assert_eq!(texts.iter().filter(|t| *t == "Use the game's copy").count(), 1, "{texts:?}");
        assert_eq!(texts.iter().filter(|t| *t == "Show the file").count(), 1, "{texts:?}");
        assert!(has("Not brought up to date: recipes.csv: it could not be written (in use)"), "{texts:?}");
        assert!(has("status_effects.csv: this version cannot read line 21"), "{texts:?}");

        // A folder written before stamps: the last update says it came from an earlier version.
        let mut first = view.clone();
        first.last_update.as_mut().unwrap().from = None;
        let texts = drawn(&first, &[]);
        assert!(texts.iter().any(|t| t.starts_with("Last update, from an earlier version to 0.1490.0")), "{texts:?}");

        let alone = View { left_alone: Some("it is a source tree's own data folder".into()), ..view.clone() };
        let texts = drawn(&alone, &[]);
        assert!(texts.iter().any(|t| t == "The game does not update this folder: it is a source tree's own data folder."), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Use the game's copy"), "nothing to change in a folder the game leaves alone: {texts:?}");
    }
}
