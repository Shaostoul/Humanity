//! A PIECE: what one seed makes of a genre (docs/design/music.md section 7c; the operator,
//! 2026-11-13: "Are we able to have like procedural seeds for music? So it generates a track based
//! on the seed and then we can vary the seed X amount of minutes to mix up the music?").
//!
//! A score plays one piece. Its seed draws, within the genre's rules (data, never code): the key
//! (the genre's keys by weight; the bridge may move to another), the tempo within the genre's range,
//! the FORM (the order its sections play in, "AABA" and the like, by weight), each section's chord
//! walk (generated from the genre's harmonic grammar: shapes of tonic, subdominant and dominant
//! functions, then secondary dominants, tritone substitutes and passing diminished chords, each by
//! its chance; or, for a genre with only a chord graph, a walk of that graph with this piece's own
//! leanings), fresh riffs (the genre's motif rules: rhythm cells, contours, steps of its scale) and
//! its lead voices. A section that returns plays its chords again, and its tune the first time
//! through the form; after that the band improvises over the same chords. A new piece (a new seed)
//! comes every few minutes (Settings > Music), so hours of one genre never sound alike.
//!
//! Everything here is fixed-size and allocation-free, so it runs on the audio thread at a phrase
//! boundary, and everything is a pure function of the seed, so one seed is the same piece twice.
//! Only the commonplace building blocks of a style are used (the 2026-11-12 copyright finding): no
//! tune's own changes or melody is written anywhere.

use crate::rng::Rng;

/// The most notes a generated riff has.
pub const MAX_MOTIF_NOTES: usize = 12;
/// The most riffs a piece makes.
pub const MAX_PIECE_MOTIFS: usize = 8;
/// The longest form (sections a time through).
pub const MAX_FORM: usize = 16;
/// The most chords in a section (the composer's plan slots).
pub const MAX_SLOTS: usize = 16;

/// A chord's function in a key, for the grammar.
pub const FN_NONE: u8 = 0;
pub const FN_TONIC: u8 = 1;
pub const FN_SUB: u8 = 2;
pub const FN_DOM: u8 = 3;

/// Mix two numbers into a well-spread seed (SplitMix64's finaliser over their sum).
pub fn mix(a: u64, b: u64) -> u64 {
    let mut z = a.wrapping_add(b.wrapping_mul(0x9e37_79b9_7f4a_7c15)).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A number in 0..1 from a seed.
pub fn unit(seed: u64) -> f32 {
    (mix(seed, 0x51) >> 40) as f32 / (1u64 << 24) as f32
}

/// How much this piece leans toward `a` (or the move from `a` to `b`): 0.3 to 1.7, the same for a
/// seed every time, different between seeds. It multiplies a weight written in the data, so a
/// piece favours its own paths through the genre's harmony while every path stays the genre's.
pub fn lean(seed: u64, a: usize, b: usize) -> f32 {
    0.3 + 1.4 * unit(mix(mix(seed, a as u64 + 1), b as u64 + 0x100))
}

/// Pick an index by weight with a draw already made (0..1); the last positive weight if the draw
/// runs off the end; 0 when every weight is zero.
pub fn pick(draw: f32, weights: impl Iterator<Item = f32> + Clone) -> usize {
    let total: f32 = weights.clone().map(|w| w.max(0.0)).sum();
    if total <= 0.0 {
        return 0;
    }
    let mut x = draw.clamp(0.0, 0.999_999) * total;
    let mut last = 0;
    for (i, w) in weights.enumerate() {
        let w = w.max(0.0);
        if w > 0.0 {
            last = i;
            if x < w {
                return i;
            }
            x -= w;
        }
    }
    last
}

/// A riff a piece made: scale steps from a chord tone, lengths in beats, how much of each sounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GenMotif {
    pub steps: [i32; MAX_MOTIF_NOTES],
    pub beats: [f32; MAX_MOTIF_NOTES],
    pub hold: [f32; MAX_MOTIF_NOTES],
    pub n: usize,
}

/// A melodic contour a generated riff follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Contour {
    /// Climbs.
    Rise,
    /// Falls.
    Fall,
    /// Up, then down.
    Arch,
    /// Down, then up.
    Valley,
    /// Circles one note.
    Hover,
    /// Alternates up and down.
    Zigzag,
}

/// A genre's riff rules, resolved (genre.rs `MotifRules`).
#[derive(Clone, Debug, Default)]
pub struct RMotifRules {
    pub count: usize,
    pub notes: (usize, usize),
    pub cells: Vec<Vec<f32>>,
    pub contours: Vec<(Contour, f32)>,
    pub leap: f32,
    pub range: (i32, i32),
    pub hold: (f32, f32),
}

