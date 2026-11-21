//! THE LONG ARC: a piece's whole shape, planned before its notes (docs/design/music.md section 7h).
//!
//! The operator, 2026-11-20: "couldn't we just say, here's how the song will look X minutes out?
//! Then isn't a 'new song' just a new seed? Would that help us have more complex songs?" A genre names
//! an ARC (`data/music/arcs/<id>.ron`, infinite-of-X): the stages a piece passes through over its whole
//! length (an intro, the theme stated, a development, a build, a planned climax, the theme's return, a
//! coda), each with its share of the length, its energy rising or falling across it, the roles that
//! play in it (instruments entering and leaving over the whole piece), whether the drums play, a key
//! move (home, away, or up toward a lifted key at the climax) and whether its sections play their own
//! tunes. [`plan`] lays a whole piece out from the piece's seed and the genre's arc: every phrase's
//! section letter, its bars, its first bar, its stage and its energy, so the composer's section and bar
//! writers only fill what the plan says, and any bar of the piece is found without playing what came
//! before it (crate::composer, "Seeking").
//!
//! A genre with no arc plays its form over and over (one stage, the whole form, its tunes the first time
//! through): the plan is still made, so every calm piece is seekable.
//!
//! Fixed-size and allocation-free: a plan is made on the audio thread when a piece (or its genre)
//! begins. Planning a 60-minute piece is a few hundred phrases of arithmetic (music.md section 14).

use crate::mood::RoleKind;
use crate::piece::{mix, pick, unit};

/// The most phrases one time round an arc holds (a 60-minute piece of 8-bar phrases at 174 bpm is
/// about 330); a longer plan stops at this many and the piece goes round again from there.
pub const MAX_PLAN: usize = 512;
/// The most stages an arc has.
pub const MAX_STAGES: usize = 16;
/// A letter that is no section of a form: a genre with neither forms nor a structure plays each phrase
/// as a section of its own (its chords from the phrase's own number).
pub const FREE: u8 = 255;
/// An endless piece goes round its arc again; each time round, its improvised sections number on from
/// this many times the round, so no round reharmonises a section the way another did.
pub const ROUND_CHORUS: u32 = 1 << 16;

/// What a stage is for (the Music page names it; the planner places the climax by it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum StageKind {
    Intro,
    Theme,
    Develop,
    Build,
    Climax,
    Return,
    Coda,
}

impl StageKind {
    /// Its words key (`music.arc.stage_<kind>`).
    pub fn words(self) -> &'static str {
        match self {
            StageKind::Intro => "music.arc.stage_intro",
            StageKind::Theme => "music.arc.stage_theme",
            StageKind::Develop => "music.arc.stage_develop",
            StageKind::Build => "music.arc.stage_build",
            StageKind::Climax => "music.arc.stage_climax",
            StageKind::Return => "music.arc.stage_return",
            StageKind::Coda => "music.arc.stage_coda",
        }
    }
}

/// Where a stage's key is: the piece's home key (a bridge may still move, as a form's does), the
/// second key the piece's bridge would take, or the key of the harmony nearest this many semitones
/// above home (a lift for a climax), never home itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Deserialize)]
pub enum KeyMove {
    #[default]
    Home,
    Away,
    Up(i32),
}

fn full() -> (f32, f32) {
    (1.0, 1.0)
}
fn yes() -> bool {
    true
}

/// One stage of an arc, as written in its file.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Stage {
    pub kind: StageKind,
    /// Its share of the piece's length (the shares are scaled to the length; the climax's start is
    /// placed where `climax_at` says and the stages either side scaled to meet it).
    pub share: f32,
    /// Its energy at its start and at its end (1 = the genre as levelled; over 1 louder and busier,
    /// under it softer and sparser): velocities and how often the riffs, the choir and the heartbeat
    /// play follow it.
    #[serde(default = "full")]
    pub energy: (f32, f32),
    /// The sections it plays in turn, as letters ("AABA"); None: the piece's own form.
    #[serde(default)]
    pub letters: Option<String>,
    /// For a genre with a structure: the structure's sections it plays in turn, by name (a name the
    /// genre's structure lacks is skipped; none left: the structure between its first and last).
    #[serde(default)]
    pub sections: Vec<String>,
    /// The roles that may play (empty: all the genre plays).
    #[serde(default)]
    pub roles: Vec<RoleKind>,
    /// Whether the genre's drums play.
    #[serde(default = "yes")]
    pub drums: bool,
    #[serde(default)]
    pub key: KeyMove,
    /// Its sections play their own tunes (a theme stated or returning); otherwise the band improvises
    /// over each section's chords, the second half reharmonised.
    #[serde(default)]
    pub tune: bool,
}

