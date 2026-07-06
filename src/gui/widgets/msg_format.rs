//! Inline chat-message formatting: minimal markdown + URL detection (v0.702).
//!
//! Native chat's help modal has advertised markdown since the chat shipped,
//! but messages rendered as plain text (2026-07-04 parity audit, top-ranked
//! native chat gap). This module closes that honestly: a PURE parser that
//! turns raw message content into (display_text, spans), where the marker
//! characters are stripped from display_text and each span says how a char
//! range of the display text should be styled. `widgets::row::message_row`
//! consumes the spans when it builds its LayoutJob, exactly like the
//! existing @mention ranges (which stay a separate, caller-resolved concept
//! because they need the user list).
//!
//! NOT a duplicate of `widgets::markdown`: that one is a BLOCK-level document
//! reader (headings/bullets/paragraphs for the Library + Accord panes) that
//! strips inline emphasis and emits separate labels. This one produces inline
//! styled spans for a single message galley, including clickable links, which
//! the doc reader neither needs nor supports.
//!
//! Supported, deliberately small (matches the web client's dialect):
//!   **bold**       -> Bold   (rendered brighter; the loaded font has no
//!                             bold face, and WHITE-as-bold is the repo's
//!                             established convention, see message_row's
//!                             header name)
//!   *italic*       -> Italic (egui TextFormat.italics)
//!   `code`         -> Code   (monospace + subtle background)
//!   ~~strike~~     -> Strike (TextFormat.strikethrough)
//!   http(s)://...  -> Link   (accent + underline + clickable; the URL is
//!                             carried in the span)
//!
//! Rules that keep this safe and predictable:
//! - Markers must be PAIRED on the same line; an unclosed marker renders
//!   verbatim (nothing is silently eaten).
//! - No styling nests inside `code` spans, and URLs inside code stay text.
//! - An empty pair (e.g. ****) renders verbatim.
//! - Char-indexed throughout (the LayoutJob/mention machinery is char-based).

/// How one char range of the display text should be styled.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanKind {
    Bold,
    Italic,
    Code,
    Strike,
    /// The URL as it appears in the display text (used by the click handler).
    Link(String),
}

/// One styled span: `(char_start, char_len)` into the DISPLAY text.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatSpan {
    pub start: usize,
    pub len: usize,
    pub kind: SpanKind,
}

