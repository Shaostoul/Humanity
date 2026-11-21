//! Cutting text without cutting a character in half (BUG-340, 2026-11-21).
//!
//! Rust panics when a `&str` is sliced at a byte that is not a character boundary. A cut like
//! `&key[..12.min(key.len())]` reads as safe (it minds the length) but counts BYTES: when byte 12
//! falls inside an accented letter, a CJK character or an emoji, the thread panics. On the main
//! frame thread that is the whole desktop app, and when the text came from a server or another
//! person (a key card that failed its check, a sealed message's sender, a contact request), any
//! of them could crash it with one frame, again on every reconnect.
//!
//! So every prefix or suffix cut of text in src/ and crates/ goes through here, cutting by
//! CHARACTERS (or, for a byte budget, back to the nearest character boundary), never panicking.
//! `tests/text_cut_lint.rs` fails on a new byte-range slice of a string; a reviewed byte cut on
//! text known to be ASCII carries a `text-cut-ok:` comment saying why it is ASCII.
//!
//! Ungated (plain `str` work), so the app, the server and the web build share it.

/// The first `n` characters of `s` (all of `s` when it has `n` or fewer). Never panics.
///
/// `prefix("café au lait", 4) == "café"`, where `&s[..4]` would panic inside the "é".
pub fn prefix(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        // `i` is where character number `n` starts: a character boundary by construction.
        Some((i, _)) => s.split_at(i).0,
        None => s,
    }
}

/// The last `n` characters of `s` (all of `s` when it has `n` or fewer). Never panics.
pub fn suffix(s: &str, n: usize) -> &str {
    if n == 0 {
        return s.split_at(s.len()).1;
    }
    match s.char_indices().rev().nth(n - 1) {
        Some((i, _)) => s.split_at(i).1,
        None => s,
    }
}

/// The longest start of `s` that is at most `max_bytes` bytes long and ends on a character
/// boundary: for a limit counted in bytes (a wire field, a log line, a file name), where a cut
/// by characters could still run over. Never panics.
pub fn prefix_bytes(s: &str, max_bytes: usize) -> &str {
    s.split_at(s.floor_char_boundary(max_bytes)).0
}

/// The longest end of `s` that is at most `max_bytes` bytes long and starts on a character
/// boundary. Never panics.
pub fn suffix_bytes(s: &str, max_bytes: usize) -> &str {
    s.split_at(s.ceil_char_boundary(s.len().saturating_sub(max_bytes))).1
}

/// `s` held to `n` characters, with "…" after the first `n` when anything was cut (the short form
/// of a long key or name in a line people read). Short text comes back whole, with no mark.
pub fn clip(s: &str, n: usize) -> std::borrow::Cow<'_, str> {
    let head = prefix(s, n);
    if head.len() == s.len() {
        head.into()
    } else {
        format!("{head}…").into()
    }
}

/// The first `head` and last `tail` characters of `s` with `sep` between them, or all of `s`
/// when it has no more than `head + tail` characters (the "3f9a1c…b7e2" form of a long key).
pub fn ends<'a>(s: &'a str, head: usize, tail: usize, sep: &str) -> std::borrow::Cow<'a, str> {
    if s.chars().count() <= head + tail {
        s.into()
    } else {
        format!("{}{sep}{}", prefix(s, head), suffix(s, tail)).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text with characters of 1, 2, 3 and 4 bytes, so a cut lands on every kind of boundary and
    /// inside every kind of character.
    const MIXED: &str = "a\u{e9}\u{20ac}\u{1f600}b\u{20ac}\u{e9}\u{1f600}c";

    /// Seen red 2026-11-21 with `prefix`'s body put back to the old byte cut
    /// (`&s[..n.min(s.len())]`): "end byte index 2 is not a char boundary; it is inside 'é' (bytes
    /// 1..3 of string)", in this test and in `clip_and_ends_mark_only_what_was_cut`.
    #[test]
    fn prefix_takes_characters_at_every_cut() {
        let chars: Vec<char> = MIXED.chars().collect();
        for n in 0..=chars.len() + 2 {
            let want: String = chars.iter().take(n).collect();
            assert_eq!(prefix(MIXED, n), want, "prefix {n}");
        }
        assert_eq!(prefix("", 5), "");
        assert_eq!(prefix("café au lait", 4), "café");
    }

    #[test]
    fn suffix_takes_characters_at_every_cut() {
        let chars: Vec<char> = MIXED.chars().collect();
        for n in 0..=chars.len() + 2 {
            let want: String = chars[chars.len().saturating_sub(n)..].iter().collect();
            assert_eq!(suffix(MIXED, n), want, "suffix {n}");
        }
        assert_eq!(suffix("", 3), "");
    }

    /// Every byte budget from 0 past the end: the cut is a boundary, within the budget, and the
    /// longest such cut (one more character would run over).
    #[test]
    fn byte_cuts_stay_on_boundaries_within_the_budget() {
        for max in 0..=MIXED.len() + 2 {
            let p = prefix_bytes(MIXED, max);
            assert!(p.len() <= max && MIXED.starts_with(p), "prefix_bytes {max}");
            let next = MIXED.strip_prefix(p).and_then(|r| r.chars().next()).map_or(0, char::len_utf8);
            assert!(p.len() == MIXED.len() || p.len() + next > max, "prefix_bytes {max} stopped short");
            let s = suffix_bytes(MIXED, max);
            assert!(s.len() <= max && MIXED.ends_with(s), "suffix_bytes {max}");
            let prev = MIXED.strip_suffix(s).and_then(|r| r.chars().next_back()).map_or(0, char::len_utf8);
            assert!(s.len() == MIXED.len() || s.len() + prev > max, "suffix_bytes {max} stopped short");
        }
    }

    #[test]
    fn clip_and_ends_mark_only_what_was_cut() {
        assert_eq!(clip("short", 12), "short");
        assert_eq!(clip("0123456789abcdef", 12), "0123456789ab…");
        assert_eq!(clip(MIXED, 3), "a\u{e9}\u{20ac}\u{2026}");
        assert_eq!(ends("0123456789abcdef0123", 8, 4, "…"), "01234567…0123");
        assert_eq!(ends("short", 8, 4, "…"), "short");
        assert_eq!(ends(MIXED, 2, 2, "..."), "a\u{e9}...\u{1f600}c");
    }
}
