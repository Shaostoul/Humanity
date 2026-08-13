//! Comment-preserving targeted-field RON value rewriter.
//!
//! WHY this exists (docs/design/artificial-planet.md, "In-game tuning"): the
//! Planet Tuner dev page must write single field values back into
//! data/planets/<id>.ron. Files like earth.ron carry extensive hand-written,
//! load-bearing comments (why the cloud coverage is 0.42, how the relief
//! exaggeration was derived, where the heightmap came from). Reserializing
//! the whole PlanetDef struct through `ron::ser` would destroy every one of
//! those comments, so instead we locate the ONE field's value span in the
//! raw text and splice in the new value, leaving every other byte of the
//! file exactly as the author wrote it.
//!
//! Scope: TOP-LEVEL fields only, meaning fields at depth 1 inside the
//! outermost parens of a RON struct literal. A nested field with the same
//! name (e.g. a `gravity` inside `core: Some((gravity: ...))`) is never
//! matched, because while scanning another field's value we only track
//! delimiter depth and never look for field names.
//!
//! The scanner walks the text byte by byte with explicit string / char /
//! comment states and a delimiter depth counter. It deliberately does NOT
//! use regexes: a regex cannot know whether "gravity:" sits in code, in a
//! comment, or inside a string literal, and earth.ron's comments mention
//! field names all over the place.
//!
//! Appending a MISSING field is out of scope for this increment: a missing
//! field returns a clear error and the caller decides what to do.
//!
//! Pure std, no feature gate: compiles identically under `native` and
//! `relay` (the relay never calls it, but keeping it ungated means
//! `cargo check --features relay --no-default-features` proves it builds
//! everywhere, matching how `terrain` and the other pure-data modules work).

use std::fmt;

/// Everything that can go wrong while locating or replacing a field value.
/// Byte offsets (where present) point into the ORIGINAL text so a caller
/// (or a future in-app editor) can show the user exactly where the problem
/// is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RonEditError {
    /// No outermost `(` struct body was found. The file is not a RON struct
    /// literal (or is empty / all comments).
    NoStructBody,
    /// The requested field does not exist at the top level. Appending is
    /// the caller's decision, not ours (see module docs).
    FieldNotFound(String),
    /// The text stopped making sense as a RON struct at this byte offset.
    /// `what` says what the scanner expected to see.
    Malformed { at: usize, what: &'static str },
    /// A string, char, block comment, or the struct body itself ran off the
    /// end of the file without closing.
    Unterminated { at: usize, what: &'static str },
    /// The field exists but has no value between the `:` and the next
    /// separator (e.g. `gravity: ,`). Nothing sensible to replace.
    EmptyValue(String),
    /// The replacement text handed to `set_top_level_field` would corrupt
    /// the file if spliced in (unbalanced delimiters, a line comment that
    /// would swallow the following comma, several values instead of one).
    /// Rejecting here keeps a buggy tuner UI from destroying a data file.
    InvalidReplacement(&'static str),
}

impl fmt::Display for RonEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RonEditError::NoStructBody => {
                write!(f, "no outermost ( ... ) RON struct body found")
            }
            RonEditError::FieldNotFound(name) => {
                write!(f, "top-level field '{name}' not found")
            }
            RonEditError::Malformed { at, what } => {
                write!(f, "malformed RON at byte {at}: {what}")
            }
            RonEditError::Unterminated { at, what } => {
                write!(f, "unterminated {what} starting at byte {at}")
            }
            RonEditError::EmptyValue(name) => {
                write!(f, "field '{name}' has an empty value")
            }
            RonEditError::InvalidReplacement(why) => {
                write!(f, "replacement value rejected: {why}")
            }
        }
    }
}

impl std::error::Error for RonEditError {}

/// Byte offsets of one field's VALUE inside the original text: the value
/// starts at `value_start` (the first code byte after the `:`, past any
/// whitespace and comments) and ends at `value_end` (one past the last code
/// byte, so trailing whitespace and same-line comments before the comma stay
/// OUTSIDE the span and therefore survive a rewrite).
///
/// Offsets always land on ASCII delimiters or value characters, so slicing
/// the text with them is UTF-8 safe even when comments contain multibyte
/// characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpan {
    pub value_start: usize,
    pub value_end: usize,
}

impl FieldSpan {
    /// The value text itself, for tests and diff displays.
    pub fn slice<'a>(&self, text: &'a str) -> &'a str {
        &text[self.value_start..self.value_end]
    }
}

/// Replace the VALUE of one top-level field, preserving every byte outside
/// the value span: comments, indentation, trailing commas, and same-line
/// comments after the value all survive untouched.
///
/// `new_value` must be exactly one RON value (it is validated before the
/// splice, see `validate_replacement`). Multi-line values are fine.
pub fn set_top_level_field(
    ron_text: &str,
    field: &str,
    new_value: &str,
) -> Result<String, RonEditError> {
    validate_replacement(new_value)?;
    let span = get_top_level_field_span(ron_text, field)?;
    // Splice: prefix + new value + suffix. Nothing else is rebuilt, which is
    // the entire point of this module.
    let mut out = String::with_capacity(
        ron_text.len() - (span.value_end - span.value_start) + new_value.len(),
    );
    out.push_str(&ron_text[..span.value_start]);
    out.push_str(new_value);
    out.push_str(&ron_text[span.value_end..]);
    Ok(out)
}

