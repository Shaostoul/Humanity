//! Minimal markdown reader shared by the in-app doc surfaces: the Humanity Accord
//! viewer modal (`pages/humanity.rs`) and the Library doc pane (`pages/library.rs`).
//!
//! Renders headings (`#`, `##`, `###`), bullets (`-`, `*`), horizontal rules
//! (`---`), pipe tables, and paragraphs; inline emphasis markers (`**`, `*`,
//! `` ` ``) are stripped for plain, readable text styled by the theme. Not a
//! full parser, just enough to read a document cleanly. The bullet glyph is
//! U+00B7 ("·"), a confirmed-rendering symbol in the bundled font.
//!
//! Wrapped source lines are JOINED before rendering, which markdown requires and
//! this did not do until v0.1305: a paragraph or a bullet may span as many source
//! lines as it likes and only a blank line ends it. Before that, every
//! hard-wrapped document rendered as a stack of one-line fragments, and a bullet
//! ended at its first continuation line (42 of the 81 Library documents wrap
//! their bullets). Tables landed in the same pass; 14 documents already used
//! them and showed raw pipe text.
//!
//! Mirrored by the web renderer, `web/shared/markdown.js`. Keep the two in step:
//! they render the SAME files from `data/library/`.

use egui::{Frame, Label, RichText};
use crate::gui::theme::Theme;

/// One heading found in a document: its level (1, 2 or 3), its visible text,
/// its slug, and the source line it sits on.
///
/// The line number is what lets a caller ask "which section is this match in?"
/// without walking the document a second time, which is the only way two walks
/// cannot disagree about fences or duplicate slugs.
#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub line: usize,
}

