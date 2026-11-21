//! THE MEI WRITER (docs/design/music-page.md sections 12 and 15a, rung 2): a piece in ticks
//! ([`Notated`]: a MIDI file read, or a composed piece as the piano roll wrote it down) notated as MEI
//! 5.1, the open format the written pieces already use, so the crate's own reader (crate::mei) plays it
//! and MuseScore, Verovio and other open tools open it. Our own writer beside our own reader.
//!
//! How notes in ticks become notes on a staff:
//! - **Bars** follow the meters (a meter change inside a bar ends that bar there, its own short meter).
//! - **Voices**: each part is one staff; notes that overlap go to further layers (a note sounding while
//!   the layer's last is still held starts a new one), notes struck and let go together are a chord.
//! - **Rhythm**: each note's start and end are snapped to a 48th of a quarter. Each quarter of a bar is
//!   then written straight (in 64ths and longer, dotted where it fits, tied across beats as needed) or,
//!   when its notes fall on a triplet grid that a straight one would miss (a swung eighth, a triplet),
//!   as a triplet (`tuplet` 3 in the time of 2); anything finer is moved to the nearest 64th. A note
//!   longer than its bar is tied over the bar line.
//! - **Pitch**: spelled in the key (sharps in a sharp key or C, flats in a flat one), the written
//!   accidental where a musician needs one (by the bar's earlier notes, in time order across the
//!   staff's layers) and the sounding one on every note (`accid.ges`), so no reader has to guess.
//! - **Loudness**: each note's MIDI velocity (`vel`).
//! - **Tempo**: a `tempo` with `midi.bpm` wherever it changes; markers as `dir`, chord symbols as `harm`.
//!
//! On the game thread or a test (a piece is kilobytes), never the audio thread.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::smf::{Notated, PPQ};

const Q: u64 = PPQ as u64;
/// Every start and end is snapped to this first (a 48th of a quarter).
const SNAP: u64 = Q / 48;
/// The straight grid (a 64th note) and the triplet grid (a triplet 32nd, as heard).
const DUPLE: u64 = Q / 16;
const TRIP: u64 = Q / 12;

/// A chord or a single note: struck together, let go together.
#[derive(Clone, Debug)]
struct Group {
    s: u64,
    e: u64,
    keys: Vec<(u8, u8)>,
}

/// A piece of a layer within one bar: a rest, or a group (by index) and whether it is tied in from
/// before and on past its end.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fill {
    Rest,
    Notes { g: usize, tie_in: bool, tie_out: bool },
}

#[derive(Clone, Copy, Debug)]
struct Seg {
    s: u64,
    e: u64,
    fill: Fill,
}

/// A bar: its start, its length and its meter.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bar {
    s: u64,
    len: u64,
    count: u8,
    unit: u8,
}

/// The bars of a piece from its meters, to its last note.
fn bars_of(n: &Notated) -> Vec<Bar> {
    let end = n.end_tick().max(1);
    let mut meters = n.meters.clone();
    if meters.first().is_none_or(|m| m.0 > 0) {
        meters.insert(0, (0, 4, 4));
    }
    let mut bars = Vec::new();
    let mut at = 0u64;
    let mut mi = 0usize;
    while at < end && bars.len() < 100_000 {
        while mi + 1 < meters.len() && meters[mi + 1].0 <= at {
            mi += 1;
        }
        let (_, count, unit) = meters[mi];
        let full = count.max(1) as u64 * 4 * Q / unit.max(1) as u64;
        let next_change = meters.get(mi + 1).map(|m| m.0).unwrap_or(u64::MAX);
        let len = if at + full > next_change { next_change - at } else { full.max(1) };
        let (c, u) = if len == full { (count, unit) } else { short_meter(len) };
        bars.push(Bar { s: at, len, count: c, unit: u });
        at += len;
    }
    bars
}

/// A bar of `len` ticks as a meter: in quarters, eighths, sixteenths ... until it is whole.
fn short_meter(len: u64) -> (u8, u8) {
    let mut unit = 4u64;
    while unit <= 64 {
        let one = 4 * Q / unit;
        if len % one == 0 && len / one <= 255 {
            return ((len / one).max(1) as u8, unit as u8);
        }
        unit *= 2;
    }
    (((len * 64) / (4 * Q)).clamp(1, 255) as u8, 64)
}

