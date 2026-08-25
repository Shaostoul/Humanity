//! Turning a drawn line into a real wall.
//!
//! The player draws where a wall goes. This module expands that line, plus an
//! assembly recipe from `data/assemblies/*.ron`, into an ORDERED LIST OF
//! PLACEMENT STEPS. That list is the single source of truth for four different
//! things, which is the whole reason it exists:
//!
//!   1. Geometry. Instant mode folds every step at once and you have a mesh.
//!   2. Animation. The same steps played over time are parts flying into place,
//!      fasteners going in, welds appearing in the order a welder would run them.
//!   3. A bill of materials. Counting the parts gives a real cut list, which is
//!      the bridge from the game to somebody's actual weekend.
//!   4. Teaching. Every step carries a `why`, so watching the build IS the
//!      lesson. Buildings mostly do not fail because a member was slightly too
//!      small; they fail at connections, at load paths that never reach the
//!      ground, and from things left out. A sequence shows exactly those.
//!
//! Instant mode is deliberately first: it is `fold(steps)`, so if the step list
//! is right the animation is a view over it and cannot disagree with the mesh.
//!
//! Nothing here is a building code. It is geometry and sequence derived from an
//! open recipe file. What it can honestly say is "a wall built this way has its
//! parts in these places in this order". It cannot say a structure is safe, is
//! permittable, or meets any code, and no caller should phrase it that way.

use serde::Deserialize;

/// A member's cross section, dressed (actual) size in metres.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct Section {
    pub thickness_m: f32,
    pub depth_m: f32,
}

impl<'de> serde::de::Deserialize<'de> for SectionTuple {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (t, dp) = <(f32, f32)>::deserialize(d)?;
        Ok(SectionTuple(Section { thickness_m: t, depth_m: dp }))
    }
}

/// Newtype so the RON can write `section: (0.038, 0.140)` as a plain tuple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionTuple(pub Section);

#[derive(Debug, Clone, Deserialize)]
pub struct MemberSpec {
    pub role: String,
    pub label: String,
    pub section: SectionTuple,
    /// How many of these a wall gets: "one_per_wall", "two_per_wall", "spaced",
    /// "two_per_opening", "one_per_opening", "one_per_window".
    pub count_rule: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SequenceStep {
    pub group: String,
    pub why: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FastenerSpec {
    pub joint: String,
    pub kind: String,
    /// Fixed count per joint, or 0 when the count is computed (sheathing).
    pub count: u32,
    pub pattern: String,
    pub why: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SheathingSpec {
    pub label: String,
    pub thickness_m: f32,
    pub panel_w_m: f32,
    pub panel_h_m: f32,
    pub why: String,
}

/// One assembly recipe, loaded from `data/assemblies/<id>.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct Assembly {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    pub members: Vec<MemberSpec>,
    pub spacing_m: f32,
    pub sequence: Vec<SequenceStep>,
    pub fasteners: Vec<FastenerSpec>,
    pub sheathing: SheathingSpec,
}

impl Assembly {
    pub fn member(&self, role: &str) -> Option<&MemberSpec> {
        self.members.iter().find(|m| m.role == role)
    }
    pub fn fastener(&self, joint: &str) -> Option<&FastenerSpec> {
        self.fasteners.iter().find(|f| f.joint == joint)
    }
    fn why_for(&self, group: &str) -> String {
        self.sequence
            .iter()
            .find(|s| s.group == group)
            .map(|s| s.why.clone())
            .unwrap_or_default()
    }
}

/// A hole in the wall: a door or a window, positioned from the wall's start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opening {
    /// Distance from the wall start to the opening's near edge, metres.
    pub x_m: f32,
    pub width_m: f32,
    pub height_m: f32,
    /// Height from the floor to the bottom of the opening. 0.0 means a door.
    pub sill_m: f32,
}

impl Opening {
    pub fn is_door(&self) -> bool {
        self.sill_m <= f32::EPSILON
    }
}

/// What the player drew.
#[derive(Debug, Clone)]
pub struct WallSpec {
    pub length_m: f32,
    pub height_m: f32,
    pub openings: Vec<Opening>,
}

/// A fastener applied at a step.
#[derive(Debug, Clone, PartialEq)]
pub struct Fastening {
    pub kind: String,
    pub count: u32,
    pub pattern: String,
}

/// One thing being put in one place. The atom of the whole system.
#[derive(Debug, Clone)]
pub struct Step {
    pub order: u32,
    pub group: String,
    pub role: String,
    pub label: String,
    /// Position of the member's start, in wall-local metres: x along the wall,
    /// y up from the floor, z through the wall thickness.
    pub at: (f32, f32, f32),
    /// Length along the member's own axis, metres. With `section` this gives the
    /// cut length, which is what a cut list needs.
    pub length_m: f32,
    pub section: Section,
    pub fastening: Option<Fastening>,
    /// Why this step happens here, in this order. The teaching payload.
    pub why: String,
}

/// One line of a cut list: how many of a thing, and how long.
#[derive(Debug, Clone, PartialEq)]
pub struct BomLine {
    pub label: String,
    pub count: u32,
    pub each_length_m: f32,
    pub total_length_m: f32,
}

/// Expand a drawn wall into the ordered steps that build it.
///
/// Order follows the recipe's `sequence` groups, because that order is real:
/// a stud wall is built flat and tilted up, so plates come before studs and the
/// sheathing can go on before the wall ever stands.
pub fn generate(spec: &WallSpec, a: &Assembly) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut order = 0u32;
    let mut push = |steps: &mut Vec<Step>,
                    group: &str,
                    role: &str,
                    label: &str,
                    at: (f32, f32, f32),
                    length_m: f32,
                    section: Section,
                    fastening: Option<Fastening>,
                    why: String| {
        steps.push(Step {
            order,
            group: group.to_string(),
            role: role.to_string(),
            label: label.to_string(),
            at,
            length_m,
            section,
            fastening,
            why,
        });
        order += 1;
    };

