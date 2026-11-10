//! Font fallback chains for the native app.
//!
//! egui resolves a glyph by walking a family's font list in order, so a
//! "family" here is really a fallback CHAIN: the first font that has the
//! codepoint wins. That property does all the work in this module. We add
//! faces to the tail of a chain to fill in what the primary face lacks, and
//! we never have to ship a font that covers everything.
//!
//! Three fallbacks are installed, in order of how much they matter:
//!
//! 1. **Hack into the Proportional chain.** epaint's default Proportional
//!    chain is `[Ubuntu-Light, NotoEmoji-Regular, emoji-icon-font]`, and
//!    Ubuntu-Light has no arrows and no box drawing. `U+2192` alone appears
//!    111 times across `src/gui`, so on Windows it rendered only because
//!    Segoe UI Emoji happened to supply it, and on Linux and macOS it was
//!    blank or tofu. Hack has 109 of 112 arrows and all 128 box-drawing
//!    characters, and epaint already compiles it in for the Monospace chain,
//!    so putting it SECOND in Proportional (after Ubuntu-Light, so Latin is
//!    untouched) fixes all of them for zero bytes and zero licence work.
//!
//! 2. **The user's system CJK font.** `data/i18n/` ships ja.json and zh.json,
//!    and not one of the four faces epaint bundles contains a single CJK
//!    codepoint, so Japanese and Chinese render as tofu in the app today. A
//!    real CJK face is 4 to 16 MB per weight, far too much for the release
//!    archive, so we read the one already installed on the user's machine.
//!
//! 3. **The user's system emoji font.** egui's bundled `NotoEmoji-Regular`
//!    covers a small subset; many common emoji render as tofu without this.
//!
//! For 2 and 3 we READ the user's installed font and redistribute nothing, so
//! Microsoft and Apple proprietary faces stay on their own machines.
//!
//! **`set_fonts` is called unconditionally.** It used to be called inside the
//! emoji-font success branch, which meant that on a machine with no emoji font
//! at a known path it was never called at all, and any other font work would
//! have silently failed to install. See docs/design/typography.md.

use egui::FontFamily;

/// Install every font fallback chain on an egui context (re-exported as `crate::gui::install_fonts`).
/// Called for the main UI context at boot AND for every in-world screen surface context, so a
/// screen can never show tofu the main UI does not: the two contexts get identical font stacks by
/// going through one function. (Moved here from gui/mod.rs on 2026-11-10, a wrapper beside what it
/// wraps, to make room under that file's line budget.)
pub fn install_fonts(ctx: &egui::Context) {
    install_font_fallbacks(ctx);
}

/// Install every font fallback chain. Call once during init AFTER the egui
/// `Context` exists but BEFORE the first frame.
///
/// Each fallback is independent: a machine with no CJK font still gets the
/// arrow fix and whatever emoji it has.
pub fn install_font_fallbacks(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions(humanity_core::reuse::enabled()));
}

/// The definitions a context installs (stutter survey 2, 2026-10-31). Every in-world page screen
/// has an egui context of its own, and each built these afresh: it READ the system CJK and emoji
/// files again (about 15.7 MB on the operator's PC: Yu Gothic and Segoe UI Emoji), and egui copied
/// each file's bytes again on the context's first frame to parse them (`FontData::from_owned`
/// clones its bytes for every context that uses it). That copy, a fresh 15.7 MB allocation, was
/// most of the 5 ms every screen's first frame cost before its page drew anything, and the
/// thirteen screens of the home held thirteen copies, about 200 MB.
///
/// So the definitions are built once a process and every context borrows the same bytes
/// (`share_font_bytes`: egui then parses a borrowed slice in place). The system font files are
/// read at startup, as the main context always read them; nothing derived is kept. With the Dev
/// page's "Turn off all reuse" (`reuse` false) each context builds its own, as before.
fn font_definitions(reuse: bool) -> egui::FontDefinitions {
    if !reuse {
        return build_font_definitions();
    }
    static SHARED: std::sync::OnceLock<egui::FontDefinitions> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| share_font_bytes(build_font_definitions())).clone()
}