/// Make a piece's riffs from `rules`; how many were made.
pub fn make_motifs(rules: &RMotifRules, rng: &mut Rng, out: &mut [GenMotif; MAX_PIECE_MOTIFS]) -> usize {
    let count = rules.count.clamp(1, MAX_PIECE_MOTIFS);
    let (lo, hi) = (rules.notes.0.clamp(2, MAX_MOTIF_NOTES), rules.notes.1.clamp(2, MAX_MOTIF_NOTES));
    let (lo, hi) = (lo.min(hi), hi.max(lo));
    let (rlo, rhi) = (rules.range.0.min(0), rules.range.1.max(1));
    for m in out.iter_mut().take(count) {
        let n = rng.int(lo as i32, hi as i32) as usize;
        // The rhythm: cells strung together until the riff has its notes.
        let mut beats = [0.5f32; MAX_MOTIF_NOTES];
        let mut k = 0;
        let mut guard = 0;
        while k < n && guard < 64 {
            guard += 1;
            if rules.cells.is_empty() {
                beats[k] = 0.5;
                k += 1;
                continue;
            }
            let cell = &rules.cells[rng.int(0, rules.cells.len() as i32 - 1) as usize];
            for &b in cell.iter() {
                if k >= n {
                    break;
                }
                beats[k] = b.clamp(0.125, 4.0);
                k += 1;
            }
        }
        // The contour, then the steps: mostly one or two scale steps, a leap now and then, inside
        // the range; the first note on the chord tone the riff is anchored to (step 0).
        let contour = if rules.contours.is_empty() { Contour::Arch } else { rules.contours[rng.weighted(rules.contours.iter().map(|c| c.1))].0 };
        let mut steps = [0i32; MAX_MOTIF_NOTES];
        let mut cur = 0i32;
        let turn = (n / 2).max(1);
        for i in 1..n {
            let dir = match contour {
                Contour::Rise => 1,
                Contour::Fall => -1,
                Contour::Arch => {
                    if i <= turn {
                        1
                    } else {
                        -1
                    }
                }
                Contour::Valley => {
                    if i <= turn {
                        -1
                    } else {
                        1
                    }
                }
                Contour::Hover => {
                    if cur > 0 {
                        -1
                    } else if cur < 0 {
                        1
                    } else if rng.chance(0.5) {
                        1
                    } else {
                        -1
                    }
                }
                Contour::Zigzag => {
                    if i % 2 == 1 {
                        1
                    } else {
                        -1
                    }
                }
            };
            let size = if rng.chance(rules.leap) {
                rng.int(3, 4)
            } else if contour == Contour::Hover {
                rng.int(0, 1)
            } else {
                rng.int(1, 2)
            };
            let mut next = cur + dir * size;
            if next > rhi || next < rlo {
                next = cur - dir * size.min(2);
            }
            cur = next.clamp(rlo, rhi);
            steps[i] = cur;
        }
        let mut hold = [0.8f32; MAX_MOTIF_NOTES];
        for h in hold.iter_mut().take(n) {
            *h = rng.range(rules.hold.0.min(rules.hold.1), rules.hold.1.max(rules.hold.0)).clamp(0.05, 1.0);
        }
        *m = GenMotif { steps, beats, hold, n };
    }
    count
}

/// A genre's harmonic grammar, resolved against its chords (resolved.rs adds the derived chords:
/// each chord's secondary dominant, each dominant's tritone substitute, the diminished chord a half
/// step under each chord).
#[derive(Clone, Debug, Default)]
pub struct RGrammar {
    /// Per chord of the harmony (derived ones included): its function, `FN_*`.
    pub func: Vec<u8>,
    /// Shapes: runs of functions, by weight.
    pub shapes: Vec<(Vec<u8>, f32)>,
    /// Per chord: the index of its secondary dominant, its tritone substitute (dominants only) and
    /// the diminished chord a half step under it.
    pub sec_dom: Vec<Option<u16>>,
    pub tritone: Vec<Option<u16>>,
    pub passing: Vec<Option<u16>>,
    pub p_sec: f32,
    pub p_tri: f32,
    pub p_pass: f32,
}

/// One section's chords: which (indices into the harmony's chords) and how many bars each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Section {
    pub n: usize,
    pub chords: [u16; MAX_SLOTS],
    pub bars: [u8; MAX_SLOTS],
}

impl Section {
    fn push(&mut self, chord: usize, bars: u32) {
        if self.n < MAX_SLOTS {
            self.chords[self.n] = chord as u16;
            self.bars[self.n] = bars.clamp(1, 255) as u8;
            self.n += 1;
        }
    }

    /// Put `chord` in its own one-bar slot before slot `k`, taking that bar from slot `k - 1`.
    fn insert_before(&mut self, k: usize, chord: u16) {
        if k == 0 || k > self.n || self.n >= MAX_SLOTS || self.bars[k - 1] < 2 {
            return;
        }
        self.bars[k - 1] -= 1;
        for i in (k..self.n).rev() {
            self.chords[i + 1] = self.chords[i];
            self.bars[i + 1] = self.bars[i];
        }
        self.chords[k] = chord;
        self.bars[k] = 1;
        self.n += 1;
    }

