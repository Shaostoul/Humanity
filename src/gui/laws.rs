//! Location-aware rules ("Laws") loader (v0.496).
//!
//! Loads `data/laws/laws.json`: a nested tree of jurisdictions (Humanity ->
//! Earth -> Country -> State -> County -> Locality) and a flat list of rules
//! attached to a jurisdiction. Two kinds: `base` (the HumanityOS base set,
//! distilled from the Humanity Accord) and `real` (plain-language summaries of
//! real-world laws, with a source to verify). The point is a SMALL memorable
//! set, not millions of statutes. Data-driven: add jurisdictions + rules to the
//! JSON and the app picks them up. See `docs/design/laws.md`.

use std::sync::OnceLock;

/// One jurisdiction node. `parent` is None only for the root (Humanity).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Jurisdiction {
    pub id: String,
    pub name: String,
    /// Display level ("Country", "State", "Locality", ...). Informational.
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub parent: Option<String>,
}

/// One rule that applies within (and below) its jurisdiction.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Rule {
    pub id: String,
    /// The jurisdiction id this rule attaches to.
    pub jurisdiction: String,
    /// "base" (our framework) or "real" (a real-world law summary).
    pub kind: String,
    #[serde(default)]
    pub category: String,
    pub title: String,
    pub summary: String,
    /// Citation / Accord article (base) or statute + link (real).
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// "routine" or "liberty_or_life". The dangerous class is the one whose
    /// failure mode is prison, deportation or a death rather than a fine: use of
    /// force, weapons carry and discharge, protection orders, mandatory
    /// reporting, anything immigration-adjacent.
    #[serde(default)]
    pub risk_class: String,
    /// When a human or a tool last checked this entry against its source. None
    /// means nobody ever has. Before 2026-08-25 NO entry carried this, and 53 of
    /// 170 admitted in prose that they were unverified.
    #[serde(default)]
    pub verified: Option<Verified>,
}

/// Provenance for one rule: when it was checked, by what, and whether a person
/// has actually reviewed it.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Verified {
    pub on: String,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub human_reviewed: bool,
}

impl Rule {
    pub fn is_base(&self) -> bool {
        self.kind.eq_ignore_ascii_case("base")
    }

    /// True when this rule must NOT show a summary.
    ///
    /// For the liberty-or-life class a summary IS the defect: compressing the
    /// rule is exactly what drops the condition that would have saved the
    /// reader. The live self-defense entry proved it, stating that Washington
    /// has no general duty to retreat while omitting that the doctrine
    /// collapses if you provoked the fight. So if nobody has checked an entry in
    /// that class, we show the citation and a human to ask, and we do not guess
    /// at the reader.
    pub fn is_gated(&self) -> bool {
        self.risk_class.eq_ignore_ascii_case("liberty_or_life") && self.verified.is_none()
    }

