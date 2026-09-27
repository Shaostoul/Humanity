//! A chat message's text, parsed (moved out of chat.rs on 2026-09-27 to keep
//! that file inside its line budget, tests/file_size_ratchet.rs): the one fence
//! scanner, the code blocks the Chat page draws as panels, the list bullets, and
//! the one-line form the in-world HUD feed draws. Chat and the HUD both read the
//! dialect through these, so they cannot disagree about what the markup means.

use egui::{Frame, RichText, Rounding, Stroke};
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// One fenced code block pulled out of a chat message by `extract_code_blocks`.
pub(crate) struct CodeBlock {
    /// Optional language token from the opening fence (```rust -> "rust"). May
    /// be empty. Shown as a small label; not used for syntax highlighting yet.
    pub(crate) lang: String,
    /// The code between the fences, verbatim (newlines preserved).
    pub(crate) code: String,
}

/// Pull fenced ```code``` blocks out of a message body, mirroring the web
/// client's `formatBody` step 1 (`/```(\w*)\n?([\s\S]*?)```/`). Returns the text
/// with the fenced regions removed and the ordered list of blocks to render as
/// separate monospace panels. Extracting BEFORE the inline parser is what keeps a
/// code block's own backticks / `*` / URLs from being mis-parsed as inline
/// markup. An UNCLOSED opening fence is left verbatim (nothing is silently eaten),
/// matching the "unclosed marker renders as text" rule in msg_format.
pub(crate) fn extract_code_blocks(s: &str) -> (String, Vec<CodeBlock>) {
    if !s.contains("```") {
        return (s.to_string(), Vec::new());
    }
    let mut text = String::with_capacity(s.len());
    let mut blocks: Vec<CodeBlock> = Vec::new();
    for seg in fence_segments(s) {
        match seg {
            FenceSeg::Text(t) => text.push_str(&t),
            FenceSeg::Code(cb) => blocks.push(cb),
        }
    }
    // Trim the blank lines the extraction can leave where a block used to be, so
    // the surrounding prose doesn't render with stray empty rows.
    let text = text.trim_matches('\n').to_string();
    (text, blocks)
}

/// One piece of a message body split at its fenced code blocks.
enum FenceSeg {
    /// Prose between fences (or the whole body when there are none). An
    /// UNCLOSED opening fence stays in here verbatim.
    Text(String),
    Code(CodeBlock),
}

/// Split a message body into prose and fenced ```code``` blocks, IN ORDER.
/// The one fence scanner: the Chat page (`extract_code_blocks`, which draws
/// the blocks as panels under the text) and the in-world feed
/// (`one_line_formatted`, which keeps each block inline where it sat) both
/// read the dialect through it, so they cannot disagree about what a fence is.
fn fence_segments(s: &str) -> Vec<FenceSeg> {
    const FENCE: &str = "```";
    let mut segs: Vec<FenceSeg> = Vec::new();
    let mut rest = s;
    loop {
        match rest.find(FENCE) {
            None => {
                if !rest.is_empty() {
                    segs.push(FenceSeg::Text(rest.to_string()));
                }
                break;
            }
            Some(open) => {
                let after_open = &rest[open + FENCE.len()..];
                match after_open.find(FENCE) {
                    None => {
                        // No closing fence: leave the rest (incl. the ```) as text.
                        segs.push(FenceSeg::Text(rest.to_string()));
                        break;
                    }
                    Some(close_rel) => {
                        if open > 0 {
                            segs.push(FenceSeg::Text(rest[..open].to_string()));
                        }
                        let inner = &after_open[..close_rel];
                        let (lang, code) = split_fence_lang(inner);
                        segs.push(FenceSeg::Code(CodeBlock { lang, code }));
                        let tail = &after_open[close_rel + FENCE.len()..];
                        // Swallow one newline right after the closing fence so the
                        // following prose doesn't render with a leading blank line
                        // where the block used to sit.
                        rest = tail
                            .strip_prefix("\r\n")
                            .or_else(|| tail.strip_prefix('\n'))
                            .unwrap_or(tail);
                    }
                }
            }
        }
    }
    segs
}