/// Locate the value span of one top-level field. Errors if the field is
/// missing (the caller decides whether to append; we never guess).
pub fn get_top_level_field_span(
    ron_text: &str,
    field: &str,
) -> Result<FieldSpan, RonEditError> {
    let mut found: Option<FieldSpan> = None;
    let stopped_early = walk_top_level_fields(ron_text, |name, span| {
        if name == field {
            found = Some(span);
            true // stop walking
        } else {
            false
        }
    })?;
    match found {
        Some(span) => {
            // The walker only stops early when the visitor said so, and the
            // visitor only says so after recording the span.
            debug_assert!(stopped_early);
            Ok(span)
        }
        None => Err(RonEditError::FieldNotFound(field.to_string())),
    }
}

/// Every top-level field in document order, with its value span. Falls out
/// of the same walker; the future Planet Tuner uses it to enumerate what is
/// tunable, and the tests use it to prove untouched fields stay byte
/// identical after a rewrite.
pub fn top_level_fields(ron_text: &str) -> Result<Vec<(String, FieldSpan)>, RonEditError> {
    let mut fields = Vec::new();
    walk_top_level_fields(ron_text, |name, span| {
        fields.push((name.to_string(), span));
        false // keep walking
    })?;
    Ok(fields)
}

// ---------------------------------------------------------------------------
// The walker
// ---------------------------------------------------------------------------

/// Core walk: find the outermost struct parens, then visit each depth-1
/// `name: value` pair in order. The visitor returns true to stop early
/// (found what it wanted). Returns Ok(true) when stopped early, Ok(false)
/// when the whole body was walked.
fn walk_top_level_fields(
    ron_text: &str,
    mut visit: impl FnMut(&str, FieldSpan) -> bool,
) -> Result<bool, RonEditError> {
    let mut s = Scanner::new(ron_text);
    s.skip_trivia()?;
    // RON allows an optional struct name before the parens
    // (`PlanetDef( ... )`); none of our data files use one, but skipping an
    // identifier here costs nothing and keeps the walker general.
    let _ = s.read_identifier();
    s.skip_trivia()?;
    if s.peek() != Some(b'(') {
        return Err(RonEditError::NoStructBody);
    }
    s.pos += 1; // now inside the struct body: depth 1, where fields live

    loop {
        s.skip_trivia()?;
        match s.peek() {
            None => {
                return Err(RonEditError::Unterminated { at: s.pos, what: "struct body" })
            }
            Some(b')') => return Ok(false), // body closed, walk complete
            _ => {}
        }
        let name_at = s.pos;
        let name = s.read_identifier().ok_or(RonEditError::Malformed {
            at: name_at,
            what: "expected a field name",
        })?;
        s.skip_trivia()?;
        if s.peek() != Some(b':') {
            return Err(RonEditError::Malformed {
                at: s.pos,
                what: "expected ':' after field name",
            });
        }
        s.pos += 1; // consume ':'
        s.skip_trivia()?; // value_start lands PAST comments, so `x: /* c */ 5`
                          // keeps its comment on a rewrite
        let value_start = s.pos;
        let value_end = s.scan_value_end()?;
        if value_end == value_start {
            return Err(RonEditError::EmptyValue(name.to_string()));
        }
        if visit(name, FieldSpan { value_start, value_end }) {
            return Ok(true);
        }
        // scan_value_end leaves pos ON the terminator: a ',' (consume it and
        // continue) or the body's ')' (leave it; the next loop iteration
        // sees it and finishes the walk).
        if s.peek() == Some(b',') {
            s.pos += 1;
        }
    }
}

