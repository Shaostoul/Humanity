//! The numbers the in-world screens' aim assist and resizing run on, from `data/gui/screens.json`
//! (docs/design/in-world-screens.md, "Aim assist, any size, and each screen's own settings",
//! 2026-11-06). Read once for the run: the data folder's copy, else the copy built into the exe
//! (said in the log the way every data file says it), else the defaults below, so a broken file
//! never takes the screens down.
//!
//! It lives in the GUI layer, not beside the screens in `engine::screens`, because Settings, the
//! saved config and the cog's panel read it too, and those may not reach up into the engine (the
//! crate split's layers, scripts/crate-layers.json); the engine reads it from here.

use serde::Deserialize;

/// The file under data/.
pub const SCREENS_FILE: &str = "gui/screens.json";

/// "1.16 x 0.66 m, 52 inch": a screen picture's size and its diagonal, in the words of the
/// person's language (`screens.size_label`): under a screen's handles in build mode and in its
/// settings panel.
pub fn size_label(w: f32, h: f32) -> String {
    let inches = ((w * w + h * h).sqrt() / 0.0254).round() as i64;
    humanity_core::i18n::tf("screens.size_label", &[("w", &format!("{w:.2}")), ("h", &format!("{h:.2}")), ("inches", &inches)])
}

/// `aim`: the cone, the hysteresis, the snap inset and the reach rule.
#[derive(Debug, Clone, Deserialize)]
pub struct AimTuning {
    /// The cone a new person starts with, in degrees (Settings > Controls changes theirs).
    pub cone_deg_default: f32,
    /// The Settings slider's range.
    pub cone_deg_min: f32,
    pub cone_deg_max: f32,
    /// The current target is kept while its angle stays within the cone times this.
    pub exit_scale: f32,
    /// A rival takes over only when its angle is below the current target's times this.
    pub switch_ratio: f32,
    /// How far inside a target's rectangle the pointer lands when it snaps, in surface pixels.
    pub inset_px: f32,
    /// The least distance a screen answers from, in metres (the old fixed reach).
    pub reach_base_m: f32,
    /// Metres of reach per metre of the picture's diagonal.
    pub reach_per_diagonal: f32,
    /// The most distance any screen answers from, in metres.
    pub reach_max_m: f32,
}

impl Default for AimTuning {
    fn default() -> Self {
        Self {
            cone_deg_default: 3.0,
            cone_deg_min: 0.5,
            cone_deg_max: 10.0,
            exit_scale: 1.25,
            switch_ratio: 0.7,
            inset_px: 4.0,
            reach_base_m: 3.5,
            reach_per_diagonal: 1.75,
            reach_max_m: 25.0,
        }
    }
}

/// One size the cog's panel offers: a diagonal in inches and an aspect (width : height).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct SizePreset {
    pub diagonal_in: f32,
    pub aspect: (f32, f32),
}

impl SizePreset {
    /// The picture's (width, height) in metres.
    pub fn metres(&self) -> (f32, f32) {
        let (a, b) = (self.aspect.0.max(0.01), self.aspect.1.max(0.01));
        let per = self.diagonal_in.max(0.0) * 0.0254 / (a * a + b * b).sqrt();
        (a * per, b * per)
    }
}

/// `sizes`: the limits, the grid step, the handle width and the presets.
#[derive(Debug, Clone, Deserialize)]
pub struct SizeTuning {
    /// The smallest picture, (width, height) in metres.
    pub min_m: (f32, f32),
    /// The largest picture.
    pub max_m: (f32, f32),
    /// The build editor's grid step for sizes and moves, in metres (with grid snap on).
    pub grid_m: f32,
    /// The edge handles' width as a share of the picture's shorter side.
    pub handle_band: f32,
    #[serde(default)]
    pub presets: Vec<SizePreset>,
}

impl Default for SizeTuning {
    fn default() -> Self {
        Self { min_m: (0.2, 0.12), max_m: (8.0, 5.0), grid_m: 0.05, handle_band: 0.12, presets: Vec::new() }
    }
}

/// The whole file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreensTuning {
    #[serde(default)]
    pub aim: AimTuning,
    #[serde(default)]
    pub sizes: SizeTuning,
}

/// Read a screens file: refused when a range is upside down or a ratio is out of its sense, so a
/// typo cannot make a cone that never picks or a choice that flickers.
pub fn parse(text: &str) -> Result<ScreensTuning, String> {
    let t: ScreensTuning = serde_json::from_str(text).map_err(|e| format!("not a screens file: {e}"))?;
    let a = &t.aim;
    if !(a.cone_deg_min > 0.0 && a.cone_deg_min <= a.cone_deg_default && a.cone_deg_default <= a.cone_deg_max) {
        return Err("aim: the cone needs 0 < min <= default <= max".into());
    }
    if !(a.exit_scale >= 1.0 && a.switch_ratio > 0.0 && a.switch_ratio <= 1.0) {
        return Err("aim: exit_scale must be at least 1 and switch_ratio in (0, 1]".into());
    }
    let s = &t.sizes;
    if !(s.min_m.0 > 0.0 && s.min_m.1 > 0.0 && s.min_m.0 <= s.max_m.0 && s.min_m.1 <= s.max_m.1) {
        return Err("sizes: min_m must be above 0 and no larger than max_m".into());
    }
    Ok(t)
}

/// The tuning, read once for the run.
pub fn tuning() -> &'static ScreensTuning {
    static TUNING: std::sync::OnceLock<ScreensTuning> = std::sync::OnceLock::new();
    TUNING.get_or_init(|| {
        let path = humanity_core::storage::data_dir().join(SCREENS_FILE);
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
            Ok(t) => t,
            Err(why) => {
                // The copy built into the exe, used when the data folder's copy cannot be, and
                // said so the way every data file says it (the compiled-in check holds this).
                humanity_core::embedded_data::note_builtin_copy("gui/screens.json", format!("{} could not be used ({why})", path.display()));
                const BUILT_IN: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/gui/screens.json"));
                parse(BUILT_IN).unwrap_or_default()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped file parses, its presets read as the sizes they name (a 100" 16:9 picture is
    /// 2.214 x 1.245 m), and a file with an upside-down cone is refused.
    ///
    /// Seen red 2026-11-06 with the cone's range check switched off: the upside-down cone parsed.
    #[test]
    fn the_shipped_screens_file_parses_and_its_presets_are_the_sizes_they_name() {
        let text = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join(SCREENS_FILE)).expect("the file is there");
        let t = parse(&text).expect("the shipped file parses");
        assert!(t.sizes.presets.len() >= 3, "it offers presets");
        let hundred = t.sizes.presets.iter().find(|p| p.diagonal_in == 100.0 && p.aspect == (16.0, 9.0)).expect("100\" 16:9");
        let (w, h) = hundred.metres();
        assert!((w - 2.214).abs() < 1e-3 && (h - 1.245).abs() < 1e-3, "{w} x {h}");
        assert!(parse(r#"{"aim": {"cone_deg_default": 3, "cone_deg_min": 5, "cone_deg_max": 10, "exit_scale": 1.2, "switch_ratio": 0.7, "inset_px": 4, "reach_base_m": 3.5, "reach_per_diagonal": 2.5, "reach_max_m": 25}}"#).is_err());
    }
}