/// A chat message as ONE styled line, for a surface with room for nothing
/// else: the in-world HUD feed, a fixed grid of one row per message.
///
/// It reads the same dialect through the same steps as the Chat page
/// (fences, then `bulletize_list_lines`, then `msg_format::parse`), so bold,
/// italic, code, strike, links and quotes keep their styling. Only the
/// block-level pieces change shape, because a single row cannot hold a
/// block: a fenced code block becomes an inline code span where it sat, with
/// its whitespace collapsed; a list item keeps its bullet; and every line
/// break becomes one space. Returns display text plus char-indexed spans,
/// the same shape as `msg_format::parse`, so the caller styles it with
/// `widgets::row::append_formatted`, the function the Chat page uses.
pub(crate) fn one_line_formatted(
    content: &str,
) -> (String, Vec<crate::gui::widgets::msg_format::FormatSpan>) {
    use crate::gui::widgets::msg_format::{parse, FormatSpan, SpanKind};
    let mut text: Vec<char> = Vec::with_capacity(content.len());
    let mut spans: Vec<FormatSpan> = Vec::new();
    for seg in fence_segments(content) {
        // A line break between pieces; the collapse below turns it into one
        // space (or nothing, at the very start).
        text.push('\n');
        match seg {
            FenceSeg::Text(t) => {
                let (display, piece_spans) = parse(&bulletize_list_lines(&t));
                let base = text.len();
                text.extend(display.chars());
                spans.extend(piece_spans.into_iter().map(|mut sp| {
                    sp.start += base;
                    sp
                }));
            }
            FenceSeg::Code(cb) => {
                let flat = cb.code.split_whitespace().collect::<Vec<_>>().join(" ");
                let base = text.len();
                let len = flat.chars().count();
                if len > 0 {
                    spans.push(FormatSpan { start: base, len, kind: SpanKind::Code });
                }
                text.extend(flat.chars());
            }
        }
    }

    // Collapse every run of whitespace (line breaks included) to one space,
    // drop it at both ends, and carry each span across the removed chars.
    // `new_at[i]` is where old char i lands (or would land) in the output.
    let n = text.len();
    let mut out: Vec<char> = Vec::with_capacity(n);
    let mut new_at: Vec<usize> = vec![0; n + 1];
    let mut after_space = true; // true at the start drops leading whitespace
    for (i, &c) in text.iter().enumerate() {
        new_at[i] = out.len();
        if c.is_whitespace() {
            if !after_space {
                out.push(' ');
                after_space = true;
            }
        } else {
            out.push(c);
            after_space = false;
        }
    }
    new_at[n] = out.len();
    if out.last() == Some(&' ') {
        out.pop();
    }
    let out_len = out.len();
    let spans = spans
        .into_iter()
        .filter_map(|sp| {
            let start = new_at[sp.start.min(n)].min(out_len);
            let end = new_at[(sp.start + sp.len).min(n)].min(out_len);
            (end > start).then(|| FormatSpan { start, len: end - start, kind: sp.kind })
        })
        .collect();
    (out.into_iter().collect(), spans)
}

/// Split the inside of a fence into an optional language token and the code,
/// matching web's `formatBody` regex `/```(\w*)\n?([\s\S]*?)```/` exactly so the
/// two clients read the same dialect: a leading run of word chars (`[A-Za-z0-9_]`)
/// is the language, then ONE optional newline is consumed, and the rest (trailing
/// newlines trimmed) is the code.
fn split_fence_lang(inner: &str) -> (String, String) {
    let lang_end = inner
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map(|(i, _)| i)
        .unwrap_or(inner.len());
    let lang = &inner[..lang_end];
    let mut code = &inner[lang_end..];
    // Consume a single optional newline right after the language token (CRLF too).
    if let Some(stripped) = code.strip_prefix("\r\n") {
        code = stripped;
    } else if let Some(stripped) = code.strip_prefix('\n') {
        code = stripped;
    }
    (lang.to_string(), code.trim_end_matches(['\n', '\r']).to_string())
}

/// Draw extracted fenced code blocks as indented monospace panels, each with an
/// optional language label and a Copy button (parity with web's
/// `code-block-wrapper`). Rendered directly under the message text.
pub(crate) fn draw_code_blocks(ui: &mut egui::Ui, theme: &Theme, blocks: &[CodeBlock]) {
    let indent = theme.avatar_size + theme.avatar_gap;
    let panel_w = (ui.available_width() - indent - 8.0).max(80.0);
    for cb in blocks {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                // Bound the width so long code lines wrap inside the column
                // instead of stretching the horizontal layout off the edge.
                ui.set_max_width(panel_w);
                Frame::none()
                    .fill(theme.bg_card())
                    .stroke(Stroke::new(theme.border_width, theme.border()))
                    .rounding(Rounding::same(4))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if !cb.lang.is_empty() {
                                ui.label(
                                    RichText::new(&cb.lang)
                                        .monospace()
                                        .size(theme.small_size)
                                        .color(theme.text_muted()),
                                );
                            }
                            if widgets::Button::ghost("Copy").show(ui, theme) {
                                ui.ctx().copy_text(cb.code.clone());
                            }
                        });
                        ui.label(
                            RichText::new(&cb.code)
                                .monospace()
                                .color(theme.text_primary()),
                        );
                    });
            });
        });
        ui.add_space(2.0);
    }
}

