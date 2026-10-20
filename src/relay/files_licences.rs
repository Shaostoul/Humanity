//! The licences an item on the Files page can carry, and the words beside them (Media round 5,
//! 2026-10-12; docs/design/media-round-5-plan.md 3.7 and 10): data/files/licences.json, read the
//! way the labels are (relay/files.rs `load_labels`): the data folder's copy first, then the one
//! built into the exe, and the log says when the built-in copy served.
//!
//! WHAT THE FILE HOLDS. Each licence's official name, its deed and one plain line of what it
//! allows (from docs/reference/findings/2026-10-12-media-item-licences.md, `words_from`); which
//! licences let an item be offered for the game (`game`: CC0 and CC BY 4.0 only, public licences
//! that cannot be withdrawn and need no recipient, the operator's answer of 2026-10-12); the
//! facts the editor states before a Creative Commons licence is saved; and the hosting paragraph
//! both apps show beside Add to Files (the operator's approved words, plan section 10). Both apps
//! take all of it from the server's answer ([`LicencesFile::answer`]), so neither hard-codes a
//! licence sentence, and a server's operator can edit the words in one place.
//!
//! WHAT IS OFFERED. "All rights reserved" is the default and stays preselected. A Creative Commons
//! licence is offered only while the file carries `cc_warning` (the words that say it cannot be
//! taken back), and "Offer this for the game" (stored as `media_items.project_use`) only with a
//! licence whose `game` is true. [`LicencesFile::words_source_problem`] holds any words filled in
//! to naming the findings they come from.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One licence an item can carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Licence {
    /// What the relay stores (`media_items.licence`): lower case a to z, digits and underscore.
    pub id: String,
    /// The short name the apps show ("CC BY 4.0").
    pub name: String,
    /// Its official name ("Attribution 4.0 International"; '' for "All rights reserved").
    #[serde(default)]
    pub official_name: String,
    /// Its deed ('' for none).
    #[serde(default)]
    pub url: String,
    /// Whether it is a Creative Commons licence (or CC0): offered only while `cc_warning` is written.
    #[serde(default)]
    pub cc: bool,
    /// Whether an item under it may be offered for the game.
    #[serde(default)]
    pub game: bool,
    /// One plain line of what it allows.
    #[serde(default)]
    pub line: String,
}

/// The "Offer this for the game" tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameOffer {
    pub label: String,
    /// A line under it ('' for none).
    #[serde(default)]
    pub line: String,
}

/// The paragraph both apps show beside Add to Files: what putting a file on this server lets the
/// people who run it do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hosting {
    pub heading: String,
    pub text: String,
}

/// data/files/licences.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LicencesFile {
    /// Where the licences' words come from: a findings document under docs/reference/findings/.
    #[serde(default)]
    pub words_from: String,
    /// The licence an item carries when none is chosen.
    pub default: String,
    pub licences: Vec<Licence>,
    pub game_offer: GameOffer,
    /// Shown before a Creative Commons licence is saved ('' and none is offered).
    #[serde(default)]
    pub cc_warning: String,
    /// Shown too when the item is members only.
    #[serde(default)]
    pub cc_members_warning: String,
    pub hosting: Hosting,
}

/// The start of every Creative Commons licence's address.
const CC_SITE: &str = "https://creativecommons.org/";
/// The folder `words_from` must name a file in.
pub const FINDINGS_DIR: &str = "docs/reference/findings/";

