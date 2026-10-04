//! Pipe MARKING (2026-10-04): the coloured bands that say what flows inside a pipe, the way
//! real piping is marked, generated from what the line actually carries.
//!
//! The research is `docs/reference/findings/2026-10-04-pipe-marking-standards.md`. Its two
//! findings that shape this module:
//!
//! - Every published scheme marks the CONTENT, never the wall material (F28), and every scheme
//!   whose text on it was read lets colour sit in bands at intervals with the real pipe between
//!   them (F5, F8, F17, F29). So the pipe body draws its material (`ship::pipe_materials`) and
//!   the content colour lives only in the markers built here.
//! - Aboard a ship the scheme is ISO 14726 (F9 to F13): a main colour per group of media and, for
//!   a specific medium, an additional colour between two bands of the main colour. Other
//!   schemes (ISO 20560-1, ASME A13.1, BS 1710, DIN 2403, MIL-STD-1247D) are carried in the same
//!   registry for the Real side, each row citing its finding.
//!
//! HONEST BY CONSTRUCTION: a marker's colours come from the connection's own kind (what the
//! simulation routes through it), never from anything a player types, so nobody can label a
//! fuel line "potable water" (findings, "What multiplayer could enforce").
//!
//! TWO MODES (the house rule for deep systems): Simplified draws one band of the main colour per
//! marker; Full draws the scheme's whole marker, main, additional, main where the scheme has an
//! additional colour (or its own layout, such as MIL-STD-1247D's two-stripe tape). Both use the
//! same colours, so nothing learned in one is wrong in the other.
//!
//! PLACEMENT follows the rule every scheme whose placement text was read shares (F4, F17, F21,
//! F22): at each end where the pipe meets its machine, at changes of direction, and at intervals
//! along straight runs (GSFC-STD-8006's 20 ft, about 6 m, by default) so at least one marker is
//! in view anywhere along a run. Valves, branches and wall penetrations are the next increment.
//!
//! The registry is `data/piping/marking_schemes.ron`, read from disk first and from the copy
//! built into the exe when that is missing or does not parse. Pure serde + glam (no renderer):
//! it compiles under `relay` too. The meshes are built in `engine::pipe_markers`.

use glam::Vec3;
use serde::Deserialize;

/// Which marker a pipe carries: one band of the main colour, or the scheme's whole marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkingMode {
    Simplified,
    Full,
}

impl MarkingMode {
    /// From the Settings switch (`AppConfig::pipe_marking_full`).
    pub fn from_full(full: bool) -> Self {
        if full {
            MarkingMode::Full
        } else {
            MarkingMode::Simplified
        }
    }
}

/// Where markers go along a run, and how a band is drawn (`placement` in the data file).
#[derive(Debug, Clone, Deserialize)]
pub struct PlacementRules {
    /// The largest gap between two markers on a run, metres (GSFC-STD-8006's 20 ft, F4).
    pub interval_m: f32,
    /// How far in from where the pipe meets its machine the end marker starts.
    pub end_clearance_m: f32,
    /// How far past a bend a marker starts, so it never lands on the elbow.
    pub bend_clearance_m: f32,
    /// Length of one colour band along the pipe.
    pub band_m: f32,
    /// Band radius = pipe radius x this + `band_radius_add_m`.
    pub band_radius_scale: f32,
    pub band_radius_add_m: f32,
    pub band_metallic: f32,
    pub band_roughness: f32,
}

impl PlacementRules {
    /// The radius of a band sleeved round a pipe of `pipe_radius`.
    pub fn band_radius(&self, pipe_radius: f32) -> f32 {
        pipe_radius * self.band_radius_scale + self.band_radius_add_m
    }
}

/// One named colour of a scheme, with our sRGB rendition of it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SchemeColour {
    pub id: String,
    pub name: String,
    pub srgb: (u8, u8, u8),
    #[serde(default)]
    pub note: String,
}

impl SchemeColour {
    /// sRGB 0..1, alpha 1 (for egui and the build editor's legend).
    pub fn srgb01(&self) -> [f32; 4] {
        let (r, g, b) = self.srgb;
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
    }

    /// Linear light, alpha 1 (for a renderer material's base colour).
    pub fn linear_rgba(&self) -> [f32; 4] {
        use crate::ship::pipe_materials::srgb_to_linear;
        let (r, g, b) = self.srgb;
        [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), 1.0]
    }
}