/// Snap a tick to the nearest multiple of `g`.
fn snap(t: u64, g: u64) -> u64 {
    (t + g / 2) / g * g
}

/// A part's notes as layers of groups over the whole piece: snapped, chords joined, overlaps in further
/// layers.
fn layers_of(notes: &[crate::smf::PNote]) -> (Vec<Group>, Vec<Vec<usize>>) {
    let mut ns: Vec<(u64, u64, u8, u8)> = notes
        .iter()
        .map(|n| {
            let s = snap(n.tick, SNAP);
            let e = snap(n.tick + n.len.max(1), SNAP).max(s + SNAP);
            (s, e, n.key, n.vel)
        })
        .collect();
    ns.sort_by_key(|n| (n.0, std::cmp::Reverse(n.1), n.2));
    ns.dedup_by(|b, a| a.0 == b.0 && a.2 == b.2);
    // A key struck again while it still sounds ends the note before (as one MIDI channel does): two
    // notes of one pitch held at once on one staff would make their ties ambiguous to every reader.
    let mut sounding_until: HashMap<u8, usize> = HashMap::new();
    for i in 0..ns.len() {
        if let Some(&j) = sounding_until.get(&ns[i].2) {
            if ns[j].1 > ns[i].0 {
                ns[j].1 = ns[i].0;
            }
        }
        sounding_until.insert(ns[i].2, i);
    }
    ns.retain(|n| n.1 > n.0);
    let mut groups: Vec<Group> = Vec::new();
    let mut layers: Vec<Vec<usize>> = Vec::new();
    for (s, e, key, vel) in ns {
        // A chord: the same start and end as the last group of a layer.
        if let Some(l) = layers.iter().find(|l| l.last().is_some_and(|&g| groups[g].s == s && groups[g].e == e)) {
            let g = *l.last().unwrap();
            if !groups[g].keys.iter().any(|k| k.0 == key) {
                groups[g].keys.push((key, vel));
            }
            continue;
        }
        groups.push(Group { s, e, keys: vec![(key, vel)] });
        let gi = groups.len() - 1;
        match layers.iter_mut().find(|l| l.last().is_none_or(|&g| groups[g].e <= s)) {
            Some(l) => l.push(gi),
            None => layers.push(vec![gi]),
        }
    }
    for g in &mut groups {
        g.keys.sort_by_key(|k| k.0);
    }
    (groups, layers)
}

/// One layer's segments within a bar (rests filling the gaps), bar-relative.
fn bar_segments(groups: &[Group], layer: &[usize], bar: Bar) -> Vec<Seg> {
    let (bs, be) = (bar.s, bar.s + bar.len);
    let mut segs = Vec::new();
    let mut at = 0u64;
    for &g in layer {
        let gr = &groups[g];
        if gr.e <= bs || gr.s >= be {
            continue;
        }
        let s = gr.s.max(bs) - bs;
        let e = gr.e.min(be) - bs;
        if s > at {
            segs.push(Seg { s: at, e: s, fill: Fill::Rest });
        }
        segs.push(Seg { s, e, fill: Fill::Notes { g, tie_in: gr.s < bs, tie_out: gr.e > be } });
        at = e;
    }
    if at < bar.len {
        segs.push(Seg { s: at, e: bar.len, fill: Fill::Rest });
    }
    segs
}

/// The span a triplet may fill: the meter's beat (a quarter in 4/4, an eighth in 3/8 or 6/8), held
/// between an eighth and a quarter.
fn beat_window(bar: Bar) -> u64 {
    (4 * Q / bar.unit.max(1) as u64).clamp(Q / 2, Q)
}