    let stud_to_plate = a.fastener("stud_to_plate").map(|f| Fastening {
        kind: f.kind.clone(),
        count: f.count,
        pattern: f.pattern.clone(),
    });

    // ── plates ──
    // Bottom plate and the FIRST top plate. The second top plate is a separate
    // group at the end because it laps the neighbouring wall's plate, and that
    // lap is what ties a room's walls into a continuous ring.
    let why_plates = a.why_for("plates");
    if let Some(m) = a.member("bottom_plate") {
        push(&mut steps, "plates", &m.role, &m.label, (0.0, 0.0, 0.0), spec.length_m, m.section.0, None, why_plates.clone());
    }
    if let Some(m) = a.member("top_plate") {
        let y = spec.height_m - m.section.0.thickness_m;
        push(&mut steps, "plates", &m.role, &m.label, (0.0, y, 0.0), spec.length_m, m.section.0, None, why_plates.clone());
    }

    // ── openings ──
    // King studs full height, jack studs cut to carry the header, header across
    // the hole. The load from above lands on the jacks; the window carries
    // nothing, which is the point people miss.
    let why_open = a.why_for("openings");
    let plate_t = a.member("bottom_plate").map(|m| m.section.0.thickness_m).unwrap_or(0.038);
    for op in &spec.openings {
        let head_y = op.sill_m + op.height_m;
        if let Some(k) = a.member("king_stud") {
            for x in [op.x_m - k.section.0.thickness_m, op.x_m + op.width_m] {
                push(&mut steps, "openings", &k.role, &k.label, (x, plate_t, 0.0),
                     spec.height_m - 2.0 * plate_t, k.section.0, stud_to_plate.clone(), why_open.clone());
            }
        }
        if let Some(j) = a.member("jack_stud") {
            for x in [op.x_m, op.x_m + op.width_m - j.section.0.thickness_m] {
                push(&mut steps, "openings", &j.role, &j.label, (x, plate_t, 0.0),
                     head_y - plate_t, j.section.0, stud_to_plate.clone(), why_open.clone());
            }
        }
        if let Some(h) = a.member("header") {
            let f = a.fastener("header_to_jack").map(|f| Fastening {
                kind: f.kind.clone(), count: f.count, pattern: f.pattern.clone(),
            });
            push(&mut steps, "openings", &h.role, &h.label, (op.x_m, head_y, 0.0),
                 op.width_m, h.section.0, f, why_open.clone());
        }
        if !op.is_door() {
            if let Some(s) = a.member("sill") {
                push(&mut steps, "openings", &s.role, &s.label, (op.x_m, op.sill_m, 0.0),
                     op.width_m, s.section.0, stud_to_plate.clone(), why_open.clone());
            }
        }
    }