/// How one scheme marks one content.
#[derive(Debug, Clone, Deserialize)]
pub struct ContentMarking {
    /// The game's connection kind ("water", "power", ...).
    pub content: String,
    /// The content as the scheme words it (restated).
    pub name: String,
    /// The main colour: Simplified mode's one band, and the outer bands of the full triple.
    pub main: String,
    /// The additional colour (the middle band of the full triple), where the scheme has one.
    #[serde(default)]
    pub additional: Option<String>,
    /// An explicit full-mode band sequence for any other layout; empty = the triple rule.
    #[serde(default)]
    pub bands: Vec<String>,
    /// The findings this mapping comes from.
    pub cite: String,
    /// True when any part of the mapping is ours; `note` says which.
    #[serde(default)]
    pub game_choice: bool,
    #[serde(default)]
    pub note: String,
}

impl ContentMarking {
    /// The colour ids of one marker, in order along the pipe.
    pub fn band_ids(&self, mode: MarkingMode) -> Vec<&str> {
        match mode {
            MarkingMode::Simplified => vec![self.main.as_str()],
            MarkingMode::Full => {
                if !self.bands.is_empty() {
                    self.bands.iter().map(String::as_str).collect()
                } else if let Some(a) = &self.additional {
                    vec![self.main.as_str(), a.as_str(), self.main.as_str()]
                } else {
                    vec![self.main.as_str()]
                }
            }
        }
    }
}

/// One marking scheme (ISO 14726, ASME A13.1, ...).
#[derive(Debug, Clone, Deserialize)]
pub struct MarkingScheme {
    pub id: String,
    pub name: String,
    pub title: String,
    pub edition: String,
    pub cite: String,
    #[serde(default)]
    pub note: String,
    pub colours: Vec<SchemeColour>,
    pub contents: Vec<ContentMarking>,
}

impl MarkingScheme {
    pub fn content(&self, content: &str) -> Option<&ContentMarking> {
        self.contents.iter().find(|c| c.content == content)
    }

    pub fn colour(&self, id: &str) -> Option<&SchemeColour> {
        self.colours.iter().find(|c| c.id == id)
    }

    /// The colours of one marker for `content` in `mode`, in order along the pipe; None when
    /// the scheme does not mark that content (or names a colour it does not define, which
    /// `problems` reports).
    pub fn marker_colours(&self, content: &str, mode: MarkingMode) -> Option<Vec<&SchemeColour>> {
        let row = self.content(content)?;
        row.band_ids(mode).into_iter().map(|id| self.colour(id)).collect()
    }
}

/// The whole registry: data/piping/marking_schemes.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct MarkingSchemes {
    pub default_scheme: String,
    pub placement: PlacementRules,
    pub schemes: Vec<MarkingScheme>,
}

impl MarkingSchemes {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn scheme(&self, id: &str) -> Option<&MarkingScheme> {
        self.schemes.iter().find(|s| s.id == id)
    }

    /// The ship's scheme (`default_scheme`).
    pub fn default_scheme(&self) -> Option<&MarkingScheme> {
        self.scheme(&self.default_scheme)
    }

    /// The default scheme's main colour for `content`, sRGB 0..1: the build editor's legend
    /// colour, so the legend, the port gizmos and the pipes' bands all say the same thing.
    pub fn main_colour_srgb01(&self, content: &str) -> Option<[f32; 4]> {
        let s = self.default_scheme()?;
        s.colour(&s.content(content)?.main).map(SchemeColour::srgb01)
    }

