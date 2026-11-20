//! THE MEI READER (docs/design/score-player.md, sections 4 to 8; the operator, 2026-11-20: "MEI
//! sounds right unless there's better"): written music stored as MEI, read into notes with exact
//! times, sounding pitches, ties joined, dynamics and lyrics, ready to be timed (crate::written) and
//! played on our own instruments.
//!
//! Our own reader of the MEI Basic 5.1 core (the profile the free MuseScore editor reads and writes),
//! with the full-MEI spellings of the same things accepted too. MEI Basic leaves out `accid.ges`, so
//! the sounding pitch is worked out as a musician reads it: the written accidental, else one written
//! earlier in the bar for the same letter and octave on the same staff, else the key signature; a
//! tied note keeps its pitch over the bar line.
//!
//! Times are EXACT fractions of a whole note ([`Frac`]): a dotted quarter is 3/8, a triplet eighth
//! 1/12, nothing rounded until the timeline turns them into seconds.
//!
//! Nothing here crashes on a file it does not understand: an element it does not know is skipped and
//! counted ([`MeiScore::skipped`]), one it knows but does not play yet is counted apart
//! ([`MeiScore::not_played`]), and a core element it cannot make sense of is a problem in words
//! ([`MeiScore::problems`]). A file that is not well-formed XML gives an error naming the line.
//!
//! Read on the music's build thread (or a test), never on the audio thread.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

/// An exact fraction (of a whole note, in this module), kept reduced with a positive denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frac {
    pub n: i64,
    pub d: i64,
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

impl Frac {
    pub const ZERO: Frac = Frac { n: 0, d: 1 };

    /// `n / d`, reduced (a zero denominator gives zero).
    pub fn new(n: i64, d: i64) -> Frac {
        if d == 0 {
            return Frac::ZERO;
        }
        let s = if d < 0 { -1 } else { 1 };
        let g = gcd(n, d);
        Frac { n: s * n / g, d: s * d / g }
    }

    pub fn int(n: i64) -> Frac {
        Frac { n, d: 1 }
    }

    pub fn add(self, o: Frac) -> Frac {
        Frac::new(self.n * o.d + o.n * self.d, self.d * o.d)
    }

    pub fn sub(self, o: Frac) -> Frac {
        Frac::new(self.n * o.d - o.n * self.d, self.d * o.d)
    }

    pub fn mul(self, o: Frac) -> Frac {
        Frac::new(self.n * o.n, self.d * o.d)
    }

    pub fn div(self, o: Frac) -> Frac {
        Frac::new(self.n * o.d, self.d * o.n)
    }

    pub fn to_f64(self) -> f64 {
        self.n as f64 / self.d as f64
    }

    pub fn is_zero(self) -> bool {
        self.n == 0
    }

    /// A decimal number written in a file ("2", "2.5", "1.25") exactly, None for anything else.
    pub fn parse_decimal(s: &str) -> Option<Frac> {
        let s = s.trim();
        let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
        if (whole.is_empty() && frac.is_empty()) || !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) || frac.len() > 9 {
            return None;
        }
        let w: i64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
        let f: i64 = if frac.is_empty() { 0 } else { frac.parse().ok()? };
        let d = 10i64.pow(frac.len() as u32);
        Some(Frac::new(w * d + f, d))
    }
}

impl Default for Frac {
    fn default() -> Frac {
        Frac::ZERO
    }
}

impl Ord for Frac {
    fn cmp(&self, o: &Frac) -> Ordering {
        (self.n as i128 * o.d as i128).cmp(&(o.n as i128 * self.d as i128))
    }
}

impl PartialOrd for Frac {
    fn partial_cmp(&self, o: &Frac) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// A staff of the score (a part: the melody, the piano's right hand).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeiStaff {
    pub n: u16,
    /// Its name as the file gives it (`staffDef@label` or a `label` inside it).
    pub label: String,
    /// A General MIDI program the file suggests (`instrDef@midi.instrnum`, 0..127), a hint only.
    pub midi_program: Option<u8>,
}

/// One sounding note: tied notes are joined into one.
#[derive(Clone, Debug, PartialEq)]
pub struct MeiNote {
    /// Its `xml:id` (empty when the file gives none).
    pub id: String,
    pub staff: u16,
    pub layer: u16,
    /// When it starts and how long it sounds, in whole notes from the start of the piece.
    pub at: Frac,
    pub dur: Frac,
    /// The sounding pitch as a MIDI note (a quarter tone is half a semitone); the voice plays
    /// `tuning::note_hz(midi, a4)`.
    pub midi: f32,
    /// 0..1: its own `vel`, else the dynamic marked before it (a hairpin moving it, an accent
    /// adding to it), else mezzo-forte.
    pub vel: f32,
    /// The bar it starts in (0 = the first, counted as played: a repeated bar twice).
    pub bar: u32,
    /// How much of its written length it is held, from its articulation (staccato 0.5, tenuto 1);
    /// None: the player's own rule (a little short, so repeated notes stay two).
    pub hold: Option<f32>,
    /// A grace note (it takes its time from the note after it).
    pub grace: bool,
}

/// Where a syllable sits in its word (`syl@wordpos`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordPos {
    Initial,
    Medial,
    Terminal,
    /// A word of one syllable (`s`, or no `wordpos` at all).
    Single,
}

impl WordPos {
    fn parse(v: Option<&str>) -> WordPos {
        match v {
            Some("i") => WordPos::Initial,
            Some("m") => WordPos::Medial,
            Some("t") => WordPos::Terminal,
            _ => WordPos::Single,
        }
    }

    /// Whether a new word begins with this syllable.
    pub fn starts_word(self) -> bool {
        matches!(self, WordPos::Initial | WordPos::Single)
    }
}

/// A syllable of the lyrics, under its note: the sing-along's hook (score-player.md section 11).
#[derive(Clone, Debug, PartialEq)]
pub struct MeiSyllable {
    /// The `xml:id` of the note it was written under, and that note's index in [`MeiScore::notes`]
    /// (the sounding note it belongs to, a tie's first note when it was written under a later one).
    pub note_id: String,
    pub note: usize,
    /// Which verse (`verse@n`, 1 by default).
    pub verse: u16,
    pub text: String,
    pub wordpos: WordPos,
    /// What joins it to the next (`syl@con`: `d` a dash, `u` an extender line, `s` a space ...).
    pub con: Option<char>,
}

/// A tempo: from `at` (whole notes), so many quarter notes a minute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeiTempo {
    pub at: Frac,
    pub qpm: f64,
}

/// A bar: where it starts, its number as written, how long it lasts, and its meter's beats (the
/// meter's count and unit: 3 and 4 for 3/4).
#[derive(Clone, Debug, PartialEq)]
pub struct MeiBar {
    pub at: Frac,
    pub n: String,
    pub len: Frac,
    pub meter: (i64, i64),
}

/// A written piece as read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeiScore {
    /// The header: the title, the composer, the availability statement (its licence), `meiversion`.
    pub title: String,
    pub composer: String,
    pub availability: String,
    pub meiversion: String,
    pub staves: Vec<MeiStaff>,
    /// Every sounding note, by start (then staff, then pitch).
    pub notes: Vec<MeiNote>,
    /// Every syllable, in the order sung (by its note's start, then verse).
    pub syllables: Vec<MeiSyllable>,
    /// The tempo marks with a number, by position.
    pub tempos: Vec<MeiTempo>,
    /// The bars as played: a repeat's bars twice, only the ending each time through takes.
    pub bars: Vec<MeiBar>,
    /// How many bars the file writes (a repeated bar once).
    pub written_bars: u32,
    /// The fermatas, as played: where each held moment starts and how long it is written (the
    /// timeline holds it longer: crate::written::FERMATA_HOLD), overlapping ones joined, by start.
    pub fermatas: Vec<(Frac, Frac)>,
    /// Where the last bar ends.
    pub end: Frac,
    /// Elements the reader does not know, by name, and how many (skipped with what they hold).
    pub skipped: BTreeMap<String, u32>,
    /// Elements it knows but does not play yet (a slur, a fermata, a grace note), by name.
    pub not_played: BTreeMap<String, u32>,
    /// Core elements it could not make sense of, in words, each with its line in the file.
    pub problems: Vec<String>,
}

