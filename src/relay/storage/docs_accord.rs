//! Humanity Accord document browser: a FIXED ALLOWLIST of the 17 governance
//! markdown files under `docs/accord/`.
//!
//! The 17 entries and their titles/categories mirror the curated set the
//! native Library page already ships (`data/library/index.json`, built by
//! `scripts/build-library.js` from this same `docs/accord/` source) so the
//! web browser and the native Library agree on what "the Accord" is. Four
//! files under `docs/accord/` are intentionally excluded from both: `README.md`
//! (an index, not a governance doc), `governance_models.md`, `curriculum.md`,
//! and `minimum_transparency_checklist.md` (design/process docs, not part of
//! the curated Accord reading list).
//!
//! Security model: `ACCORD_DOCS` is the ONLY source of truth for which files
//! are servable. `find_by_slug` does a linear scan over `&'static str`
//! literals and compares with `==` -- there is no string concatenation of a
//! caller-supplied slug into a filesystem path anywhere in this module.
//! `read_doc` only accepts an already-resolved `&'static AccordDoc`, never a
//! raw string, so it is structurally impossible for an HTTP slug parameter
//! to reach `std::fs::read_to_string` without first passing through
//! `find_by_slug`'s exact-match check against the fixed table. A path
//! traversal payload (`../../etc/passwd`) simply fails to match any `slug`
//! in the table and `find_by_slug` returns `None` -- it never touches the
//! filesystem.

/// A single Humanity Accord document entry in the fixed allowlist.
#[derive(Debug, Clone, Copy)]
pub struct AccordDoc {
    /// URL-safe identifier used by `/api/docs/accord/{slug}`.
    pub slug: &'static str,
    /// Human-readable title shown in the browser's nav pane.
    pub title: &'static str,
    /// Grouping used to organize the nav pane.
    pub category: &'static str,
    /// Repo-relative path to the markdown source. Never derived from
    /// caller input -- always one of these 17 compile-time literals.
    pub path: &'static str,
}

/// The fixed allowlist of all 17 Humanity Accord documents.
///
/// This list is intentionally hardcoded (not data-driven from a directory
/// scan) because it IS the security boundary: only entries in this table
/// are servable over HTTP, regardless of what else may exist on disk under
/// `docs/accord/`. Categories/order mirror `data/library/index.json` (the
/// native Library's curated Accord set) so web and native agree.
pub const ACCORD_DOCS: &[AccordDoc] = &[
    // ── The Accord ──
    AccordDoc {
        slug: "humanity-accord",
        title: "The Humanity Accord",
        category: "The Accord",
        path: "docs/accord/humanity_accord.md",
    },
    AccordDoc {
        slug: "absolute-prohibitions",
        title: "Absolute Prohibitions",
        category: "The Accord",
        path: "docs/accord/absolute_prohibitions.md",
    },
    // ── Your Rights and Consent ──
    AccordDoc {
        slug: "rights-and-responsibilities",
        title: "Rights and Responsibilities",
        category: "Your Rights and Consent",
        path: "docs/accord/rights_and_responsibilities.md",
    },
    AccordDoc {
        slug: "consent-and-control",
        title: "Consent and Control",
        category: "Your Rights and Consent",
        path: "docs/accord/consent_and_control.md",
    },
    AccordDoc {
        slug: "communication-and-association",
        title: "Communication and Association",
        category: "Your Rights and Consent",
        path: "docs/accord/communication_and_association.md",
    },
    // ── How We Decide ──
    AccordDoc {
        slug: "ethical-principles",
        title: "Ethical Principles",
        category: "How We Decide",
        path: "docs/accord/ethical_principles.md",
    },
    AccordDoc {
        slug: "conflict-resolution",
        title: "Conflict Resolution",
        category: "How We Decide",
        path: "docs/accord/conflict_resolution.md",
    },
    AccordDoc {
        slug: "transparency-guarantees",
        title: "Transparency Guarantees",
        category: "How We Decide",
        path: "docs/accord/transparency_guarantees.md",
    },
    AccordDoc {
        slug: "failure-of-legitimacy",
        title: "When Legitimacy Fails",
        category: "How We Decide",
        path: "docs/accord/failure_of_legitimacy.md",
    },
    // ── Safety and Care ──
    AccordDoc {
        slug: "human-needs",
        title: "Human Needs",
        category: "Safety and Care",
        path: "docs/accord/human_needs.md",
    },
    AccordDoc {
        slug: "safety-and-responsibility",
        title: "Safety and Responsibility",
        category: "Safety and Care",
        path: "docs/accord/safety_and_responsibility.md",
    },
    AccordDoc {
        slug: "harm-and-responsibility",
        title: "Harm and Responsibility",
        category: "Safety and Care",
        path: "docs/accord/harm_and_responsibility.md",
    },
    AccordDoc {
        slug: "irreversible-actions",
        title: "Irreversible Actions",
        category: "Safety and Care",
        path: "docs/accord/irreversible_actions.md",
    },
    AccordDoc {
        slug: "user-safety-overview",
        title: "User Safety Overview",
        category: "Safety and Care",
        path: "docs/accord/user_safety_overview.md",
    },
    // ── Reference ──
    AccordDoc {
        slug: "glossary",
        title: "Glossary",
        category: "Reference",
        path: "docs/accord/glossary.md",
    },
    AccordDoc {
        slug: "scope-boundaries",
        title: "Scope and Boundaries",
        category: "Reference",
        path: "docs/accord/scope_boundaries.md",
    },
    AccordDoc {
        slug: "knowledge-sources",
        title: "Knowledge Sources",
        category: "Reference",
        path: "docs/accord/knowledge_sources.md",
    },
];