/// Whether each beat of the bar is written as a triplet, after moving any boundary that fits
/// neither grid to the nearest 64th; the segments come back snapped, split at each triplet quarter's
/// edges, with collapsed ones gone and neighbouring rests joined.
fn rhythm(mut segs: Vec<Seg>, bar_len: u64, win: u64) -> (Vec<Seg>, Vec<bool>) {
    let windows = bar_len.div_ceil(win) as usize;
    let mut trip = vec![false; windows];
    for (w, t) in trip.iter_mut().enumerate() {
        let (ws, we) = (w as u64 * win, ((w as u64 + 1) * win).min(bar_len));
        let inside: Vec<u64> = segs.iter().flat_map(|s| [s.s, s.e]).filter(|&b| b > ws && b < we).collect();
        let duple = inside.iter().all(|b| b % DUPLE == 0);
        *t = !duple && we - ws == win && inside.iter().all(|b| (b - ws) % TRIP == 0);
    }
    // Boundaries in a straight quarter move to its grid.
    let fix = |b: u64| -> u64 {
        let w = (b / win) as usize;
        if b % win == 0 || b >= bar_len || trip.get(w).copied().unwrap_or(false) {
            b
        } else {
            snap(b, DUPLE).min(bar_len)
        }
    };
    for s in &mut segs {
        s.s = fix(s.s);
        s.e = fix(s.e);
    }
    segs.retain(|s| s.e > s.s);
    // Split at each triplet quarter's edges.
    let mut cuts: Vec<u64> = Vec::new();
    for (w, &t) in trip.iter().enumerate() {
        if t {
            cuts.push(w as u64 * win);
            cuts.push((w as u64 + 1) * win);
        }
    }
    let mut out: Vec<Seg> = Vec::new();
    for s in segs {
        let mut cur = s;
        for &c in &cuts {
            if c > cur.s && c < cur.e {
                let (a, b) = match cur.fill {
                    Fill::Notes { g, tie_in, tie_out } => (Fill::Notes { g, tie_in, tie_out: true }, Fill::Notes { g, tie_in: true, tie_out }),
                    Fill::Rest => (Fill::Rest, Fill::Rest),
                };
                out.push(Seg { s: cur.s, e: c, fill: a });
                cur = Seg { s: c, e: cur.e, fill: b };
            }
        }
        out.push(cur);
    }
    // Neighbouring rests within one kind of quarter join; a gap left by a collapsed note is a rest.
    let mut joined: Vec<Seg> = Vec::new();
    let mut at = 0u64;
    for s in out {
        if s.s > at {
            joined.push(Seg { s: at, e: s.s, fill: Fill::Rest });
        }
        let s = Seg { s: s.s.max(at), ..s };
        if s.e <= s.s {
            continue;
        }
        if let Some(last) = joined.last_mut() {
            let same_window = !cuts.contains(&s.s);
            if last.fill == Fill::Rest && s.fill == Fill::Rest && last.e == s.s && same_window {
                last.e = s.e;
                at = s.e;
                continue;
            }
        }
        at = s.e;
        joined.push(s);
    }
    if at < bar_len {
        joined.push(Seg { s: at, e: bar_len, fill: Fill::Rest });
    }
    (joined, trip)
}

/// A written length (ticks, a multiple of a 64th) as note values: each its `dur` (1 whole, 2 half ...
/// 64) and dots, longest first.
fn values(mut len: u64) -> Vec<(u32, u8)> {
    let mut out = Vec::new();
    while len >= DUPLE && out.len() < 64 {
        let mut pick = None;
        for dur in [1u32, 2, 4, 8, 16, 32, 64] {
            let v = 4 * Q / dur as u64;
            for dots in [2u8, 1, 0] {
                let c = match dots {
                    2 => v + v / 2 + v / 4,
                    1 => v + v / 2,
                    _ => v,
                };
                if (dots == 2 && v < 4 * DUPLE) || (dots == 1 && v < 2 * DUPLE) {
                    continue;
                }
                if c <= len && (len - c) % DUPLE == 0 {
                    pick = Some((dur, dots, c));
                    break;
                }
            }
            if pick.is_some() {
                break;
            }
        }
        let Some((dur, dots, c)) = pick else { break };
        out.push((dur, dots));
        len -= c;
    }
    out
}

/// The letters' order: C D E F G A B (0..6), each one's semitones above C.
const LETTER_PC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const LETTERS: [&str; 7] = ["c", "d", "e", "f", "g", "a", "b"];

