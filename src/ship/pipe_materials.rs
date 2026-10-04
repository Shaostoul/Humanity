//! What a pipe, hose or cable is MADE OF, as data (2026-10-04).
//!
//! The pipe body draws its real material (copper, rubber hose, a cord's jacket)
//! everywhere between its marker bands; what flows inside is said by the bands
//! alone (`ship::pipe_marking`). No marking standard colours the wall material
//! (findings F28, `docs/reference/findings/2026-10-04-pipe-marking-standards.md`).
//! Before this, every pipe was painted wholly in a utility colour
//! (`MachineHome::connection_color`), and the material colours
//! `ConduitKind::color()` held had no caller; they moved here (copper's then
//! corrected from a web swatch to measured copper's reflectance).
//!
//! The registry is `data/piping/pipe_materials.ron`, read from disk first (so it can
//! be modded) and from the copy built into the exe when the file is missing or does
//! not parse. Pure serde, no renderer types: it compiles under `relay` too.

use crate::ship::conduits::ConduitKind;
use serde::Deserialize;

/// One material a conduit body can be drawn in.
#[derive(Debug, Clone, Deserialize)]
pub struct PipeMaterial {
    pub id: String,
    pub name: String,
    /// sRGB, 0-255: our rendition of a dielectric's colour, or a bare metal's MEASURED
    /// reflectance (the shader takes a metal's F0 straight from its base colour).
    pub srgb: (u8, u8, u8),
    pub metallic: f32,
    pub roughness: f32,
    #[serde(default)]
    pub note: String,
}

impl PipeMaterial {
    /// The colour in LINEAR light, alpha 1, for a renderer material's base colour
    /// (the PBR shader treats `base_color` as linear albedo).
    pub fn linear_rgba(&self) -> [f32; 4] {
        let (r, g, b) = self.srgb;
        [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), 1.0]
    }
}

/// How a run's body is drawn (`PipeMaterials::body_look`).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyLook {
    /// The engine's material cache key: one material per body material, shared by every run.
    pub key: String,
    /// Base colour in LINEAR light, alpha 1.
    pub linear: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}

/// Which material a conduit kind's body is drawn in.
#[derive(Debug, Clone, Deserialize)]
pub struct KindMaterial {
    pub kind: String,
    pub material: String,
}

/// The whole registry: data/piping/pipe_materials.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct PipeMaterials {
    pub kinds: Vec<KindMaterial>,
    pub materials: Vec<PipeMaterial>,
}

impl PipeMaterials {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn material(&self, id: &str) -> Option<&PipeMaterial> {
        self.materials.iter().find(|m| m.id == id)
    }

    /// The material a run of `kind` is drawn in.
    pub fn for_kind(&self, kind: ConduitKind) -> Option<&PipeMaterial> {
        let row = self.kinds.iter().find(|k| k.kind == kind.id())?;
        self.material(&row.material)
    }

    /// The material the body of a run carrying `content` (a connection kind such as
    /// "water") is drawn in: the conduit kind `ConduitKind::for_resource` picks for
    /// it, then that kind's material. The content's own colour never paints the body.
    pub fn for_content(&self, content: &str) -> Option<&PipeMaterial> {
        self.for_kind(ConduitKind::for_resource(content))
    }

    /// How the BODY of a run whose connection kind is `line_kind` is drawn: the material
    /// cache key and its linear colour, metal and roughness. `engine::home_meshes` draws every
    /// pipe body with exactly this, so a test of it is a test of what is drawn (2026-10-04
    /// review: the old test checked a helper of its own).
    pub fn body_look(&self, line_kind: &str) -> BodyLook {
        match self.for_content(line_kind) {
            Some(m) => BodyLook {
                key: format!("pipebody:{}", m.id),
                linear: m.linear_rgba(),
                metallic: m.metallic,
                roughness: m.roughness,
            },
            // A kind with no material row (the registry test forbids it): neutral grey.
            None => BodyLook { key: "pipebody:unknown".to_string(), linear: [0.3, 0.3, 0.3, 1.0], metallic: 0.0, roughness: 0.6 },
        }
    }

    /// Everything wrong with the registry, as sentences (empty = sound): every
    /// conduit kind has a row, every row names a known material, ids are unique,
    /// and the numbers are in range.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for kind in ConduitKind::ALL {
            match self.kinds.iter().filter(|k| k.kind == kind.id()).count() {
                0 => out.push(format!("conduit kind `{}` has no material row", kind.id())),
                1 => {}
                n => out.push(format!("conduit kind `{}` has {n} material rows", kind.id())),
            }
        }
        for k in &self.kinds {
            if !ConduitKind::ALL.iter().any(|c| c.id() == k.kind) {
                out.push(format!("material row for unknown conduit kind `{}`", k.kind));
            }
            if self.material(&k.material).is_none() {
                out.push(format!("conduit kind `{}` names unknown material `{}`", k.kind, k.material));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for m in &self.materials {
            if !seen.insert(m.id.as_str()) {
                out.push(format!("material `{}` is defined twice", m.id));
            }
            if !(0.0..=1.0).contains(&m.metallic) || !(0.0..=1.0).contains(&m.roughness) {
                out.push(format!("material `{}`: metallic and roughness must be 0..1", m.id));
            }
        }
        out
    }
}

/// The shipped file, the fallback when the data folder's copy is missing or does
/// not parse.
const SHIPPED_PIPE_MATERIALS: &str = include_str!("../../data/piping/pipe_materials.ron");

