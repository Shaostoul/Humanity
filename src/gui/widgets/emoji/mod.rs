//! Emoji in a piece of text, for the desktop app (2026-11-20, docs/design/emoji.md rungs 1 and 3).
//!
//! **Rung 1, the matcher** (this file): finds every emoji in a string with the SAME rules as the
//! website's `web/shared/emoji.js` (the comment at its top is the rule book), and names the picture
//! that draws it from the pack's list (`assets/emoji/index.json`). Both apps are held to the same
//! answers, `scripts/tests/fixtures/emoji-v1.json` (`tests.rs` here, `scripts/tests/emoji-web.test.js`
//! there). In short:
//!   - an emoji is an emoji-by-default character (😀), any pictograph followed by U+FE0F (the red
//!     heart, the copyright sign), a person or hand with a skin tone, sequences joined by U+200D, a flag (two regional letters),
//!     a keycap, or a black flag with tag letters; U+FE0E (draw as text) keeps text; the one
//!     text-by-default character drawn bare is the red heart U+2764 (the chat stores the heart
//!     reaction bare, and Twemoji drew it so);
//!   - its picture is looked up by its code points in upper-case hex joined by "-" with every U+FE0F
//!     taken out of both sides (OpenMoji names a lone emoji without it and keeps it in sequences);
//!   - when no set has the whole sequence: a joined sequence shows its parts, two letters that are no
//!     flag show the letters, a subdivision flag nobody drew shows the black flag, and anything with no
//!     picture stays text.
//!
//! Unicode's emoji properties come from `props.rs`, generated from the same Unicode data the website's
//! engine has (`scripts/gen-emoji-props.js`; why not the regex crate's tables: emoji.md, rung 1).
//!
//! **Rung 2, the pictures**: `pack.rs` (resvg drawing the pack's SVGs into textures, kept in memory
//! only, obeying the reuse switch). **Rung 3, pictures in a line of text**: `inline.rs`, and the
//! reaction buttons and the pack's credit: `button.rs`.

mod props;
pub mod button;
pub mod inline;
pub mod pack;

#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod pack_tests;

use std::collections::HashMap;
use std::ops::Range;

pub use inline::{install, layout, layout_singleline, pack_installed, paint, picture_rects, source, with_pictures, EmojiPictures, EmojiSource, Placed};
pub use pack::install_pack;

const ZWJ: char = '\u{200D}';
const VS16: char = '\u{FE0F}'; // glyph-exempt: the selector as a code point the matcher reads, never drawn
const VS15: char = '\u{FE0E}'; // glyph-exempt: the selector as a code point the matcher reads, never drawn
const KEYCAP: char = '\u{20E3}';
const BLACK_FLAG: char = '\u{1F3F4}';
const TAG_END: char = '\u{E007F}';
const RED_HEART: char = '\u{2764}';

/// The Unicode version `props.rs` was made from (the website's engine's, when it was made).
pub fn unicode_version() -> &'static str {
    props::UNICODE_VERSION
}