    /// Everything wrong with the registry, as sentences (empty = sound).
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.default_scheme().is_none() {
            out.push(format!("default_scheme `{}` is not a scheme in the file", self.default_scheme));
        }
        let p = &self.placement;
        for (name, v) in [
            ("interval_m", p.interval_m),
            ("band_m", p.band_m),
            ("band_radius_scale", p.band_radius_scale),
        ] {
            if !(v > 0.0) {
                out.push(format!("placement.{name} must be above zero, is {v}"));
            }
        }
        if p.end_clearance_m < 0.0 || p.bend_clearance_m < 0.0 || p.band_radius_add_m < 0.0 {
            out.push("placement clearances and band_radius_add_m must not be negative".to_string());
        }
        if !(0.0..=1.0).contains(&p.band_metallic) || !(0.0..=1.0).contains(&p.band_roughness) {
            out.push("placement band_metallic and band_roughness must be 0..1".to_string());
        }
        let mut scheme_ids = std::collections::HashSet::new();
        for s in &self.schemes {
            if !scheme_ids.insert(s.id.as_str()) {
                out.push(format!("scheme `{}` is defined twice", s.id));
            }
            if s.cite.trim().is_empty() || s.edition.trim().is_empty() {
                out.push(format!("scheme `{}` needs its edition and the findings it cites", s.id));
            }
            let mut colour_ids = std::collections::HashSet::new();
            for c in &s.colours {
                if !colour_ids.insert(c.id.as_str()) {
                    out.push(format!("{}: colour `{}` is defined twice", s.id, c.id));
                }
            }
            let mut contents = std::collections::HashSet::new();
            for row in &s.contents {
                if !contents.insert(row.content.as_str()) {
                    out.push(format!("{}: content `{}` is mapped twice", s.id, row.content));
                }
                if row.cite.trim().is_empty() {
                    out.push(format!("{}: `{}` cites no finding", s.id, row.content));
                }
                if row.game_choice && row.note.trim().is_empty() {
                    out.push(format!("{}: `{}` is a game choice and must say which part is ours", s.id, row.content));
                }
                for mode in [MarkingMode::Simplified, MarkingMode::Full] {
                    for id in row.band_ids(mode) {
                        if s.colour(id).is_none() {
                            out.push(format!("{}: `{}` names colour `{id}`, which the scheme does not define", s.id, row.content));
                        }
                    }
                }
                if !row.bands.is_empty() && row.bands.iter().all(|b| b != &row.main) {
                    out.push(format!("{}: `{}`'s full bands must include its main colour", s.id, row.content));
                }
            }
        }
        out
    }
}

/// The shipped file, the fallback when the data folder's copy is missing or does not parse.
const SHIPPED_MARKING_SCHEMES: &str = include_str!("../../data/piping/marking_schemes.ron");

/// The registry, loaded once: the data folder's copy first, the shipped copy if that is missing
/// or does not parse (and the log says so, BUG-133).
pub fn marking() -> &'static MarkingSchemes {
    static REG: std::sync::OnceLock<MarkingSchemes> = std::sync::OnceLock::new();
    REG.get_or_init(load_marking)
}

fn load_marking() -> MarkingSchemes {
    let path = crate::data_dir().join("piping").join("marking_schemes.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match MarkingSchemes::parse(&text) {
            Ok(m) => return m,
            Err(e) => crate::embedded_data::note_builtin_copy(
                "piping/marking_schemes.ron",
                format_args!("{} does not parse ({e})", path.display()),
            ),
        },
        Err(e) => crate::embedded_data::note_builtin_copy(
            "piping/marking_schemes.ron",
            format_args!("{} could not be read ({e})", path.display()),
        ),
    }
    MarkingSchemes::parse(SHIPPED_MARKING_SCHEMES).unwrap_or_else(|e| {
        log::error!("the shipped data/piping/marking_schemes.ron does not parse: {e}");
        MarkingSchemes {
            default_scheme: String::new(),
            placement: PlacementRules {
                interval_m: 6.1,
                end_clearance_m: 0.3,
                bend_clearance_m: 0.15,
                band_m: 0.06,
                band_radius_scale: 1.15,
                band_radius_add_m: 0.002,
                band_metallic: 0.0,
                band_roughness: 0.5,
            },
            schemes: Vec::new(),
        }
    })
}

// ── Placement ───────────────────────────────────────────────────────────────

/// Why a marker is where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerReason {
    /// Beside a machine (or a junction) at one end of the run.
    End,
    /// Just past a change of direction.
    Bend,
    /// Along a straight run, so no gap exceeds the interval.
    Interval,
}

/// One marker on a run: the centre of its band group, the run's direction there, and the arc
/// length along the run from its first point to that centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkerSite {
    pub s: f32,
    pub centre: Vec3,
    pub dir: Vec3,
    /// Index of the straight leg (between two kept polyline points) the marker sits on.
    pub leg: usize,
    pub reason: MarkerReason,
}

/// A straight leg of a run.
#[derive(Debug, Clone, Copy)]
struct Leg {
    a: Vec3,
    dir: Vec3,
    len: f32,
    /// Arc length at `a`.
    s0: f32,
}