/// Render line-leading list markers as a real bullet on native, matching the
/// web client, whose `formatBody` turns `- x` / `* x` into a `<ul>` list.
///
/// This is a WIDTH-PRESERVING text substitution done BEFORE
/// `msg_format::parse`, which is the whole reason it's safe: the 2-char marker
/// `"- "` / `"* "` becomes the 2-char `"\u{2022} "` (bullet + space), so the
/// char-indexed inline spans (bold/italic/mention/link) computed by `parse`
/// stay perfectly aligned. `\u{2022}` (•) is in the General Punctuation block,
/// which the app font renders reliably.
///
/// Requires the trailing space (same rule as web's `/^[-*] /`), so an inline
/// `*italic*` and a bare `-` in prose are left alone. Leading indentation is
/// preserved so nested-looking lists keep their indent. Block-level quotes and
/// headings remain a follow-up (native `msg_format` is inline-only).
pub(crate) fn bulletize_list_lines(s: &str) -> String {
    // Fast path: the overwhelmingly common message has no line-leading marker,
    // so avoid rebuilding the string. `.any` over the (usually one) line is cheap.
    let has_marker = s.split('\n').any(|l| {
        let t = l.trim_start();
        t.starts_with("- ") || t.starts_with("* ")
    });
    if !has_marker {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let indent_len = line.len() - line.trim_start().len();
        let (indent, rest) = line.split_at(indent_len);
        if let Some(after) = rest.strip_prefix("- ").or_else(|| rest.strip_prefix("* ")) {
            out.push_str(indent);
            out.push('\u{2022}'); // • bullet, General Punctuation (font-safe)
            out.push(' ');
            out.push_str(after);
        } else {
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod code_block_tests {
    use super::extract_code_blocks;

    #[test]
    fn no_fence_is_untouched() {
        let (t, b) = extract_code_blocks("just a plain message with `inline` code");
        assert_eq!(t, "just a plain message with `inline` code");
        assert!(b.is_empty());
    }

    #[test]
    fn a_fenced_block_with_language_is_extracted() {
        let (t, b) = extract_code_blocks("before\n```rust\nlet x = 1;\n```\nafter");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lang, "rust");
        assert_eq!(b[0].code, "let x = 1;");
        // The surrounding prose survives; the fence is gone.
        assert!(t.contains("before"));
        assert!(t.contains("after"));
        assert!(!t.contains("```"));
        assert!(!t.contains("let x = 1;"));
    }

    #[test]
    fn a_fence_without_language_keeps_the_first_line_as_code() {
        let (_t, b) = extract_code_blocks("```\nplain code\nline two\n```");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lang, "");
        assert_eq!(b[0].code, "plain code\nline two");
    }

    #[test]
    fn language_token_is_the_leading_word_chars_matching_web() {
        // Mirrors web's /```(\w*)\n?.../: "echo" is word chars so it's the lang;
        // the space stops the token and " hello\nworld" is the code (one optional
        // newline after the lang would have been consumed, but here it's a space).
        let (_t, b) = extract_code_blocks("```echo hello\nworld\n```");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lang, "echo");
        assert_eq!(b[0].code, " hello\nworld");
    }

    #[test]
    fn unclosed_fence_is_left_verbatim() {
        let (t, b) = extract_code_blocks("look: ```rust\nlet x = 1; (never closed)");
        assert!(b.is_empty());
        assert_eq!(t, "look: ```rust\nlet x = 1; (never closed)");
    }

    #[test]
    fn two_blocks_are_both_extracted_in_order() {
        let (_t, b) = extract_code_blocks("```py\na\n```\nmid\n```js\nb\n```");
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].lang, "py");
        assert_eq!(b[0].code, "a");
        assert_eq!(b[1].lang, "js");
        assert_eq!(b[1].code, "b");
    }
}

/// The in-world feed's one-line form of a message (2026-09-27): the markup is
/// read, never shown, and every span still covers the text it styled.
#[cfg(test)]
mod one_line_tests {
    use super::one_line_formatted;
    use crate::gui::widgets::msg_format::SpanKind;

    /// The text a span covers, for asserting against.
    fn covered(text: &str, start: usize, len: usize) -> String {
        text.chars().skip(start).take(len).collect()
    }