fn has(table: &[(u32, u32)], c: char) -> bool {
    let c = c as u32;
    table
        .binary_search_by(|&(a, b)| {
            if b < c {
                std::cmp::Ordering::Less
            } else if a > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn presentation(c: char) -> bool {
    has(props::EMOJI_PRESENTATION, c)
}
fn pictographic(c: char) -> bool {
    has(props::EXTENDED_PICTOGRAPHIC, c)
}
fn modifier_base(c: char) -> bool {
    has(props::EMOJI_MODIFIER_BASE, c)
}
fn modifier(c: char) -> bool {
    has(props::EMOJI_MODIFIER, c)
}
fn regional(c: char) -> bool {
    has(props::REGIONAL_INDICATOR, c)
}

/// The characters of a string with their byte offsets, read past the end as '\0' (no emoji starts
/// or continues with it), so the matcher can look ahead without bounds checks.
struct Chars<'a> {
    text: &'a str,
    at: Vec<(usize, char)>,
}

impl Chars<'_> {
    fn c(&self, i: usize) -> char {
        self.at.get(i).map_or('\0', |&(_, c)| c)
    }
    fn byte(&self, i: usize) -> usize {
        self.at.get(i).map_or(self.text.len(), |&(b, _)| b)
    }
    /// `j` moved past one U+FE0F, if one is there (the optional U+FE0F of the rules).
    fn vs16(&self, j: usize) -> usize {
        if self.c(j) == VS16 { j + 1 } else { j }
    }
}

/// The emoji starting at character `i`, as the index of the character after it. The alternatives in
/// emoji.js's order, because the first that matches wins: the flag pair, the keycap and the tag flag
/// before a lone character could take their first letter.
fn match_at(s: &Chars, i: usize) -> Option<usize> {
    let c = s.c(i);
    // A pair of regional letters (a flag).
    if regional(c) && regional(s.c(i + 1)) {
        return Some(i + 2);
    }
    // A keycap: # * or a digit, U+FE0F optional, U+20E3.
    if matches!(c, '#' | '*' | '0'..='9') {
        let j = s.vs16(i + 1);
        if s.c(j) == KEYCAP {
            return Some(j + 1);
        }
    }
    // A black flag with tag letters and the cancel tag (a subdivision flag, England's).
    if c == BLACK_FLAG {
        let mut j = i + 1;
        while ('\u{E0020}'..='\u{E007E}').contains(&s.c(j)) {
            j += 1;
        }
        if j > i + 1 && s.c(j) == TAG_END {
            return Some(j + 1);
        }
    }
    // One emoji character with what may follow it (emoji.js FIRST), then joined parts.
    let mut end = first_at(s, i)?;
    while s.c(end) == ZWJ {
        match next_at(s, end + 1) {
            Some(k) => end = k,
            None => break,
        }
    }
    Some(end)
}

/// emoji.js FIRST: a person or hand with a skin tone; an emoji-by-default character unless U+FE0E
/// follows; the bare red heart (the one text-by-default exception); a pictograph with U+FE0F; a
/// pictograph that starts a joined sequence typed without U+FE0F (❤ U+200D 🔥).
fn first_at(s: &Chars, i: usize) -> Option<usize> {
    let c = s.c(i);
    if modifier_base(c) {
        let j = s.vs16(i + 1);
        if modifier(s.c(j)) {
            return Some(j + 1);
        }
    }
    if presentation(c) && s.c(i + 1) != VS15 {
        return Some(s.vs16(i + 1));
    }
    if c == RED_HEART && s.c(i + 1) != VS15 {
        return Some(s.vs16(i + 1));
    }
    if pictographic(c) && s.c(i + 1) == VS16 {
        return Some(i + 2);
    }
    if pictographic(c) && s.c(i + 1) == ZWJ && {
        let n = s.c(i + 2);
        pictographic(n) || presentation(n)
    } {
        return Some(i + 1);
    }
    None
}

/// emoji.js NEXT, after a joiner: a text-by-default pictograph needs no U+FE0F there.
fn next_at(s: &Chars, i: usize) -> Option<usize> {
    let c = s.c(i);
    if modifier_base(c) {
        let j = s.vs16(i + 1);
        if modifier(s.c(j)) {
            return Some(j + 1);
        }
    }
    if pictographic(c) || presentation(c) {
        return Some(s.vs16(i + 1));
    }
    None
}

/// The byte range of every emoji in `text`, in order (before any picture is looked up).
pub fn find(text: &str) -> Vec<Range<usize>> {
    let s = Chars { text, at: text.char_indices().collect() };
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.at.len() {
        match match_at(&s, i) {
            Some(end) => {
                out.push(s.byte(i)..s.byte(end));
                i = end;
            }
            None => i += 1,
        }
    }
    out
}

/// The key a piece of text is looked up by: its code points in upper-case hex (at least four
/// digits), joined by "-", every U+FE0F left out (emoji.js `keyOf`).
pub fn key_of(text: &str) -> String {
    text.chars().filter(|&c| c != VS16).map(hex_of).collect::<Vec<_>>().join("-")
}

fn hex_of(c: char) -> String {
    format!("{:04X}", c as u32)
}

/// The pack's list of pictures (`assets/emoji/index.json`, generated by
/// `scripts/gen-emoji-index.js`): each set's folder, and for each look-up key the first set that has
/// it and the picture's file name there (without `.svg`).
#[derive(Debug, Clone, Default)]
pub struct EmojiIndex {
    /// Set folders in look-up order (`assets/emoji/sets.json`).
    pub sets: Vec<String>,
    files: HashMap<String, (usize, String)>,
}

impl EmojiIndex {
    /// The list from index.json's text.
    pub fn from_json(text: &str) -> Result<EmojiIndex, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("index.json is not JSON: {e}"))?;
        let sets = v["sets"].as_array().ok_or("index.json has no sets")?;
        let mut index = EmojiIndex::default();
        for set in sets {
            let dir = set["dir"].as_str().ok_or("a set in index.json has no dir")?;
            let n = index.sets.len();
            index.sets.push(dir.to_string());
            for name in set["names"].as_str().unwrap_or_default().split(' ').filter(|n| !n.is_empty()) {
                let key = name.split('-').filter(|cp| *cp != "FE0F").collect::<Vec<_>>().join("-");
                index.files.entry(key).or_insert_with(|| (n, name.to_string()));
            }
        }
        Ok(index)
    }

    /// The list from a file (the pack's own: `assets_dir()/emoji/index.json`).
    pub fn load(path: &std::path::Path) -> Result<EmojiIndex, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        EmojiIndex::from_json(&text)
    }

    /// Where the shipped pack's list sits beside the exe.
    pub fn shipped_path() -> std::path::PathBuf {
        humanity_core::storage::assets_dir().join("emoji").join("index.json")
    }

    /// How many pictures the list names (each key once).
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The set folder and file name of the picture for `key`, if any set has it.
    pub fn get(&self, key: &str) -> Option<(&str, &str)> {
        self.files.get(key).map(|(set, name)| (self.sets[*set].as_str(), name.as_str()))
    }
}