fn legs_of(points: &[Vec3]) -> Vec<Leg> {
    let mut legs = Vec::new();
    let mut s0 = 0.0;
    for w in points.windows(2) {
        let d = w[1] - w[0];
        let len = d.length();
        if len < 1e-4 {
            continue;
        }
        legs.push(Leg { a: w[0], dir: d / len, len, s0 });
        s0 += len;
    }
    legs
}

/// Where the markers go along a routed run (`points`, a polyline from one machine to the
/// other), for a band group `group_len` metres long. At each end, just past each change of
/// direction, and then at intervals so that no two consecutive markers are more than
/// `rules.interval_m` apart. Every marker's whole band group lies on one straight leg, clear of
/// the bends. A run too short for any of that still gets one marker, mid-way along its longest
/// leg, if that leg can hold the group; a run shorter than one marker gets none.
pub fn place_markers(points: &[Vec3], rules: &PlacementRules, group_len: f32) -> Vec<MarkerSite> {
    let legs = legs_of(points);
    if legs.is_empty() {
        return Vec::new();
    }
    let half = group_len * 0.5;
    let site = |i: usize, offset: f32, reason: MarkerReason| -> MarkerSite {
        let l = legs[i];
        MarkerSite { s: l.s0 + offset, centre: l.a + l.dir * offset, dir: l.dir, leg: i, reason }
    };
    // The offset of a group's centre on leg `i` that keeps it `clear` metres from the leg's
    // start (or its end), if the leg is long enough to hold it there.
    let fit_from_start = |i: usize, clear: f32| (legs[i].len >= clear + group_len).then(|| clear + half);
    let fit_from_end = |i: usize, clear: f32| (legs[i].len >= clear + group_len).then(|| legs[i].len - clear - half);

    // Candidates in priority order: the two ends, then the bends.
    let mut candidates: Vec<MarkerSite> = Vec::new();
    let last = legs.len() - 1;
    // The start: on the first leg that holds a group (past the machine, or past a bend).
    if let Some((i, off)) = (0..legs.len()).find_map(|i| {
        let clear = if i == 0 { rules.end_clearance_m } else { rules.bend_clearance_m };
        fit_from_start(i, clear).map(|o| (i, o))
    }) {
        candidates.push(site(i, off, MarkerReason::End));
    }
    // The finish: on the last leg that holds a group, counted back from the machine.
    if let Some((i, off)) = (0..legs.len()).rev().find_map(|i| {
        let clear = if i == last { rules.end_clearance_m } else { rules.bend_clearance_m };
        fit_from_end(i, clear).map(|o| (i, o))
    }) {
        candidates.push(site(i, off, MarkerReason::End));
    }
    // Each change of direction: just after it on the outgoing leg, else just before it on the
    // incoming one. A joint where the direction does not change is no bend.
    for i in 0..last {
        if legs[i].dir.dot(legs[i + 1].dir) > 0.999 {
            continue;
        }
        if let Some(off) = fit_from_start(i + 1, rules.bend_clearance_m) {
            candidates.push(site(i + 1, off, MarkerReason::Bend));
        } else if let Some(off) = fit_from_end(i, rules.bend_clearance_m) {
            candidates.push(site(i, off, MarkerReason::Bend));
        }
    }
    // Nothing fits anywhere (a stub of a run): one marker in the middle of the longest leg, if
    // that leg holds a whole group. A run shorter than one marker carries none, as a real short
    // nipple between two fittings does; the runs either side carry the markers. (A group centred
    // on a leg shorter than itself stuck out past both ends, 2026-10-04 review.)
    if candidates.is_empty() {
        let (i, l) = legs.iter().enumerate().fold((0, legs[0]), |best, (i, l)| if l.len > best.1.len { (i, *l) } else { best });
        if l.len < group_len {
            return Vec::new();
        }
        return vec![site(i, l.len * 0.5, MarkerReason::End)];
    }
    // Keep the first of any two candidates whose groups would touch (the ends win).
    let min_sep = group_len + 0.05;
    let mut sites: Vec<MarkerSite> = Vec::new();
    for c in candidates {
        if sites.iter().all(|k| (k.s - c.s).abs() >= min_sep) {
            sites.push(c);
        }
    }
    sites.sort_by(|a, b| a.s.total_cmp(&b.s));
    // Fill every gap longer than the interval with evenly spaced markers.
    let mut fill: Vec<MarkerSite> = Vec::new();
    for w in sites.windows(2) {
        let (p, q) = (w[0].s, w[1].s);
        let gap = q - p;
        if gap <= rules.interval_m {
            continue;
        }
        let n = (gap / rules.interval_m).ceil() as usize;
        for k in 1..n {
            let s = p + gap * k as f32 / n as f32;
            let Some(i) = legs.iter().rposition(|l| l.s0 <= s) else { continue };
            let l = legs[i];
            let (lo, hi) = (rules.bend_clearance_m + half, l.len - rules.bend_clearance_m - half);
            if lo > hi {
                continue; // the leg cannot hold a group clear of its bends
            }
            fill.push(site(i, (s - l.s0).clamp(lo, hi), MarkerReason::Interval));
        }
    }
    sites.extend(fill);
    sites.sort_by(|a, b| a.s.total_cmp(&b.s));
    sites
}