    pub fn total_bars(&self) -> u32 {
        self.bars[..self.n].iter().map(|&b| b as u32).sum()
    }
}

/// `a`'s bars before `at_bar` and `b`'s from `at_bar` on (a slot that crosses the line is cut there):
/// a returning section played again with its second half reharmonised, the way a band varies the
/// changes on a later time through while the section stays recognisable.
pub fn splice(a: &Section, b: &Section, at_bar: u32) -> Section {
    let mut out = Section::default();
    let mut t = 0u32;
    for i in 0..a.n {
        let len = a.bars[i] as u32;
        if t >= at_bar {
            break;
        }
        out.push(a.chords[i] as usize, len.min(at_bar - t));
        t += len;
    }
    let mut t = 0u32;
    for i in 0..b.n {
        let len = b.bars[i] as u32;
        let end = t + len;
        if end > at_bar {
            let from = t.max(at_bar);
            // The same chord on both sides of the line is one longer slot.
            if out.n > 0 && out.chords[out.n - 1] == b.chords[i] && from == at_bar {
                out.bars[out.n - 1] = (out.bars[out.n - 1] as u32 + end - from).min(255) as u8;
            } else {
                out.push(b.chords[i] as usize, end - from);
            }
        }
        t = end;
    }
    out
}

/// A section from the grammar: a shape by weight, each slot a chord of its function chosen with
/// this piece's leanings (and not the chord just played when another will do), `bars` bars in all
/// with `chord_bars` per chord, the last slot a tonic (`end_home`) or, as a turnaround, a tonic or a
/// dominant; then the embellishments, each by its chance.
pub fn grammar_section(chord_count: usize, g: &RGrammar, rng: &mut Rng, seed: u64, bars: u32, chord_bars: (u32, u32), end_home: bool) -> Section {
    let mut s = Section::default();
    if g.shapes.is_empty() || chord_count == 0 {
        return s;
    }
    let shape = &g.shapes[rng.weighted(g.shapes.iter().map(|x| x.1))].0;
    let (lo, hi) = (chord_bars.0.max(1), chord_bars.1.max(chord_bars.0.max(1)));
    let mut used = 0u32;
    let mut i = 0usize;
    let mut prev: Option<usize> = None;
    while used < bars && s.n < MAX_SLOTS {
        let mut b = rng.int(lo as i32, hi as i32) as u32;
        if used + b > bars || s.n == MAX_SLOTS - 1 {
            b = bars - used;
        }
        let last = used + b >= bars;
        let mut f = shape[i % shape.len().max(1)];
        if last {
            f = if end_home || !rng.chance(0.4) { FN_TONIC } else { FN_DOM };
        }
        let weight = |c: usize| -> f32 {
            if g.func.get(c).copied() != Some(f) {
                0.0
            } else if Some(c) == prev {
                0.15 * lean(seed, c, 0)
            } else {
                lean(seed, c, 0)
            }
        };
        let c = rng.weighted((0..chord_count).map(weight));
        let c = if g.func.get(c).copied() == Some(f) { c } else { (0..chord_count).find(|&c| g.func.get(c).copied() == Some(f)).unwrap_or(0) };
        s.push(c, b);
        prev = Some(c);
        used += b;
        i += 1;
    }
    // Embellishments, from the end back, so an insertion leaves the slots still to visit in place.
    let mut k = s.n;
    while k > 1 {
        k -= 1;
        let target = s.chords[k] as usize;
        let tri = (g.func.get(target).copied() == Some(FN_DOM) && rng.chance(g.p_tri)).then(|| g.tritone.get(target).copied().flatten()).flatten();
        if let Some(t) = tri {
            s.chords[k] = t;
        }
        let before = if rng.chance(g.p_sec) {
            g.sec_dom.get(target).copied().flatten()
        } else if rng.chance(g.p_pass) {
            g.passing.get(target).copied().flatten()
        } else {
            None
        };
        if let Some(e) = before.filter(|&e| e != s.chords[k - 1] && e != s.chords[k]) {
            s.insert_before(k, e);
        }
    }
    s
}