/// The licences file's text: refused when it is not the file's shape, lists none, names a default
/// it does not list, or has an id that is not lower-case a to z, digits and underscore, is listed
/// twice or has no name, or a Creative Commons licence whose address is not on
/// creativecommons.org, or the tick or the hosting paragraph has no words.
pub fn parse_licences(text: &str) -> Result<LicencesFile, String> {
    let file: LicencesFile = serde_json::from_str(text).map_err(|e| format!("not a licences file: {e}"))?;
    if file.licences.is_empty() {
        return Err("it lists no licences".into());
    }
    let mut seen = std::collections::HashSet::new();
    for l in &file.licences {
        if l.id.is_empty() || !l.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(format!("the id \"{}\" is not lower-case a to z, digits and underscore", l.id));
        }
        if !seen.insert(l.id.clone()) {
            return Err(format!("the id \"{}\" is listed twice", l.id));
        }
        if l.name.trim().is_empty() {
            return Err(format!("the licence \"{}\" has no name", l.id));
        }
        if l.cc && !l.url.starts_with(CC_SITE) {
            return Err(format!("the Creative Commons licence \"{}\" does not link to {CC_SITE}", l.id));
        }
    }
    if file.default.is_empty() || !seen.contains(&file.default) {
        return Err(format!("the default \"{}\" is not one of its licences", file.default));
    }
    if file.game_offer.label.trim().is_empty() {
        return Err("the game tick has no label".into());
    }
    if file.hosting.heading.trim().is_empty() || file.hosting.text.trim().is_empty() {
        return Err("the hosting paragraph has no words".into());
    }
    Ok(file)
}

/// The fallback when neither the data folder's copy nor the built-in one parses: "All rights
/// reserved" alone, which is what every uploader has with no statement at all, and the heading of
/// the hosting paragraph with no paragraph under it.
fn bare_minimum() -> LicencesFile {
    LicencesFile {
        words_from: String::new(),
        default: crate::relay::storage::files::DEFAULT_LICENCE.into(),
        licences: vec![Licence {
            id: crate::relay::storage::files::DEFAULT_LICENCE.into(),
            name: "All rights reserved".into(),
            official_name: String::new(),
            url: String::new(),
            cc: false,
            game: false,
            line: String::new(),
        }],
        game_offer: GameOffer { label: "Offer this for the game".into(), line: String::new() },
        cc_warning: String::new(),
        cc_members_warning: String::new(),
        hosting: Hosting { heading: "Your uploads stay yours.".into(), text: String::new() },
    }
}

/// The licences from `path`, else the copy built into the exe (and the log says so, as for every
/// relay data file), else "All rights reserved" alone (and the log says that too).
pub fn load_licences(path: &std::path::Path) -> LicencesFile {
    let shown = path.display().to_string();
    let from_disk = match std::fs::read_to_string(path) {
        Ok(text) => parse_licences(&text).map_err(|e| format!("{shown} does not parse ({e})")),
        Err(e) => Err(format!("{shown} could not be read ({e})")),
    };
    from_disk.unwrap_or_else(|why| {
        crate::embedded_data::note_builtin_copy("files/licences.json", why);
        // Declared after the disk read, so the compiled-in check sees this file read disk first.
        const BUILT_IN: &str = include_str!("../../data/files/licences.json");
        parse_licences(BUILT_IN).unwrap_or_else(|e| {
            tracing::error!("The built-in files/licences.json does not parse either ({e}); only \"All rights reserved\" is offered");
            bare_minimum()
        })
    })
}

/// Where the licences file is in the data folder `data`: `files/licences.json`, the folder the
/// built-in copy comes from too (read from `media/` until 2026-10-20, a day after it moved, so
/// every relay served the built-in copy; relay/files.rs `labels_path` is its twin).
pub fn licences_path(data: &std::path::Path) -> std::path::PathBuf {
    data.join("files").join("licences.json")
}

/// The licences this relay knows, read once from the data folder (`crate::data_dir`, which is
/// "data" for the headless relay).
pub fn licences() -> &'static LicencesFile {
    static LICENCES: std::sync::OnceLock<LicencesFile> = std::sync::OnceLock::new();
    LICENCES.get_or_init(|| load_licences(&licences_path(&crate::data_dir())))
}

/// What is said when a licence is chosen that this server does not offer.
pub const NOT_OFFERED: &str = "That licence is not offered on this server yet.";
/// What is said when an item is offered for the game under a licence that does not allow it.
pub const NOT_FOR_GAME: &str = "Only an item under CC0 or CC BY can be offered for the game.";