    #[test]
    fn a_fenced_block_becomes_inline_code_where_it_sat() {
        let (t, s) = one_line_formatted(
            "Here's the tick loop:\n```rust\nfor sys in systems {\n    sys.tick(dt);\n}\n```\nLooks good to me.",
        );
        assert_eq!(t, "Here's the tick loop: for sys in systems { sys.tick(dt); } Looks good to me.");
        assert!(!t.contains('`'), "no fence may be painted: {t}");
        assert!(!t.contains("rust"), "the fence's language token is not prose: {t}");
        let code: Vec<_> = s.iter().filter(|sp| sp.kind == SpanKind::Code).collect();
        assert_eq!(code.len(), 1);
        assert_eq!(covered(&t, code[0].start, code[0].len), "for sys in systems { sys.tick(dt); }");
    }

    #[test]
    fn bold_quote_and_bullets_fold_onto_one_line_with_spans_aligned() {
        let (t, s) = one_line_formatted(
            "> from the design notes\nWe should try __two towers__ per plot:\n- more light\n- easier harvest",
        );
        assert_eq!(
            t,
            "| from the design notes We should try two towers per plot: \u{2022} more light \u{2022} easier harvest"
        );
        assert!(!t.contains('\n') && !t.contains("__"));
        let bold: Vec<_> = s.iter().filter(|sp| sp.kind == SpanKind::Bold).collect();
        assert_eq!(bold.len(), 1);
        assert_eq!(covered(&t, bold[0].start, bold[0].len), "two towers");
        let quote: Vec<_> = s.iter().filter(|sp| sp.kind == SpanKind::Quote).collect();
        assert_eq!(quote.len(), 1);
        assert_eq!(covered(&t, quote[0].start, quote[0].len), "| from the design notes");
    }

    #[test]
    fn inline_markers_and_links_are_styled_not_shown() {
        let (t, s) = one_line_formatted("see **this** and *that* `x` ~~old~~ at https://a.example/x");
        assert_eq!(t, "see this and that x old at https://a.example/x");
        let kinds: Vec<_> = s.iter().map(|sp| (covered(&t, sp.start, sp.len), sp.kind.clone())).collect();
        assert!(kinds.contains(&("this".into(), SpanKind::Bold)));
        assert!(kinds.contains(&("that".into(), SpanKind::Italic)));
        assert!(kinds.contains(&("x".into(), SpanKind::Code)));
        assert!(kinds.contains(&("old".into(), SpanKind::Strike)));
        assert!(kinds.contains(&(
            "https://a.example/x".into(),
            SpanKind::Link("https://a.example/x".into())
        )));
    }

    #[test]
    fn plain_text_is_untouched_and_whitespace_runs_collapse() {
        let (t, s) = one_line_formatted("  hello   there\n\n  friend  ");
        assert_eq!(t, "hello there friend");
        assert!(s.is_empty());
    }

    #[test]
    fn an_unclosed_fence_stays_verbatim_like_the_chat_page() {
        let (t, _) = one_line_formatted("look: ```rust\nlet x = 1;");
        assert_eq!(t, "look: ```rust let x = 1;");
    }
}

#[cfg(test)]
mod bulletize_tests {
    use super::bulletize_list_lines;

    #[test]
    fn plain_message_is_untouched() {
        assert_eq!(bulletize_list_lines("hello there"), "hello there");
        assert_eq!(bulletize_list_lines("line one\nline two"), "line one\nline two");
    }

    #[test]
    fn dash_and_star_line_starts_become_a_bullet() {
        assert_eq!(bulletize_list_lines("- apples\n- pears"), "\u{2022} apples\n\u{2022} pears");
        assert_eq!(bulletize_list_lines("* one"), "\u{2022} one");
    }

    #[test]
    fn leading_indentation_is_preserved() {
        assert_eq!(bulletize_list_lines("  - nested"), "  \u{2022} nested");
    }

    #[test]
    fn inline_emphasis_and_mid_line_dashes_are_left_alone() {
        // No trailing space after the leading `*` => an italic run, not a list.
        assert_eq!(bulletize_list_lines("*italic* text"), "*italic* text");
        // A dash that isn't at the line start is prose, not a bullet.
        assert_eq!(bulletize_list_lines("a - b"), "a - b");
        // A bare marker with no following space is left as-is.
        assert_eq!(bulletize_list_lines("-x"), "-x");
    }

    #[test]
    fn substitution_is_width_preserving_so_inline_spans_stay_aligned() {
        // The whole point: "- " (2 chars) -> "\u{2022} " (2 chars), so a bold
        // run later on the same line keeps its char offsets. The char count of
        // each line must be identical before and after.
        let src = "- buy *milk* today";
        let out = bulletize_list_lines(src);
        assert_eq!(src.chars().count(), out.chars().count());
        assert!(out.starts_with("\u{2022} buy *milk* today"));
    }
}