/// One piece of a string: text kept as text, or a picture standing for the text in `range`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Text drawn as text (a byte range of the string).
    Text(Range<usize>),
    /// A picture: the byte range of the text it stands for (what copying gives), the set folder it
    /// comes from and its file name there without `.svg` (OpenMoji's naming).
    Picture { range: Range<usize>, set: String, file: String },
}

impl Piece {
    pub fn range(&self) -> &Range<usize> {
        match self {
            Piece::Text(r) | Piece::Picture { range: r, .. } => r,
        }
    }
}

/// A whole string as pictures and text, in order, neighbouring text joined (emoji.js `tokens`).
pub fn pieces(text: &str, index: &EmojiIndex) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    let mut at = 0;
    for m in find(text) {
        if m.start > at {
            push(&mut out, Piece::Text(at..m.start));
        }
        let mut parts = Vec::new();
        pieces_of(text, m.clone(), index, &mut parts);
        for p in parts {
            push(&mut out, p);
        }
        at = m.end;
    }
    if at < text.len() {
        push(&mut out, Piece::Text(at..text.len()));
    }
    out
}

fn push(out: &mut Vec<Piece>, p: Piece) {
    if let (Piece::Text(r), Some(Piece::Text(last))) = (&p, out.last_mut()) {
        if last.end == r.start {
            last.end = r.end;
            return;
        }
    }
    out.push(p);
}

fn picture(range: Range<usize>, (set, file): (&str, &str)) -> Piece {
    Piece::Picture { range, set: set.to_string(), file: file.to_string() }
}

/// One matched emoji as pictures and text: one picture when a set has it whole, otherwise its parts
/// (emoji.js `pieces`, the fallbacks in the comment at the top of this file).
fn pieces_of(text: &str, range: Range<usize>, index: &EmojiIndex, out: &mut Vec<Piece>) {
    let seq = &text[range.clone()];
    if let Some(found) = index.get(&key_of(seq)) {
        out.push(picture(range, found));
        return;
    }
    let chars: Vec<char> = seq.chars().filter(|&c| c != VS16).collect();
    if chars.len() <= 1 {
        out.push(Piece::Text(range));
        return;
    }
    if seq.contains(ZWJ) {
        let mut start = range.start;
        for (n, part) in seq.split(ZWJ).enumerate() {
            if n > 0 {
                out.push(Piece::Text(start - ZWJ.len_utf8()..start));
            }
            if !part.is_empty() {
                pieces_of(text, start..start + part.len(), index, out);
            }
            start += part.len() + ZWJ.len_utf8();
        }
        return;
    }
    if chars[0] == BLACK_FLAG && chars.len() > 2 {
        match index.get(&hex_of(BLACK_FLAG)) {
            Some(found) => out.push(picture(range, found)),
            None => out.push(Piece::Text(range)),
        }
        return;
    }
    for (b, c) in seq.char_indices() {
        let r = range.start + b..range.start + b + c.len_utf8();
        match (c != VS16).then(|| index.get(&hex_of(c))).flatten() {
            Some(found) => out.push(picture(r, found)),
            None => out.push(Piece::Text(r)),
        }
    }
}