/// A section walked from a chord graph (`moves`, per chord its moves with weights; `cadence`, the
/// chords a phrase may end on) from `start`, with this piece's leanings on each move.
pub fn graph_section(moves: &[Vec<(usize, f32)>], cadence: &[bool], rng: &mut Rng, seed: u64, start: usize, bars: u32, chord_bars: (u32, u32)) -> Section {
    let mut s = Section::default();
    if moves.is_empty() {
        return s;
    }
    let (lo, hi) = (chord_bars.0.max(1), chord_bars.1.max(chord_bars.0.max(1)));
    let mut cur = start.min(moves.len() - 1);
    let mut used = 0u32;
    while used < bars && s.n < MAX_SLOTS {
        let mut b = rng.int(lo as i32, hi as i32) as u32;
        if used + b > bars || s.n == MAX_SLOTS - 1 {
            b = bars - used;
        }
        s.push(cur, b);
        used += b;
        if used < bars {
            let last = used + lo > bars || used + hi >= bars;
            cur = walk(moves, cadence, rng, seed, cur, last);
        }
    }
    s
}

/// One step along a chord graph, this piece's leanings on each move; `want_cadence` asks for a
/// phrase-ending chord when the graph allows one.
pub fn walk(moves: &[Vec<(usize, f32)>], cadence: &[bool], rng: &mut Rng, seed: u64, cur: usize, want_cadence: bool) -> usize {
    let mv = &moves[cur.min(moves.len() - 1)];
    if mv.is_empty() {
        return cur;
    }
    let is_cad = |c: usize| cadence.get(c).copied().unwrap_or(false);
    if want_cadence && mv.iter().any(|(c, w)| is_cad(*c) && *w > 0.0) {
        let i = rng.weighted(mv.iter().map(|(c, w)| if is_cad(*c) { *w * lean(seed, cur, *c) } else { 0.0 }));
        return mv[i].0;
    }
    let i = rng.weighted(mv.iter().map(|(c, w)| *w * lean(seed, cur, *c)));
    mv[i].0
}

/// What a piece drew from its seed, kept as plain draws so they apply to whichever genre and
/// harmony play (a blend can change the dominant genre, a place the mood).
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub seed: u64,
    /// The form's letters (0 = A) and its length (0: no form, every phrase walked afresh), and the
    /// genre it was chosen from (`u16::MAX`: none yet).
    pub form: [u8; MAX_FORM],
    pub form_len: usize,
    pub form_genre: u16,
    /// Calm phrases begun in this piece (its place in the form).
    pub played: u64,
    /// Where in the tempo range, which key, whether the bridge moves key and to which.
    pub tempo_t: f32,
    pub key_draw: f32,
    pub bridge_moves: f32,
    pub bridge_draw: f32,
    /// Per role: the draw that picks its lead voice (a genre's leads, a mood's options).
    pub lead_draw: [f32; 8],
    /// The draw that picks the drum kit (a genre's `kits`).
    pub kit_draw: f32,
}

impl Piece {
    /// The piece `seed` makes.
    pub fn new(seed: u64) -> Self {
        let d = |k: u64| unit(mix(seed, k));
        let mut lead_draw = [0.0f32; 8];
        for (i, x) in lead_draw.iter_mut().enumerate() {
            *x = d(0x1ead + i as u64);
        }
        Piece { seed, form: [0; MAX_FORM], form_len: 0, form_genre: u16::MAX, played: 0, tempo_t: d(0x7e), key_draw: d(0x6e), bridge_moves: d(0xb1), bridge_draw: d(0xb2), lead_draw, kit_draw: d(0x6b17) }
    }

    /// The kit this piece plays of `kits` (weights; None: the genre has no kits, every drum plays).
    pub fn kit(&self, kits: &[f32]) -> Option<u8> {
        (!kits.is_empty()).then(|| pick(self.kit_draw, kits.iter().copied()) as u8)
    }

    /// Choose the form from `forms` (letters as text, by weight) for genre `genre` (its index; `key`,
    /// its id's hash, draws, so the choice never depends on the genre's place in the list); no
    /// forms, no form.
    pub fn choose_form(&mut self, genre: u16, key: u64, forms: &[(Vec<u8>, f32)]) {
        self.form_genre = genre;
        self.form_len = 0;
        if forms.is_empty() {
            return;
        }
        let i = pick(unit(mix(self.seed, 0xf0 ^ key)), forms.iter().map(|f| f.1));
        let f = &forms[i].0;
        self.form_len = f.len().min(MAX_FORM);
        self.form[..self.form_len].copy_from_slice(&f[..self.form_len]);
    }

    /// The section the phrase `index` of the piece plays: (its letter, which time through the form,
    /// whether the letter already played earlier in this time through), or None without a form.
    pub fn section(&self, index: u64) -> Option<(u8, u64, bool)> {
        if self.form_len == 0 {
            return None;
        }
        let len = self.form_len as u64;
        let pos = (index % len) as usize;
        let letter = self.form[pos];
        Some((letter, index / len, self.form[..pos].contains(&letter)))
    }

    /// The seed of a section: the same letter in the same harmony is the same section all piece.
    pub fn section_seed(&self, harmony: u64, letter: u8) -> u64 {
        mix(mix(self.seed, harmony.wrapping_add(0x5ec)), letter as u64 + 1)
    }
}