/// The same definitions, every font's bytes borrowed: a font read from a file (owned bytes) is
/// kept for the rest of the process (the font files are a few megabytes, held for as long as any
/// context draws text, which is the whole run) and handed out as a `'static` slice, so a context's
/// first frame parses it where it lies instead of copying it. Built-in fonts are borrowed already.
fn share_font_bytes(mut defs: egui::FontDefinitions) -> egui::FontDefinitions {
    for data in defs.font_data.values_mut() {
        if let std::borrow::Cow::Owned(bytes) = &data.font {
            let kept: &'static [u8] = Box::leak(bytes.clone().into_boxed_slice());
            let mut shared = egui::FontData::from_static(kept);
            shared.index = data.index;
            shared.tweak = data.tweak;
            *data = std::sync::Arc::new(shared);
        }
    }
    defs
}

/// Build the fallback chains. Split out from [`install_font_fallbacks`] so the
/// tests can assert on the REAL definitions this app installs rather than on a
/// copy of the logic, which would be a check that cannot fail.
fn build_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    // ── 0. Noto Sans as the face people actually read ──
    // Chosen for global access: 2,965 codepoints against Ubuntu-Light's 1,194,
    // with complete Cyrillic (256/256), Cyrillic Supplement (48/48), Greek
    // Extended (233/256), Latin Extended-B (208/208) and Vietnamese. The
    // project's stated mission is to account for all humans, and this is the
    // only family whose own stated mission is the same shape: no tofu for any
    // human language.
    //
    // Also a weight fix. Nobody ever chose Ubuntu-LIGHT; it arrived with
    // egui's default_fonts feature. Light weights measure worse for legibility
    // with higher oculomotor load (Burmistrov et al. 2016, NordiCHI), and that
    // penalty landed on every user on every platform with nothing toggled.
    //
    // Static, not variable, and that is forced: epaint's FontData exposes only
    // font, index and tweak, and nothing in epaint or egui ever calls
    // ab_glyph's set_variation, so a variable file would render at its default
    // instance forever. Unhinted, because ttf-parser (via ab_glyph) has no
    // hinting interpreter at all, so the ~190 KB of hinting per file cannot
    // change a single rendered pixel here.
    //
    // Ubuntu-Light stays in the chain below rather than being removed: it
    // carries U+221E and a few other symbols Noto Sans does not, and it is
    // already compiled in, so keeping it costs nothing.
    //
    // OFL-1.1, and its copyright statement declares no Reserved Font Name, so
    // unlike most alternatives this face may be modified and still shipped
    // under its own name. Licence text ships at data/fonts/OFL.txt.
    // See docs/design/typography.md.
    fonts.font_data.insert(
        "NotoSans".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/fonts/NotoSans-Regular.ttf")))),
    );
    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        proportional.insert(0, "NotoSans".to_owned());
    }

    // ── 1. Hack into the Proportional chain ──
    // Behind BOTH text faces, so it only ever supplies codepoints neither of
    // them has: arrows, box drawing, most of Math Operators. Noto Sans carries
    // 0 of the 112 codepoints in the Arrows block and Ubuntu-Light carries 0
    // as well, while Hack carries 109, and epaint puts Hack in Monospace only.
    // That is why U+2192, which appears 111 times across src/gui, was blank or
    // tofu on Linux and macOS and rendered on Windows purely because Segoe UI
    // Emoji happened to cover it.
    //
    // The position is computed rather than hardcoded, so adding or reordering
    // a text face above cannot silently push Hack in front of one of them.
    // "Hack" is already in `font_data` because epaint bundles it for
    // Monospace, so this costs zero bytes.
    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        if !proportional.iter().any(|f| f == "Hack") {
            let after_text_faces = proportional
                .iter()
                .position(|f| f == "Ubuntu-Light")
                .map(|i| i + 1)
                .unwrap_or_else(|| usize::min(1, proportional.len()));
            proportional.insert(after_text_faces, "Hack".to_owned());
        }
    }

    // ── 2. System CJK font ──
    if let Some((name, bytes, path)) = read_first_font(&platform_cjk_paths(), "system_cjk") {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        // Append to the TAIL of both chains so it never competes for Latin.
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
        log::info!("fonts: CJK fallback from {:?}", path);
    } else {
        log::warn!(
            "fonts: no system CJK font found, Japanese and Chinese will render as tofu. \
             Tried Windows Yu Gothic / MS Gothic / SimSun, macOS Hiragino / PingFang, \
             and the usual Linux Noto CJK paths."
        );
    }

    // ── 3. System emoji font ──
    if let Some((name, bytes, path)) = read_first_font(&platform_emoji_paths(), "system_emoji") {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
        log::info!("fonts: emoji fallback from {:?}", path);
    } else {
        log::warn!(
            "fonts: no system emoji font found, the full emoji set may render as tofu. \
             Tried Windows Segoe UI Emoji / macOS Apple Color Emoji / Linux Noto Color Emoji."
        );
    }

    fonts
}