    // ── studs ──
    // Regular studs on the layout, skipping anywhere an opening already occupies.
    let why_studs = a.why_for("studs");
    if let Some(m) = a.member("stud") {
        let clear = spec.height_m - 2.0 * plate_t;
        let t = m.section.0.thickness_m;
        let end_x = spec.length_m - t;

        // On-layout positions, then a stud flush with the far end.
        //
        // The end stud is not optional and it is not "on layout". Real framing
        // runs the 16 inch layout from one end and ALSO puts a stud at each end
        // of the wall, however awkwardly the last spacing lands, because the
        // corner needs something to nail to and the last sheathing panel needs
        // an edge to land on. Leaving it out was this generator's first bug: a
        // 16 ft wall came out with 12 studs instead of 13.
        let mut xs: Vec<f32> = Vec::new();
        let mut x = 0.0f32;
        while x <= end_x + 1e-4 {
            xs.push(x);
            x += a.spacing_m;
        }
        if xs.last().map_or(true, |last| (end_x - last).abs() > 1e-3) {
            xs.push(end_x);
        }

        for x in xs {
            let blocked = spec
                .openings
                .iter()
                .any(|o| x + t > o.x_m - 1e-4 && x < o.x_m + o.width_m - 1e-4);
            if !blocked {
                push(&mut steps, "studs", &m.role, &m.label, (x, plate_t, 0.0),
                     clear, m.section.0, stud_to_plate.clone(), why_studs.clone());
            }
        }
    }

    // ── sheathing ──
    // The part that stops the wall folding over sideways.
    let why_sheath = a.why_for("sheathing");
    let sh = &a.sheathing;
    let cols = panels_needed(spec.length_m, sh.panel_w_m);
    let rows = panels_needed(spec.height_m, sh.panel_h_m);
    let sheath_f = a.fastener("sheathing_to_stud").map(|f| Fastening {
        kind: f.kind.clone(),
        // Perimeter at 150mm plus field at 300mm, computed rather than guessed,
        // because the count depends on the panel and the stud spacing.
        count: sheathing_nails(sh.panel_w_m, sh.panel_h_m, a.spacing_m),
        pattern: f.pattern.clone(),
    });
    for r in 0..rows {
        for c in 0..cols {
            push(&mut steps, "sheathing", "sheathing", &sh.label,
                 (c as f32 * sh.panel_w_m, r as f32 * sh.panel_h_m, 0.0),
                 sh.panel_w_m, Section { thickness_m: sh.thickness_m, depth_m: sh.panel_h_m },
                 sheath_f.clone(), why_sheath.clone());
        }
    }

    // ── cap plate ──
    let why_cap = a.why_for("cap_plate");
    if let Some(m) = a.member("top_plate") {
        let f = a.fastener("plate_lap").map(|f| Fastening {
            kind: f.kind.clone(), count: f.count, pattern: f.pattern.clone(),
        });
        push(&mut steps, "cap_plate", "cap_plate", "2x6 cap plate",
             (0.0, spec.height_m - 2.0 * m.section.0.thickness_m, 0.0),
             spec.length_m, m.section.0, f, why_cap);
    }

    steps
}

/// How many panels cover a span, with a tolerance so an exact fit does not buy
/// an extra one.
///
/// This needs the epsilon and it is not fussiness. A 16 ft wall is exactly four
/// 4 ft panels and an 8 ft wall is exactly one 8 ft panel, but in metres those
/// divisions come out at 4.0008 and 1.0008, so a bare ceil() ordered 5 columns
/// and 2 rows: ten sheets for a wall that needs four. On a list somebody carries
/// to a lumber yard, that is a two-and-a-half times over-order and it is exactly
/// the kind of error that makes the whole cut list untrustworthy.
fn panels_needed(span_m: f32, panel_m: f32) -> u32 {
    if panel_m <= 0.0 {
        return 1;
    }
    // A millimetre of slack: real panels have a deliberate gap at the joints
    // anyway, so this is physically honest as well as numerically necessary.
    ((span_m / panel_m) - 1e-3).ceil().max(1.0) as u32
}

/// Nails in one sheathing panel: perimeter at 150 mm, field at the stud spacing
/// with 300 mm along each intermediate stud.
fn sheathing_nails(w: f32, h: f32, spacing_m: f32) -> u32 {
    let perimeter = 2.0 * (w + h);
    let edge = (perimeter / 0.150).ceil();
    let interior_studs = (w / spacing_m).floor().max(0.0) - 1.0;
    let field = interior_studs.max(0.0) * (h / 0.300).ceil();
    (edge + field) as u32
}