/// Parse raw message content -> (display text with markers stripped, spans).
/// Pure and panic-free; pathological input just renders verbatim.
pub fn parse(content: &str) -> (String, Vec<FormatSpan>) {
    let chars: Vec<char> = content.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut spans: Vec<FormatSpan> = Vec::new();
    let mut i = 0usize;

    // Find the closing marker (same line, non-empty body). Returns the char
    // index where `marker` begins.
    let find_close = |chars: &[char], from: usize, marker: &[char]| -> Option<usize> {
        let mut j = from;
        while j + marker.len() <= chars.len() {
            if chars[j] == '\n' {
                return None;
            }
            if chars[j..j + marker.len()] == *marker {
                // Non-empty body required.
                return if j > from { Some(j) } else { None };
            }
            j += 1;
        }
        None
    };

    while i < chars.len() {
        let rest = &chars[i..];

        // URL detection (outside code spans; code is handled before we ever
        // get here because ` consumes through its closing tick).
        let is_url_start = rest.starts_with(&['h', 't', 't', 'p', ':', '/', '/'])
            || rest.starts_with(&['h', 't', 't', 'p', 's', ':', '/', '/']);
        if is_url_start {
            let mut j = i;
            while j < chars.len() && !chars[j].is_whitespace() {
                j += 1;
            }
            // Trim common trailing punctuation that is sentence, not URL.
            let mut end = j;
            while end > i {
                match chars[end - 1] {
                    '.' | ',' | ')' | ']' | '!' | '?' | ';' | ':' | '\'' | '"' => end -= 1,
                    _ => break,
                }
            }
            if end > i + 8 {
                let url: String = chars[i..end].iter().collect();
                spans.push(FormatSpan {
                    start: out.len(),
                    len: end - i,
                    kind: SpanKind::Link(url),
                });
                out.extend(&chars[i..end]);
                i = end;
                continue;
            }
        }

        // Paired markers, longest first so ** beats *.
        let mut matched = false;
        for (marker, kind) in [
            (&['*', '*'][..], SpanKind::Bold),
            (&['~', '~'][..], SpanKind::Strike),
            (&['*'][..], SpanKind::Italic),
            (&['`'][..], SpanKind::Code),
        ] {
            if rest.starts_with(marker) {
                if let Some(close) = find_close(&chars, i + marker.len(), marker) {
                    let body = &chars[i + marker.len()..close];
                    spans.push(FormatSpan {
                        start: out.len(),
                        len: body.len(),
                        kind: kind.clone(),
                    });
                    out.extend(body);
                    i = close + marker.len();
                } else {
                    // Unclosed: emit the WHOLE marker verbatim and move past
                    // it, so its tail chars can't re-match as a shorter
                    // marker (`**x ... *y*` must not turn the second `*` of
                    // the dead `**` into an italic opener).
                    out.extend(marker);
                    i += marker.len();
                }
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    (out.into_iter().collect(), spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through_unchanged() {
        let (d, s) = parse("hello world, no markup here");
        assert_eq!(d, "hello world, no markup here");
        assert!(s.is_empty());
    }

    #[test]
    fn bold_italic_code_strike_all_strip_markers() {
        let (d, s) = parse("**b** *i* `c` ~~s~~");
        assert_eq!(d, "b i c s");
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], FormatSpan { start: 0, len: 1, kind: SpanKind::Bold });
        assert_eq!(s[1], FormatSpan { start: 2, len: 1, kind: SpanKind::Italic });
        assert_eq!(s[2], FormatSpan { start: 4, len: 1, kind: SpanKind::Code });
        assert_eq!(s[3], FormatSpan { start: 6, len: 1, kind: SpanKind::Strike });
    }

    #[test]
    fn unclosed_markers_render_verbatim() {
        let (d, s) = parse("**not closed and *also open");
        assert_eq!(d, "**not closed and *also open");
        assert!(s.is_empty());
    }

    #[test]
    fn empty_pairs_render_verbatim() {
        let (d, s) = parse("**** and ``");
        assert_eq!(d, "**** and ``");
        assert!(s.is_empty());
    }

    #[test]
    fn markers_do_not_pair_across_lines() {
        let (d, s) = parse("**line one\nline two**");
        assert_eq!(d, "**line one\nline two**");
        assert!(s.is_empty());
    }

    #[test]
    fn urls_are_linked_and_trailing_punctuation_stays_text() {
        let (d, s) = parse("see https://united-humanity.us/home, then reply");
        assert_eq!(d, "see https://united-humanity.us/home, then reply");
        assert_eq!(s.len(), 1);
        let FormatSpan { start, len, kind } = &s[0];
        assert_eq!(*start, 4);
        assert_eq!(*len, "https://united-humanity.us/home".chars().count());
        assert_eq!(*kind, SpanKind::Link("https://united-humanity.us/home".to_string()));
    }

    #[test]
    fn url_inside_code_span_stays_plain_code() {
        let (d, s) = parse("`https://example.com`");
        assert_eq!(d, "https://example.com");
        assert_eq!(s.len(), 1);
        assert!(matches!(s[0].kind, SpanKind::Code));
    }

    #[test]
    fn code_span_protects_markers_inside() {
        let (d, s) = parse("`a ** b`");
        assert_eq!(d, "a ** b");
        assert_eq!(s.len(), 1);
        assert!(matches!(s[0].kind, SpanKind::Code));
    }

    #[test]
    fn mixed_message_maps_spans_to_display_positions() {
        let (d, s) = parse("try **this** at https://a.example/x now");
        assert_eq!(d, "try this at https://a.example/x now");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0], FormatSpan { start: 4, len: 4, kind: SpanKind::Bold });
        assert_eq!(s[1].start, 12);
        assert_eq!(s[1].kind, SpanKind::Link("https://a.example/x".to_string()));
    }

    #[test]
    fn multibyte_chars_are_char_indexed_not_byte_indexed() {
        let (d, s) = parse("héllo **wörld** ok");
        assert_eq!(d, "héllo wörld ok");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0], FormatSpan { start: 6, len: 5, kind: SpanKind::Bold });
    }
}