impl LicencesFile {
    /// The licences an uploader may choose now: every licence that is not a Creative Commons one,
    /// and the Creative Commons ones only while `cc_warning` is written.
    pub fn offered(&self) -> Vec<&Licence> {
        let cc_ready = !self.cc_warning.trim().is_empty();
        self.licences.iter().filter(|l| !l.cc || cc_ready).collect()
    }

    /// Licence `id`, when it may be chosen now.
    pub fn offered_one(&self, id: &str) -> Option<&Licence> {
        self.offered().into_iter().find(|l| l.id == id)
    }

    /// A choice an upload or an edit makes, checked: refused with [`NOT_OFFERED`] when the licence
    /// is not offered, and with [`NOT_FOR_GAME`] when the item is offered for the game under a
    /// licence that does not allow it.
    pub fn check_choice(&self, licence: &str, for_game: bool) -> Result<(), String> {
        let Some(l) = self.offered_one(licence) else {
            return Err(NOT_OFFERED.to_string());
        };
        if for_game && !l.game {
            return Err(NOT_FOR_GAME.to_string());
        }
        Ok(())
    }

    /// The `licences` member of the list's answer and the categories' answer (plan 3.3, as section
    /// 10 changed it): the default, the licences offered now, the game tick, the warnings and the
    /// hosting paragraph.
    pub fn answer(&self) -> Value {
        json!({
            "default": self.default,
            "offered": self.offered(),
            "game_offer": self.game_offer,
            "cc_warning": self.cc_warning,
            "cc_members_warning": self.cc_members_warning,
            "hosting": self.hosting,
        })
    }

    /// Whether any licence words are filled in: a licence's line or a warning.
    pub fn has_words(&self) -> bool {
        !self.cc_warning.trim().is_empty() || !self.cc_members_warning.trim().is_empty() || self.licences.iter().any(|l| !l.line.trim().is_empty())
    }

    /// When licence words are filled in, `words_from` must name a findings document that exists
    /// (`exists` answers for a path from the repository's root): the words are a reading of primary
    /// sources, and that file says whose and when. The problem in words, or None.
    pub fn words_source_problem(&self, exists: impl Fn(&str) -> bool) -> Option<String> {
        if !self.has_words() {
            return None;
        }
        let from = self.words_from.trim();
        if !from.starts_with(FINDINGS_DIR) || from.contains("..") {
            return Some(format!("words are filled in, but words_from (\"{from}\") does not name a file under {FINDINGS_DIR}"));
        }
        if !exists(from) {
            return Some(format!("words are filled in, but words_from names {from}, which does not exist"));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped_text() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/files/licences.json");
        std::fs::read_to_string(&path).expect("data/files/licences.json is shipped")
    }

    /// THE RELAY READS THE LABELS AND LICENCES FROM data/files/ (2026-10-20). Both files moved from
    /// data/media/ to data/files/ with the page's name on 2026-10-19, but the loaders kept reading
    /// data/media/, so every relay served its built-in copies, an edit to either file did nothing,
    /// and each start logged "[built-in data copy] data/files/labels.json: data\media\labels.json
    /// could not be read", which scripts/verify-copresence.js refuses. Here a data folder whose
    /// files/ holds changed copies: the relay's paths find them, and what is read is the change.
    ///
    /// Seen red 2026-10-20 with both paths back on `media/`: "the labels come from data/files/, not
    /// the built-in copy" failed (the built-in spoiler, flashing and graphic, no only_here).
    #[test]
    fn the_labels_and_licences_are_read_from_the_files_folder() {
        let data = crate::test_temp::dir("relay_files_folder");
        std::fs::create_dir_all(data.join("files")).unwrap();

        let labels = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/files/labels.json");
        let changed = std::fs::read_to_string(labels).unwrap().replacen(
            "\"labels\": [",
            "\"labels\": [\n    { \"id\": \"only_here\", \"label\": \"Only here\", \"desc\": \"In the test's data folder alone\" },",
            1,
        );
        // Written where the files are shipped (data/files/), read through the relay's own path.
        std::fs::write(data.join("files").join("labels.json"), changed).unwrap();
        let read = crate::relay::files::load_labels(&crate::relay::files::labels_path(&data));
        assert!(read.iter().any(|l| l.id == "only_here"), "the labels come from data/files/, not the built-in copy: {:?}", read.iter().map(|l| &l.id).collect::<Vec<_>>());

        // The shipped licences with the Creative Commons warning emptied: only All rights reserved.
        let mut file: serde_json::Value = serde_json::from_str(&shipped_text()).unwrap();
        file["cc_warning"] = serde_json::json!("");
        std::fs::write(data.join("files").join("licences.json"), file.to_string()).unwrap();
        let read = load_licences(&licences_path(&data));
        let ids: Vec<&str> = read.offered().iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, vec!["all_rights_reserved"], "the licences come from data/files/, not the built-in copy");
    }