/// An arc file (`data/music/arcs/<id>.ron`).
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Arc {
    pub id: String,
    /// Where the climax stage begins, as a fraction of the piece (a piece's seed picks within it).
    pub climax_at: (f32, f32),
    pub stages: Vec<Stage>,
}

/// A stage ready to plan.
#[derive(Clone, Debug)]
pub struct RStage {
    pub kind: StageKind,
    pub share: f32,
    pub energy: (f32, f32),
    /// Letters (A = 0); None: the piece's form. For a structure: positions in it.
    pub letters: Option<Vec<u8>>,
    pub sections: Vec<u8>,
    /// One bit a role (`RoleKind::index`); 0xff: every role.
    pub roles: u8,
    pub drums: bool,
    pub key: KeyMove,
    pub tune: bool,
}

/// An arc ready to plan, for one genre (its section names already found in that genre's structure).
#[derive(Clone, Debug)]
pub struct RArc {
    pub climax_at: (f32, f32),
    pub stages: Vec<RStage>,
}

impl RArc {
    /// `arc` for a genre whose structure's sections are named `names` (empty: no structure). Errors
    /// name the arc and the field.
    pub fn resolve(arc: &Arc, names: &[String]) -> Result<RArc, String> {
        let owner = format!("arcs/{}.ron", arc.id);
        if arc.stages.is_empty() || arc.stages.len() > MAX_STAGES {
            return Err(format!("{owner}: an arc has 1 to {MAX_STAGES} stages"));
        }
        let (a, b) = arc.climax_at;
        if !(0.0..=1.0).contains(&a) || !(a..=1.0).contains(&b) {
            return Err(format!("{owner}: climax_at must be two fractions of the piece, the first not after the second"));
        }
        let mut stages = Vec::new();
        for (i, s) in arc.stages.iter().enumerate() {
            if !(s.share > 0.0) {
                return Err(format!("{owner}: stage {i}'s share must be over 0"));
            }
            let letters = match &s.letters {
                Some(text) => {
                    let mut v = Vec::new();
                    for c in text.chars().filter(|c| !c.is_whitespace()) {
                        if !c.is_ascii_uppercase() {
                            return Err(format!("{owner}: stage {i}'s letters must be capital letters, not {c:?}"));
                        }
                        v.push(c as u8 - b'A');
                    }
                    if v.is_empty() {
                        return Err(format!("{owner}: stage {i}'s letters are empty"));
                    }
                    Some(v)
                }
                None => None,
            };
            // A structure's sections by name: the first section of that name (a returning section
            // shares its letter, so its first place stands for it).
            let sections = s.sections.iter().filter_map(|n| names.iter().position(|x| x == n)).map(|p| p as u8).collect();
            let roles = if s.roles.is_empty() { 0xff } else { s.roles.iter().fold(0u8, |m, r| m | 1 << r.index()) };
            let e = |x: f32| x.clamp(0.1, 2.0);
            stages.push(RStage { kind: s.kind, share: s.share, energy: (e(s.energy.0), e(s.energy.1)), letters, sections, roles, drums: s.drums, key: s.key, tune: s.tune });
        }
        Ok(RArc { climax_at: (a, b), stages })
    }
}

/// One phrase of a planned piece.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlanPhrase {
    /// Its first bar in the round, and how many bars it lasts.
    pub start_bar: u32,
    pub bars: u32,
    /// Its section: a letter (A = 0; [`FREE`]: a section of its own), and for a structure its place in
    /// it (255: none).
    pub letter: u8,
    pub part: u8,
    /// The stage it is in (an index in the arc's stages; 0 for a genre without an arc) and that stage's kind.
    pub stage: u8,
    pub kind: Option<StageKind>,
    /// Its energy (1 = as levelled).
    pub energy: f32,
    /// The roles that may play (one bit a role) and whether the drums do.
    pub roles: u8,
    pub drums: bool,
    pub key: KeyMove,
    /// It plays its section's tune; the tune was already heard (a theme returning); the improvised
    /// times this letter has played so far in the round, this one counted (0 for a tune).
    pub tune: bool,
    pub returning: bool,
    pub chorus: u32,
    /// Its chords end at home (a stage's or the round's end).
    pub end_home: bool,
}