/// Read the first readable font from a candidate list.
/// Returns `(family_name, bytes, path_it_came_from)`.
fn read_first_font(
    candidates: &[std::path::PathBuf],
    name: &str,
) -> Option<(String, Vec<u8>, std::path::PathBuf)> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            return Some((name.to_owned(), bytes, path.clone()));
        }
    }
    None
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
        // Linux + others, try common Noto Emoji install paths.
        out.push(PathBuf::from("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf"));
        out.push(PathBuf::from("/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf"));
        out.push(PathBuf::from("/usr/share/fonts/noto/NotoColorEmoji.ttf"));
        out.push(PathBuf::from("/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf"));
    }
    out
}

/// Platform-specific candidate paths for a font with CJK coverage.
///
/// Ordered by how broad the coverage is: a face covering both Japanese kana
/// and Chinese hanzi is preferred over a Japanese-only or Chinese-only one,
/// because a single fallback entry has to serve both `ja` and `zh`.
fn platform_cjk_paths() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut out = Vec::new();
    if cfg!(target_os = "windows") {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_owned());
        // Yu Gothic covers kana + a large hanzi set; MS Gothic and SimSun are
        // the older fallbacks that exist on almost every install.
        for f in ["YuGothR.ttc", "YuGothM.ttc", "msgothic.ttc", "simsun.ttc", "msyh.ttc"] {
            out.push(PathBuf::from(format!("{}\\Fonts\\{}", windir, f)));
        }
    } else if cfg!(target_os = "macos") {
        out.push(PathBuf::from("/System/Library/Fonts/PingFang.ttc"));
        out.push(PathBuf::from("/System/Library/Fonts/Hiragino Sans GB.ttc"));
        out.push(PathBuf::from("/Library/Fonts/Arial Unicode.ttf"));
    } else {
        // Linux and Android. Noto CJK is the near-universal answer; the
        // Android path is included because a future Android build reads the
        // same list.
        for p in [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJKjp-Regular.otf",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/system/fonts/NotoSansCJK-Regular.ttc",
            "/system/fonts/DroidSansFallback.ttf",
        ] {
            out.push(PathBuf::from(p));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The arrow fix is the one fallback that must work on every machine,
    /// because it depends on no system font at all. This asserts on the REAL
    /// definitions `install_font_fallbacks` installs, and pins the ORDER as
    /// well as the presence: Hack must come after Ubuntu-Light, or it would
    /// become the proportional face people read.
    ///
    /// Proven red before being trusted: deleting the insert in
    /// `build_font_definitions` fails this on the `[1] == "Hack"` assert, and
    /// changing the index to 0 fails it on the `[0] == "Ubuntu-Light"` assert.
    #[test]
    fn hack_backfills_proportional_after_ubuntu() {
        let fonts = build_font_definitions();
        let proportional = fonts
            .families
            .get(&FontFamily::Proportional)
            .expect("proportional family exists");

        let idx = |name: &str| {
            proportional
                .iter()
                .position(|f| f == name)
                .unwrap_or_else(|| panic!("{name} missing from the Proportional chain"))
        };

        assert_eq!(
            proportional[0], "NotoSans",
            "Noto Sans must be the face people read"
        );
        assert!(
            idx("Ubuntu-Light") > idx("NotoSans"),
            "Ubuntu-Light is a coverage backfill (it carries U+221E), not the primary face"
        );
        assert!(
            idx("Hack") > idx("Ubuntu-Light"),
            "Hack must sit behind BOTH text faces, or a monospace glyph would win \
             for a codepoint a text face already has"
        );
        assert!(
            idx("Hack") < idx("NotoEmoji-Regular"),
            "Hack must come before the emoji faces so arrows and box drawing \
             resolve from it rather than tofuing"
        );
    }

    /// The default epaint chain is the premise the fix rests on. If epaint
    /// ever ships arrows in Ubuntu-Light or adds Hack to Proportional itself,
    /// this fallback becomes redundant and should be removed rather than left
    /// to rot.
    #[test]
    fn epaint_default_still_lacks_hack_in_proportional() {
        let stock = egui::FontDefinitions::default();
        let proportional = stock.families.get(&FontFamily::Proportional).unwrap();
        assert_eq!(
            proportional.first().map(String::as_str),
            Some("Ubuntu-Light"),
            "epaint's default proportional face changed; revisit the insert index"
        );
        assert!(
            !proportional.iter().any(|f| f == "Hack"),
            "epaint now ships Hack in Proportional; this fallback is redundant, delete it"
        );
    }

    /// Whatever the machine has or lacks, the chains are built and the
    /// proportional family is never empty. This is the regression guard for
    /// the bug where `set_fonts` lived inside the emoji success branch and was
    /// skipped entirely when no emoji font was found.
    #[test]
    fn definitions_are_built_regardless_of_system_fonts() {
        let fonts = build_font_definitions();
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let chain = fonts.families.get(&family).expect("family exists");
            assert!(!chain.is_empty(), "{family:?} chain is empty");
            for name in chain {
                assert!(
                    fonts.font_data.contains_key(name),
                    "{family:?} names {name} but no font data was registered for it"
                );
            }
        }
    }

    /// A missing system font must not stop the chain being installed. This is
    /// the regression guard for the bug where set_fonts lived inside the emoji
    /// success branch and was skipped entirely when no emoji font was found.
    #[test]
    fn read_first_font_is_none_for_missing_paths_and_does_not_panic() {
        let nowhere = vec![
            std::path::PathBuf::from("/definitely/not/a/font/one.ttf"),
            std::path::PathBuf::from("/definitely/not/a/font/two.ttc"),
        ];
        assert!(read_first_font(&nowhere, "system_cjk").is_none());
    }

    /// Both probes must offer candidates on whatever platform the tests run
    /// on, or the fallback silently cannot fire there.
    #[test]
    fn both_probes_offer_candidates_on_this_platform() {
        assert!(!platform_emoji_paths().is_empty(), "no emoji candidates for this platform");
        assert!(!platform_cjk_paths().is_empty(), "no CJK candidates for this platform");
    }

    /// EVERY CONTEXT BORROWS ONE COPY OF THE FONT FILES (stutter survey 2, 2026-10-31). Each wall
    /// screen's context copied the system CJK and emoji files (owned bytes, which egui copies again
    /// on a context's first frame to parse them), the bulk of every screen's first-frame cost
    /// before its page drew anything. With reuse on, two contexts' definitions are the same
    /// borrowed bytes, and they are the very fonts and chains a context built afresh gets (the Dev
    /// page's switch off), byte for byte.
    ///
    /// Seen red 2026-10-31 with `font_definitions` building afresh whatever the switch said (as
    /// before): "system_cjk is a copy of its own".
    #[test]
    fn every_context_borrows_one_copy_of_the_font_files() {
        let (a, b) = (font_definitions(true), font_definitions(true));
        for (name, data) in &a.font_data {
            assert!(matches!(data.font, std::borrow::Cow::Borrowed(_)), "{name} is a copy of its own");
            assert_eq!(data.font.as_ptr(), b.font_data[name].font.as_ptr(), "{name}: two contexts read one copy of its bytes");
        }
        let fresh = font_definitions(false);
        assert_eq!(fresh.families, a.families, "the same fallback chains");
        assert_eq!(fresh.font_data.keys().collect::<Vec<_>>(), a.font_data.keys().collect::<Vec<_>>(), "the same fonts");
        for (name, data) in &fresh.font_data {
            let shared = &a.font_data[name];
            assert!(*data.font == *shared.font, "{name}: the same bytes");
            assert_eq!((data.index, data.tweak), (shared.index, shared.tweak), "{name}: the same face and tweak");
        }
    }
}