    /// THE LICENCES FILE PARSES AND OFFERS A CREATIVE COMMONS LICENCE ONLY WITH ITS WARNING: the
    /// shipped file parses, defaults to "All rights reserved" and offers the five licences, each a
    /// Creative Commons one with its deed and a line; the game tick goes only with CC0 and CC BY
    /// 4.0; the same file without its warning offers "All rights reserved" alone. A malformed file
    /// (a bad id, an id twice, a default it does not list, a Creative Commons licence pointing
    /// elsewhere, no hosting paragraph) is refused, and an unreadable one falls back to the built-in
    /// copy.
    ///
    /// Seen red 2026-10-12 with `offered()` returning every licence whatever the warning: "without
    /// the warning, only All rights reserved is offered / left: [\"all_rights_reserved\",
    /// \"cc_by_4\", \"cc_by_sa_4\", \"cc_by_nc_4\", \"cc0_1\"] / right: [\"all_rights_reserved\"]".
    #[test]
    fn the_licences_file_parses_and_offers_cc_only_with_its_warning() {
        let shipped = parse_licences(&shipped_text()).unwrap_or_else(|e| panic!("data/files/licences.json parses: {e}"));
        assert_eq!(shipped.default, "all_rights_reserved");
        let ids = |f: &LicencesFile| f.offered().iter().map(|l| l.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&shipped), vec!["all_rights_reserved", "cc_by_4", "cc_by_sa_4", "cc_by_nc_4", "cc0_1"]);
        for l in shipped.licences.iter().filter(|l| l.cc) {
            assert!(l.url.ends_with("/deed.en") && !l.line.trim().is_empty() && !l.official_name.is_empty(), "{} has its deed, its name and a line", l.id);
        }
        let game: Vec<&str> = shipped.licences.iter().filter(|l| l.game).map(|l| l.id.as_str()).collect();
        assert_eq!(game, vec!["cc_by_4", "cc0_1"], "only CC BY 4.0 and CC0 may be offered for the game");
        assert_eq!(shipped.check_choice("cc0_1", true), Ok(()));
        assert_eq!(shipped.check_choice("cc_by_nc_4", true), Err(NOT_FOR_GAME.to_string()));
        assert_eq!(shipped.check_choice("all_rights_reserved", true), Err(NOT_FOR_GAME.to_string()));
        assert_eq!(shipped.check_choice("all_rights_reserved", false), Ok(()));
        assert_eq!(shipped.check_choice("cc_by_9", false), Err(NOT_OFFERED.to_string()));
        let answer = shipped.answer();
        assert_eq!(answer["offered"].as_array().map(Vec::len), Some(5));
        assert_eq!(answer["game_offer"]["label"], "Offer this for the game");
        assert_eq!(answer["hosting"]["heading"], "Your uploads stay yours.");

        let mut unwarned = shipped.clone();
        unwarned.cc_warning.clear();
        assert_eq!(ids(&unwarned), vec!["all_rights_reserved"], "without the warning, only All rights reserved is offered");
        assert_eq!(unwarned.check_choice("cc0_1", false), Err(NOT_OFFERED.to_string()));