/// A whole piece laid out: one round of its arc (an endless piece goes round again).
#[derive(Clone, Copy, Debug)]
pub struct ArcPlan {
    pub n: usize,
    pub phrases: [PlanPhrase; MAX_PLAN],
    /// Bars in one round.
    pub bars: u32,
    /// The first phrase of the climax stage (None: the arc has none).
    pub climax: Option<usize>,
    /// The genre has no arc: the round is one time through its form, and every later round improvises
    /// (its tunes were the first round's).
    pub trivial: bool,
}

impl Default for ArcPlan {
    fn default() -> Self {
        ArcPlan { n: 0, phrases: [PlanPhrase::default(); MAX_PLAN], bars: 0, climax: None, trivial: true }
    }
}

impl ArcPlan {
    /// The phrase of the piece numbered `i` (counted over every round): the phrase, its round, and
    /// the piece bar it begins on.
    pub fn phrase(&self, i: u64) -> Option<(PlanPhrase, u64, u64)> {
        if self.n == 0 {
            return None;
        }
        let round = i / self.n as u64;
        let p = self.phrases[(i % self.n as u64) as usize];
        Some((p, round, round * self.bars as u64 + p.start_bar as u64))
    }

    /// The number of the phrase that piece bar `bar` falls in (counted over every round).
    pub fn phrase_at_bar(&self, bar: u64) -> Option<u64> {
        if self.n == 0 || self.bars == 0 {
            return None;
        }
        let round = bar / self.bars as u64;
        let b = (bar % self.bars as u64) as u32;
        // The last phrase starting at or before the bar (the phrases are in order).
        let k = self.phrases[..self.n].partition_point(|p| p.start_bar <= b).saturating_sub(1);
        Some(round * self.n as u64 + k as u64)
    }
}

/// What a plan is made from.
pub struct PlanInput<'a> {
    /// The genre's arc (None: its form over and over).
    pub arc: Option<&'a RArc>,
    /// The piece's form, letters (A = 0; empty: no form). For a structure: its sections' letters in order.
    pub form: &'a [u8],
    /// For a structure: each section's bars (empty: no structure, every letter's bars are drawn).
    pub structure_bars: &'a [u32],
    /// The section lengths a letter draws from, bars.
    pub phrase_bars: &'a [u32],
    /// The plan's seed (crate::composer's seed scheme) and the piece's length, bars.
    pub seed: u64,
    pub length_bars: u32,
}