    /// One line saying who checked this and when, shown on every rule. It rides
    /// with the ENTRY rather than the page, because the entry is what travels:
    /// people screenshot a single row or deep-link to it, and a caveat sitting
    /// at the top of the page does not go with it.
    pub fn provenance_line(&self) -> String {
        match &self.verified {
            Some(v) if v.human_reviewed => format!("Checked {} by a reviewer.", v.on),
            Some(v) => format!("Checked {}, not yet reviewed by a person.", v.on),
            None => "Never checked by anyone. Treat as a draft.".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct LawsFile {
    #[serde(default)]
    disclaimer: String,
    #[serde(default)]
    jurisdictions: Vec<Jurisdiction>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    rules: Vec<Rule>,
}

static LAWS: OnceLock<Laws> = OnceLock::new();

pub struct Laws {
    pub disclaimer: String,
    pub jurisdictions: Vec<Jurisdiction>,
    pub categories: Vec<String>,
    pub rules: Vec<Rule>,
}

impl Laws {
    /// The chain of jurisdiction ids from `location` up to the root (Humanity),
    /// e.g. ["silverdale", "kitsap", "wa", "usa", "earth", "humanity"]. Guards
    /// against cycles. An unknown id yields an empty path.
    pub fn path_to_root(&self, location: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut cur = location.to_string();
        for _ in 0..32 {
            let Some(j) = self.jurisdictions.iter().find(|j| j.id == cur) else { break };
            chain.push(j.id.clone());
            match &j.parent {
                Some(p) if !p.is_empty() => cur = p.clone(),
                _ => break,
            }
        }
        chain
    }

    /// All rules that apply at `location` = every rule attached to a
    /// jurisdiction in its path to the root. Ordered BROADEST first (Humanity)
    /// down to the most local, which reads naturally ("these apply to everyone,
    /// then these where you live").
    pub fn applicable_rules<'a>(&'a self, location: &str) -> Vec<&'a Rule> {
        let mut path = self.path_to_root(location);
        path.reverse(); // root (Humanity) first
        let mut out = Vec::new();
        for jid in &path {
            for r in &self.rules {
                if &r.jurisdiction == jid {
                    out.push(r);
                }
            }
        }
        out
    }

    pub fn jurisdiction_name(&self, id: &str) -> String {
        self.jurisdictions
            .iter()
            .find(|j| j.id == id)
            .map(|j| j.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// A readable location breadcrumb, most local first:
    /// "Silverdale, Kitsap County, Washington, United States, Earth, Humanity".
    pub fn location_breadcrumb(&self, location: &str) -> String {
        self.path_to_root(location)
            .iter()
            .map(|id| self.jurisdiction_name(id).to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Load + cache the laws data. Missing/malformed file yields an empty set
/// (the page shows a friendly note) so the app still works.
pub fn install() -> &'static Laws {
    LAWS.get_or_init(|| {
        let path = crate::data_dir().join("laws").join("laws.json");
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Laws not loaded ({}): {}", path.display(), e);
                return Laws { disclaimer: String::new(), jurisdictions: Vec::new(), categories: Vec::new(), rules: Vec::new() };
            }
        };
        let parsed: LawsFile = match serde_json::from_slice(&bytes) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Laws parse failed: {e}");
                return Laws { disclaimer: String::new(), jurisdictions: Vec::new(), categories: Vec::new(), rules: Vec::new() };
            }
        };
        log::info!(
            "Laws loaded: {} rules across {} jurisdictions.",
            parsed.rules.len(),
            parsed.jurisdictions.len()
        );
        Laws {
            disclaimer: parsed.disclaimer,
            jurisdictions: parsed.jurisdictions,
            categories: parsed.categories,
            rules: parsed.rules,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate is the only thing standing between a reader and an unchecked
    /// summary of deadly-force law, so it gets a test that fails on purpose if
    /// someone deletes it.
    #[test]
    fn deadly_rules_nobody_checked_are_gated() {
        let unchecked: Rule = serde_json::from_str(
            r#"{"id":"a","jurisdiction":"wa","kind":"real","title":"t","summary":"s",
                "risk_class":"liberty_or_life"}"#,
        )
        .expect("fixture parses");
        assert!(unchecked.is_gated(), "an unchecked deadly rule must not show a summary");
        assert_eq!(unchecked.provenance_line(), "Never checked by anyone. Treat as a draft.");

        let checked: Rule = serde_json::from_str(
            r#"{"id":"b","jurisdiction":"wa","kind":"real","title":"t","summary":"s",
                "risk_class":"liberty_or_life",
                "verified":{"on":"2026-08-25","by":"x","human_reviewed":false}}"#,
        )
        .expect("fixture parses");
        assert!(!checked.is_gated(), "a checked rule should render normally");
        assert!(checked.provenance_line().contains("not yet reviewed by a person"));

        let routine: Rule = serde_json::from_str(
            r#"{"id":"c","jurisdiction":"wa","kind":"real","title":"t","summary":"s"}"#,
        )
        .expect("fixture parses");
        assert!(!routine.is_gated(), "an ordinary rule is never gated");
    }

    /// Guards the SHIPPED corpus, not a fixture. If someone adds a use-of-force
    /// or weapons entry and forgets to classify it, this catches it, because an
    /// unclassified deadly rule renders its summary like any other.
    #[test]
    fn shipped_dangerous_topics_are_all_classified() {
        // Read the shipped file directly rather than through install(), which
        // caches in a OnceLock and resolves data_dir() at runtime.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("laws")
            .join("laws.json");
        let bytes = std::fs::read(&path).expect("shipped laws.json is readable");
        let laws: LawsFile = serde_json::from_slice(&bytes).expect("shipped laws.json parses");
        assert!(!laws.rules.is_empty(), "shipped laws.json should have rules");

        let dangerous = |r: &Rule| {
            let hay = format!("{} {}", r.title, r.tags.join(" ")).to_lowercase();
            ["firearm", "weapon", "self-defense", "use of force", "protection order"]
                .iter()
                .any(|k| hay.contains(k))
        };
        let missed: Vec<&str> = laws
            .rules
            .iter()
            .filter(|r| dangerous(r) && !r.risk_class.eq_ignore_ascii_case("liberty_or_life"))
            .map(|r| r.id.as_str())
            .collect();
        assert!(missed.is_empty(), "dangerous topics not marked liberty_or_life: {missed:?}");

        // And every rule must be able to say where it came from.
        assert!(
            laws.rules.iter().all(|r| !r.provenance_line().is_empty()),
            "every rule needs a provenance line"
        );
    }

    fn fixture() -> Laws {
        let json = r#"{
            "disclaimer":"d",
            "jurisdictions":[
                {"id":"humanity","name":"Humanity","parent":null},
                {"id":"earth","name":"Earth","parent":"humanity"},
                {"id":"usa","name":"United States","parent":"earth"},
                {"id":"wa","name":"Washington","parent":"usa"},
                {"id":"silverdale","name":"Silverdale","parent":"wa"}
            ],
            "categories":["Rights"],
            "rules":[
                {"id":"h1","jurisdiction":"humanity","kind":"base","title":"H","summary":"s"},
                {"id":"u1","jurisdiction":"usa","kind":"real","title":"U","summary":"s"},
                {"id":"w1","jurisdiction":"wa","kind":"real","title":"W","summary":"s"},
                {"id":"x1","jurisdiction":"other","kind":"real","title":"X","summary":"s"}
            ]
        }"#;
        let f: LawsFile = serde_json::from_str(json).unwrap();
        Laws { disclaimer: f.disclaimer, jurisdictions: f.jurisdictions, categories: f.categories, rules: f.rules }
    }

    #[test]
    fn path_walks_to_root() {
        let l = fixture();
        assert_eq!(l.path_to_root("silverdale"), vec!["silverdale", "wa", "usa", "earth", "humanity"]);
        assert!(l.path_to_root("nonexistent").is_empty());
    }

    #[test]
    fn applicable_is_broad_to_local_and_excludes_others() {
        let l = fixture();
        let ids: Vec<&str> = l.applicable_rules("wa").iter().map(|r| r.id.as_str()).collect();
        // Humanity rule first, then USA, then WA. The unrelated "x1" (jurisdiction
        // "other", not in the path) is excluded.
        assert_eq!(ids, vec!["h1", "u1", "w1"]);
    }

    #[test]
    fn breadcrumb_is_local_first() {
        let l = fixture();
        assert_eq!(l.location_breadcrumb("silverdale"), "Silverdale, Washington, United States, Earth, Humanity");
    }
}