        let with = |change: &dyn Fn(&mut Value)| {
            let mut v: Value = serde_json::from_str(&shipped_text()).unwrap();
            change(&mut v);
            parse_licences(&v.to_string())
        };
        assert!(with(&|v| v["licences"][1]["id"] = json!("CC-BY")).unwrap_err().contains("lower-case"));
        assert!(with(&|v| v["licences"][1]["id"] = json!("all_rights_reserved")).unwrap_err().contains("twice"));
        assert!(with(&|v| v["default"] = json!("cc_by_9")).unwrap_err().contains("default"));
        assert!(with(&|v| v["licences"][1]["url"] = json!("https://example.org/by")).unwrap_err().contains("creativecommons.org"));
        assert!(with(&|v| v["hosting"]["text"] = json!(" ")).unwrap_err().contains("hosting"));
        let missing = crate::test_temp::dir("licences_missing");
        assert_eq!(load_licences(&missing.join("licences.json")), shipped, "an unreadable file falls back to the built-in copy");
    }

    /// THE LICENCE WORDS NAME THEIR SOURCE (the plan's 3.7 lint): while any licence words are
    /// filled in (a licence's line, a warning), `words_from` must name a findings document that
    /// exists under docs/reference/findings/. Checked on the shipped file, and on copies pointing at
    /// a missing file or at a design document, which must be refused.
    ///
    /// Seen red 2026-10-12 with data/files/licences.json's `words_from` pointed at
    /// docs/reference/findings/2026-10-12-missing.md: "the shipped licences file: words are filled
    /// in, but words_from names docs/reference/findings/2026-10-12-missing.md, which does not
    /// exist".
    #[test]
    fn the_licence_words_name_their_source() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let exists = |p: &str| root.join(p).is_file();
        let shipped = parse_licences(&shipped_text()).expect("the shipped file parses");
        if let Some(problem) = shipped.words_source_problem(exists) {
            panic!("the shipped licences file: {problem}");
        }
        assert!(shipped.has_words(), "the shipped file's words are filled in, so the check above is a real one");

        let mut moved = shipped.clone();
        moved.words_from = "docs/reference/findings/2026-10-12-missing.md".into();
        assert!(moved.words_source_problem(exists).is_some_and(|p| p.contains("does not exist")), "words whose source is missing are refused");
        moved.words_from = "docs/design/media-round-5-plan.md".into();
        assert!(moved.words_source_problem(exists).is_some_and(|p| p.contains("under docs/reference/findings/")), "a design document is not a source");
        let mut blank = moved.clone();
        blank.cc_warning.clear();
        blank.cc_members_warning.clear();
        for l in &mut blank.licences {
            l.line.clear();
        }
        assert_eq!(blank.words_source_problem(exists), None, "no words, nothing to name");
    }

    /// THE RULES PAGE CARRIES THE HOSTING PARAGRAPH WORD FOR WORD (2026-10-12): rule 7 of
    /// web/pages/rules.html is the paragraph both apps show beside Add to Files, read from this
    /// file's `hosting`. Two copies of words people agree to must never drift apart, so a change
    /// to one without the other fails here.
    ///
    /// Seen red 2026-10-12 with rules.html's "keep backup copies" edited to "keep copies": "rule 7
    /// of web/pages/rules.html is not the hosting paragraph of data/files/licences.json".
    #[test]
    fn the_rules_page_carries_the_hosting_paragraph() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let rules = std::fs::read_to_string(root.join("web/pages/rules.html")).expect("web/pages/rules.html is readable");
        // The page wraps its lines; compare with every run of white space made one space.
        let flat = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        let page = flat(&rules);
        let shipped = parse_licences(&shipped_text()).expect("the shipped file parses");
        let rule = flat(&format!("<b>7. {}</b> <span>{}</span>", shipped.hosting.heading, shipped.hosting.text));
        assert!(page.contains(&rule), "rule 7 of web/pages/rules.html is not the hosting paragraph of data/files/licences.json");
    }
}