/// Roll the steps up into a cut list. This is the bridge from a game object to
/// a list somebody can take to a lumber yard.
pub fn bill_of_materials(steps: &[Step]) -> Vec<BomLine> {
    let mut out: Vec<BomLine> = Vec::new();
    for s in steps {
        // Group by label AND length, because "eight 2x6 at 2.4 m" and "two 2x6
        // at 1.1 m" are different lines on a real cut list.
        let key = (s.label.clone(), (s.length_m * 1000.0).round() as i64);
        if let Some(line) = out
            .iter_mut()
            .find(|l| l.label == key.0 && (l.each_length_m * 1000.0).round() as i64 == key.1)
        {
            line.count += 1;
            line.total_length_m += s.length_m;
        } else {
            out.push(BomLine {
                label: s.label.clone(),
                count: 1,
                each_length_m: s.length_m,
                total_length_m: s.length_m,
            });
        }
    }
    out
}

/// Load a recipe from `data/assemblies/<id>.ron`.
pub fn load(data_dir: &std::path::Path, id: &str) -> Option<Assembly> {
    let path = data_dir.join("assemblies").join(format!("{id}.ron"));
    let text = std::fs::read_to_string(&path).ok()?;
    match ron::from_str::<Assembly>(&text) {
        Ok(a) => Some(a),
        Err(e) => {
            log::warn!("assembly {} failed to parse: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe() -> Assembly {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        load(&dir, "stud_wall_2x6").expect("shipped stud_wall_2x6.ron parses")
    }

    fn plain_wall(len: f32) -> WallSpec {
        WallSpec { length_m: len, height_m: 2.44, openings: vec![] }
    }

    #[test]
    fn shipped_recipe_parses_and_has_its_parts() {
        let a = recipe();
        assert_eq!(a.id, "stud_wall_2x6");
        assert!((a.spacing_m - 0.4064).abs() < 1e-6, "16 inch on centre in metres");
        for role in ["bottom_plate", "top_plate", "stud", "king_stud", "jack_stud", "header"] {
            assert!(a.member(role).is_some(), "recipe is missing {role}");
        }
        // A 2x6 is not 2 inches by 6 inches, and the file must say the real size.
        let stud = a.member("stud").unwrap().section.0;
        assert!((stud.thickness_m - 0.038).abs() < 1e-6);
        assert!((stud.depth_m - 0.140).abs() < 1e-6);
    }

    #[test]
    fn a_plain_wall_gets_plates_studs_and_sheathing_in_build_order() {
        let a = recipe();
        let steps = generate(&plain_wall(4.877), &a); // 16 ft
        assert!(!steps.is_empty());

        // Order is monotonic, because animated mode replays it directly.
        for (i, s) in steps.iter().enumerate() {
            assert_eq!(s.order as usize, i, "steps must be strictly ordered");
        }

        // Plates before studs before sheathing. This is the real build sequence
        // (flat on the deck, then tilt up), not an arbitrary sort.
        let first = |g: &str| steps.iter().position(|s| s.group == g).unwrap();
        assert!(first("plates") < first("studs"), "plates are laid out before studs");
        assert!(first("studs") < first("sheathing"), "sheathe after the frame exists");

        // 16 ft at 16 in centres: studs at 0, 0.4064, ... up to the end.
        let studs = steps.iter().filter(|s| s.role == "stud").count();
        assert_eq!(studs, 13, "16 ft at 16 in o.c. should lay out 13 studs");

        // Every stud is fastened. A stud resting on a plate is not a wall.
        assert!(
            steps.iter().filter(|s| s.role == "stud").all(|s| s.fastening.is_some()),
            "every stud needs its nailing"
        );
        // And every step explains itself, since the sequence IS the lesson.
        assert!(steps.iter().all(|s| !s.why.is_empty()), "every step needs a why");
    }

    #[test]
    fn an_opening_gets_a_header_carried_by_jack_studs() {
        let a = recipe();
        let spec = WallSpec {
            length_m: 4.877,
            height_m: 2.44,
            openings: vec![Opening { x_m: 1.5, width_m: 0.914, height_m: 2.03, sill_m: 0.0 }],
        };
        let steps = generate(&spec, &a);

        assert_eq!(steps.iter().filter(|s| s.role == "header").count(), 1);
        assert_eq!(steps.iter().filter(|s| s.role == "king_stud").count(), 2);
        assert_eq!(steps.iter().filter(|s| s.role == "jack_stud").count(), 2);
        // A door has no sill; a window would.
        assert_eq!(steps.iter().filter(|s| s.role == "sill").count(), 0);

        // The header spans the opening and sits at its head height.
        let h = steps.iter().find(|s| s.role == "header").unwrap();
        assert!((h.length_m - 0.914).abs() < 1e-3, "header spans the opening");
        assert!((h.at.1 - 2.03).abs() < 1e-3, "header sits at the head height");
        // It is fastened to the jacks, because that joint is the load path.
        assert!(h.fastening.is_some(), "the header to jack joint carries the load");

        // No regular stud is left standing inside the hole.
        let in_hole = steps.iter().any(|s| {
            s.role == "stud" && s.at.0 + s.section.thickness_m > 1.5 && s.at.0 < 1.5 + 0.914
        });
        assert!(!in_hole, "a regular stud must not run through the opening");
    }

    #[test]
    fn the_steps_roll_up_into_a_real_cut_list() {
        let a = recipe();
        let steps = generate(&plain_wall(4.877), &a);
        let bom = bill_of_materials(&steps);
        assert!(!bom.is_empty());

        // Same part at the same length collapses to one line with a count,
        // which is what a lumber yard order looks like.
        let studs = bom.iter().find(|l| l.label.contains("stud")).expect("studs on the list");
        assert_eq!(studs.count, 13);
        assert!((studs.total_length_m - studs.each_length_m * 13.0).abs() < 1e-3);

        // Nothing is counted twice.
        let counted: u32 = bom.iter().map(|l| l.count).sum();
        assert_eq!(counted as usize, steps.len(), "every step appears exactly once");
    }

    /// Not a test, a viewer. Prints the build sequence and the cut list for a
    /// 16 ft wall with a door, so a human can sanity check the thing a player
    /// would watch and the list they could take to a lumber yard.
    ///
    ///   cargo test --features native --lib show_the_wall -- --ignored --nocapture
    #[test]
    #[ignore = "output viewer, not an assertion"]
    fn show_the_wall() {
        let a = recipe();
        let spec = WallSpec {
            length_m: 4.877,
            height_m: 2.44,
            openings: vec![Opening { x_m: 1.5, width_m: 0.914, height_m: 2.03, sill_m: 0.0 }],
        };
        let steps = generate(&spec, &a);

        println!("\n=== BUILD SEQUENCE: {} ===", a.label);
        let mut group = String::new();
        for s in &steps {
            if s.group != group {
                group = s.group.clone();
                println!("\n-- {} --\n   why: {}", group.to_uppercase(), s.why);
            }
            let f = s
                .fastening
                .as_ref()
                .map(|f| format!("  [{} x{} {}]", f.kind, f.count, f.pattern))
                .unwrap_or_default();
            println!(
                "  {:>3}. {:<22} at x={:>5.3} y={:>5.3}  len {:>5.3} m{}",
                s.order, s.label, s.at.0, s.at.1, s.length_m, f
            );
        }

        println!("\n=== CUT LIST ===");
        for l in bill_of_materials(&steps) {
            println!(
                "  {:>3} x  {:<22} @ {:>5.3} m   (total {:>6.2} m)",
                l.count, l.label, l.each_length_m, l.total_length_m
            );
        }
        println!();
    }

    #[test]
    fn an_exact_fit_does_not_order_a_spare_sheet() {
        // 16 ft is exactly four 4 ft panels; 8 ft is exactly one 8 ft panel.
        // In metres those divide to 4.0008 and 1.0008, so a bare ceil() ordered
        // ten sheets for a wall that needs four.
        assert_eq!(panels_needed(4.877, 1.219), 4, "16 ft is four 4 ft panels, not five");
        assert_eq!(panels_needed(2.44, 2.438), 1, "8 ft is one 8 ft panel, not two");
        // And a genuine overhang still buys the extra sheet.
        assert_eq!(panels_needed(5.0, 1.219), 5);
        assert_eq!(panels_needed(1.0, 1.219), 1, "a short wall still needs one");

        let a = recipe();
        let steps = generate(&plain_wall(4.877), &a);
        let panels = steps.iter().filter(|s| s.role == "sheathing").count();
        assert_eq!(panels, 4, "a 16 by 8 ft wall is four sheets");
    }

    #[test]
    fn sheathing_is_nailed_tighter_at_the_edges_than_in_the_field() {
        // The edge-versus-field distinction is the whole reason sheathing holds
        // a wall square, so the count must actually reflect it.
        let dense = sheathing_nails(1.219, 2.438, 0.4064);
        assert!(dense > 40, "a 4x8 panel takes many nails, got {dense}");
        // Wider stud spacing means fewer intermediate studs, so fewer field nails.
        assert!(sheathing_nails(1.219, 2.438, 0.6096) < dense);
    }
}
