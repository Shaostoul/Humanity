//! The word of caution under a file someone was sent (2026-10-17): what kind of thing it is when
//! that deserves care (a program, a script, an archive, a document that can carry macros, a
//! shortcut, a web page, a Blender file), and loudest when its name pretends to be something
//! harmless. What to say comes from `net::file_risk`; this only draws it, in the theme's warning
//! colour, or its danger colour for a disguised file. Used by the chat's file cards (private and
//! public), the Media item page's file rows and files sent device to device.
//!
//! Web mirror: the `file-caution` class (`file-caution disguised`), written by the pages that use
//! web/shared/file-risk.js.

use crate::gui::theme::Theme;
use crate::net::file_risk::Caution;
use egui::{Label, RichText, Ui};

/// Draw `caution` as one wrapped line under the file it is about.
pub fn show(ui: &mut Ui, theme: &Theme, caution: &Caution) {
    let color = if caution.disguised { theme.danger() } else { theme.warning() };
    let text = RichText::new(format!("\u{26a0} {}", caution.words)).size(theme.font_size_small).color(color);
    ui.add(Label::new(if caution.disguised { text.strong() } else { text }).wrap());
}

/// Show `path` in the computer's file manager, selected where it can be (Windows), else its folder.
/// The files' cards offer this rather than opening a file straight from the app: the person then
/// opens it themselves, with whatever program they choose, after the word of caution.
pub fn show_in_folder(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        // Explorer reads its own command line, and wants `/select,"<path>"` exactly as written,
        // which the usual argument quoting (the whole argument in quotes) does not give.
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let dir = if path.is_file() { path.parent().unwrap_or(path) } else { path };
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(dir).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}