/// Read an MEI file's text. An error only when it is not well-formed XML or not MEI at all; anything
/// else the reader cannot use is counted or listed in the result.
pub fn read(text: &str) -> Result<MeiScore, String> {
    let root = parse_tree(text)?;
    if root.name != "mei" {
        return Err(format!("not an MEI file: its first element is <{}>, not <mei>", root.name));
    }
    let mut w = Walk::new();
    w.out.meiversion = root.attr("meiversion").unwrap_or_default().to_string();
    let mut saw_music = false;
    for k in root.els() {
        match k.name.as_str() {
            "meiHead" => w.header(k),
            "music" => {
                saw_music = true;
                // The bars as written, then in the order they play (repeats and endings expanded).
                let mut units = Vec::new();
                w.collect(k, &[], &mut units);
                let mut r = 0;
                while r < units.len() {
                    if units[r].ending.is_empty() {
                        r += 1;
                        continue;
                    }
                    let run_end = (r..units.len()).find(|&j| units[j].ending.is_empty()).unwrap_or(units.len());
                    let most = units[r..run_end].iter().flat_map(|u| u.ending.iter().copied()).max().unwrap_or(1);
                    for u in &mut units[r..run_end] {
                        u.ending_max = most;
                    }
                    r = run_end;
                }
                w.out.written_bars = units.iter().filter(|u| u.el.name == "measure").count() as u32;
                for i in play_order(&units) {
                    let el = units[i].el;
                    match el.name.as_str() {
                        "measure" => w.measure(el),
                        "scoreDef" => w.score_def(el),
                        _ => w.staff_def(el),
                    }
                }
            }
            other => w.skip(other),
        }
    }
    if !saw_music {
        return Err("the MEI file has no <music> element".into());
    }
    w.finish();
    Ok(w.out)
}

// ---------------------------------------------------------------------------------------------
// The tree: the file as elements, attributes and text.
// ---------------------------------------------------------------------------------------------

struct El {
    name: String,
    attrs: Vec<(String, String)>,
    kids: Vec<Node>,
    line: u32,
}

enum Node {
    El(El),
    Text(String),
}

impl El {
    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }

    fn els(&self) -> impl Iterator<Item = &El> {
        self.kids.iter().filter_map(|n| match n {
            Node::El(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// All the text inside, nested elements' included, with runs of white space made one space.
    fn text(&self) -> String {
        fn gather(e: &El, out: &mut String) {
            for k in &e.kids {
                match k {
                    Node::Text(t) => out.push_str(t),
                    Node::El(x) => gather(x, out),
                }
            }
        }
        let mut s = String::new();
        gather(self, &mut s);
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The first element named `name` at any depth inside.
    fn find(&self, name: &str) -> Option<&El> {
        for k in self.els() {
            if k.name == name {
                return Some(k);
            }
            if let Some(x) = k.find(name) {
                return Some(x);
            }
        }
        None
    }
}

/// The element names of a tag without a namespace prefix (`mei:note` is `note`).
fn local(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    match s.rsplit_once(':') {
        Some((_, l)) => l.to_string(),
        None => s.to_string(),
    }
}

fn parse_tree(text: &str) -> Result<El, String> {
    let mut r = Reader::from_str(text);
    let mut stack: Vec<El> = Vec::new();
    let mut root: Option<El> = None;
    // Line numbers, counted as the reader moves on (each newline once).
    let bytes = text.as_bytes();
    let (mut seen, mut line) = (0usize, 1u32);
    let mut line_at = |pos: usize| -> u32 {
        let pos = pos.min(bytes.len());
        if pos > seen {
            line += bytes[seen..pos].iter().filter(|&&b| b == b'\n').count() as u32;
            seen = pos;
        }
        line
    };
    fn push_text(stack: &mut [El], s: &str) {
        if let Some(top) = stack.last_mut() {
            if let Some(Node::Text(t)) = top.kids.last_mut() {
                t.push_str(s);
            } else {
                top.kids.push(Node::Text(s.to_string()));
            }
        }
    }
    fn element(e: &quick_xml::events::BytesStart, line: u32) -> Result<El, String> {
        let mut attrs = Vec::new();
        for a in e.attributes() {
            let a = a.map_err(|x| format!("line {line}: {x}"))?;
            let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
            let val = a.normalized_value(XmlVersion::Implicit1_0).map_err(|x| format!("line {line}: {x}"))?;
            attrs.push((key, val.to_string()));
        }
        Ok(El { name: local(e.name().as_ref()), attrs, kids: Vec::new(), line })
    }
    loop {
        let start = r.buffer_position() as usize;
        let ev = match r.read_event() {
            Ok(ev) => ev,
            Err(e) => return Err(format!("line {}: {e}", line_at(r.error_position() as usize))),
        };
        // The line the event starts on (white space between tags is a text event of its own, so a
        // tag's event starts at its '<').
        let ln = line_at(start);
        match ev {
            Event::Start(e) => stack.push(element(&e, ln)?),
            Event::Empty(e) => {
                let el = element(&e, ln)?;
                match stack.last_mut() {
                    Some(top) => top.kids.push(Node::El(el)),
                    None if root.is_none() => root = Some(el),
                    None => return Err(format!("line {ln}: a second top-level element <{}>", el.name)),
                }
            }
            Event::End(_) => {
                let Some(el) = stack.pop() else { return Err(format!("line {ln}: a closing tag with nothing open")) };
                match stack.last_mut() {
                    Some(top) => top.kids.push(Node::El(el)),
                    None if root.is_none() => root = Some(el),
                    None => return Err(format!("line {ln}: a second top-level element <{}>", el.name)),
                }
            }
            Event::Text(t) => {
                let s = t.xml10_content().map_err(|e| format!("line {ln}: {e}"))?;
                // White space between tags is kept as one space (it may part two words of a title).
                push_text(&mut stack, if s.trim().is_empty() { " " } else { &s });
            }
            Event::CData(c) => {
                let s = c.decode().map_err(|e| format!("line {ln}: {e}"))?;
                push_text(&mut stack, &s);
            }
            Event::GeneralRef(g) => {
                if let Some(ch) = g.resolve_char_ref().map_err(|e| format!("line {ln}: {e}"))? {
                    push_text(&mut stack, ch.encode_utf8(&mut [0u8; 4]));
                } else {
                    let name = g.decode().map_err(|e| format!("line {ln}: {e}"))?;
                    match quick_xml::escape::resolve_predefined_entity(&name) {
                        Some(s) => push_text(&mut stack, s),
                        None => return Err(format!("line {ln}: the entity &{name}; is not one XML defines")),
                    }
                }
            }
            Event::Eof => break,
            // The declaration, processing instructions (the schema's `xml-model`), comments.
            _ => {}
        }
    }
    if let Some(open) = stack.last() {
        return Err(format!("the file ends inside <{}> (opened on line {})", open.name, open.line));
    }
    root.ok_or_else(|| "the file holds no element".to_string())
}

// ---------------------------------------------------------------------------------------------
// Pitch, duration and the key.
// ---------------------------------------------------------------------------------------------

/// A pitch name's letter: c = 0 .. b = 6.
fn letter_of(p: &str) -> Option<u8> {
    Some(match p.trim().to_ascii_lowercase().as_str() {
        "c" => 0,
        "d" => 1,
        "e" => 2,
        "f" => 3,
        "g" => 4,
        "a" => 5,
        "b" => 6,
        _ => return None,
    })
}

/// Each letter's pitch class.
const PC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// An accidental's change in semitones (MEI's `data.ACCIDENTAL.WRITTEN` and `.GESTURAL`; an arrow
/// modifier's small shift is not played).
fn accid_alter(v: &str) -> Option<f32> {
    Some(match v.trim() {
        "s" | "ns" | "su" | "sd" => 1.0,
        "f" | "nf" | "fu" | "fd" => -1.0,
        "ss" | "x" => 2.0,
        "ff" => -2.0,
        "xs" | "sx" | "ts" => 3.0,
        "tf" => -3.0,
        "n" | "nu" | "nd" => 0.0,
        "1qs" => 0.5,
        "3qs" => 1.5,
        "1qf" => -0.5,
        "3qf" => -1.5,
        _ => return None,
    })
}

/// A key signature as fifths ("2s" = 2, "3f" = -3, "0" = 0).
fn parse_keysig(v: &str) -> Option<i32> {
    let v = v.trim();
    if v == "0" {
        return Some(0);
    }
    let (num, kind) = v.split_at(v.len().checked_sub(1)?);
    let n: i32 = num.parse().ok().filter(|n| (0..=7).contains(n))?;
    match kind {
        "s" => Some(n),
        "f" => Some(-n),
        _ => None,
    }
}

/// What a key of `fifths` does to a letter: +1 for a sharp in it, -1 for a flat.
fn key_alter(fifths: i32, letter: u8) -> f32 {
    const SHARPS: [u8; 7] = [3, 0, 4, 1, 5, 2, 6];
    const FLATS: [u8; 7] = [6, 2, 5, 1, 4, 0, 3];
    if fifths > 0 && SHARPS[..fifths.min(7) as usize].contains(&letter) {
        1.0
    } else if fifths < 0 && FLATS[..(-fifths).min(7) as usize].contains(&letter) {
        -1.0
    } else {
        0.0
    }
}

/// A written duration (`dur`): `long` (four whole notes), `breve` (two), then 1, 2, 4 ... 2048.
fn base_dur(v: &str) -> Option<Frac> {
    match v.trim() {
        "long" => Some(Frac::int(4)),
        "breve" => Some(Frac::int(2)),
        s => {
            let d: i64 = s.parse().ok()?;
            ((1..=2048).contains(&d) && (d & (d - 1)) == 0).then(|| Frac::new(1, d))
        }
    }
}

/// An element's duration with its dots (`dots`, or `dot` elements inside), None without a `dur`.
fn dur_of(el: &El) -> Option<Frac> {
    let base = base_dur(el.attr("dur")?)?;
    let dots = el.attr("dots").and_then(|d| d.trim().parse::<u32>().ok()).unwrap_or_else(|| el.els().filter(|k| k.name == "dot").count() as u32).min(8);
    // Each dot adds half the value before it: base x (2 - 1/2^dots).
    let p = 1i64 << dots;
    Some(base.mul(Frac::new(2 * p - 1, p)))
}

/// A meter's count ("3", or "3+2" added up) and unit; `meter.sym` common is 4/4 and cut 2/2.
fn meter_of(el: &El, count_key: &str, unit_key: &str, sym_key: &str) -> Option<(i64, i64)> {
    let count = el.attr(count_key).and_then(|c| c.split('+').map(|x| x.trim().parse::<i64>().ok()).sum::<Option<i64>>());
    let unit = el.attr(unit_key).and_then(|u| u.trim().parse::<i64>().ok());
    match (count, unit) {
        (Some(c), Some(u)) if c > 0 && u > 0 => Some((c, u)),
        _ => match el.attr(sym_key) {
            Some("common") => Some((4, 4)),
            Some("cut") => Some((2, 2)),
            _ => None,
        },
    }
}

/// A dynamic's words to a velocity (None: words that are not one of the letters).
fn dynamic_vel(words: &str) -> Option<f32> {
    Some(match words.trim() {
        "ppp" => 0.2,
        "pp" => 0.3,
        "p" => 0.42,
        "mp" => 0.55,
        "mf" => 0.68,
        "f" => 0.8,
        "ff" => 0.9,
        "fff" => 1.0,
        _ => return None,
    })
}

/// The velocity of a note with no dynamic before it: mezzo-forte.
pub const DEFAULT_VEL: f32 = 0.68;

/// Elements the reader knows but does not play yet (score-player.md section 5's "later" column).
const NOT_PLAYED: [&str; 28] = [
    "slur", "dir", "harm", "fing", "fingGrp", "pedal", "trill", "mordent", "turn", "arpeg", "breath", "caesura", "phrase", "reh", "gliss", "lv", "bend", "ornam", "symbol", "annot", "attacca", "bracketSpan", "beamSpan", "clef", "clefGrp", "custos", "barLine", "parts",
];

/// Elements that change the timing or the pitch, which the reader does not apply yet: each one is
/// a problem, since the music would play wrong.
const NOT_APPLIED: [&str; 6] = ["mRpt", "mRpt2", "beatRpt", "halfmRpt", "multiRpt", "tupletSpan"];

/// How much louder an accent (`sf`, `sfz`, `fz`) strikes its one note than the level around it.
pub const SFORZANDO_ADD: f32 = 0.25;

/// How far a hairpin with no dynamic at its end moves the level: one step of the letters' scale.
pub const HAIRPIN_STEP: f32 = 0.12;

/// The longest a slashed grace note (an acciaccatura) takes, whatever its written value: a 32nd.
const ACCIACCATURA: Frac = Frac { n: 1, d: 32 };

/// What an articulation does: the share of its written length a note is held (None: unchanged) and
/// what it adds to how hard the note is struck. None for one the player does not play yet.
fn artic_effect(v: &str) -> Option<(Option<f32>, f32)> {
    Some(match v {
        "stacc" => (Some(0.5), 0.0),
        "stacciss" => (Some(0.25), 0.0),
        "spicc" => (Some(0.4), 0.0),
        "ten" => (Some(1.0), 0.0),
        "ten-stacc" => (Some(0.75), 0.0),
        "acc" | "acc-soft" => (None, 0.12),
        "marc" => (None, 0.18),
        "marc-stacc" => (Some(0.5), 0.18),
        "acc-stacc" => (Some(0.5), 0.12),
        _ => return None,
    })
}

/// How a grace note takes its time (`@grace`): an appoggiatura (`acc`) takes a share of the note
/// after it (half; a third of a dotted one); a slashed or plain grace note (`unacc`, `unknown`)
/// is quick, at most a 32nd.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraceKind {
    Acc,
    Unacc,
}

impl GraceKind {
    fn parse(v: &str) -> GraceKind {
        if v.trim() == "acc" {
            GraceKind::Acc
        } else {
            GraceKind::Unacc
        }
    }
}

/// A line over some notes: an octave line (semitones it moves them) or a hairpin (louder or softer).
#[derive(Clone, Copy, Debug, PartialEq)]
enum SpanKind {
    Octave(f32),
    Hairpin { louder: bool },
}

/// A span read: its kind, its staff (None: every staff), where it starts and ends (whole notes).
#[derive(Clone, Copy, Debug)]
struct Span {
    kind: SpanKind,
    staff: Option<u16>,
    start: Frac,
    end: Frac,
}

/// What a span still open waits for: the note or rest with this id, the bar it ends in (counted as
/// played) and the beats into it, or (a hairpin with no end written) the next dynamic.
#[derive(Clone, Debug, PartialEq)]
enum SpanEnd {
    Id(String),
    Bar(u32, Frac),
    NextDynamic,
}

/// A dynamic mark: a level (`p`, `f`) from where it stands, an accent on its one moment (`sf`), or
/// forte then piano (`fp`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum DynKind {
    Level(f32),
    Accent,
    FortePiano,
}

/// A bar or a definition in the order written, with the ending it sits in: the times through it
/// plays (empty: not in an ending) and the most times any ending of its run names.
struct Unit<'a> {
    el: &'a El,
    ending: Vec<u32>,
    ending_max: u32,
}

/// The numbers an ending's `n` (or `label`) names: "1", "1, 2", "1-3", "2." (empty: none).
fn ending_numbers(el: &El) -> Vec<u32> {
    let text = el.attr("n").or_else(|| el.attr("label")).unwrap_or_default();
    let mut out = Vec::new();
    for part in text.split([',', ' ', ';', '.']).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                    out.extend(a.min(b)..=b.max(a).min(a.min(b) + 16));
                }
            }
            None => out.extend(part.trim().parse::<u32>().ok()),
        }
    }
    out
}