/// Find a document by its slug. Linear scan over the fixed table, exact
/// `==` comparison -- no path concatenation, no filesystem access. Returns
/// `None` for anything not in `ACCORD_DOCS`, including path-traversal-shaped
/// input; such input never reaches a `Path`/`PathBuf` at all.
pub fn find_by_slug(slug: &str) -> Option<&'static AccordDoc> {
    ACCORD_DOCS.iter().find(|d| d.slug == slug)
}

/// Read a document's markdown content from disk.
///
/// Takes an already-resolved `&AccordDoc` (never a raw caller-supplied
/// string), so the only paths this function can ever read are the 17
/// compile-time literals in `ACCORD_DOCS`. There is structurally no way
/// for an HTTP slug parameter to reach this function without first
/// surviving `find_by_slug`'s exact-match filter.
pub fn read_doc(doc: &AccordDoc) -> Result<String, std::io::Error> {
    std::fs::read_to_string(doc.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_seventeen_entries() {
        assert_eq!(ACCORD_DOCS.len(), 17, "ACCORD_DOCS must have exactly 17 entries (the curated Accord set, matching data/library/index.json)");
    }

    #[test]
    fn all_slugs_are_unique() {
        let mut slugs: Vec<&str> = ACCORD_DOCS.iter().map(|d| d.slug).collect();
        slugs.sort_unstable();
        let mut deduped = slugs.clone();
        deduped.dedup();
        assert_eq!(slugs.len(), deduped.len(), "duplicate slug found in ACCORD_DOCS");
    }

    #[test]
    fn all_paths_are_unique() {
        let mut paths: Vec<&str> = ACCORD_DOCS.iter().map(|d| d.path).collect();
        paths.sort_unstable();
        let mut deduped = paths.clone();
        deduped.dedup();
        assert_eq!(paths.len(), deduped.len(), "duplicate path found in ACCORD_DOCS");
    }

    #[test]
    fn all_paths_are_under_docs_accord() {
        for doc in ACCORD_DOCS {
            assert!(
                doc.path.starts_with("docs/accord/"),
                "doc {} has a path outside docs/accord/: {}", doc.slug, doc.path
            );
            assert!(!doc.path.contains(".."), "doc {} path contains '..'", doc.slug);
        }
    }

    #[test]
    fn find_by_slug_resolves_a_real_slug() {
        let doc = find_by_slug("humanity-accord");
        assert!(doc.is_some());
        assert_eq!(doc.unwrap().path, "docs/accord/humanity_accord.md");
    }

    #[test]
    fn find_by_slug_rejects_bogus_slugs() {
        assert!(find_by_slug("does-not-exist").is_none());
        assert!(find_by_slug("../../etc/passwd").is_none());
        assert!(find_by_slug("..%2f..%2fetc%2fpasswd").is_none());
        assert!(find_by_slug("Cargo.toml").is_none());
        assert!(find_by_slug("docs/accord/humanity_accord.md").is_none());
        assert!(find_by_slug("").is_none());
        assert!(find_by_slug("humanity-accord\0").is_none());
        // Excluded-but-real files under docs/accord/ must NOT resolve --
        // they exist on disk but are deliberately outside the curated 17.
        assert!(find_by_slug("readme").is_none());
        assert!(find_by_slug("governance-models").is_none());
        assert!(find_by_slug("curriculum").is_none());
        assert!(find_by_slug("minimum-transparency-checklist").is_none());
    }

    #[test]
    fn read_doc_returns_real_nonempty_content_for_humanity_accord() {
        let doc = find_by_slug("humanity-accord").expect("humanity-accord must resolve");
        let content = read_doc(doc).expect("humanity_accord.md must be readable");
        assert!(!content.is_empty());
        assert!(content.contains("Humanity Accord"));
    }

    #[test]
    fn read_doc_returns_real_nonempty_content_for_glossary() {
        let doc = find_by_slug("glossary").expect("glossary must resolve");
        let content = read_doc(doc).expect("glossary.md must be readable");
        assert!(!content.is_empty());
        assert!(content.contains("Glossary"));
    }

    #[test]
    fn every_doc_in_table_is_actually_readable() {
        for doc in ACCORD_DOCS {
            let content = read_doc(doc);
            assert!(content.is_ok(), "doc {} at {} failed to read: {:?}", doc.slug, doc.path, content.err());
            assert!(!content.unwrap().is_empty(), "doc {} at {} read as empty", doc.slug, doc.path);
        }
    }
}
