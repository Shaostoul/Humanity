//! What a pipe, hose or cable is MADE OF, as drawn today (2026-10-04; since 2026-11-02 a view of
//! the one material catalog).
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
//! THE CATALOG ITSELF moved down to the core on 2026-11-02 (spline routing rung 0):
//! `humanity_core::run_materials` reads `data/piping/pipe_materials.ron`, which grew into the one
//! catalog of what a run is made of (looks, behaviour classes, sizes and their specs, the old cable
//! registry merged in), because the core's cable checks read it and the core may not reach up
//! here. What stays here is the part that knows today's four drawn kinds (`ConduitKind`): which
//! catalog material each kind's body is drawn in, and the draw call's look. Rung 1 gives every run
//! its own material and this view goes with `ConduitKind`.

use crate::ship::conduits::ConduitKind;
use humanity_core::run_materials::{RunMaterial, RunMaterials};

/// One sRGB channel (0-255) to linear light; in the core with the catalog, named here too for the
/// callers that colour other things by it.
pub use humanity_core::run_materials::srgb_to_linear;

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

/// Today's drawn kinds over the material catalog: which material each `ConduitKind` is drawn in.
#[derive(Debug, Clone, Copy)]
pub struct PipeMaterials<'a> {
    pub catalog: &'a RunMaterials,
}

impl<'a> PipeMaterials<'a> {
    pub fn material(&self, id: &str) -> Option<&'a RunMaterial> {
        self.catalog.material(id)
    }

    /// The material a run of `kind` is drawn in.
    pub fn for_kind(&self, kind: ConduitKind) -> Option<&'a RunMaterial> {
        let row = self.catalog.kinds.iter().find(|k| k.kind == kind.id())?;
        self.material(&row.material)
    }

    /// The material the body of a run carrying `content` (a connection kind such as
    /// "water") is drawn in: the conduit kind `ConduitKind::for_resource` picks for
    /// it, then that kind's material. The content's own colour never paints the body.
    pub fn for_content(&self, content: &str) -> Option<&'a RunMaterial> {
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
                linear: m.look.linear_rgba(),
                metallic: m.look.metallic,
                roughness: m.look.roughness,
            },
            // A kind with no material row (the registry test forbids it): neutral grey.
            None => BodyLook { key: "pipebody:unknown".to_string(), linear: [0.3, 0.3, 0.3, 1.0], metallic: 0.0, roughness: 0.6 },
        }
    }

    /// Everything wrong with the kinds' rows, as sentences (empty = sound): every conduit kind has
    /// exactly one row and no row names an unknown kind. (The catalog's own soundness, its ids,
    /// numbers and sources, is `RunMaterials::problems`.)
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for kind in ConduitKind::ALL {
            match self.catalog.kinds.iter().filter(|k| k.kind == kind.id()).count() {
                0 => out.push(format!("conduit kind `{}` has no material row", kind.id())),
                1 => {}
                n => out.push(format!("conduit kind `{}` has {n} material rows", kind.id())),
            }
        }
        for k in &self.catalog.kinds {
            if !ConduitKind::ALL.iter().any(|c| c.id() == k.kind) {
                out.push(format!("material row for unknown conduit kind `{}`", k.kind));
            }
        }
        out
    }
}

/// The catalog as drawn, loaded once (`humanity_core::run_materials::run_materials`: the data
/// folder's copy first, the shipped copy if that is missing or does not parse, and the log says so).
pub fn pipe_materials() -> PipeMaterials<'static> {
    PipeMaterials { catalog: humanity_core::run_materials::run_materials() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped catalog (the core's own tests check its soundness and its sources).
    const SHIPPED_CATALOG: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/pipe_materials.ron"));

    fn shipped() -> RunMaterials {
        RunMaterials::parse(SHIPPED_CATALOG).expect("the shipped pipe_materials.ron parses")
    }

    #[test]
    fn shipped_pipe_materials_registry_parses_and_is_sound() {
        let cat = shipped();
        let reg = PipeMaterials { catalog: &cat };
        assert_eq!(reg.problems(), Vec::<String>::new());
        assert_eq!(cat.problems(), Vec::<String>::new());
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
        let cat = shipped();
        let reg = PipeMaterials { catalog: &cat };
        let copper = reg.material("copper_l_drawn").expect("copper_l_drawn");
        let water = reg.body_look("water");
        assert_eq!(water.linear, copper.look.linear_rgba(), "a water pipe's body is copper, not the colour of its content: {water:?}");
        assert_eq!((water.metallic, water.roughness), (copper.look.metallic, copper.look.roughness), "and has copper's finish");
        assert_eq!(water.key, "pipebody:copper_l_drawn", "one cached material per body material");
        let cord = reg.material("power_cord").expect("power_cord");
        assert_eq!(reg.body_look("power").linear, cord.look.linear_rgba(), "a power run is its cord");
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

    /// The four drawn kinds keep the look they had before the catalog grew (2026-11-02): the merge
    /// moved rows, it did not repaint a pipe. The values are the 2026-10-04 rows'.
    #[test]
    fn the_drawn_kinds_keep_their_looks_through_the_catalog_merge() {
        let cat = shipped();
        let reg = PipeMaterials { catalog: &cat };
        for (kind, srgb, metallic, roughness) in [
            (ConduitKind::RigidCopper, (250, 209, 194), 1.0, 0.4),
            (ConduitKind::FlexibleHose, (51, 56, 61), 0.0, 0.75),
            (ConduitKind::PowerCord, (26, 26, 31), 0.0, 0.6),
            (ConduitKind::DataCable, (168, 170, 172), 0.0, 0.55),
        ] {
            let m = reg.for_kind(kind).unwrap_or_else(|| panic!("{kind:?} has a material"));
            assert_eq!((m.look.srgb, m.look.metallic, m.look.roughness), (srgb, metallic, roughness), "{kind:?} is drawn as before");
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
        let cat = shipped();
        let mut metals = 0;
        for m in cat.materials.iter().filter(|m| m.look.metallic >= 0.5) {
            metals += 1;
            let l = m.look.linear_rgba();
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
        assert!(metals >= 1, "the catalog carries a metal (copper)");
        // Copper is measured copper: F0 (0.955, 0.638, 0.538) linear, Table 9.2.
        let cu = cat.material("copper_l_drawn").expect("copper_l_drawn").look.linear_rgba();
        for (got, want) in cu[..3].iter().zip([0.955, 0.638, 0.538]) {
            assert!((got - want).abs() < 0.01, "copper's reflectance is the measured one: {cu:?}");
        }
    }
}