/// GitHub's heading-anchor grammar: lowercase, every run of non-alphanumerics
/// becomes one hyphen, ends trimmed. Both clients derive heading anchors this
/// way, the same way they already share the document slug grammar, so a link to
/// a section resolves identically in the app and on the website.
pub fn heading_slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut dash = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if c.is_alphanumeric() {
            // Keep non-ASCII letters rather than mangling them to hyphens.
            out.extend(c.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// GitHub's duplicate-anchor rule: the second heading that slugs to
/// `section-1` becomes `section-1-1`, the third `section-1-2`.
///
/// Not an edge case here. The Constitution has a "Section 1" under Article I,
/// another under Article II and another under Article III, nine repeated
/// headings in all; without this every one of them anchors to the first, so the
/// outline would silently send a reader looking for Article III to Article I.
fn dedupe_slug(seen: &mut std::collections::HashMap<String, usize>, base: String) -> String {
    let n = seen.entry(base.clone()).or_insert(0);
    let out = if *n == 0 { base } else { format!("{base}-{n}") };
    *n += 1;
    out
}

/// Every `#`, `##` and `###` heading in `md`, in document order.
///
/// The Library builds its "Contents" outline from this rather than re-deriving
/// markdown rules, so the outline can never list a heading the renderer does not
/// draw, or miss one it does. Both walk the document the same way, code fences
/// included: a `# comment` line inside a shell example is not a section.
pub fn headings(md: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashMap::new();
    let mut in_fence = false;
    for (n, line) in md.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let (level, rest) = if let Some(r) = t.strip_prefix("### ") {
            (3u8, r)
        } else if let Some(r) = t.strip_prefix("## ") {
            (2, r)
        } else if let Some(r) = t.strip_prefix("# ") {
            (1, r)
        } else {
            continue;
        };
        let text = strip_md(rest);
        let base = heading_slug(&text);
        if base.is_empty() {
            continue;
        }
        let slug = dedupe_slug(&mut seen, base);
        out.push(Heading { level, text, slug, line: n });
    }
    out
}

/// Render `md` as themed, readable text into `ui`.
pub fn render_markdown(ui: &mut egui::Ui, theme: &Theme, md: &str) {
    render_markdown_impl(ui, theme, md, false, &mut None, &mut None, None);
}

/// Render `md` with its inline links CLICKABLE. A click stores the link target
/// (for example `/library#self-hosting`) into `link`; the caller decides what
/// that means, which for the Library page is "navigate to that document".
///
/// Without this the native reader had no link handling at all and printed the
/// raw markdown: 107 links across 15 shipped documents read as
/// `[the roadmap](/library#roadmap)` on screen.
pub fn render_markdown_linked(
    ui: &mut egui::Ui,
    theme: &Theme,
    md: &str,
    link: &mut Option<String>,
) {
    let mut l = Some(link);
    render_markdown_impl(ui, theme, md, false, &mut None, &mut l, None);
}

/// As [`render_markdown_linked`], but scroll the heading whose slug is
/// `scroll_to` into view on this frame.
///
/// This is what turns the Contents outline and a search hit into navigation
/// rather than a hint. The search index already knew which heading a match sat
/// under and could only ever open the document at the top, which on a document
/// the length of the Constitution is not an answer.
pub fn render_markdown_linked_scrolled(
    ui: &mut egui::Ui,
    theme: &Theme,
    md: &str,
    link: &mut Option<String>,
    scroll_to: Option<&str>,
) {
    let mut l = Some(link);
    render_markdown_impl(ui, theme, md, false, &mut None, &mut l, scroll_to);
}

/// Render `md` with DEFINE MODE on (v0.989, operator: "clicking a button to
/// the click on a word to bring up the definition"): body and bullet text
/// lay out word by word, every word is clickable, and words the dictionary
/// knows are underlined in the accent color. A click stores
/// `(word, Some(entry))` on a hit or `(word, None)` on a miss into
/// `clicked` - the caller owns the popup. Headings stay plain labels (they
/// are titles, not prose). Costs more layout than the plain path, which is
/// why it only runs while the reader has the toggle on.
pub fn render_markdown_defining(
    ui: &mut egui::Ui,
    theme: &Theme,
    md: &str,
    clicked: &mut Option<String>,
) {
    let mut c = Some(clicked);
    render_markdown_impl(ui, theme, md, true, &mut c, &mut None, None);
}

/// A GFM table separator row (`|---|:--:|`): pipes, dashes, colons and space
/// only, with at least one dash AND one pipe. The pipe requirement is what
/// keeps a bare `---` horizontal rule from being read as a separator.
fn is_separator_row(s: &str) -> bool {
    let t = s.trim();
    t.contains('-')
        && t.contains('|')
        && !t.is_empty()
        && t.chars().all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
}

/// Split one pipe row into trimmed cells, tolerating optional outer pipes.
fn table_cells(row: &str) -> Vec<String> {
    let mut s = row.trim();
    s = s.strip_prefix('|').unwrap_or(s);
    s = s.strip_suffix('|').unwrap_or(s);
    s.split('|').map(|c| strip_md(c.trim())).collect()
}

fn render_markdown_impl(
    ui: &mut egui::Ui,
    theme: &Theme,
    md: &str,
    define: bool,
    clicked: &mut Option<&mut Option<String>>,
    link: &mut Option<&mut Option<String>>,
    scroll_to: Option<&str>,
) {
    // Collected so tables can look ahead one line for their separator row.
    let lines: Vec<&str> = md.lines().collect();

    // Markdown joins wrapped source lines; rendering one element per LINE (what
    // this did before) split every hard-wrapped paragraph into ragged pieces and
    // silently ended a bullet at its first continuation line. 42 of the 81
    // Library documents wrap their list items. Buffer, then flush.
    let mut para: Vec<String> = Vec::new();
    let mut li: Option<Vec<String>> = None;
    let mut quote: Vec<String> = Vec::new();
    let mut seen_slugs: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    macro_rules! flush_para {
        () => {
            if !para.is_empty() {
                let joined = para.join(" ");
                if define {
                    let text = strip_md(&joined);
                    defining_words(ui, theme, &text, theme.font_size_small, clicked);
                } else if link.is_some() && joined.contains("](") {
                    // Lay the paragraph out segment by segment so the link parts
                    // can be their own clickable labels. Only taken when the
                    // paragraph actually has a link, so ordinary prose keeps the
                    // cheap single-label path.
                    let segs = link_segments(&joined);
                    let mut hit: Option<String> = None;
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (text, target) in segs {
                            let shown = strip_md(&text);
                            match target {
                                Some(t) => {
                                    let r = ui.add(
                                        Label::new(
                                            RichText::new(shown)
                                                .size(theme.font_size_small)
                                                .underline()
                                                .color(theme.accent()),
                                        )
                                        .sense(egui::Sense::click()),
                                    );
                                    if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                        hit = Some(t);
                                    }
                                }
                                None => {
                                    ui.add(
                                        Label::new(
                                            RichText::new(shown)
                                                .size(theme.font_size_small)
                                                .color(theme.text_secondary()),
                                        )
                                        .wrap(),
                                    );
                                }
                            }
                        }
                    });
                    if let (Some(t), Some(sink)) = (hit, link.as_deref_mut()) {
                        *sink = Some(t);
                    }
                } else {
                    let text = strip_md(&joined);
                    ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_secondary()));
                }
                para.clear();
            }
        };
    }
    // Block quotes had NO branch at all before v0.1305: a "> " line fell through
    // to the paragraph branch and rendered its own marker as literal text. The
    // Constitution's ratification notes are multi-line quotes, so they are
    // joined and drawn once, indented and muted with an accent rule, which is
    // what separates editorial matter from constitutional text on screen.
    macro_rules! flush_quote {
        () => {
            if !quote.is_empty() {
                let text = strip_md(&quote.join(" "));
                ui.horizontal_top(|ui| {
                    ui.add_space(theme.spacing_sm);
                    ui.label(RichText::new("\u{2502}").color(theme.accent()));
                    // Inside a horizontal layout a plain label does NOT wrap: it
                    // runs off the right edge and is clipped. Constraining the
                    // remaining width and asking the label to wrap is what makes
                    // a long quote (or bullet) reflow instead of vanishing.
                    ui.scope(|ui| {
                        ui.set_max_width(ui.available_width());
                        ui.add(
                            Label::new(
                                RichText::new(text)
                                    .size(theme.font_size_small)
                                    .italics()
                                    .color(theme.text_muted()),
                            )
                            .wrap(),
                        );
                    });
                });
                quote.clear();
            }
        };
    }
    // Draw a heading, and if it is the one we were asked to jump to, scroll it
    // to the top of the view. `scroll_to_me` only works on the frame the widget
    // is actually drawn, which is why the target slug is threaded all the way
    // down here instead of being resolved by the caller.
    macro_rules! heading {
        ($rest:expr, $size:expr, $color:expr) => {{
            let text = strip_md($rest);
            let r = ui.label(RichText::new(&text).size($size).strong().color($color));
            // Slugged with the same running duplicate counter `headings()` uses,
            // walking the same document the same way, so the outline's Nth
            // "Section 1" and the renderer's Nth "Section 1" are the same anchor.
            let base = heading_slug(&text);
            if !base.is_empty() {
                let slug = dedupe_slug(&mut seen_slugs, base);
                if scroll_to == Some(slug.as_str()) {
                    r.scroll_to_me(Some(egui::Align::TOP));
                }
            }
        }};
    }

    macro_rules! flush_li {
        () => {
            if let Some(parts) = li.take() {
                let text = strip_md(&parts.join(" "));
                ui.horizontal_top(|ui| {
                    ui.add_space(theme.spacing_sm);
                    ui.label(RichText::new("\u{00b7}").color(theme.accent()));
                    if define {
                        defining_words(ui, theme, &text, theme.font_size_small, clicked);
                    } else {
                        // Same wrapping problem as the block quote: a long bullet
                        // was clipped at the right edge rather than reflowing.
                        // Visible in the Credits page, where the longest line ran
                        // off the pane. Pre-existing; fixed here with the joining.
                        ui.scope(|ui| {
                            ui.set_max_width(ui.available_width());
                            ui.add(
                                Label::new(
                                    RichText::new(text)
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                )
                                .wrap(),
                            );
                        });
                    }
                });
            }
        };
    }

    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim_start();

        if trimmed.is_empty() {
            flush_quote!();
            flush_li!();
            flush_para!();
            ui.add_space(theme.spacing_sm);
            i += 1;
            continue;
        }

        // ── Fenced code block ──
        // Everything between the fences is verbatim, not markdown. Without this
        // the renderer read shell comments as headings: SELF-HOSTING.md alone
        // has 50 `# comment` lines inside ``` blocks, every one of which drew as
        // a title, and its actual sections were lost among them.
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            flush_quote!();
            flush_li!();
            flush_para!();
            let closer = if trimmed.starts_with("~~~") { "~~~" } else { "```" };
            let mut code: Vec<&str> = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(closer) {
                code.push(lines[i]);
                i += 1;
            }
            i += 1; // step past the closing fence (or off the end, if unclosed)
            Frame::none()
                .fill(theme.bg_card())
                .rounding(egui::Rounding::same(theme.border_radius as u8))
                .stroke(egui::Stroke::new(1.0, theme.border()))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for c in code {
                        ui.label(
                            RichText::new(c)
                                .monospace()
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    }
                });
            ui.add_space(theme.spacing_xs);
            continue;
        }

        let bullet = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "));

        // A wrapped list item: indented, not itself a bullet, and a bullet is
        // open. Belongs to that bullet rather than to a paragraph of its own.
        if li.is_some() && bullet.is_none() && raw.starts_with(char::is_whitespace) {
            if let Some(parts) = li.as_mut() {
                parts.push(trimmed.to_string());
            }
            i += 1;
            continue;
        }

        // Block quote. A run of "> " lines is ONE quote, so accumulate here and
        // let the next non-quote line flush it. Matches a bare ">" too, which is
        // how a blank line inside a quote is written.
        if trimmed.starts_with('>') {
            flush_li!();
            flush_para!();
            quote.push(trimmed.trim_start_matches('>').trim().to_string());
            i += 1;
            continue;
        }

        // Horizontal rule, checked before bullets so "---" is never a bullet.
        if trimmed.starts_with("---") && trimmed.chars().all(|c| c == '-') {
            flush_quote!();
            flush_li!();
            flush_para!();
            ui.separator();
            i += 1;
            continue;
        }

        // Tables. A pipe row followed by a separator row opens one; it runs
        // until the first blank or pipe-less line. 14 Library documents use
        // tables and rendered as raw pipe text before this existed.
        if trimmed.contains('|') && lines.get(i + 1).is_some_and(|n| is_separator_row(n)) {
            flush_quote!();
            flush_li!();
            flush_para!();
            let header = table_cells(trimmed);
            let cols = header.len().max(1);
            let mut rows: Vec<Vec<String>> = Vec::new();
            let mut n = i + 2;
            while n < lines.len() {
                let r = lines[n];
                if r.trim().is_empty() || !r.contains('|') {
                    break;
                }
                rows.push(table_cells(r));
                n += 1;
            }

            // Split the pane evenly and let each cell wrap inside its share, so
            // a long prose cell grows downward instead of pushing the table
            // wider than the reading pane.
            let avail = ui.available_width();
            let col_w = ((avail - theme.spacing_sm * (cols as f32 + 1.0)) / cols as f32).max(60.0);
            ui.add_space(theme.spacing_xs);
            egui::Grid::new(("md_table", i))
                .striped(true)
                .num_columns(cols)
                .show(ui, |ui| {
                    for h in header.iter() {
                        ui.scope(|ui| {
                            ui.set_max_width(col_w);
                            ui.label(
                                RichText::new(h).size(theme.font_size_small).strong().color(theme.text_primary()),
                            );
                        });
                    }
                    ui.end_row();
                    for row in rows.iter() {
                        for c in 0..cols {
                            ui.scope(|ui| {
                                ui.set_max_width(col_w);
                                let cell = row.get(c).map(String::as_str).unwrap_or("");
                                ui.label(
                                    RichText::new(cell)
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                            });
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(theme.spacing_xs);
            i = n;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("### ") {
            flush_quote!();
            flush_li!();
            flush_para!();
            ui.add_space(theme.spacing_xs);
            heading!(rest, theme.font_size_body, theme.accent());
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            flush_quote!();
            flush_li!();
            flush_para!();
            ui.add_space(theme.spacing_sm);
            heading!(rest, theme.font_size_heading, theme.text_primary());
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            flush_quote!();
            flush_li!();
            flush_para!();
            ui.add_space(theme.spacing_sm);
            heading!(rest, theme.font_size_title, theme.text_primary());
        } else if let Some(rest) = bullet {
            flush_quote!();
            flush_li!();
            flush_para!();
            li = Some(vec![rest.to_string()]);
        } else {
            para.push(trimmed.to_string());
        }
        i += 1;
    }

    flush_quote!();
    flush_li!();
    flush_para!();
}

/// One prose line as clickable words, wrapping like normal text. Dictionary
/// hits are underlined accent; misses stay body-colored but still click
/// (the popup then says the word isn't defined yet - honest, not silent).
fn defining_words(
    ui: &mut egui::Ui,
    theme: &Theme,
    text: &str,
    size: f32,
    clicked: &mut Option<&mut Option<String>>,
) {
    let gl = crate::gui::glossary::glossary();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for word in text.split_whitespace() {
            let known = gl.lookup_word(word).is_some();
            let rt = if known {
                RichText::new(word).size(size).color(theme.accent()).underline()
            } else {
                RichText::new(word).size(size).color(theme.text_secondary())
            };
            let resp = ui.add(egui::Label::new(rt).sense(egui::Sense::click()));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                if let Some(slot) = clicked.as_deref_mut() {
                    *slot = Some(word.to_string());
                }
            }
        }
    });
}

/// Strip the common inline markdown markers so text reads cleanly as plain text.
pub fn strip_md(s: &str) -> String {
    let s = strip_links(s);
    s.replace("**", "").replace('`', "").replace('*', "")
}

/// Reduce every inline link to just its visible text.
///
/// Without this a reader sees the raw punctuation and the URL: the Library's own
/// cross-references came out as `[the roadmap](/library#roadmap)` on screen, 107
/// of them across 15 shipped documents, because the native reader had no link
/// handling at all. The paragraph path below goes further and makes them
/// clickable; this is the floor that keeps every OTHER surface (headings,
/// bullets, table cells, block quotes) readable.
pub fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '[' {
            if let Some((text, _target, next)) = parse_link(&b, i) {
                out.push_str(&text);
                i = next;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Parse `[text](target)` starting at `open`. Returns (text, target, index just
/// past the closing paren). Rejects anything that is not a complete link so a
/// bare `[` in prose is left alone.
fn parse_link(b: &[char], open: usize) -> Option<(String, String, usize)> {
    let close = (open + 1..b.len()).find(|&k| b[k] == ']')?;
    if close + 1 >= b.len() || b[close + 1] != '(' {
        return None;
    }
    let end = (close + 2..b.len()).find(|&k| b[k] == ')')?;
    let text: String = b[open + 1..close].iter().collect();
    let target: String = b[close + 2..end].iter().collect();
    if text.is_empty() {
        return None;
    }
    Some((text, target, end + 1))
}

/// Split a line into plain and link segments, for the clickable paragraph path.
pub(crate) fn link_segments(s: &str) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut plain = String::new();
    while i < b.len() {
        if b[i] == '[' {
            if let Some((text, target, next)) = parse_link(&b, i) {
                if !plain.is_empty() {
                    out.push((std::mem::take(&mut plain), None));
                }
                out.push((text, Some(target)));
                i = next;
                continue;
            }
        }
        plain.push(b[i]);
        i += 1;
    }
    if !plain.is_empty() {
        out.push((plain, None));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_slugs_follow_the_github_grammar() {
        assert_eq!(heading_slug("Article I"), "article-i");
        assert_eq!(heading_slug("The one rule, and why"), "the-one-rule-and-why");
        assert_eq!(heading_slug("  Trailing punctuation!!  "), "trailing-punctuation");
        assert_eq!(heading_slug("7075-T6 aluminium"), "7075-t6-aluminium");
        // Leading punctuation must not produce a leading hyphen, or the anchor
        // written by one client will not match the id emitted by the other.
        assert_eq!(heading_slug("...and then"), "and-then");
        assert_eq!(heading_slug("###"), "");
    }

    #[test]
    fn a_shell_comment_inside_a_fence_is_not_a_heading() {
        let md = "# Real\n\n```bash\n# just a comment\nsudo certbot\n```\n\n## Also real\n";
        let hs = headings(md);
        assert_eq!(hs.len(), 2, "got {hs:?}");
        assert_eq!(hs[0].text, "Real");
        assert_eq!(hs[1].text, "Also real");
    }

    #[test]
    fn repeated_heading_names_get_distinct_anchors() {
        // The Constitution's shape: three Articles, each with its own Section 1.
        let md = "# Article I\n## Section 1\n# Article II\n## Section 1\n# Article III\n## Section 1\n";
        let slugs: Vec<String> = headings(md).into_iter().map(|h| h.slug).collect();
        assert_eq!(
            slugs,
            vec!["article-i", "section-1", "article-ii", "section-1-1", "article-iii", "section-1-2"]
        );
    }

    /// The outline and the rendered anchors are produced by two different walks
    /// of the document. If they ever disagree, every Contents link past the
    /// first duplicate silently points at the wrong section, which is invisible
    /// until someone follows one. This pins the grammar both walks share.
    #[test]
    fn the_shipped_constitution_has_unique_anchors_for_every_section() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/library/us-constitution.md");
        // No silent skip on a read error. The first version of this test looked
        // for `us_constitution.md`, could not find it, and returned early, so it
        // passed with the dedupe rule deliberately deleted. A test that cannot
        // fail is worse than no test.
        let md = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let hs = headings(&md);
        assert!(hs.len() > 40, "expected the full Constitution, got {} headings", hs.len());
        let mut seen = std::collections::HashSet::new();
        for h in &hs {
            assert!(seen.insert(h.slug.clone()), "duplicate anchor {:?}", h.slug);
        }
        // And the duplicates really are there, so this test is not passing
        // because the document happens to have no repeats.
        assert!(
            hs.iter().any(|h| h.slug == "section-1-1"),
            "expected a second \"Section 1\"; the dedupe rule is no longer exercised"
        );
    }
}