/// Lay out a piece: every phrase of one round of `inp.arc` (or of the form), into `out`.
pub fn plan(inp: &PlanInput, out: &mut ArcPlan) {
    out.n = 0;
    out.bars = 0;
    out.climax = None;
    out.trivial = inp.arc.is_none();
    let structure = !inp.structure_bars.is_empty();
    // A letter's bars: the structure's section, else drawn once a letter from the plan's seed (the
    // same letter is as long all piece, as it is the same chords).
    let bars_of = |letter: u8, part: u8| -> u32 {
        if structure {
            return inp.structure_bars.get(part as usize).copied().unwrap_or(8).clamp(1, 64);
        }
        if inp.phrase_bars.is_empty() {
            return 8;
        }
        let i = pick(unit(mix(inp.seed, 0x1e77 + letter as u64)), inp.phrase_bars.iter().map(|_| 1.0));
        inp.phrase_bars[i].clamp(1, 64)
    };
    let mut heard = [false; 256];
    let mut improvised = [0u32; 256];
    let mut push = |out: &mut ArcPlan, letter: u8, part: u8, stage: u8, s: Option<&RStage>, tune: bool| {
        if out.n >= MAX_PLAN {
            return false;
        }
        let bars = bars_of(letter, part);
        let (tune, returning, chorus) = if tune && letter != FREE {
            let r = heard[letter as usize];
            heard[letter as usize] = true;
            (true, r, 0)
        } else {
            improvised[letter as usize] += 1;
            (false, false, improvised[letter as usize])
        };
        out.phrases[out.n] = PlanPhrase {
            start_bar: out.bars,
            bars,
            letter,
            part,
            stage,
            kind: s.map(|s| s.kind),
            energy: 1.0,
            roles: s.map(|s| s.roles).unwrap_or(0xff),
            drums: s.is_none_or(|s| s.drums),
            key: s.map(|s| s.key).unwrap_or_default(),
            tune,
            returning,
            chorus,
            end_home: false,
        };
        out.n += 1;
        out.bars += bars;
        true
    };
    let Some(arc) = inp.arc else {
        // No arc: one time through the form (a structure's sections in order), its tunes heard once.
        if inp.form.is_empty() {
            push(out, FREE, 255, 0, None, false);
        } else {
            for (k, &l) in inp.form.iter().enumerate() {
                push(out, l, if structure { k as u8 } else { 255 }, 0, None, true);
            }
        }
        if out.n > 0 {
            out.phrases[out.n - 1].end_home = true;
        }
        return;
    };
    // The stages' bars: the climax's start drawn within `climax_at`, the stages before it scaled to
    // end there and the rest to fill the piece.
    let stages = &arc.stages[..arc.stages.len().min(MAX_STAGES)];
    let climax = stages.iter().position(|s| s.kind == StageKind::Climax);
    let length = inp.length_bars.max(1) as f32;
    let mut budget = [0f32; MAX_STAGES];
    let total: f32 = stages.iter().map(|s| s.share).sum::<f32>().max(1e-6);
    match climax {
        Some(c) if c > 0 => {
            let at = arc.climax_at.0 + (arc.climax_at.1 - arc.climax_at.0) * unit(mix(inp.seed, 0xc11a));
            let pre: f32 = stages[..c].iter().map(|s| s.share).sum::<f32>().max(1e-6);
            let post: f32 = stages[c..].iter().map(|s| s.share).sum::<f32>().max(1e-6);
            for (k, s) in stages.iter().enumerate() {
                budget[k] = length * if k < c { s.share / pre * at } else { s.share / post * (1.0 - at) };
            }
        }
        _ => {
            for (k, s) in stages.iter().enumerate() {
                budget[k] = length * s.share / total;
            }
        }
    }
    // The structure between its first and last sections: what a stage that names none plays.
    let body_range = if structure && inp.form.len() > 2 { 1..inp.form.len() - 1 } else { 0..inp.form.len() };
    for (k, s) in stages.iter().enumerate() {
        let first = out.n;
        let start_bar = out.bars;
        // This stage's sections in turn: (letter, place in the structure).
        let count = if structure {
            if s.sections.is_empty() { body_range.len() } else { s.sections.len() }
        } else {
            match &s.letters {
                Some(l) => l.len(),
                None => inp.form.len().max(1),
            }
        };
        let nth = |j: usize| -> (u8, u8) {
            if structure {
                let p = if s.sections.is_empty() { (body_range.start + j % count.max(1)) as u8 } else { s.sections[j % count] };
                (inp.form.get(p as usize).copied().unwrap_or(0), p)
            } else {
                match &s.letters {
                    Some(l) => (l[j % l.len()], 255),
                    None if inp.form.is_empty() => (FREE, 255),
                    None => (inp.form[j % inp.form.len()], 255),
                }
            }
        };
        let mut j = 0usize;
        loop {
            let (letter, part) = nth(j);
            let next = bars_of(letter, part) as f32;
            // At least one phrase a stage; then on while the stage has room for most of the next.
            if j > 0 && (out.bars - start_bar) as f32 + next * 0.5 > budget[k] {
                break;
            }
            if !push(out, letter, part, k as u8, Some(s), s.tune) {
                break;
            }
            j += 1;
        }
        // Energy across the stage, by each phrase's middle.
        let used = (out.bars - start_bar).max(1) as f32;
        for p in out.phrases[first..out.n].iter_mut() {
            let mid = (p.start_bar - start_bar) as f32 + p.bars as f32 * 0.5;
            p.energy = s.energy.0 + (s.energy.1 - s.energy.0) * (mid / used);
        }
        if out.n > first {
            if Some(k) == climax {
                out.climax = Some(first);
            }
            if matches!(s.kind, StageKind::Theme | StageKind::Climax | StageKind::Return | StageKind::Coda) {
                out.phrases[out.n - 1].end_home = true;
            }
        }
        if out.n >= MAX_PLAN {
            break;
        }
    }
    if out.n > 0 {
        out.phrases[out.n - 1].end_home = true;
    }
}

/// How an energy moves a phrase's velocities (1 at 1: the genre as levelled).
pub fn velocity_of(energy: f32) -> f32 {
    (0.4 + 0.6 * energy).clamp(0.3, 1.4)
}

/// How an energy moves how often the riffs, the choir and the heartbeat play (a factor on each role's
/// density, 1 at 1).
pub fn density_of(energy: f32) -> f32 {
    energy.clamp(0.15, 1.6)
}
