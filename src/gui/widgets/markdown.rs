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

use egui::{Label, RichText};
use crate::gui::theme::Theme;

/// Render `md` as themed, readable text into `ui`.
pub fn render_markdown(ui: &mut egui::Ui, theme: &Theme, md: &str) {
    render_markdown_impl(ui, theme, md, false, &mut None);
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
    render_markdown_impl(ui, theme, md, true, &mut c);
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

    macro_rules! flush_para {
        () => {
            if !para.is_empty() {
                let text = strip_md(&para.join(" "));
                if define {
                    defining_words(ui, theme, &text, theme.font_size_small, clicked);
                } else {
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
            ui.label(RichText::new(strip_md(rest)).size(theme.font_size_body).strong().color(theme.accent()));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            flush_quote!();
            flush_li!();
            flush_para!();
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(strip_md(rest)).size(theme.font_size_heading).strong().color(theme.text_primary()));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            flush_quote!();
            flush_li!();
            flush_para!();
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(strip_md(rest)).size(theme.font_size_title).strong().color(theme.text_primary()));
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
    s.replace("**", "").replace('`', "").replace('*', "")
}