/// The order the bars play in (indices into `units`): a repeat's bars again from its start sign (or
/// the last repeat's end, or the beginning), each ending only on the times through it names, a run
/// of endings ending the repeat. A safety limit keeps a malformed file from playing forever.
fn play_order(units: &[Unit]) -> Vec<usize> {
    let sign = |u: &Unit, side: &str, signs: &[&str]| u.el.name == "measure" && u.el.attr(side).is_some_and(|v| signs.contains(&v.trim()));
    let mut out = Vec::new();
    let (mut i, mut start, mut pass, mut in_endings) = (0usize, 0usize, 1u32, false);
    let limit = units.len() * 16 + 64;
    while i < units.len() && out.len() < limit {
        let u = &units[i];
        if !u.ending.is_empty() {
            in_endings = true;
            if !u.ending.contains(&pass) {
                i += 1;
                continue;
            }
        } else if in_endings {
            // Past the run of endings: the repeat is over.
            in_endings = false;
            pass = 1;
            start = i;
        }
        if sign(u, "left", &["rptstart", "rptboth"]) && i != start {
            start = i;
            pass = 1;
        }
        out.push(i);
        if sign(u, "right", &["rptend", "rptboth"]) {
            let times = if u.ending.is_empty() { 2 } else { u.ending_max.max(2) };
            if pass < times {
                pass += 1;
                in_endings = false;
                i = start;
                continue;
            }
            if u.ending.is_empty() {
                pass = 1;
                start = i + 1;
            }
        }
        i += 1;
    }
    out
}

/// Layout and definitions the reader understands as having no sound of their own.
const SILENT: [&str; 8] = ["pb", "sb", "pgHead", "pgFoot", "label", "labelAbbr", "instrGrp", "symbolTable"];

// ---------------------------------------------------------------------------------------------
// The walk: the tree into notes.
// ---------------------------------------------------------------------------------------------

/// A note read in this bar whose pitch is worked out at the bar's end, in time order per staff.
struct Pending {
    idx: usize,
    staff: u16,
    at: Frac,
    order: usize,
    letter: u8,
    oct: i32,
    written: Option<f32>,
    ges: Option<f32>,
    tie_from: Option<usize>,
}