/// Reject replacement text that would corrupt the file when spliced into a
/// value span. The dangerous shapes, in order of how likely a buggy caller
/// is to produce them:
/// - unbalanced delimiters (half a tuple): the file stops parsing;
/// - a depth-0 comma (two values): silently injects an extra field slot;
/// - a line comment: the splice puts the file's own ',' AFTER the '//', so
///   the comment swallows the comma and the file stops parsing. Block
///   comments are fine because they self-terminate.
/// - empty / all-whitespace: nothing to write.
fn validate_replacement(new_value: &str) -> Result<(), RonEditError> {
    let mut s = Scanner::new(new_value);
    let mut depth: usize = 0;
    let mut saw_code = false;
    loop {
        match s.peek() {
            None => break,
            Some(b) if b.is_ascii_whitespace() => s.pos += 1,
            Some(b'/') if s.peek_at(1) == Some(b'/') => {
                return Err(RonEditError::InvalidReplacement(
                    "line comments in a replacement value would comment out the \
                     following separator; use /* ... */ instead",
                ));
            }
            Some(b'/') if s.peek_at(1) == Some(b'*') => {
                s.skip_block_comment().map_err(|_| {
                    RonEditError::InvalidReplacement("unterminated block comment")
                })?;
            }
            Some(b'"') => {
                s.skip_string().map_err(|_| {
                    RonEditError::InvalidReplacement("unterminated string literal")
                })?;
                saw_code = true;
            }
            Some(b'\'') => {
                s.skip_char_literal().map_err(|_| {
                    RonEditError::InvalidReplacement("unterminated char literal")
                })?;
                saw_code = true;
            }
            Some(b'r') if s.raw_string_hashes().is_some() => {
                let hashes = s.raw_string_hashes().unwrap();
                s.skip_raw_string(hashes).map_err(|_| {
                    RonEditError::InvalidReplacement("unterminated raw string")
                })?;
                saw_code = true;
            }
            Some(b'(') | Some(b'[') | Some(b'{') => {
                depth += 1;
                s.pos += 1;
                saw_code = true;
            }
            Some(b')') | Some(b']') | Some(b'}') => {
                if depth == 0 {
                    return Err(RonEditError::InvalidReplacement(
                        "unbalanced closing delimiter",
                    ));
                }
                depth -= 1;
                s.pos += 1;
                saw_code = true;
            }
            Some(b',') if depth == 0 => {
                return Err(RonEditError::InvalidReplacement(
                    "top-level comma: a replacement must be exactly one value",
                ));
            }
            Some(_) => {
                s.pos += 1;
                saw_code = true;
            }
        }
    }
    if depth != 0 {
        return Err(RonEditError::InvalidReplacement("unclosed opening delimiter"));
    }
    if !saw_code {
        return Err(RonEditError::InvalidReplacement("empty replacement value"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The byte scanner
// ---------------------------------------------------------------------------

/// Byte-wise scanner. Working on bytes (not chars) is deliberate and safe:
/// every character the scanner REACTS to (quotes, slashes, brackets, comma,
/// colon) is ASCII, and UTF-8 guarantees no multibyte sequence contains an
/// ASCII byte, so multibyte text inside comments and strings just streams
/// through the `Some(_)` arms untouched. All produced offsets therefore sit
/// on character boundaries.
struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        Scanner { bytes: text.as_bytes(), pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    /// Skip whitespace, line comments, and (nested) block comments. This is
    /// what makes field-name text inside comments invisible to the walker.
    fn skip_trivia(&mut self) -> Result<(), RonEditError> {
        loop {
            match self.peek() {
                Some(b) if b.is_ascii_whitespace() => self.pos += 1,
                Some(b'/') if self.peek_at(1) == Some(b'/') => self.skip_line_comment(),
                Some(b'/') if self.peek_at(1) == Some(b'*') => self.skip_block_comment()?,
                _ => return Ok(()),
            }
        }
    }

    /// Skip `// ...` to (not past) the newline; the newline is whitespace
    /// and the caller's loop picks it up. Reaching EOF is fine for a line
    /// comment.
    fn skip_line_comment(&mut self) {
        while let Some(b) = self.peek() {
            if b == b'\n' {
                return;
            }
            self.pos += 1;
        }
    }

    /// Skip `/* ... */`. RON block comments NEST (same rule as Rust), so a
    /// depth counter, not a substring search.
    fn skip_block_comment(&mut self) -> Result<(), RonEditError> {
        let start = self.pos;
        self.pos += 2; // consume the opening "/*"
        let mut depth = 1usize;
        while depth > 0 {
            match (self.peek(), self.peek_at(1)) {
                (Some(b'/'), Some(b'*')) => {
                    depth += 1;
                    self.pos += 2;
                }
                (Some(b'*'), Some(b'/')) => {
                    depth -= 1;
                    self.pos += 2;
                }
                (Some(_), _) => self.pos += 1,
                (None, _) => {
                    return Err(RonEditError::Unterminated { at: start, what: "block comment" })
                }
            }
        }
        Ok(())
    }

    /// Skip a `"..."` string, honoring backslash escapes so an embedded
    /// `\"` cannot fake the closing quote.
    fn skip_string(&mut self) -> Result<(), RonEditError> {
        let start = self.pos;
        self.pos += 1; // opening quote
        loop {
            match self.peek() {
                Some(b'\\') => self.pos += 2, // escape: skip the pair
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some(_) => self.pos += 1,
                None => {
                    return Err(RonEditError::Unterminated { at: start, what: "string literal" })
                }
            }
        }
    }

    /// Skip a `'x'` char literal (RON has them; no lifetimes exist in RON so
    /// a single quote is always a char). Escapes handled like strings.
    fn skip_char_literal(&mut self) -> Result<(), RonEditError> {
        let start = self.pos;
        self.pos += 1; // opening quote
        loop {
            match self.peek() {
                Some(b'\\') => self.pos += 2,
                Some(b'\'') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some(_) => self.pos += 1,
                None => {
                    return Err(RonEditError::Unterminated { at: start, what: "char literal" })
                }
            }
        }
    }

    /// If the scanner sits at the start of a raw string (`r"..."` or
    /// `r#"..."#` with any number of hashes), return the hash count.
    /// The previous byte must not be an identifier character, otherwise the
    /// 'r' is just the tail of a word like `color`.
    fn raw_string_hashes(&self) -> Option<usize> {
        if self.peek() != Some(b'r') {
            return None;
        }
        if self.pos > 0 {
            let prev = self.bytes[self.pos - 1];
            if prev == b'_' || prev.is_ascii_alphanumeric() {
                return None;
            }
        }
        let mut hashes = 0;
        while self.peek_at(1 + hashes) == Some(b'#') {
            hashes += 1;
        }
        if self.peek_at(1 + hashes) == Some(b'"') {
            Some(hashes)
        } else {
            None
        }
    }

    /// Skip a raw string whose hash count `raw_string_hashes` already
    /// measured. The closer is a quote followed by exactly that many hashes.
    fn skip_raw_string(&mut self, hashes: usize) -> Result<(), RonEditError> {
        let start = self.pos;
        self.pos += 1 + hashes + 1; // r + hashes + opening quote
        loop {
            match self.peek() {
                Some(b'"') => {
                    let mut closes = true;
                    for i in 0..hashes {
                        if self.peek_at(1 + i) != Some(b'#') {
                            closes = false;
                            break;
                        }
                    }
                    self.pos += 1;
                    if closes {
                        self.pos += hashes;
                        return Ok(());
                    }
                }
                Some(_) => self.pos += 1,
                None => {
                    return Err(RonEditError::Unterminated { at: start, what: "raw string" })
                }
            }
        }
    }

    /// Read `[A-Za-z_][A-Za-z0-9_]*` and return it, or None if the current
    /// byte cannot start an identifier. Reading the WHOLE identifier is what
    /// makes prefix names safe: matching `noise` can never fire on
    /// `noise_frequency` because the comparison sees the full word.
    fn read_identifier(&mut self) -> Option<&'a str> {
        let start = self.pos;
        match self.peek() {
            Some(b) if b == b'_' || b.is_ascii_alphabetic() => self.pos += 1,
            _ => return None,
        }
        while let Some(b) = self.peek() {
            if b == b'_' || b.is_ascii_alphanumeric() {
                self.pos += 1;
            } else {
                break;
            }
        }
        // Identifier bytes are pure ASCII by construction, so this cannot
        // fail; expect() documents the invariant.
        Some(std::str::from_utf8(&self.bytes[start..self.pos]).expect("ASCII identifier"))
    }

    /// Scan one field VALUE starting at the current position. Returns the
    /// end of the value (one past its last code byte) and leaves `pos` ON
    /// the terminator: the ',' after the value, or the ')' that closes the
    /// struct body (for the last field of a struct without a trailing
    /// comma).
    ///
    /// Depth is RELATIVE to the value: tuples, arrays, Some(...) wrappers,
    /// and maps nest freely; commas and parens inside them never terminate
    /// the value. `last_code_end` only advances on CODE bytes, so trailing
    /// whitespace and same-line comments between the value and its comma
    /// stay outside the span (and therefore survive a rewrite).
    fn scan_value_end(&mut self) -> Result<usize, RonEditError> {
        let mut depth = 0usize;
        let mut last_code_end = self.pos;
        loop {
            self.skip_trivia()?;
            match self.peek() {
                None => {
                    return Err(RonEditError::Unterminated { at: self.pos, what: "field value" })
                }
                Some(b'"') => {
                    self.skip_string()?;
                    last_code_end = self.pos;
                }
                Some(b'\'') => {
                    self.skip_char_literal()?;
                    last_code_end = self.pos;
                }
                Some(b'r') if self.raw_string_hashes().is_some() => {
                    let hashes = self.raw_string_hashes().unwrap();
                    self.skip_raw_string(hashes)?;
                    last_code_end = self.pos;
                }
                Some(b'(') | Some(b'[') | Some(b'{') => {
                    depth += 1;
                    self.pos += 1;
                    last_code_end = self.pos;
                }
                Some(b')') => {
                    if depth == 0 {
                        // The struct body's own closer: the value ended just
                        // before it (last field, no trailing comma).
                        return Ok(last_code_end);
                    }
                    depth -= 1;
                    self.pos += 1;
                    last_code_end = self.pos;
                }
                Some(b']') | Some(b'}') => {
                    if depth == 0 {
                        return Err(RonEditError::Malformed {
                            at: self.pos,
                            what: "unbalanced ']' or '}' in field value",
                        });
                    }
                    depth -= 1;
                    self.pos += 1;
                    last_code_end = self.pos;
                }
                Some(b',') => {
                    if depth == 0 {
                        return Ok(last_code_end);
                    }
                    self.pos += 1;
                    last_code_end = self.pos;
                }
                Some(_) => {
                    self.pos += 1;
                    last_code_end = self.pos;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::planet::PlanetDef;

    /// The REAL shipped Earth definition, read off disk so these tests
    /// exercise exactly the file the Planet Tuner will rewrite, comments and
    /// all. Panics loudly rather than skipping: a checkout without it cannot
    /// verify anything this module claims (same posture as
    /// renderer::ground_textures' earth_def test helper).
    fn earth_text() -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/planets/earth.ron");
        std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("{} must be readable ({e})", p.display()))
    }

    fn parse_planet(text: &str) -> PlanetDef {
        ron::from_str(text).expect("rewritten RON must still parse as a PlanetDef")
    }

    /// Every pure comment line of `original`, trimmed. Used to assert (c):
    /// a rewrite may only touch one value span, so every comment line must
    /// reappear verbatim in the result.
    fn comment_lines(text: &str) -> Vec<&str> {
        text.lines()
            .map(|l| l.trim())
            .filter(|l| l.starts_with("//"))
            .collect()
    }

    fn assert_comments_preserved(original: &str, rewritten: &str) {
        let after = comment_lines(rewritten);
        for line in comment_lines(original) {
            assert!(
                after.contains(&line),
                "comment line lost by rewrite: {line}"
            );
        }
    }

    /// Assert (d): every top-level field EXCEPT `changed` has a byte
    /// identical value text before and after. Walks both texts
    /// independently, so this also proves the walker still parses the
    /// rewritten file.
    fn assert_other_fields_identical(original: &str, rewritten: &str, changed: &str) {
        let before = top_level_fields(original).expect("original must walk");
        let after = top_level_fields(rewritten).expect("rewritten must walk");
        assert_eq!(
            before.len(),
            after.len(),
            "rewrite changed the top-level field count"
        );
        for ((name_a, span_a), (name_b, span_b)) in before.iter().zip(after.iter()) {
            assert_eq!(name_a, name_b, "field order changed");
            if name_a != changed {
                assert_eq!(
                    span_a.slice(original),
                    span_b.slice(rewritten),
                    "untouched field '{name_a}' changed"
                );
            }
        }
    }

    /// One rewrite against the real earth.ron, checking all four contract
    /// points: (a) parses as PlanetDef, (b) the new value round-trips,
    /// (c) comments preserved, (d) other fields byte identical.
    fn rewrite_earth_and_check(field: &str, new_value: &str) -> PlanetDef {
        let original = earth_text();
        let rewritten =
            set_top_level_field(&original, field, new_value).expect("rewrite must succeed");
        // (b) round-trip: locating the field in the REWRITTEN text yields
        // exactly the replacement we spliced in.
        let span = get_top_level_field_span(&rewritten, field).expect("field must still exist");
        assert_eq!(span.slice(&rewritten), new_value, "value did not round-trip");
        // (c) + (d)
        assert_comments_preserved(&original, &rewritten);
        assert_other_fields_identical(&original, &rewritten, field);
        // (a)
        parse_planet(&rewritten)
    }

    // ── The five contract fields against the real earth.ron ──

    #[test]
    fn earth_gravity_rewrites_as_bare_scalar() {
        let def = rewrite_earth_and_check("gravity", "3.71");
        assert!((def.gravity - 3.71).abs() < 1e-6, "gravity: {}", def.gravity);
    }

    #[test]
    fn earth_sea_level_rewrites_as_bare_scalar() {
        let def = rewrite_earth_and_check("sea_level", "0.62");
        assert!((def.sea_level - 0.62).abs() < 1e-6, "sea_level: {}", def.sea_level);
    }

    #[test]
    fn earth_cloud_coverage_rewrites_inside_option() {
        let def = rewrite_earth_and_check("cloud_coverage", "Some(0.77)");
        assert_eq!(def.cloud_coverage, Some(0.77));
    }

    #[test]
    fn earth_cloud_coverage_rewrites_to_none() {
        let def = rewrite_earth_and_check("cloud_coverage", "None");
        assert_eq!(def.cloud_coverage, None);
    }

    #[test]
    fn earth_atmosphere_color_rewrites_as_option_tuple() {
        let def = rewrite_earth_and_check("atmosphere_color", "Some((0.2, 0.3, 0.9, 0.6))");
        assert_eq!(def.atmosphere_color, Some([0.2, 0.3, 0.9, 0.6]));
    }

    #[test]
    fn earth_heightmap_rewrites_as_option_string() {
        let def = rewrite_earth_and_check("heightmap", "Some(\"planets/test_heightmap.bin\")");
        assert_eq!(def.heightmap.as_deref(), Some("planets/test_heightmap.bin"));
    }

    #[test]
    fn earth_heightmap_rewrites_to_none() {
        let def = rewrite_earth_and_check("heightmap", "None");
        assert_eq!(def.heightmap, None);
    }

    /// Chain all five tuner fields in sequence, then verify once: the
    /// realistic Planet Tuner flow is several slider changes before a save.
    #[test]
    fn earth_chained_rewrites_accumulate() {
        let original = earth_text();
        let mut text = original.clone();
        text = set_top_level_field(&text, "gravity", "19.62").unwrap();
        text = set_top_level_field(&text, "sea_level", "0.7").unwrap();
        text = set_top_level_field(&text, "cloud_coverage", "Some(0.9)").unwrap();
        text = set_top_level_field(&text, "atmosphere_color", "Some((1.0, 0.5, 0.2, 0.8))").unwrap();
        text = set_top_level_field(&text, "heightmap", "None").unwrap();
        let def = parse_planet(&text);
        assert!((def.gravity - 19.62).abs() < 1e-5);
        assert!((def.sea_level - 0.7).abs() < 1e-6);
        assert_eq!(def.cloud_coverage, Some(0.9));
        assert_eq!(def.atmosphere_color, Some([1.0, 0.5, 0.2, 0.8]));
        assert_eq!(def.heightmap, None);
        assert_comments_preserved(&original, &text);
    }

    /// Structural (not value-brittle) span checks on the live file, so the
    /// operator hand-tuning earth.ron cannot spuriously break this test:
    /// gravity must be a bare number, the two Option fields must look like
    /// Option syntax, and the walker must see the whole field roster.
    #[test]
    fn earth_spans_have_the_expected_shapes() {
        let text = earth_text();
        let gravity = get_top_level_field_span(&text, "gravity").unwrap();
        assert!(
            gravity.slice(&text).parse::<f64>().is_ok(),
            "gravity span is not a bare number: {:?}",
            gravity.slice(&text)
        );
        let atmos = get_top_level_field_span(&text, "atmosphere_color").unwrap();
        let atmos_text = atmos.slice(&text);
        assert!(
            atmos_text == "None" || (atmos_text.starts_with("Some((") && atmos_text.ends_with("))")),
            "atmosphere_color span shape: {atmos_text:?}"
        );
        let hm = get_top_level_field_span(&text, "heightmap").unwrap();
        let hm_text = hm.slice(&text);
        assert!(
            hm_text == "None" || (hm_text.starts_with("Some(\"") && hm_text.ends_with("\")")),
            "heightmap span shape: {hm_text:?}"
        );
        // The walker must enumerate the real roster, not stop early. 20 is a
        // floor, not an exact count, so adding fields to earth.ron is free.
        let fields = top_level_fields(&text).unwrap();
        assert!(fields.len() >= 20, "only {} top-level fields walked", fields.len());
        // Sanity: the roster contains no duplicates (each name once).
        let mut names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), fields.len(), "duplicate field names walked");
    }

    // ── Adversarial cases on synthetic text ──

    /// A field name that is a strict prefix of another: matching must be on
    /// the WHOLE identifier. earth.ron has noise_frequency; here we add the
    /// hypothetical bare `noise` next to it.
    #[test]
    fn prefix_field_names_never_cross_match() {
        let text = "(\n    noise: 1.0,\n    noise_frequency: 2.2,\n)\n";

        let set_noise = set_top_level_field(text, "noise", "5.5").unwrap();
        assert!(set_noise.contains("noise: 5.5,"), "{set_noise}");
        assert!(set_noise.contains("noise_frequency: 2.2,"), "{set_noise}");

        let set_freq = set_top_level_field(text, "noise_frequency", "7.25").unwrap();
        assert!(set_freq.contains("noise: 1.0,"), "{set_freq}");
        assert!(set_freq.contains("noise_frequency: 7.25,"), "{set_freq}");

        // A name that only exists as a SUFFIX of a real field must not match.
        assert_eq!(
            get_top_level_field_span(text, "frequency"),
            Err(RonEditError::FieldNotFound("frequency".into()))
        );
    }

    /// The gravity_curve shape from docs/design/artificial-planet.md: a
    /// multi-line Some([...]) with nested tuples, negative numbers, and
    /// comments INSIDE the value. Rewriting a sibling must keep all of it;
    /// rewriting the curve itself must replace all of it.
    #[test]
    fn multi_line_option_array_value() {
        let text = concat!(
            "// Agartha: manufactured gravity (see the artificial-planet doc).\n",
            "(\n",
            "    gravity: 9.81, // outer-surface baseline\n",
            "    gravity_curve: Some([\n",
            "        // inner surface: machine-supplied 1 g\n",
            "        (-400000.0, 9.81),\n",
            "        (-250000.0, 0.0), // mid-shell transit band: weightless\n",
            "        (0.0, 9.81),\n",
            "    ]),\n",
            "    label: \"shell\",\n",
            ")\n",
        );

        #[derive(serde::Deserialize)]
        struct Probe {
            gravity: f32,
            gravity_curve: Option<Vec<(f64, f32)>>,
            label: String,
        }

        // The whole multi-line literal is one span.
        let span = get_top_level_field_span(text, "gravity_curve").unwrap();
        let span_text = span.slice(text);
        assert!(span_text.starts_with("Some(["), "{span_text}");
        assert!(span_text.ends_with("])"), "{span_text}");
        assert!(span_text.contains("(-400000.0, 9.81)"), "{span_text}");

        // Rewriting the SIBLING keeps the curve and every comment.
        let sib = set_top_level_field(text, "gravity", "3.71").unwrap();
        let p: Probe = ron::from_str(&sib).unwrap();
        assert!((p.gravity - 3.71).abs() < 1e-6);
        assert_eq!(p.gravity_curve.as_ref().map(|c| c.len()), Some(3));
        assert!(sib.contains("// outer-surface baseline"));
        assert!(sib.contains("// mid-shell transit band: weightless"));

        // Rewriting the curve itself: new multi-line value round-trips.
        let new_curve = "Some([\n        (0.0, 4.9),\n        (100000.0, 2.0),\n    ])";
        let cur = set_top_level_field(text, "gravity_curve", new_curve).unwrap();
        let p: Probe = ron::from_str(&cur).unwrap();
        assert_eq!(p.gravity_curve, Some(vec![(0.0, 4.9), (100000.0, 2.0)]));
        assert_eq!(p.label, "shell");
        let round = get_top_level_field_span(&cur, "gravity_curve").unwrap();
        assert_eq!(round.slice(&cur), new_curve);
        // The comment before the struct survives; comments INSIDE the old
        // value are gone WITH the old value, which is correct: they
        // documented data that no longer exists.
        assert!(cur.contains("// Agartha: manufactured gravity"));
    }

    /// Same-line comment after the value must stay; comment between the
    /// colon and the value must stay too (value_start lands past it).
    #[test]
    fn same_line_comments_around_the_value_survive() {
        let text = "(\n    gravity: /* tuned live */ 9.81, // baseline, do not bump\n)\n";
        let span = get_top_level_field_span(text, "gravity").unwrap();
        assert_eq!(span.slice(text), "9.81");
        let out = set_top_level_field(text, "gravity", "1.62").unwrap();
        assert_eq!(
            out,
            "(\n    gravity: /* tuned live */ 1.62, // baseline, do not bump\n)\n"
        );
    }

    /// A trailing comment BETWEEN the value and its comma (comma on the next
    /// line): the span must end at the value, not swallow the comment.
    #[test]
    fn comment_between_value_and_comma_survives() {
        let text = "(\n    gravity: 9.81 // why: measured\n    ,\n    sea_level: 0.5,\n)\n";
        let out = set_top_level_field(text, "gravity", "2.0").unwrap();
        assert_eq!(
            out,
            "(\n    gravity: 2.0 // why: measured\n    ,\n    sea_level: 0.5,\n)\n"
        );
    }

    /// Field names inside comments and inside string values must never
    /// match: only real depth-1 `name:` positions count.
    #[test]
    fn names_inside_comments_and_strings_do_not_match() {
        let text = concat!(
            "// gravity: 999.0 (this is prose, not a field)\n",
            "(\n",
            "    name: \"gravity: 5 inside a string\",\n",
            "    /* block comment gravity: 777 */\n",
            "    gravity: 9.81,\n",
            ")\n",
        );
        let out = set_top_level_field(text, "gravity", "1.0").unwrap();
        assert!(out.contains("// gravity: 999.0 (this is prose, not a field)"));
        assert!(out.contains("\"gravity: 5 inside a string\""));
        assert!(out.contains("/* block comment gravity: 777 */"));
        assert!(out.contains("gravity: 1.0,"));
        // And the string value is still exactly what it was.
        let name_span = get_top_level_field_span(&out, "name").unwrap();
        assert_eq!(name_span.slice(&out), "\"gravity: 5 inside a string\"");
    }

    /// A nested struct field with the SAME name, appearing BEFORE the real
    /// top-level field, must be skipped: depth-1 only.
    #[test]
    fn nested_field_of_same_name_is_never_matched() {
        let text = concat!(
            "(\n",
            "    core: Some((gravity: 99.0, label: \"inner machine\")),\n",
            "    gravity: 9.81,\n",
            ")\n",
        );

        #[derive(serde::Deserialize)]
        struct Core {
            gravity: f32,
            #[allow(dead_code)]
            label: String,
        }
        #[derive(serde::Deserialize)]
        struct Outer {
            core: Option<Core>,
            gravity: f32,
        }

        let out = set_top_level_field(text, "gravity", "1.62").unwrap();
        let p: Outer = ron::from_str(&out).unwrap();
        assert!((p.gravity - 1.62).abs() < 1e-6, "top-level gravity not rewritten");
        assert!(
            (p.core.unwrap().gravity - 99.0).abs() < 1e-6,
            "nested gravity was wrongly touched"
        );
    }

    /// Missing field is a clear error, never a silent no-op: the caller
    /// (the tuner) decides whether to append.
    #[test]
    fn missing_field_is_a_clear_error() {
        let text = earth_text();
        assert_eq!(
            set_top_level_field(&text, "magnetic_field_t", "31.0e-6"),
            Err(RonEditError::FieldNotFound("magnetic_field_t".into()))
        );
    }

    /// Last field of a struct WITHOUT a trailing comma: the value terminates
    /// at the body's closing paren.
    #[test]
    fn last_field_without_trailing_comma() {
        let text = "(gravity: 9.81)";
        let span = get_top_level_field_span(text, "gravity").unwrap();
        assert_eq!(span.slice(text), "9.81");
        assert_eq!(set_top_level_field(text, "gravity", "1.0").unwrap(), "(gravity: 1.0)");
    }

    /// None to Some and back: Option values are just text spans like any
    /// other, but this is THE tuner flow for optional planet features.
    #[test]
    fn none_to_some_and_back() {
        let text = "(\n    cloud_coverage: None,\n)\n";
        let some = set_top_level_field(text, "cloud_coverage", "Some(0.5)").unwrap();
        assert!(some.contains("cloud_coverage: Some(0.5),"));
        let back = set_top_level_field(&some, "cloud_coverage", "None").unwrap();
        assert_eq!(back, text);
    }

    /// An empty value (nothing between ':' and ',') cannot be located
    /// meaningfully; the error names the field.
    #[test]
    fn empty_value_is_reported() {
        let text = "(\n    gravity: ,\n)\n";
        assert_eq!(
            get_top_level_field_span(text, "gravity"),
            Err(RonEditError::EmptyValue("gravity".into()))
        );
    }

    /// Files that are not struct literals fail fast instead of scanning
    /// garbage.
    #[test]
    fn non_struct_text_is_rejected() {
        assert_eq!(
            get_top_level_field_span("[1, 2, 3]", "gravity"),
            Err(RonEditError::NoStructBody)
        );
        assert_eq!(
            get_top_level_field_span("// only comments\n", "gravity"),
            Err(RonEditError::NoStructBody)
        );
    }

    /// The replacement validator: shapes that would corrupt the file are
    /// rejected BEFORE any splice happens.
    #[test]
    fn corrupting_replacements_are_rejected() {
        let text = "(\n    gravity: 9.81,\n)\n";
        // Unbalanced delimiters
        assert!(matches!(
            set_top_level_field(text, "gravity", "Some((1.0, 2.0)"),
            Err(RonEditError::InvalidReplacement(_))
        ));
        // Two values via a top-level comma
        assert!(matches!(
            set_top_level_field(text, "gravity", "1.0, extra: 2.0"),
            Err(RonEditError::InvalidReplacement(_))
        ));
        // A line comment would swallow the file's own comma after the splice
        assert!(matches!(
            set_top_level_field(text, "gravity", "1.0 // tuned"),
            Err(RonEditError::InvalidReplacement(_))
        ));
        // Empty / whitespace-only
        assert!(matches!(
            set_top_level_field(text, "gravity", "   "),
            Err(RonEditError::InvalidReplacement(_))
        ));
        // A block comment inside the replacement is fine (self-terminating).
        let ok = set_top_level_field(text, "gravity", "/* 2x earth radius, 1 g */ 9.81").unwrap();
        assert!(ok.contains("gravity: /* 2x earth radius, 1 g */ 9.81,"));
    }

    /// Escaped quotes inside strings must not end the string early: the
    /// text after a `\"` is still string content, so a `,` there cannot
    /// terminate the value.
    #[test]
    fn escaped_quotes_inside_strings() {
        let text = "(\n    name: \"the \\\"blue\\\" marble, isn't it\",\n    gravity: 9.81,\n)\n";
        let span = get_top_level_field_span(text, "name").unwrap();
        assert_eq!(span.slice(text), "\"the \\\"blue\\\" marble, isn't it\"");
        let out = set_top_level_field(text, "gravity", "1.0").unwrap();
        assert!(out.contains("gravity: 1.0,"));
        assert!(out.contains("\"the \\\"blue\\\" marble, isn't it\""));
    }

    /// The other shipped planet files must walk cleanly too: the tuner will
    /// open whichever body is frame-locked, not just Earth.
    #[test]
    fn all_shipped_planet_files_walk() {
        for id in ["earth", "moon", "mars", "pluto"] {
            let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("data/planets/{id}.ron"));
            let text = std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("{} must be readable ({e})", p.display()));
            let fields = top_level_fields(&text)
                .unwrap_or_else(|e| panic!("{id}.ron did not walk: {e}"));
            assert!(!fields.is_empty(), "{id}.ron walked to zero fields");
            // Every shipped def declares at least these two.
            let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
            assert!(names.contains(&"radius"), "{id}.ron has no radius field");
            assert!(names.contains(&"gravity"), "{id}.ron has no gravity field");
        }
    }
}