/// The alteration the key gives each letter.
fn key_alters(fifths: i8) -> [i8; 7] {
    let mut a = [0i8; 7];
    // The order sharps are added (F C G D A E B) and flats (B E A D G C F), as letter indices.
    const SHARPS: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
    const FLATS: [usize; 7] = [6, 2, 5, 1, 4, 0, 3];
    if fifths > 0 {
        for &l in SHARPS.iter().take(fifths.min(7) as usize) {
            a[l] = 1;
        }
    } else {
        for &l in FLATS.iter().take((-fifths).min(7) as usize) {
            a[l] = -1;
        }
    }
    a
}

/// A MIDI key spelled in the key: its letter (0..6), alteration and octave (middle C is C4).
fn spell(key: u8, fifths: i8) -> (usize, i8, i32) {
    let pc = (key % 12) as i32;
    let alters = key_alters(fifths);
    // In the key first (F sharp in G, B flat in F), else sharps in a sharp key or C, flats in a flat one.
    let in_key = (0..7).find(|&l| (LETTER_PC[l] + alters[l] as i32).rem_euclid(12) == pc && (LETTER_PC[l] + alters[l] as i32) >= 0 && (LETTER_PC[l] + alters[l] as i32) < 12);
    let (letter, alter) = match in_key {
        Some(l) => (l, alters[l]),
        None => {
            let flat = fifths < 0;
            match (0..7).find(|&l| LETTER_PC[l] == pc) {
                Some(l) => (l, 0),
                None if flat => ((0..7).find(|&l| LETTER_PC[l] == pc + 1).unwrap_or(0), -1),
                None => ((0..7).find(|&l| LETTER_PC[l] == pc - 1).unwrap_or(0), 1),
            }
        }
    };
    let oct = (key as i32 - alter as i32 - LETTER_PC[letter]) / 12 - 1;
    (letter, alter, oct)
}

fn accid_word(a: i8) -> &'static str {
    match a {
        1 => "s",
        -1 => "f",
        2 => "x",
        -2 => "ff",
        _ => "n",
    }
}

/// Text made safe inside XML.
fn esc(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => String::new(),
            c => c.to_string(),
        })
        .collect()
}