struct Walk {
    out: MeiScore,
    meter: (i64, i64),
    key: i32,
    staff_key: HashMap<u16, i32>,
    staff_trans: HashMap<u16, f32>,
    bar_start: Frac,
    bar_no: u32,
    ids: HashMap<String, usize>,
    /// `@tie` starts waiting for their end: staff, letter, octave, note.
    open_ties: Vec<(u16, u8, i32, usize)>,
    /// `tie` elements: the end note's id to the start note's.
    tie_ends: HashMap<String, String>,
    ties: Vec<(usize, usize)>,
    /// Dynamics: where, which staff (None: all), what kind.
    dynamics: Vec<(Frac, Option<u16>, DynKind)>,
    explicit_vel: Vec<bool>,
    /// Per note: its octave given as sounding (`oct.ges`: an octave line leaves it alone), and what
    /// its articulation adds to how hard it is struck.
    oct_ges: Vec<bool>,
    accent: Vec<f32>,
    pending: Vec<Pending>,
    /// Grace notes read and waiting for the note after them: their indices (a grace chord's several),
    /// written value, kind and `grace.time` (percent of the note after); and a `graceGrp`'s kind.
    graces: Vec<(Vec<usize>, Frac, GraceKind, Option<Frac>)>,
    grace_ctx: Option<GraceKind>,
    /// Octave lines and hairpins: those with both ends, and those waiting for their end.
    spans: Vec<Span>,
    open_spans: Vec<(Span, SpanEnd)>,
    /// The ids of the notes and rests read in the bar being read (an end met in its own bar).
    bar_ids: std::collections::HashSet<String>,
    /// Rests by id: where each starts and how long it lasts (a fermata may stand on one).
    rest_ids: HashMap<String, (u16, Frac, Frac)>,
    rests: Vec<(u16, Frac, Frac)>,
    /// Fermatas: where and how long what they hold is written.
    fermatas: Vec<(Frac, Frac)>,
    /// A measured tremolo being read: its strokes' value; and whether the note being read is one of
    /// its later strokes.
    trem: Option<Frac>,
    trem_copy: bool,
    /// The duration of the last note or rest of each staff and layer, for one written without its
    /// own. reuse: the reader's own state while it reads one file (made new for each read and
    /// dropped after), not a result kept between reads.
    last_dur: HashMap<(u16, u16), Frac>,
}

fn strip_hash(v: &str) -> String {
    v.trim().trim_start_matches('#').to_string()
}

impl Walk {
    fn new() -> Walk {
        Walk {
            out: MeiScore::default(),
            meter: (4, 4),
            key: 0,
            staff_key: HashMap::new(),
            staff_trans: HashMap::new(),
            bar_start: Frac::ZERO,
            bar_no: 0,
            ids: HashMap::new(),
            open_ties: Vec::new(),
            tie_ends: HashMap::new(),
            ties: Vec::new(),
            dynamics: Vec::new(),
            explicit_vel: Vec::new(),
            oct_ges: Vec::new(),
            accent: Vec::new(),
            pending: Vec::new(),
            graces: Vec::new(),
            grace_ctx: None,
            spans: Vec::new(),
            open_spans: Vec::new(),
            bar_ids: Default::default(),
            rest_ids: HashMap::new(),
            rests: Vec::new(),
            fermatas: Vec::new(),
            trem: None,
            trem_copy: false,
            last_dur: HashMap::new(),
        }
    }

    fn skip(&mut self, name: &str) {
        *self.out.skipped.entry(name.to_string()).or_insert(0) += 1;
    }

    fn not_played(&mut self, name: &str) {
        *self.out.not_played.entry(name.to_string()).or_insert(0) += 1;
    }

    fn problem(&mut self, line: u32, what: String) {
        self.out.problems.push(format!("line {line}: {what}"));
    }

    /// An element that is not structure: counted as not played, a problem, silent, or skipped.
    fn other(&mut self, el: &El) {
        let name = el.name.as_str();
        if NOT_PLAYED.contains(&name) {
            self.not_played(name);
        } else if NOT_APPLIED.contains(&name) {
            self.problem(el.line, format!("<{name}> is not applied yet, so the music around it may play wrong (docs/design/score-player.md section 5)"));
        } else if !SILENT.contains(&name) {
            self.skip(name);
        }
    }

    /// The header: the title, the composer and the availability statement.
    fn header(&mut self, head: &El) {
        if let Some(ts) = head.find("titleStmt") {
            if let Some(t) = ts.find("title") {
                self.out.title = t.text();
            }
            if let Some(c) = ts.find("composer") {
                self.out.composer = c.text();
            }
        }
        if self.out.composer.is_empty() {
            if let Some(p) = find_where(head, &|e| e.name == "persName" && e.attr("role") == Some("composer")) {
                self.out.composer = p.text();
            }
        }
        if let Some(a) = head.find("availability") {
            self.out.availability = a.text();
        }
    }