/// The registry, loaded once: the data folder's copy first, the shipped copy if that
/// is missing or does not parse (and the log says so, BUG-133).
pub fn pipe_materials() -> &'static PipeMaterials {
    static REG: std::sync::OnceLock<PipeMaterials> = std::sync::OnceLock::new();
    REG.get_or_init(load_pipe_materials)
}

fn load_pipe_materials() -> PipeMaterials {
    let path = crate::data_dir().join("piping").join("pipe_materials.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match PipeMaterials::parse(&text) {
            Ok(m) => return m,
            Err(e) => crate::embedded_data::note_builtin_copy(
                "piping/pipe_materials.ron",
                format_args!("{} does not parse ({e})", path.display()),
            ),
        },
        Err(e) => crate::embedded_data::note_builtin_copy(
            "piping/pipe_materials.ron",
            format_args!("{} could not be read ({e})", path.display()),
        ),
    }
    PipeMaterials::parse(SHIPPED_PIPE_MATERIALS).unwrap_or_else(|e| {
        log::error!("the shipped data/piping/pipe_materials.ron does not parse: {e}");
        PipeMaterials { kinds: Vec::new(), materials: Vec::new() }
    })
}

/// One sRGB channel (0-255) to linear light (the IEC 61966-2-1 curve).
pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> PipeMaterials {
        PipeMaterials::parse(SHIPPED_PIPE_MATERIALS).expect("the shipped pipe_materials.ron parses")
    }

    #[test]
    fn shipped_pipe_materials_registry_parses_and_is_sound() {
        let reg = shipped();
        assert_eq!(reg.problems(), Vec::<String>::new());
    }

    /// The pipe body is drawn in its MATERIAL, not in the colour of what it
    /// carries: a water run is copper, a power run a black cord, and none takes
    /// the marking scheme's content colour. This checks `body_look`, the very
    /// call `engine::home_meshes` draws every pipe body with (2026-10-04
    /// review: the first version checked a helper of the test's own, which
    /// would have stayed green had the pipes gone back to their utility paint).
    ///
    /// Seen red with `body_look` painting the run in its legend colour
    /// (`MachineHome::connection_color`, the pipes before this change): "a
    /// water pipe's body is copper, not the colour of its content".
    #[test]
    fn pipe_body_colour_comes_from_its_material_not_its_content() {
        let reg = shipped();
        let copper = reg.material("copper").expect("copper");
        let water = reg.body_look("water");
        assert_eq!(water.linear, copper.linear_rgba(), "a water pipe's body is copper, not the colour of its content: {water:?}");
        assert_eq!((water.metallic, water.roughness), (copper.metallic, copper.roughness), "and has copper's finish");
        assert_eq!(water.key, "pipebody:copper", "one cached material per body material");
        let cord = reg.material("power_cord").expect("power_cord");
        assert_eq!(reg.body_look("power").linear, cord.linear_rgba(), "a power run is its cord");
        // And never the content's own colour, in any routed content.
        let scheme = crate::ship::pipe_marking::marking().default_scheme().expect("the ship's scheme");
        for row in &scheme.contents {
            let Some(main) = scheme.colour(&row.main) else { continue };
            assert_ne!(
                reg.body_look(&row.content).linear,
                main.linear_rgba(),
                "`{}`'s body must not be painted its marker colour",
                row.content
            );
        }
    }

    /// A bare metal's colour is its reflectance (the shader takes a metal's F0 straight from its
    /// base colour, `mix(0.04, albedo, metallic)`, and gives it no diffuse), so a metal row must
    /// reflect at least the 4% any non-metal does in every channel, and, like every metal in
    /// Real-Time Rendering's measured table (4th ed., Table 9.2; titanium is the lowest at 0.542),
    /// at least half the light in its brightest one (2026-10-04 review).
    ///
    /// Seen red with the copper row at the old #B87333 web swatch: "`copper` reflects
    /// [0.47932, 0.17144, 0.03310] in linear light: under the 4% any non-metal reflects".
    #[test]
    fn a_metal_reflects_what_measured_metals_reflect() {
        let reg = shipped();
        let mut metals = 0;
        for m in reg.materials.iter().filter(|m| m.metallic >= 0.5) {
            metals += 1;
            let l = m.linear_rgba();
            assert!(
                l[..3].iter().all(|c| *c >= 0.04),
                "`{}` reflects [{:.5}, {:.5}, {:.5}] in linear light: under the 4% any non-metal reflects",
                m.id,
                l[0],
                l[1],
                l[2]
            );
            assert!(l[..3].iter().cloned().fold(0.0, f32::max) >= 0.5, "`{}` reflects under half the light in every channel: {l:?}", m.id);
        }
        assert!(metals >= 1, "the registry carries a metal (copper)");
        // Copper is measured copper: F0 (0.955, 0.638, 0.538) linear, Table 9.2.
        let cu = reg.material("copper").expect("copper").linear_rgba();
        for (got, want) in cu[..3].iter().zip([0.955, 0.638, 0.538]) {
            assert!((got - want).abs() < 0.01, "copper's reflectance is the measured one: {cu:?}");
        }
    }

    #[test]
    fn srgb_to_linear_matches_the_curve() {
        assert_eq!(srgb_to_linear(0), 0.0);
        assert!((srgb_to_linear(255) - 1.0).abs() < 1e-6);
        // Mid grey 128 is about 0.2158 in linear light.
        assert!((srgb_to_linear(128) - 0.2158).abs() < 1e-3);
    }
}