/// A tick inside bar `bar` as a `tstamp` (1 = the bar's start, in the meter's unit).
fn tstamp(t: u64, bar: Bar) -> String {
    let beats = (t - bar.s) as f64 * bar.unit as f64 / (4 * Q) as f64 + 1.0;
    let s = format!("{beats:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Write `n` as an MEI 5.1 file. `availability`: the rights statement for its header (whose music it
/// is and under what terms), written as given.
pub fn write_mei(n: &Notated, availability: &str) -> String {
    let fifths = n.key.map(|k| k.0).unwrap_or(0);
    let keysig = match fifths {
        0 => "0".to_string(),
        f if f > 0 => format!("{f}s"),
        f => format!("{}f", -f),
    };
    let bars = bars_of(n);
    let parts: Vec<(Vec<Group>, Vec<Vec<usize>>)> = n.parts.iter().map(|p| layers_of(&p.notes)).collect();
    let mut x = String::with_capacity(4096 + n.parts.iter().map(|p| p.notes.len() * 90).sum::<usize>());
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<mei xmlns=\"http://www.music-encoding.org/ns/mei\" meiversion=\"5.1\">\n  <meiHead>\n    <fileDesc>\n      <titleStmt>\n");
    let _ = writeln!(x, "        <title>{}</title>", esc(&n.title));
    if !n.composer.is_empty() {
        let _ = writeln!(x, "        <composer>{}</composer>", esc(&n.composer));
    }
    x.push_str("      </titleStmt>\n      <pubStmt>\n");
    let _ = writeln!(x, "        <availability>{}</availability>", esc(availability));
    x.push_str("      </pubStmt>\n    </fileDesc>\n  </meiHead>\n  <music>\n    <body>\n      <mdiv>\n        <score>\n");
    let first = bars.first().copied().unwrap_or(Bar { s: 0, len: 4 * Q, count: 4, unit: 4 });
    let _ = writeln!(x, "          <scoreDef keysig=\"{keysig}\" meter.count=\"{}\" meter.unit=\"{}\">", first.count, first.unit);
    x.push_str("            <staffGrp>\n");
    for (i, p) in n.parts.iter().enumerate() {
        let avg = if p.notes.is_empty() { 60.0 } else { p.notes.iter().map(|k| k.key as f64).sum::<f64>() / p.notes.len() as f64 };
        let (shape, line) = if avg >= 57.0 { ("G", 2) } else { ("F", 4) };
        let label = if p.name.is_empty() { format!("Part {}", i + 1) } else { p.name.clone() };
        let _ = write!(x, "              <staffDef n=\"{}\" lines=\"5\" clef.shape=\"{shape}\" clef.line=\"{line}\" label=\"{}\"", i + 1, esc(&label));
        match p.program {
            Some(prog) => {
                let _ = writeln!(x, ">\n                <instrDef midi.instrnum=\"{prog}\"/>\n              </staffDef>");
            }
            None => x.push_str("/>\n"),
        }
    }
    x.push_str("            </staffGrp>\n          </scoreDef>\n          <section>\n");
    let mut prev_meter = (first.count, first.unit);
    for (bi, bar) in bars.iter().enumerate() {
        if (bar.count, bar.unit) != prev_meter {
            let _ = writeln!(x, "            <scoreDef meter.count=\"{}\" meter.unit=\"{}\"/>", bar.count, bar.unit);
            prev_meter = (bar.count, bar.unit);
        }
        let _ = writeln!(x, "            <measure n=\"{}\">", bi + 1);
        for (pi, (groups, layers)) in parts.iter().enumerate() {
            let _ = writeln!(x, "              <staff n=\"{}\">", pi + 1);
            // The written accidentals: by the bar's note heads in time order across the layers.
            let laid: Vec<(Vec<Seg>, Vec<bool>)> = layers.iter().map(|l| rhythm(bar_segments(groups, l, *bar), bar.len, beat_window(*bar))).collect();
            let mut heads: Vec<(u64, usize, usize, u8)> = Vec::new();
            for (li, (segs, _)) in laid.iter().enumerate() {
                for (si, s) in segs.iter().enumerate() {
                    if let Fill::Notes { g, tie_in: false, .. } = s.fill {
                        for &(k, _) in &groups[g].keys {
                            heads.push((s.s, li, si, k));
                        }
                    }
                }
            }
            heads.sort();
            let alters = key_alters(fifths);
            let mut state: HashMap<(usize, i32), i8> = HashMap::new();
            let mut written: HashMap<(usize, usize, u8), i8> = HashMap::new();
            for &(_, li, si, k) in &heads {
                let (l, a, o) = spell(k, fifths);
                let now = *state.get(&(l, o)).unwrap_or(&alters[l]);
                if now != a {
                    written.insert((li, si, k), a);
                    state.insert((l, o), a);
                }
            }
            let mut any = false;
            for (li, (segs, trip)) in laid.iter().enumerate() {
                if li > 0 && segs.iter().all(|s| s.fill == Fill::Rest) {
                    continue;
                }
                let _ = writeln!(x, "                <layer n=\"{}\">", li + 1);
                if segs.iter().all(|s| s.fill == Fill::Rest) {
                    x.push_str("                  <mRest/>\n");
                } else {
                    let mut open_tuplet: Option<usize> = None;
                    for (si, s) in segs.iter().enumerate() {
                        let win = beat_window(*bar);
                        let w = (s.s / win) as usize;
                        let in_trip = trip.get(w).copied().unwrap_or(false) && s.e <= (w as u64 + 1) * win;
                        if open_tuplet.is_some_and(|ow| ow != w || !in_trip) {
                            x.push_str("                  </tuplet>\n");
                            open_tuplet = None;
                        }
                        if in_trip && open_tuplet.is_none() {
                            x.push_str("                  <tuplet num=\"3\" numbase=\"2\">\n");
                            open_tuplet = Some(w);
                        }
                        let len = if in_trip { (s.e - s.s) * 3 / 2 } else { s.e - s.s };
                        let vals = values(len);
                        let count = vals.len();
                        for (vi, (dur, dots)) in vals.into_iter().enumerate() {
                            let dots_attr = if dots > 0 { format!(" dots=\"{dots}\"") } else { String::new() };
                            match s.fill {
                                Fill::Rest => {
                                    let _ = writeln!(x, "                  <rest dur=\"{dur}\"{dots_attr}/>");
                                }
                                Fill::Notes { g, tie_in, tie_out } => {
                                    let tin = vi > 0 || tie_in;
                                    let tout = vi + 1 < count || tie_out;
                                    let tie = match (tin, tout) {
                                        (false, true) => " tie=\"i\"",
                                        (true, true) => " tie=\"m\"",
                                        (true, false) => " tie=\"t\"",
                                        _ => "",
                                    };
                                    let keys = &groups[g].keys;
                                    let note = |x: &mut String, k: u8, vel: u8, indent: &str, with_dur: bool| {
                                        let (l, a, o) = spell(k, fifths);
                                        let acc = if vi == 0 && !tin { written.get(&(li, si, k)).map(|a| format!(" accid=\"{}\"", accid_word(*a))).unwrap_or_default() } else { String::new() };
                                        let d = if with_dur { format!(" dur=\"{dur}\"{dots_attr}") } else { String::new() };
                                        let _ = writeln!(x, "{indent}<note{d} pname=\"{}\" oct=\"{o}\"{acc} accid.ges=\"{}\" vel=\"{vel}\"{tie}/>", LETTERS[l], accid_word(a));
                                    };
                                    if keys.len() == 1 {
                                        note(&mut x, keys[0].0, keys[0].1, "                  ", true);
                                    } else {
                                        let _ = writeln!(x, "                  <chord dur=\"{dur}\"{dots_attr}>");
                                        for &(k, vel) in keys {
                                            note(&mut x, k, vel, "                    ", false);
                                        }
                                        x.push_str("                  </chord>\n");
                                    }
                                }
                            }
                        }
                    }
                    if open_tuplet.is_some() {
                        x.push_str("                  </tuplet>\n");
                    }
                }
                x.push_str("                </layer>\n");
                any = true;
            }
            if !any {
                x.push_str("                <layer n=\"1\">\n                  <mRest/>\n                </layer>\n");
            }
            x.push_str("              </staff>\n");
        }
        let be = bar.s + bar.len;
        for &(t, qpm) in n.tempos.iter().filter(|t| t.0 >= bar.s && t.0 < be) {
            let q = (qpm * 100.0).round() / 100.0;
            let _ = writeln!(x, "              <tempo staff=\"1\" tstamp=\"{}\" midi.bpm=\"{q}\">{}</tempo>", tstamp(t, *bar), q.round());
        }
        for (t, m) in n.markers.iter().filter(|m| m.0 >= bar.s && m.0 < be) {
            let _ = writeln!(x, "              <dir staff=\"1\" tstamp=\"{}\">{}</dir>", tstamp(*t, *bar), esc(m));
        }
        for (t, m) in n.texts.iter().filter(|m| m.0 >= bar.s && m.0 < be) {
            let _ = writeln!(x, "              <harm staff=\"1\" tstamp=\"{}\">{}</harm>", tstamp(*t, *bar), esc(m));
        }
        x.push_str("            </measure>\n");
    }
    x.push_str("          </section>\n        </score>\n      </mdiv>\n    </body>\n  </music>\n</mei>\n");
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Note values: a dotted quarter, a half and a tied remainder, worked out by hand.
    #[test]
    fn note_values_cover_a_length_longest_first() {
        assert_eq!(values(Q), vec![(4, 0)]);
        assert_eq!(values(Q + Q / 2), vec![(4, 1)]);
        assert_eq!(values(5 * Q), vec![(1, 0), (4, 0)]);
        assert_eq!(values(Q / 16), vec![(64, 0)]);
        assert_eq!(values(Q + Q / 16), vec![(4, 0), (64, 0)]);
    }

    /// Spelling in the key: F sharp in D major, B flat in F major, C sharp in C, E flat in B flat.
    #[test]
    fn keys_are_spelled_in_the_key() {
        assert_eq!(spell(66, 2), (3, 1, 4));
        assert_eq!(spell(70, -1), (6, -1, 4));
        assert_eq!(spell(61, 0), (0, 1, 4));
        assert_eq!(spell(63, -2), (2, -1, 4));
        assert_eq!(spell(60, 0), (0, 0, 4));
        assert_eq!(spell(59, 0), (6, 0, 3));
    }
}