/// One colour band of a marker: a sleeve `len` long round the pipe, starting at `start` and
/// running along `dir`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band<'a> {
    pub colour: &'a SchemeColour,
    pub start: Vec3,
    pub dir: Vec3,
    pub len: f32,
    pub radius: f32,
}

/// The bands of one marker at `site`, centred on it, one `rules.band_m` long each, contiguous,
/// in the order the colours are given.
pub fn marker_bands<'a>(
    site: &MarkerSite,
    colours: &[&'a SchemeColour],
    rules: &PlacementRules,
    pipe_radius: f32,
) -> Vec<Band<'a>> {
    let group_len = rules.band_m * colours.len() as f32;
    let first = site.centre - site.dir * (group_len * 0.5);
    let radius = rules.band_radius(pipe_radius);
    colours
        .iter()
        .enumerate()
        .map(|(j, c)| Band { colour: c, start: first + site.dir * (rules.band_m * j as f32), dir: site.dir, len: rules.band_m, radius })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> MarkingSchemes {
        MarkingSchemes::parse(SHIPPED_MARKING_SCHEMES).expect("the shipped marking_schemes.ron parses")
    }

    /// Every content the game routes through a pipe or cable: the utilities machines declare
    /// ports for, every connection and conduit-edge kind in the shipped machine layouts, and the
    /// kinds the build editor's legend has always coloured.
    fn routed_contents() -> std::collections::BTreeSet<String> {
        let mut out: std::collections::BTreeSet<String> =
            crate::utilities::Utility::ALL.iter().map(|u| u.id().to_string()).collect();
        for legend in ["water", "hot_water", "air", "gas", "fuel", "nutrient", "waste", "greywater", "power", "data"] {
            out.insert(legend.to_string());
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
        for file in ["home.ron", "home_solo.ron", "ship.ron"] {
            let home = crate::machines::MachineHome::load(&dir.join(file)).unwrap_or_else(|| panic!("data/machines/{file} loads"));
            out.extend(home.connections.iter().map(|c| c.kind.clone()));
            out.extend(home.conduit_edges.iter().map(|e| e.kind.clone()));
        }
        out
    }

    /// The registry loads, is internally sound, and the ship's scheme marks every content the
    /// code routes, so no pipe goes unmarked.
    ///
    /// Seen red with the `food` row taken out of ISO 14726 (the grain the fields send to the
    /// silo): "content `food` is routed but iso_14726 does not mark it".
    #[test]
    fn shipped_marking_registry_covers_every_routed_content() {
        let reg = shipped();
        assert_eq!(reg.problems(), Vec::<String>::new());
        let ship = reg.default_scheme().expect("the default scheme exists");
        assert_eq!(ship.id, "iso_14726", "the ship's scheme is ISO 14726 (the findings' recommendation)");
        let routed = routed_contents();
        assert!(routed.len() >= 10, "sanity: found only {routed:?}");
        for content in &routed {
            assert!(ship.content(content).is_some(), "content `{content}` is routed but {} does not mark it", ship.id);
            for mode in [MarkingMode::Simplified, MarkingMode::Full] {
                assert!(ship.marker_colours(content, mode).is_some(), "`{content}` resolves its colours in {mode:?}");
            }
        }
        // Every row says where it comes from, and a game choice says so plainly.
        for s in &reg.schemes {
            for row in &s.contents {
                assert!(row.cite.contains('F'), "{}: `{}` cites a finding number: {:?}", s.id, row.content, row.cite);
            }
        }
        // Electrical lines follow the findings' suggestion, MIL-STD-1247D's brown and orange
        // stripes, and are no longer yellow (flammable gas aboard, F11).
        let power = ship.content("power").unwrap();
        assert_eq!(power.band_ids(MarkingMode::Full), vec!["brown", "orange"]);
        assert!(power.band_ids(MarkingMode::Full).iter().all(|c| *c != "yellow"));
    }

    fn rules() -> PlacementRules {
        shipped().placement
    }

    /// Largest gap between consecutive markers along a run.
    fn max_gap(sites: &[MarkerSite]) -> f32 {
        sites.windows(2).map(|w| w[1].s - w[0].s).fold(0.0, f32::max)
    }

    /// Every marker's band group lies wholly on its own straight leg (never across a bend).
    fn assert_groups_on_their_legs(points: &[Vec3], sites: &[MarkerSite], group_len: f32) {
        let legs = legs_of(points);
        for m in sites {
            let l = legs[m.leg];
            let off = m.s - l.s0;
            assert!(
                off - group_len * 0.5 >= -1e-4 && off + group_len * 0.5 <= l.len + 1e-4,
                "marker at s={} overhangs leg {} (0..{})",
                m.s,
                m.leg,
                l.len
            );
            assert!((l.a + l.dir * off - m.centre).length() < 1e-4, "the centre lies on the run");
        }
    }

    /// Markers go where the schemes put them: one beside each machine, one just past every
    /// change of direction, and enough along each straight that no gap exceeds the interval
    /// (GSFC's 20 ft, about 6 m) on three hand-built runs.
    ///
    /// Seen red with `place_markers` returning no markers (the pipes before this change):
    /// "the up-over-down run gets an end marker at each machine: []".
    #[test]
    fn markers_sit_at_both_ends_every_bend_and_every_interval() {
        let r = rules();
        let group = 3.0 * r.band_m;

        // 1. The router's up-over-down shape: a riser, two service-height legs, a drop.
        let run = [
            Vec3::new(0.0, 0.8, 0.0),
            Vec3::new(0.0, 2.7, 0.0),
            Vec3::new(20.0, 2.7, 0.0),
            Vec3::new(20.0, 2.7, 10.0),
            Vec3::new(20.0, 0.8, 10.0),
        ];
        let total = 1.9 + 20.0 + 10.0 + 1.9;
        let m = place_markers(&run, &r, group);
        let ends: Vec<_> = m.iter().filter(|s| s.reason == MarkerReason::End).collect();
        assert_eq!(ends.len(), 2, "the up-over-down run gets an end marker at each machine: {m:?}");
        assert!(ends[0].leg == 0 && ends[0].s < 0.5, "the first sits on the riser by the machine: {:?}", ends[0]);
        assert!(ends[1].leg == 3 && ends[1].s > total - 0.5, "the last on the drop by the other: {:?}", ends[1]);
        let bends: Vec<_> = m.iter().filter(|s| s.reason == MarkerReason::Bend).collect();
        assert_eq!(bends.len(), 3, "one marker past each of the three bends: {m:?}");
        for (b, leg) in bends.iter().zip([1usize, 2, 3]) {
            // Past the bend on the outgoing leg, unless it holds the end marker already.
            assert!(b.leg == leg || b.leg == leg - 1, "bend marker on a leg beside its bend: {b:?}");
        }
        assert!(max_gap(&m) <= r.interval_m + 1e-3, "no gap over {} m: {}", r.interval_m, max_gap(&m));
        assert!(m.iter().any(|s| s.reason == MarkerReason::Interval), "the 20 m leg needs interval markers");
        assert_groups_on_their_legs(&run, &m, group);

        // 2. One straight 30 m run: the two ends and evenly spaced markers between them.
        let straight = [Vec3::ZERO, Vec3::new(30.0, 0.0, 0.0)];
        let m = place_markers(&straight, &r, group);
        assert_eq!(m.first().unwrap().reason, MarkerReason::End);
        assert_eq!(m.last().unwrap().reason, MarkerReason::End);
        assert!(m.iter().all(|s| s.reason != MarkerReason::Bend), "a straight run has no bends");
        assert!(max_gap(&m) <= r.interval_m + 1e-3, "no gap over the interval: {}", max_gap(&m));
        // 30 m less the two end clearances is about 29.2 m between the ends: five gaps.
        assert_eq!(m.len(), 6, "two ends and four between: {m:?}");
        assert_groups_on_their_legs(&straight, &m, group);

        // 3. A stub too short for an end clearance still carries one marker.
        let stub = [Vec3::ZERO, Vec3::new(0.0, 0.0, 0.4)];
        let m = place_markers(&stub, &r, group);
        assert_eq!(m.len(), 1, "a 0.4 m run still carries a marker: {m:?}");
        assert_groups_on_their_legs(&stub, &m, group);
    }

    /// A run shorter than one marker carries none, rather than a marker that sticks out past
    /// both ends into the fittings (2026-10-04 review): two conduit nodes 0.1 m apart at service
    /// height route as one 0.1 m leg, and Full mode's 0.18 m triple cannot fit on it. The runs
    /// either side carry the markers, as on a real short nipple between two fittings. A single
    /// Simplified band (6 cm) still fits.
    ///
    /// Seen red before the fix: "marker at s=0.05 overhangs leg 0 (0..0.1)".
    #[test]
    fn a_run_shorter_than_its_marker_carries_none() {
        let r = rules();
        let full = 3.0 * r.band_m;
        let tiny = [Vec3::new(0.0, 2.7, 0.0), Vec3::new(0.1, 2.7, 0.0)];
        let m = place_markers(&tiny, &r, full);
        assert_groups_on_their_legs(&tiny, &m, full);
        assert!(m.is_empty(), "a 0.1 m run cannot hold a {full} m marker: {m:?}");
        let one_band = place_markers(&tiny, &r, r.band_m);
        assert_eq!(one_band.len(), 1, "one 6 cm band fits on 0.1 m: {one_band:?}");
        assert_groups_on_their_legs(&tiny, &one_band, r.band_m);
    }

    /// Simplified mode draws one band of the main colour per marker; full mode draws the
    /// scheme's whole marker: main, additional, main for ISO 14726 potable water, the two
    /// stripes of an electrical line, and one band where the group colour alone is the code.
    ///
    /// Seen red with `band_ids` returning the main colour alone in both modes: "full mode marks
    /// potable water blue, green, blue: [\"blue\"]".
    #[test]
    fn simplified_mode_draws_one_band_and_full_mode_three() {
        let reg = shipped();
        let r = &reg.placement;
        let ship = reg.default_scheme().unwrap();
        let water = ship.content("water").unwrap();
        assert_eq!(water.band_ids(MarkingMode::Simplified), vec!["blue"], "simplified: the main colour alone");
        assert_eq!(
            water.band_ids(MarkingMode::Full),
            vec!["blue", "green", "blue"],
            "full mode marks potable water blue, green, blue: {:?}",
            water.band_ids(MarkingMode::Full)
        );
        assert_eq!(ship.content("power").unwrap().band_ids(MarkingMode::Full).len(), 2, "the electrical stripe pair");
        assert_eq!(ship.content("fuel").unwrap().band_ids(MarkingMode::Full), vec!["brown"], "a group colour alone");
        // Every content: simplified is exactly one band, its main colour.
        for row in &ship.contents {
            assert_eq!(row.band_ids(MarkingMode::Simplified), vec![row.main.as_str()], "{}", row.content);
        }

        // The geometry: one band, or three contiguous bands centred on the marker.
        let site = MarkerSite { s: 5.0, centre: Vec3::new(5.0, 2.7, 0.0), dir: Vec3::X, leg: 0, reason: MarkerReason::Interval };
        let one = marker_bands(&site, &ship.marker_colours("water", MarkingMode::Simplified).unwrap(), r, 0.012);
        assert_eq!(one.len(), 1);
        assert!((one[0].start.x - (5.0 - r.band_m * 0.5)).abs() < 1e-5, "a single band is centred on the marker");
        let three = marker_bands(&site, &ship.marker_colours("water", MarkingMode::Full).unwrap(), r, 0.012);
        assert_eq!(three.iter().map(|b| b.colour.id.as_str()).collect::<Vec<_>>(), vec!["blue", "green", "blue"]);
        assert!((three[0].start.x - (5.0 - 1.5 * r.band_m)).abs() < 1e-5, "the triple is centred on the marker");
        for w in three.windows(2) {
            assert!((w[0].start + w[0].dir * w[0].len - w[1].start).length() < 1e-5, "the bands touch end to end");
        }
        assert!(three.iter().all(|b| b.radius > 0.012), "a band sits proud of the pipe");
    }
}