    /// A container of the music (`music`, `body`, `mdiv`, `score`, `section`, `ending`): its bars and
    /// definitions in the order written, each with the ending it sits in (`ending`: the times through
    /// it plays; an ending with no number takes its place in its run).
    fn collect<'a>(&mut self, el: &'a El, ending: &[u32], out: &mut Vec<Unit<'a>>) {
        for k in el.els() {
            match k.name.as_str() {
                "body" | "mdiv" | "score" | "section" => self.collect(k, ending, out),
                "ending" => {
                    let mut ns = ending_numbers(k);
                    if ns.is_empty() {
                        let before = out.iter().rev().take_while(|u| !u.ending.is_empty()).filter_map(|u| u.ending.iter().max()).max().copied().unwrap_or(0);
                        ns.push(before + 1);
                    }
                    self.collect(k, &ns, out);
                }
                "scoreDef" | "staffDef" | "measure" => out.push(Unit { el: k, ending: ending.to_vec(), ending_max: 0 }),
                _ => self.other(k),
            }
        }
    }

    fn score_def(&mut self, el: &El) {
        let key = el.attr("keysig").or_else(|| el.attr("key.sig")).or_else(|| el.els().find(|k| k.name == "keySig").and_then(|k| k.attr("sig")));
        if let Some(k) = key {
            match parse_keysig(k) {
                Some(f) => {
                    self.key = f;
                    self.staff_key.clear();
                }
                None => self.problem(el.line, format!("the key signature {k:?} is not read (a key like \"2s\" or \"3f\")")),
            }
        }
        let meter = meter_of(el, "meter.count", "meter.unit", "meter.sym").or_else(|| el.els().find(|k| k.name == "meterSig").and_then(|m| meter_of(m, "count", "unit", "sym")));
        if let Some(m) = meter {
            self.meter = m;
        }
        for k in el.els() {
            match k.name.as_str() {
                "staffGrp" => self.staff_grp(k),
                "staffDef" => self.staff_def(k),
                "keySig" | "meterSig" => {}
                _ => self.other(k),
            }
        }
    }

    fn staff_grp(&mut self, el: &El) {
        for k in el.els() {
            match k.name.as_str() {
                "staffGrp" => self.staff_grp(k),
                "staffDef" => self.staff_def(k),
                "instrDef" => {}
                _ => self.other(k),
            }
        }
    }

    fn staff_def(&mut self, el: &El) {
        let Some(n) = el.attr("n").and_then(|n| n.trim().parse::<u16>().ok()) else {
            self.problem(el.line, "a <staffDef> without its number (n)".into());
            return;
        };
        let key = el.attr("keysig").or_else(|| el.attr("key.sig")).or_else(|| el.els().find(|k| k.name == "keySig").and_then(|k| k.attr("sig")));
        if let Some(k) = key {
            match parse_keysig(k) {
                Some(f) => {
                    self.staff_key.insert(n, f);
                }
                None => self.problem(el.line, format!("the key signature {k:?} is not read (a key like \"2s\" or \"3f\")")),
            }
        }
        if let Some(m) = meter_of(el, "meter.count", "meter.unit", "meter.sym").or_else(|| el.els().find(|k| k.name == "meterSig").and_then(|m| meter_of(m, "count", "unit", "sym"))) {
            self.meter = m;
        }
        if let Some(t) = el.attr("trans.semi").and_then(|t| t.trim().parse::<f32>().ok()) {
            self.staff_trans.insert(n, t);
        }
        let label = el.attr("label").map(str::to_string).or_else(|| el.els().find(|k| k.name == "label").map(El::text)).unwrap_or_default();
        let program = el.els().find(|k| k.name == "instrDef").and_then(|i| i.attr("midi.instrnum")).and_then(|p| p.trim().parse::<u8>().ok()).filter(|p| *p < 128);
        match self.out.staves.iter_mut().find(|s| s.n == n) {
            Some(s) => {
                if !label.is_empty() {
                    s.label = label;
                }
                if program.is_some() {
                    s.midi_program = program;
                }
            }
            None => self.out.staves.push(MeiStaff { n, label, midi_program: program }),
        }
        for k in el.els() {
            match k.name.as_str() {
                "label" | "labelAbbr" | "instrDef" | "keySig" | "meterSig" | "clef" => {}
                _ => self.other(k),
            }
        }
    }

    /// Where a control event sits: its `tstamp` (beats of the meter's unit, 1 = the bar's start), else
    /// the note its `startid` names, else the bar's start.
    fn control_at(&self, el: &El, bar: Frac) -> Frac {
        if let Some(t) = el.attr("tstamp").and_then(Frac::parse_decimal) {
            let beats = t.sub(Frac::int(1));
            if beats >= Frac::ZERO {
                return bar.add(beats.mul(Frac::new(1, self.meter.1)));
            }
        }
        if let Some(id) = el.attr("startid").map(strip_hash) {
            if let Some(i) = self.ids.get(&id).copied() {
                return self.out.notes[i].at;
            }
            if let Some(&(_, at, _)) = self.rest_ids.get(&id) {
                return at;
            }
        }
        bar
    }

    /// The staff a control event belongs to: its `staff` (the first named), else its start note's.
    fn control_staff(&self, el: &El) -> Option<u16> {
        el.attr("staff").and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse::<u16>().ok()).or_else(|| {
            let id = strip_hash(el.attr("startid")?);
            self.ids.get(&id).map(|&i| self.out.notes[i].staff).or_else(|| self.rest_ids.get(&id).map(|r| r.0))
        })
    }

    /// Where a span (an octave line, a hairpin) ends: now (its end id read in this bar, or a
    /// `tstamp2` inside this bar), later (an id not read yet, a later bar), or not written.
    fn span_end(&self, el: &El, bar: Frac) -> Option<Result<Frac, SpanEnd>> {
        if let Some(id) = el.attr("endid").map(strip_hash) {
            if self.bar_ids.contains(&id) {
                if let Some(&i) = self.ids.get(&id) {
                    return Some(Ok(self.out.notes[i].at));
                }
                if let Some(&(_, at, _)) = self.rest_ids.get(&id) {
                    return Some(Ok(at));
                }
            }
            return Some(Err(SpanEnd::Id(id)));
        }
        let t2 = el.attr("tstamp2")?;
        let (bars, beat) = match t2.split_once("m+") {
            Some((b, x)) => (b.trim().parse::<u32>().unwrap_or(0), x),
            None => (0, t2),
        };
        let beats = Frac::parse_decimal(beat).map(|b| b.sub(Frac::int(1))).filter(|b| *b >= Frac::ZERO).unwrap_or(Frac::ZERO);
        Some(if bars == 0 { Ok(bar.add(beats.mul(Frac::new(1, self.meter.1)))) } else { Err(SpanEnd::Bar(self.bar_no + bars, beats)) })
    }

    /// An octave line or a hairpin (or a "cresc." written as words): kept with its ends to be applied
    /// once the whole piece is read.
    fn span(&mut self, el: &El, kind: SpanKind, bar: Frac, bar_end: Frac) {
        let span = Span { kind, staff: self.control_staff(el), start: self.control_at(el, bar), end: Frac::ZERO };
        match self.span_end(el, bar) {
            Some(Ok(end)) => self.spans.push(Span { end, ..span }),
            Some(Err(wait)) => self.open_spans.push((span, wait)),
            None if matches!(kind, SpanKind::Hairpin { .. }) => self.open_spans.push((span, SpanEnd::NextDynamic)),
            None => {
                self.problem(el.line, format!("an <{}> without its end (endid or tstamp2): applied to the end of its bar", el.name));
                self.spans.push(Span { end: bar_end, ..span });
            }
        }
    }

    /// An octave line: `dis` 8, 15 or 22 (one, two or three octaves), above or below.
    fn octave(&mut self, el: &El, bar: Frac, bar_end: Frac) {
        let semis = match el.attr("dis").map(str::trim) {
            Some("8") => 12.0,
            Some("15") => 24.0,
            Some("22") => 36.0,
            other => {
                self.problem(el.line, format!("an octave line's dis {other:?} is not 8, 15 or 22: not applied"));
                return;
            }
        };
        let sign = if el.attr("dis.place").map(str::trim) == Some("below") { -1.0 } else { 1.0 };
        self.span(el, SpanKind::Octave(semis * sign), bar, bar_end);
    }

    /// A fermata: what it holds is the note or rest it names, else the longest one starting where it
    /// stands on its staff, else one beat.
    fn fermata(&mut self, el: &El, bar: Frac) {
        let at = self.control_at(el, bar);
        let staff = self.control_staff(el);
        let named = el.attr("startid").map(strip_hash).and_then(|id| self.ids.get(&id).map(|&i| self.out.notes[i].dur).or_else(|| self.rest_ids.get(&id).map(|r| r.2)));
        let found = || {
            let on = |s: u16| staff.is_none_or(|x| x == s);
            let n = self.out.notes.iter().filter(|n| n.at == at && on(n.staff)).map(|n| n.dur);
            let r = self.rests.iter().filter(|r| r.1 == at && on(r.0)).map(|r| r.2);
            n.chain(r).max()
        };
        let len = named.or_else(found).unwrap_or(Frac::new(1, self.meter.1));
        self.fermatas.push((at, len));
    }

    fn measure(&mut self, m: &El) {
        let start = self.bar_start;
        let meter_len = Frac::new(self.meter.0, self.meter.1);
        self.bar_ids.clear();
        // Spans that end in this bar, by its number and a beat.
        let mut k = 0;
        while k < self.open_spans.len() {
            match self.open_spans[k].1 {
                SpanEnd::Bar(n, beats) if n == self.bar_no => {
                    let (s, _) = self.open_spans.remove(k);
                    self.spans.push(Span { end: start.add(beats.mul(Frac::new(1, self.meter.1))), ..s });
                }
                _ => k += 1,
            }
        }
        // The bar's tie elements first, so a tie that ends in this bar is known when its note is read.
        for k in m.els().filter(|k| k.name == "tie") {
            match (k.attr("startid"), k.attr("endid")) {
                (Some(s), Some(e)) => {
                    self.tie_ends.insert(strip_hash(e), strip_hash(s));
                }
                _ => self.problem(k.line, "a <tie> without both its startid and its endid".into()),
            }
        }
        let mut longest = Frac::ZERO;
        let mut controls: Vec<&El> = Vec::new();
        for k in m.els() {
            match k.name.as_str() {
                "staff" => {
                    let staff = k.attr("n").and_then(|n| n.trim().parse::<u16>().ok()).unwrap_or(1);
                    let mut li = 0u16;
                    for l in k.els() {
                        if l.name != "layer" {
                            self.other(l);
                            continue;
                        }
                        li += 1;
                        let layer = l.attr("n").and_then(|n| n.trim().parse::<u16>().ok()).unwrap_or(li);
                        let mut t = start;
                        self.events(l, staff, layer, &mut t, Frac::int(1), meter_len);
                        if !self.graces.is_empty() {
                            // Grace notes with nothing after them in their layer keep their written place.
                            self.graces.clear();
                            self.problem(l.line, "grace notes with no note after them in their layer: played at their written length".into());
                        }
                        longest = longest.max(t.sub(start));
                    }
                }
                "tie" => {}
                "tempo" | "dynam" | "octave" | "hairpin" | "fermata" => controls.push(k),
                _ => self.other(k),
            }
        }
        self.resolve_pitches();
        let len = if longest > Frac::ZERO { longest } else { meter_len };
        for c in controls {
            let at = self.control_at(c, start);
            match c.name.as_str() {
                "tempo" => self.tempo(c, at),
                "dynam" => self.dynam(c, at, start, start.add(len)),
                "octave" => self.octave(c, start, start.add(len)),
                "hairpin" => match c.attr("form").map(str::trim) {
                    Some(f @ ("cres" | "dim")) => self.span(c, SpanKind::Hairpin { louder: f == "cres" }, start, start.add(len)),
                    other => self.problem(c.line, format!("a hairpin's form {other:?} is not cres or dim: not applied")),
                },
                _ => self.fermata(c, start),
            }
        }
        self.out.bars.push(MeiBar { at: start, n: m.attr("n").unwrap_or_default().to_string(), len, meter: self.meter });
        self.bar_start = start.add(len);
        self.bar_no += 1;
    }

    fn tempo(&mut self, el: &El, at: Frac) {
        let num = |k: &str| el.attr(k).and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite() && *v > 0.0);
        let qpm = if let Some(b) = num("midi.bpm") {
            Some(b)
        } else if let Some(mspb) = num("midi.mspb") {
            Some(60_000_000.0 / mspb)
        } else if let (Some(mm), Some(unit)) = (num("mm"), num("mm.unit")) {
            // `mm` beats of the `mm.unit` note (dots lengthen it): in quarter notes.
            let dots = el.attr("mm.dots").and_then(|d| d.trim().parse::<i32>().ok()).unwrap_or(0).clamp(0, 4);
            Some(mm * (4.0 / unit) * (2.0 - 0.5f64.powi(dots)))
        } else {
            None
        };
        match qpm {
            Some(q) if (5.0..=1000.0).contains(&q) => self.out.tempos.push(MeiTempo { at, qpm: q }),
            Some(q) => self.problem(el.line, format!("a tempo of {q:.1} quarter notes a minute is not played (5 to 1000)")),
            // Words alone ("Allegro"): no number to play.
            None => self.not_played("tempo (words without a number)"),
        }
    }

    /// A dynamic: a level (`ppp` to `fff`), an accent on one moment (`sf`, `sfz`, `fz` ...), forte
    /// then piano (`fp`), or "cresc." / "dim." as words (a hairpin to its end, else to the next
    /// dynamic).
    fn dynam(&mut self, el: &El, at: Frac, bar: Frac, bar_end: Frac) {
        let staff = el.attr("staff").and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse::<u16>().ok());
        let words = el.text();
        let w = words.trim();
        if let Some(v) = dynamic_vel(w) {
            self.dynamics.push((at, staff, DynKind::Level(v)));
        } else if matches!(w, "sf" | "sfz" | "sffz" | "fz" | "ffz" | "rf" | "rfz") {
            self.dynamics.push((at, staff, DynKind::Accent));
        } else if matches!(w, "fp" | "sfp") {
            self.dynamics.push((at, staff, DynKind::FortePiano));
        } else if w.starts_with("cresc") || w.starts_with("dim") || w.starts_with("decresc") {
            self.span(el, SpanKind::Hairpin { louder: w.starts_with("cresc") }, bar, bar_end);
        } else {
            self.not_played("dynam (words other than ppp to fff, sf, fp, cresc. and dim.)");
        }
    }

    /// The grace notes waiting before a note (or a rest) at `t` lasting `dur`: each placed from `t`
    /// on, taking its time from it (score-player.md section 5, rung 2). Where the note after them
    /// starts.
    fn place_graces(&mut self, t: Frac, dur: Frac) -> Frac {
        let graces = std::mem::take(&mut self.graces);
        let Some(first) = graces.first() else { return t };
        let (kind, percent) = (first.2, graces.iter().find_map(|g| g.3));
        let weight = |w: Frac| if kind == GraceKind::Unacc { w.min(ACCIACCATURA) } else { w };
        let sum = graces.iter().fold(Frac::ZERO, |a, g| a.add(weight(g.1)));
        let half = dur.mul(Frac::new(1, 2));
        let total = match (percent, kind) {
            (Some(p), _) => dur.mul(p).min(dur.mul(Frac::new(9, 10))),
            // One appoggiatura takes half the note after it, a third of a dotted one (two of its three).
            (None, GraceKind::Acc) if graces.len() == 1 => {
                if dur.n == 3 {
                    dur.mul(Frac::new(2, 3))
                } else {
                    half
                }
            }
            (None, _) => sum.min(half),
        };
        let mut at = t;
        for (idxs, w, _, _) in &graces {
            let d = if sum.is_zero() { Frac::ZERO } else { total.mul(weight(*w)).div(sum) };
            for &i in idxs {
                self.out.notes[i].at = at;
                self.out.notes[i].dur = d;
                if let Some(p) = self.pending.iter_mut().find(|p| p.idx == i) {
                    p.at = at;
                }
            }
            at = at.add(d);
        }
        t.add(total)
    }

    /// How a note of `len` is played inside a measured tremolo: so many strokes of its unit (one
    /// stroke of its own length outside one, or when the unit does not divide it).
    fn strokes(&self, len: Frac) -> (i64, Frac) {
        match self.trem.map(|u| (u, len.div(u))) {
            Some((u, n)) if n.d == 1 && n.n > 1 && n.n <= 256 => (n.n, u),
            _ => (1, len),
        }
    }

    /// A grace note (or each note of a grace chord) read: placed once the note after it is known.
    fn grace(&mut self, el: &El, kind: GraceKind, staff: u16, layer: u16, t: Frac) {
        let notes: Vec<&El> = if el.name == "chord" { el.els().filter(|n| n.name == "note").collect() } else { vec![el] };
        let w = dur_of(el).or_else(|| notes.first().and_then(|n| dur_of(n))).unwrap_or(Frac::new(1, 8));
        let percent = el.attr("grace.time").and_then(Frac::parse_decimal).map(|p| p.mul(Frac::new(1, 100)));
        let chord = (el.name == "chord").then_some(el);
        let idxs: Vec<usize> = notes.iter().filter_map(|n| self.note(n, chord, staff, layer, t, w)).collect();
        for &i in &idxs {
            self.out.notes[i].grace = true;
        }
        if !idxs.is_empty() {
            self.graces.push((idxs, w, kind, percent));
        }
    }

    /// Spans waiting for the note or rest `id` as their end: they end at `at`.
    fn close_spans(&mut self, id: &str, at: Frac) {
        let mut k = 0;
        while k < self.open_spans.len() {
            if matches!(&self.open_spans[k].1, SpanEnd::Id(x) if x == id) {
                let (s, _) = self.open_spans.remove(k);
                self.spans.push(Span { end: at, ..s });
            } else {
                k += 1;
            }
        }
    }

    /// A rest (or a whole-bar rest) at `t` lasting `dur`: kept by id for fermatas and spans, and its
    /// own fermata held.
    fn rest(&mut self, el: &El, staff: u16, t: Frac, dur: Frac) {
        self.rests.push((staff, t, dur));
        if let Some(id) = el.attr("xml:id") {
            self.rest_ids.insert(id.to_string(), (staff, t, dur));
            self.bar_ids.insert(id.to_string());
            self.close_spans(id, t);
        }
        if el.attr("fermata").is_some() {
            self.fermatas.push((t, dur));
        }
    }

    /// A layer's events (or a beam's or a tuplet's), from time `t` on, durations times `scale`.
    fn events(&mut self, el: &El, staff: u16, layer: u16, t: &mut Frac, scale: Frac, meter_len: Frac) {
        for k in el.els() {
            match k.name.as_str() {
                "note" => {
                    // A grace note takes no time of its own: it takes it from the note after it.
                    if let Some(g) = k.attr("grace").map(GraceKind::parse).or(self.grace_ctx) {
                        self.grace(k, g, staff, layer, *t);
                        continue;
                    }
                    let Some(dur) = self.duration(k, staff, layer) else { continue };
                    let dur = dur.mul(scale);
                    let at = self.place_graces(*t, dur);
                    let (strokes, each) = self.strokes(t.add(dur).sub(at));
                    for n in 0..strokes {
                        self.trem_copy = n > 0;
                        if let Some(i) = self.note(k, None, staff, layer, at.add(each.mul(Frac::int(n))), each) {
                            if n == 0 {
                                self.lyrics(k, i);
                            }
                        }
                    }
                    self.trem_copy = false;
                    *t = t.add(dur);
                }
                "rest" | "space" => {
                    if let Some(dur) = self.duration(k, staff, layer) {
                        let dur = dur.mul(scale);
                        self.place_graces(*t, dur);
                        if k.name == "rest" {
                            self.rest(k, staff, *t, dur);
                        }
                        *t = t.add(dur);
                    }
                }
                "mRest" => {
                    self.rest(k, staff, *t, meter_len);
                    *t = t.add(meter_len);
                }
                "multiRest" => {
                    let bars = k.attr("num").and_then(|n| n.trim().parse::<i64>().ok()).unwrap_or(1).max(1);
                    *t = t.add(meter_len.mul(Frac::int(bars)));
                }
                "graceGrp" => {
                    let outer = self.grace_ctx;
                    self.grace_ctx = Some(k.attr("grace").map(GraceKind::parse).unwrap_or(GraceKind::Unacc));
                    self.events(k, staff, layer, t, scale, meter_len);
                    self.grace_ctx = outer;
                }
                "chord" => {
                    if let Some(g) = k.attr("grace").map(GraceKind::parse).or(self.grace_ctx) {
                        self.grace(k, g, staff, layer, *t);
                        continue;
                    }
                    // The chord's duration (else its first note's), its notes struck together.
                    let dur = dur_of(k).or_else(|| k.els().find(|n| n.name == "note").and_then(dur_of));
                    let dur = match dur {
                        Some(d) => {
                            self.last_dur.insert((staff, layer), d);
                            d
                        }
                        None => match self.last_dur.get(&(staff, layer)).copied() {
                            Some(d) => {
                                self.problem(k.line, "a chord with no duration: the one before it was used".into());
                                d
                            }
                            None => {
                                self.problem(k.line, "a chord with no duration and none before it: not played".into());
                                continue;
                            }
                        },
                    }
                    .mul(scale);
                    let at = self.place_graces(*t, dur);
                    let (strokes, each) = self.strokes(t.add(dur).sub(at));
                    let mut first = None;
                    for stroke in 0..strokes {
                        self.trem_copy = stroke > 0;
                        let when = at.add(each.mul(Frac::int(stroke)));
                        for n in k.els() {
                            match n.name.as_str() {
                                "note" => {
                                    if let Some(i) = self.note(n, Some(k), staff, layer, when, each) {
                                        if stroke == 0 {
                                            self.lyrics(n, i);
                                            first.get_or_insert(i);
                                        }
                                    }
                                }
                                "artic" | "verse" | "syl" => {}
                                _ if stroke == 0 => self.other(n),
                                _ => {}
                            }
                        }
                    }
                    self.trem_copy = false;
                    if let Some(i) = first {
                        self.lyrics(k, i);
                    }
                    *t = t.add(dur);
                }
                "beam" => self.events(k, staff, layer, t, scale, meter_len),
                "tuplet" => {
                    // `num` notes in the time of `numbase`: each lasts numbase/num of its value.
                    let num = k.attr("num").and_then(|n| n.trim().parse::<i64>().ok()).filter(|n| *n > 0);
                    let base = k.attr("numbase").and_then(|n| n.trim().parse::<i64>().ok()).filter(|n| *n > 0);
                    match (num, base) {
                        (Some(n), Some(b)) => self.events(k, staff, layer, t, scale.mul(Frac::new(b, n)), meter_len),
                        _ => {
                            self.problem(k.line, "a <tuplet> without num and numbase: its notes play at their written values".into());
                            self.events(k, staff, layer, t, scale, meter_len);
                        }
                    }
                }
                // A tremolo: its notes play plainly (the tremolo itself later).
                // A measured tremolo (`unitdur`: the strokes' value) plays its strokes; a fingered one
                // (or one with no unit) plays its notes plainly, the tremolo itself later.
                "bTrem" if k.attr("unitdur").and_then(base_dur).is_some() => {
                    let outer = self.trem;
                    self.trem = k.attr("unitdur").and_then(base_dur);
                    self.events(k, staff, layer, t, scale, meter_len);
                    self.trem = outer;
                }
                "bTrem" | "fTrem" => {
                    self.not_played(&k.name);
                    self.events(k, staff, layer, t, scale, meter_len);
                }
                "keySig" | "meterSig" => self.problem(k.line, format!("a <{}> inside a bar is not applied yet", k.name)),
                _ => self.other(k),
            }
        }
    }

    /// A note's or a rest's duration; one with none takes the one before it in its layer (a problem).
    fn duration(&mut self, el: &El, staff: u16, layer: u16) -> Option<Frac> {
        if let Some(d) = dur_of(el) {
            self.last_dur.insert((staff, layer), d);
            return Some(d);
        }
        match self.last_dur.get(&(staff, layer)).copied() {
            Some(d) => {
                self.problem(el.line, format!("a <{}> with no duration: the one before it was used", el.name));
                Some(d)
            }
            None => {
                self.problem(el.line, format!("a <{}> with no duration and none before it: not played", el.name));
                None
            }
        }
    }

    /// A note at `at` lasting `dur` (its pitch worked out at the bar's end). Its index, or None.
    fn note(&mut self, el: &El, chord: Option<&El>, staff: u16, layer: u16, at: Frac, dur: Frac) -> Option<usize> {
        let Some(letter) = el.attr("pname.ges").or_else(|| el.attr("pname")).and_then(letter_of) else {
            self.problem(el.line, "a note without a pitch name (pname): not played".into());
            return None;
        };
        let Some(oct) = el.attr("oct.ges").or_else(|| el.attr("oct")).and_then(|o| o.trim().parse::<i32>().ok()) else {
            self.problem(el.line, "a note without an octave (oct): not played".into());
            return None;
        };
        let accid_el = el.els().find(|k| k.name == "accid");
        let written = el.attr("accid").or_else(|| accid_el.and_then(|a| a.attr("accid"))).and_then(accid_alter);
        let ges = el.attr("accid.ges").or_else(|| accid_el.and_then(|a| a.attr("accid.ges"))).and_then(accid_alter);
        let vel = el.attr("vel").and_then(|v| v.trim().parse::<f32>().ok()).map(|v| (v / 127.0).clamp(0.0, 1.0));
        // A tremolo's later strokes are the same note again: no id of their own, no ties.
        let id = if self.trem_copy { String::new() } else { el.attr("xml:id").unwrap_or_default().to_string() };
        let idx = self.out.notes.len();
        self.out.notes.push(MeiNote { id: id.clone(), staff, layer, at, dur, midi: f32::NAN, vel: vel.unwrap_or(DEFAULT_VEL), bar: self.bar_no, hold: None, grace: false });
        self.explicit_vel.push(vel.is_some());
        self.oct_ges.push(el.attr("oct.ges").is_some());
        // Its articulations (on it or its chord, as an attribute or `artic` elements): how long it
        // is held and how hard it is struck.
        let mut accent = 0.0;
        let artics = |e: &El| -> Vec<String> {
            let own = e.attr("artic").map(|a| a.split_whitespace().map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
            own.into_iter().chain(e.els().filter(|k| k.name == "artic").flat_map(|k| k.attr("artic").unwrap_or_default().split_whitespace().map(str::to_string).collect::<Vec<_>>())).collect()
        };
        for a in artics(el).into_iter().chain(chord.map(artics).unwrap_or_default()) {
            match artic_effect(&a) {
                Some((hold, add)) => {
                    if hold.is_some() {
                        self.out.notes[idx].hold = hold;
                    }
                    accent += add;
                }
                None => self.not_played(&format!("artic {a}")),
            }
        }
        self.accent.push(accent);
        if el.attr("fermata").or_else(|| chord.and_then(|c| c.attr("fermata"))).is_some() {
            self.fermatas.push((at, dur));
        }
        if !id.is_empty() {
            self.ids.insert(id.clone(), idx);
            self.bar_ids.insert(id.clone());
            self.close_spans(&id, at);
        }
        // Ties: the attribute (i, m, t) on the note or its chord, and the tie elements by id.
        let mut tie_from = None;
        let tie = if self.trem_copy { "" } else { el.attr("tie").or_else(|| chord.and_then(|c| c.attr("tie"))).unwrap_or_default() };
        if tie.contains('t') || tie.contains('m') {
            match self.open_ties.iter().position(|&(s, l, o, _)| s == staff && l == letter && o == oct) {
                Some(p) => {
                    let (_, _, _, from) = self.open_ties.remove(p);
                    tie_from = Some(from);
                    self.ties.push((from, idx));
                }
                None => self.problem(el.line, "a tie's end with no tie started before it on the same pitch".into()),
            }
        }
        if let Some(start) = self.tie_ends.remove(&id) {
            match self.ids.get(&start).copied() {
                Some(from) if from != idx => {
                    tie_from = Some(from);
                    self.ties.push((from, idx));
                }
                _ => self.problem(el.line, format!("a <tie> from {start:?}, a note not read before its end")),
            }
        }
        if tie.contains('i') || tie.contains('m') {
            self.open_ties.push((staff, letter, oct, idx));
        }
        for k in el.els() {
            match k.name.as_str() {
                "accid" | "dot" | "verse" | "syl" | "artic" => {}
                _ => self.other(k),
            }
        }
        self.pending.push(Pending { idx, staff, at, order: self.pending.len(), letter, oct, written, ges, tie_from });
        Some(idx)
    }

    /// The lyrics written under a note (or a chord): `verse` > `syl`, a `syl` directly inside, or
    /// the `syl` attribute.
    fn lyrics(&mut self, el: &El, note: usize) {
        let note_id = el.attr("xml:id").map(str::to_string).unwrap_or_else(|| self.out.notes[note].id.clone());
        let add = |out: &mut Vec<MeiSyllable>, verse: u16, s: &El| {
            out.push(MeiSyllable { note_id: note_id.clone(), note, verse, text: s.text(), wordpos: WordPos::parse(s.attr("wordpos")), con: s.attr("con").and_then(|c| c.trim().chars().next()) });
        };
        for k in el.els() {
            match k.name.as_str() {
                "verse" => {
                    let verse = k.attr("n").and_then(|n| n.trim().parse::<u16>().ok()).unwrap_or(1);
                    for s in k.els() {
                        if s.name == "syl" {
                            add(&mut self.out.syllables, verse, s);
                        }
                    }
                }
                "syl" => add(&mut self.out.syllables, 1, k),
                _ => {}
            }
        }
        if let Some(text) = el.attr("syl") {
            self.out.syllables.push(MeiSyllable { note_id: note_id.clone(), note, verse: 1, text: text.trim().to_string(), wordpos: WordPos::Single, con: None });
        }
    }

    /// The bar's pitches, staff by staff in time order: the bar's accidentals hold to its end.
    fn resolve_pitches(&mut self) {
        let mut pend = std::mem::take(&mut self.pending);
        pend.sort_by(|a, b| (a.staff, a.at, a.order).cmp(&(b.staff, b.at, b.order)));
        let mut carry: HashMap<(u16, u8, i32), f32> = HashMap::new();
        for p in &pend {
            let key = self.staff_key.get(&p.staff).copied().unwrap_or(self.key);
            let trans = self.staff_trans.get(&p.staff).copied().unwrap_or(0.0);
            let natural = (12 * (p.oct + 1) + PC[p.letter as usize]) as f32 + trans;
            let tied = p.tie_from.map(|i| self.out.notes[i].midi).filter(|m| m.is_finite());
            let midi = if let Some(g) = p.ges {
                natural + g
            } else if let Some(w) = p.written {
                carry.insert((p.staff, p.letter, p.oct), w);
                natural + w
            } else if let Some(m) = tied {
                m
            } else if let Some(&c) = carry.get(&(p.staff, p.letter, p.oct)) {
                natural + c
            } else {
                natural + key_alter(key, p.letter)
            };
            self.out.notes[p.idx].midi = midi;
        }
    }

    /// After the last bar: join the ties, apply the dynamics, drop the joined notes, sort.
    fn finish(&mut self) {
        self.resolve_pitches();
        // Spans still open: an end never met plays to the end; a hairpin with no end written runs to
        // the next dynamic on its staff, else two bars.
        let end = self.bar_start;
        for (s, wait) in std::mem::take(&mut self.open_spans) {
            let to = match wait {
                SpanEnd::NextDynamic => {
                    let next = self.dynamics.iter().filter(|(at, st, k)| *at > s.start && matches!(k, DynKind::Level(_)) && (st.is_none() || s.staff.is_none() || *st == s.staff)).map(|d| d.0).min();
                    next.unwrap_or_else(|| {
                        let bar = self.out.bars.iter().rev().find(|b| b.at <= s.start).map(|b| b.len).unwrap_or(Frac::int(1));
                        s.start.add(bar.mul(Frac::int(2))).min(end)
                    })
                }
                other => {
                    self.out.problems.push(format!("a line from {:.3} whole notes waits for {other:?}, which never came: applied to the end", s.start.to_f64()));
                    end
                }
            };
            self.spans.push(Span { end: to, ..s });
        }
        // Octave lines move the notes under them (ends included), unless a note gives its sounding
        // octave itself.
        for s in self.spans.iter().filter(|s| matches!(s.kind, SpanKind::Octave(_))) {
            let SpanKind::Octave(semis) = s.kind else { continue };
            for (i, note) in self.out.notes.iter_mut().enumerate() {
                if !self.oct_ges[i] && s.staff.is_none_or(|x| x == note.staff) && note.at >= s.start && note.at <= s.end {
                    note.midi += semis;
                }
            }
        }
        let n = self.out.notes.len();
        let mut root: Vec<usize> = (0..n).collect();
        let mut removed = vec![false; n];
        let mut ties = std::mem::take(&mut self.ties);
        ties.sort_by(|a, b| self.out.notes[a.0].at.cmp(&self.out.notes[b.0].at));
        for (s, e) in ties {
            let mut r = s;
            while root[r] != r {
                r = root[r];
            }
            if removed[e] || r == e {
                continue;
            }
            let (rn, en) = (&self.out.notes[r], &self.out.notes[e]);
            if (rn.midi - en.midi).abs() > 0.01 {
                let what = format!("a tie joins two different pitches (notes {:?} and {:?}): they play as two notes", rn.id, en.id);
                self.out.problems.push(what);
                continue;
            }
            if en.at != rn.at.add(rn.dur) {
                let what = format!("a tie joins notes that do not follow one another ({:?} and {:?}): they play as two notes", rn.id, en.id);
                self.out.problems.push(what);
                continue;
            }
            let end = en.at.add(en.dur);
            let at = rn.at;
            self.out.notes[r].dur = end.sub(at);
            removed[e] = true;
            root[e] = r;
        }
        for (_, _, _, i) in std::mem::take(&mut self.open_ties) {
            let id = self.out.notes[i].id.clone();
            self.out.problems.push(format!("the tie from note {id:?} has no end: it plays its own length"));
        }
        for (end, start) in std::mem::take(&mut self.tie_ends) {
            self.out.problems.push(format!("a <tie> from {start:?} ends at {end:?}, a note that was never read"));
        }
        self.dynamics_and_holds();
        // Drop the joined notes, sort what sounds, and point each syllable at its sounding note.
        let mut order: Vec<usize> = (0..n).filter(|&i| !removed[i]).collect();
        order.sort_by(|&a, &b| {
            let (x, y) = (&self.out.notes[a], &self.out.notes[b]);
            (x.at, x.staff).cmp(&(y.at, y.staff)).then(x.midi.total_cmp(&y.midi))
        });
        let mut new_index = vec![usize::MAX; n];
        for (k, &i) in order.iter().enumerate() {
            new_index[i] = k;
        }
        let notes = std::mem::take(&mut self.out.notes);
        self.out.notes = order.iter().map(|&i| notes[i].clone()).collect();
        for s in self.out.syllables.iter_mut() {
            let mut r = s.note;
            while root[r] != r {
                r = root[r];
            }
            s.note = new_index[r];
        }
        let notes = &self.out.notes;
        self.out.syllables.sort_by(|a, b| notes[a.note].at.cmp(&notes[b.note].at).then(a.verse.cmp(&b.verse)));
        self.out.end = self.bar_start;
    }

    /// The loudness of every note without its own velocity: the level marked last at or before it on
    /// its staff (an `fp`'s piano from just after its note), moved by the hairpins it sits under (from
    /// the level where each starts to the dynamic at its end, else one step on, which then holds),
    /// then its accents (`sf`, its articulation). And the fermatas, overlapping ones joined.
    fn dynamics_and_holds(&mut self) {
        // (where, from just after it, staff, level)
        let mut levels: Vec<(Frac, bool, Option<u16>, f32)> = Vec::new();
        for &(at, staff, kind) in &self.dynamics {
            match kind {
                DynKind::Level(v) => levels.push((at, false, staff, v)),
                DynKind::FortePiano => levels.push((at, true, staff, 0.42)),
                DynKind::Accent => {}
            }
        }
        let on = |a: Option<u16>, b: Option<u16>| a.is_none() || b.is_none() || a == b;
        let level_at = |levels: &[(Frac, bool, Option<u16>, f32)], at: Frac, staff: Option<u16>| {
            levels.iter().filter(|l| (l.0 < at || (l.0 == at && !l.1)) && on(l.2, staff)).max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1))).map(|l| l.3).unwrap_or(DEFAULT_VEL)
        };
        // Each hairpin's two levels; one with no dynamic at its end sets the level it reaches there.
        let mut ramps = Vec::new();
        for s in &self.spans {
            let SpanKind::Hairpin { louder } = s.kind else { continue };
            if s.end <= s.start {
                continue;
            }
            let v0 = level_at(&levels, s.start, s.staff);
            let at_end = levels.iter().find(|l| l.0 == s.end && !l.1 && on(l.2, s.staff)).map(|l| l.3);
            let v1 = at_end.unwrap_or_else(|| (v0 + if louder { HAIRPIN_STEP } else { -HAIRPIN_STEP }).clamp(0.1, 1.0));
            ramps.push((*s, v0, v1, at_end.is_none()));
        }
        for (s, _, v1, synthetic) in &ramps {
            if *synthetic {
                levels.push((s.end, false, s.staff, *v1));
            }
        }
        for (i, note) in self.out.notes.iter_mut().enumerate() {
            if self.explicit_vel.get(i).copied().unwrap_or(false) {
                continue;
            }
            note.vel = level_at(&levels, note.at, Some(note.staff));
            for (s, v0, v1, _) in &ramps {
                if note.at >= s.start && note.at < s.end && on(s.staff, Some(note.staff)) {
                    let f = note.at.sub(s.start).div(s.end.sub(s.start)).to_f64() as f32;
                    note.vel = v0 + (v1 - v0) * f;
                }
            }
            for &(at, staff, kind) in &self.dynamics {
                if at == note.at && on(staff, Some(note.staff)) {
                    match kind {
                        DynKind::Accent => note.vel = (note.vel + SFORZANDO_ADD).min(1.0),
                        DynKind::FortePiano => note.vel = dynamic_vel("f").unwrap_or(0.8),
                        DynKind::Level(_) => {}
                    }
                }
            }
            note.vel = (note.vel + self.accent.get(i).copied().unwrap_or(0.0)).min(1.0);
        }
        // The fermatas, by start, overlapping ones joined.
        let mut holds = std::mem::take(&mut self.fermatas);
        holds.retain(|h| h.1 > Frac::ZERO);
        holds.sort();
        let mut joined: Vec<(Frac, Frac)> = Vec::new();
        for (at, len) in holds {
            match joined.last_mut() {
                Some(last) if at <= last.0.add(last.1) => {
                    let end = last.0.add(last.1).max(at.add(len));
                    last.1 = end.sub(last.0);
                }
                _ => joined.push((at, len)),
            }
        }
        self.out.fermatas = joined;
    }
}

/// The first element at any depth that `pred` accepts.
fn find_where<'a>(el: &'a El, pred: &dyn Fn(&El) -> bool) -> Option<&'a El> {
    for k in el.els() {
        if pred(k) {
            return Some(k);
        }
        if let Some(x) = find_where(k, pred) {
            return Some(x);
        }
    }
    None
}
