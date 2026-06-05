//! Emoji font fallback (v0.190.0).
//!
//! Egui's bundled `NotoEmoji-Regular` covers a small subset; many common
//! emoji (colored hearts, hand gestures, the modern face set, etc.)
//! render as tofu (▢). This module loads the user's SYSTEM emoji font
//! at startup as an additional fallback so the bundled subset is
//! supplemented by everything the OS ships with.
//!
//! Platform paths:
//!   - Windows: `C:\Windows\Fonts\seguiemj.ttf` (Segoe UI Emoji)
//!   - macOS:   `/System/Library/Fonts/Apple Color Emoji.ttc`
//!   - Linux:   `/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf` and a
//!              few fallback locations
//!
//! We load by READING the user's installed font (no redistribution), so
//! Microsoft / Apple proprietary fonts stay on their machines.

use egui::FontFamily;

/// Try to install a system emoji font as a fallback for both proportional
/// and monospace families. Silent no-op if no platform font is available.
/// Call once during app init AFTER the egui Context exists but BEFORE the
/// first frame.
pub fn install_system_emoji_fallback(ctx: &egui::Context) {
    let candidates = platform_emoji_paths();
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            // Found a font — install it as a fallback.
            let mut fonts = egui::FontDefinitions::default();
            let font_name = "system_emoji".to_string();
            fonts.font_data.insert(
                font_name.clone(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            // Append (NOT prepend) so default fonts win for ASCII / Latin /
            // arrows, and the emoji font fills in only for codepoints the
            // defaults don't have. This is how egui's fallback chain works.
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(font_name.clone());
            }
            ctx.set_fonts(fonts);
            log::info!("Installed system emoji font from {:?}", path);
            return;
        }
    }
    log::warn!(
        "No system emoji font found, full emoji set may render as tofu. \
         Tried: Windows Segoe UI Emoji / macOS Apple Color Emoji / Linux Noto Color Emoji."
    );
}

/// Platform-specific candidate paths for an emoji-capable font.
fn platform_emoji_paths() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut out = Vec::new();
    if cfg!(target_os = "windows") {
        if let Ok(windir) = std::env::var("WINDIR") {
            out.push(PathBuf::from(format!("{}\\Fonts\\seguiemj.ttf", windir)));
        }
        out.push(PathBuf::from(r"C:\Windows\Fonts\seguiemj.ttf"));
    } else if cfg!(target_os = "macos") {
        out.push(PathBuf::from("/System/Library/Fonts/Apple Color Emoji.ttc"));
        out.push(PathBuf::from("/Library/Fonts/Apple Color Emoji.ttc"));
    } else {
        // Linux + others — try common Noto Emoji install paths.
        out.push(PathBuf::from("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf"));
        out.push(PathBuf::from("/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf"));
        out.push(PathBuf::from("/usr/share/fonts/noto/NotoColorEmoji.ttf"));
        out.push(PathBuf::from("/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf"));
    }
    out
}
