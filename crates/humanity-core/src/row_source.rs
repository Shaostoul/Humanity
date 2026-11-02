//! Where a data row's numbers come from (2026-11-02, spline routing rung 0,
//! `docs/design/spline-routing.md` sections 5.3 and 7.1).
//!
//! Every row of the run catalogs (`data/piping/pipe_materials.ron`, `run_rules.ron`,
//! `fittings.ron`, `anchors.ron`, `benders.ron`) carries a `source`: one or more findings of the
//! two dated findings documents of 2026-11-01, by the ids the design gives them (`R(13)` is
//! finding 13 of the pipe and cable routing rules, `D(4)` finding 4 of the manual door releases),
//! or `Ours("why")` where no public source was read and the project chose the number. The editor
//! (rung 8) never calls a row "code" unless its source says it is law there.
//!
//! The finding ids are NOT a list in code: [`Findings::parse`] reads the documents' own numbered
//! paragraphs (`**13. Cable supports in a manufactured home ...**`), so a finding added to a
//! document is known the moment it is written, and a row naming one that does not exist (a typo,
//! a renumbering) fails the check. The documents are not shipped with the game, so that check
//! runs in the tests (`validate-data`), never at run time; the loaders check only what needs no
//! document (a row with no source at all, an `Ours` with no reason).
//!
//! Plain serde and std, no `native` gate: the relay compiles it with the catalogs.

use serde::Deserialize;
use std::collections::BTreeSet;

/// Where one row's numbers come from. A row carries a list of these (`source: [R(9), R(10)]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum Source {
    /// A finding of `docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md`, by its
    /// number (the design's R numbers; the document's numbered paragraphs are the list, and
    /// scripts/check-run-sources.js reads them, so a new finding needs no code change).
    R(u16),
    /// A finding of `docs/reference/findings/2026-11-01-manual-door-releases.md` (D1 to D19).
    D(u16),
    /// No public source was read: the project's own number, and why it was chosen.
    Ours(String),
}

/// The shortest reason an `Ours` may give. A few words at least: "ours" alone says nothing.
pub const OURS_REASON_MIN_CHARS: usize = 12;

impl Source {
    /// The id the design writes it as: "R13", "D4", or "ours".
    pub fn label(&self) -> String {
        match self {
            Source::R(n) => format!("R{n}"),
            Source::D(n) => format!("D{n}"),
            Source::Ours(_) => "ours".to_string(),
        }
    }

    /// The findings document a finding source is in, relative to the repo root.
    pub fn document(&self) -> Option<&'static str> {
        match self {
            Source::R(_) => Some(ROUTING_FINDINGS),
            Source::D(_) => Some(DOOR_FINDINGS),
            Source::Ours(_) => None,
        }
    }
}

/// The pipe and cable routing rules finding, whose findings a row cites as `R(n)`.
pub const ROUTING_FINDINGS: &str = "docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md";
/// The manual door releases finding, whose findings a row cites as `D(n)`.
pub const DOOR_FINDINGS: &str = "docs/reference/findings/2026-11-01-manual-door-releases.md";

/// The finding numbers each findings document defines.
#[derive(Debug, Clone, Default)]
pub struct Findings {
    /// The routing rules document's (R).
    pub routing: BTreeSet<u16>,
    /// The door releases document's (D).
    pub doors: BTreeSet<u16>,
}

impl Findings {
    /// Read both documents' numbered findings from their text.
    pub fn parse(routing_md: &str, doors_md: &str) -> Findings {
        Findings { routing: finding_numbers(routing_md), doors: finding_numbers(doors_md) }
    }

    /// Read both documents from a checkout's root (tests and data checks: the documents are not
    /// shipped with the game).
    pub fn from_repo(root: &std::path::Path) -> std::io::Result<Findings> {
        let routing = std::fs::read_to_string(root.join(ROUTING_FINDINGS))?;
        let doors = std::fs::read_to_string(root.join(DOOR_FINDINGS))?;
        Ok(Findings::parse(&routing, &doors))
    }

    /// Does the finding a source names exist? `Ours` always does.
    pub fn knows(&self, source: &Source) -> bool {
        match source {
            Source::R(n) => self.routing.contains(n),
            Source::D(n) => self.doors.contains(n),
            Source::Ours(_) => true,
        }
    }
}

/// The finding numbers a findings document defines: every line that begins with a bold number and
/// a full stop, `**13. Title...**`, the form the findings README's documents number their findings
/// in. Numbers inside the text ("finding 13", "R13") are not definitions and are not counted.
pub fn finding_numbers(markdown: &str) -> BTreeSet<u16> {
    let mut out = BTreeSet::new();
    for line in markdown.lines() {
        let Some(rest) = line.trim_start().strip_prefix("**") else { continue };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
            continue;
        }
        if let Ok(n) = digits.parse::<u16>() {
            out.insert(n);
        }
    }
    out
}

/// What is wrong with one row's sources on their face, needing no document: no source at all, or
/// an `Ours` that gives no reason. `row` names the row in the sentence.
pub fn shape_problems(row: &str, sources: &[Source]) -> Vec<String> {
    let mut out = Vec::new();
    if sources.is_empty() {
        out.push(format!("{row} has no source: name a finding (R1.., D1..) or give Ours(\"why\")"));
    }
    for s in sources {
        if let Source::Ours(why) = s {
            if why.trim().chars().count() < OURS_REASON_MIN_CHARS {
                out.push(format!("{row}: Ours(\"{why}\") does not say why the number is ours"));
            }
        }
    }
    out
}

/// What is wrong with one row's sources against the findings documents: [`shape_problems`], and
/// any finding the documents do not define.
pub fn problems(row: &str, sources: &[Source], findings: &Findings) -> Vec<String> {
    let mut out = shape_problems(row, sources);
    for s in sources {
        if !findings.knows(s) {
            out.push(format!(
                "{row} cites {}, which {} does not define",
                s.label(),
                s.document().unwrap_or("no findings document")
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        "## Findings\n\n",
        "**1. Copper tube bend radius (CDA).** Source: ...\n\n",
        "Some text naming finding 7 and R12, which define nothing.\n",
        "**13. Cable supports in a manufactured home (24 CFR 3280.808).** Source: ...\n",
        "  **2. Indented still counts.**\n",
        "**Bold but not numbered.**\n",
        "**4 no full stop**\n",
    );

    #[test]
    fn finding_numbers_reads_only_the_numbered_bold_paragraphs() {
        let got: Vec<u16> = finding_numbers(SAMPLE).into_iter().collect();
        assert_eq!(got, vec![1, 2, 13]);
    }

    #[test]
    fn a_source_naming_a_missing_finding_is_a_problem() {
        let f = Findings::parse(SAMPLE, "**1. Door release.**\n");
        assert!(problems("row a", &[Source::R(13)], &f).is_empty());
        assert!(problems("row b", &[Source::D(1), Source::Ours("the maker's figure, not read".into())], &f).is_empty());
        let bad = problems("row c", &[Source::R(45)], &f);
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert!(bad[0].contains("R45"), "{bad:?}");
        assert_eq!(problems("row d", &[Source::D(2)], &f).len(), 1, "D2 is not in the doors document");
    }

    #[test]
    fn a_row_with_no_source_or_a_bare_ours_is_a_problem() {
        let f = Findings::default();
        assert_eq!(shape_problems("row e", &[]).len(), 1);
        assert_eq!(problems("row f", &[Source::Ours("ours".into())], &f).len(), 1);
    }
}
