//! Warnings, and the recovery-phrase guard (step F of docs/design/blocking-and-safe-mode.md,
//! sections 6.3 and 10g, 2026-10-10): the native client's rules, with no socket and no GUI so
//! each one is unit tested. The app-state glue (who is a friend, the switch, "Got it", the guard
//! on each send) is src/engine/warnings.rs; what is drawn under a message is
//! src/gui/pages/chat/warnings.rs.
//!
//! THE MATCHING RULE (10g, identical on both clients, so the shared cases in
//! scripts/tests/fixtures/warning-cases.json hold for each): lower-case a text with the standard
//! Unicode lower-casing, turn every run of characters that are not letters or digits
//! (`char::is_alphanumeric`) into one space, and trim. A message matches an entry when
//! `" " + text + " "` contains `" " + phrase + " "` for any phrase in the entry's `any`, each
//! phrase normalised the same way. So "I'm an admin!!" matches "i'm an admin" (both become
//! "i m an admin"), and "admin" never matches inside "administer".
//!
//! Everything here runs on this device. Nothing a warning looks at leaves it, and a warning
//! never blocks a message or reports anything (6.3).

use serde::Deserialize;

/// The warnings file, under `data/`. Read by both clients; built into the desktop app as
/// `embedded_data::WARNINGS_JSON` so an install whose data folder lacks it still warns.
pub const WARNINGS_FILE: &str = "safety/warnings.json";

/// The guard stops a text holding this many or more of the phrase's words, consecutively and
/// in the phrase's order (10g). Three do not: short runs of common words happen in ordinary
/// writing, and three of 24 words give nobody the identity.
pub const PHRASE_RUN: usize = 4;

/// What the guard says when it stops a send (10g, word for word). There is no "send anyway".
pub const GUARD_LINE: &str = "This is your recovery phrase. Anyone who has it owns your identity and everything in it. \
Nobody legitimate will ever ask for it. Remove it to send the rest.";

/// The line under a stranger's direct message that holds a link (10g), with the sender's name in
/// front: "<name> is not your friend. Links open only when you choose."
pub fn link_line(name: &str) -> String {
    format!("{name} is not your friend. Links open only when you choose.")
}

/// One entry of the warnings file, as the file has it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Warning {
    pub id: String,
    /// Whose messages it is shown on: `strangers`, `friends`, or both.
    pub applies_to: Vec<String>,
    pub title: String,
    pub explain: String,
    pub advice: String,
    /// The phrases, as written in the file.
    pub any: Vec<String>,
}

/// Who sent a message, as the warnings see it (10g): a friend is a mutual follow (you follow
/// them and they follow you, the test both clients use when they give passes); everyone else is
/// a stranger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sender {
    Friend,
    Stranger,
}

impl Sender {
    /// The word the file's `applies_to` uses for this sender.
    pub fn applies_word(self) -> &'static str {
        match self {
            Sender::Friend => "friends",
            Sender::Stranger => "strangers",
        }
    }
}

/// The loaded warnings, each with its phrases normalised and padded once, in the file's order.
#[derive(Debug, Clone, Default)]
pub struct Warnings {
    entries: Vec<(Warning, Vec<String>)>,
}

impl Warnings {
    /// The entries in the file's order.
    pub fn entries(&self) -> impl Iterator<Item = &Warning> {
        self.entries.iter().map(|(w, _)| w)
    }

    /// The entry at `index` (an index `matching_indices` gave).
    pub fn get(&self, index: usize) -> Option<&Warning> {
        self.entries.get(index).map(|(w, _)| w)
    }

    /// Every entry that matches `text` and applies to `from`, in the file's order (10g: several
    /// matching entries show in the file's order, each once).
    pub fn matching(&self, text: &str, from: Sender) -> Vec<&Warning> {
        self.matching_indices(text, from).into_iter().filter_map(|i| self.get(i)).collect()
    }

    /// `matching`, as the entries' indices in the file.
    pub fn matching_indices(&self, text: &str, from: Sender) -> Vec<usize> {
        let norm = normalize(text);
        if norm.is_empty() {
            return Vec::new();
        }
        let padded = format!(" {norm} ");
        let word = from.applies_word();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, (w, _))| w.applies_to.iter().any(|a| a == word))
            .filter(|(_, (_, phrases))| phrases.iter().any(|p| padded.contains(p.as_str())))
            .map(|(i, _)| i)
            .collect()
    }
}

/// Read the warnings file: `{"warnings":[...]}` (the shape data/safety/warnings.json has), or the
/// list itself. An entry with no id, or with no phrase that survives normalising, is skipped; a
/// file with no usable entry is an error, so the app logs that warnings are off rather than
/// quietly matching nothing.
pub fn parse_warnings(bytes: &[u8]) -> Result<Warnings, String> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum File {
        Wrapped { warnings: Vec<Warning> },
        List(Vec<Warning>),
    }
    let file: File = serde_json::from_slice(bytes).map_err(|e| format!("warnings: {e}"))?;
    let list = match file {
        File::Wrapped { warnings } => warnings,
        File::List(list) => list,
    };
    let entries: Vec<(Warning, Vec<String>)> = list
        .into_iter()
        .filter(|w| !w.id.trim().is_empty())
        .map(|w| {
            // A phrase of punctuation alone normalises to nothing. Padded, it is two spaces,
            // which a normalised text never holds, so it could never match: it is dropped, and
            // an entry left with no phrase at all is skipped like one with no id.
            let phrases = w.any.iter().map(|p| normalize(p)).filter(|p| !p.is_empty()).map(|p| format!(" {p} ")).collect();
            (w, phrases)
        })
        .filter(|(_, phrases): &(Warning, Vec<String>)| !phrases.is_empty())
        .collect();
    if entries.is_empty() {
        return Err("warnings: the list is empty".to_string());
    }
    Ok(Warnings { entries })
}

/// Load the warnings from the data folder (`data/safety/warnings.json`), or the copy built into
/// the exe when the folder's copy is missing or this version cannot read it.
pub fn load_warnings(data_dir: &std::path::Path) -> Result<Warnings, String> {
    crate::embedded_data::load_data_or_embedded(data_dir, WARNINGS_FILE, parse_warnings)
}

/// 10g's normalising: lower-case `text` (standard Unicode lower-casing), turn every run of
/// characters that are not letters or digits (`char::is_alphanumeric`, Unicode-aware) into one
/// space, and trim.
pub fn normalize(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut gap = false;
    for c in lower.chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}

/// The recovery-phrase guard's test (10g): whether `text` holds a run of `PHRASE_RUN` or more of
/// `phrase`'s words, consecutively and in the phrase's order, both normalised as above. A run is
/// any `PHRASE_RUN` words standing next to each other in the phrase that also stand next to each
/// other, in that order, in the text; a longer run contains one, so the full phrase is stopped
/// too. A phrase shorter than `PHRASE_RUN` words is never matched.
pub fn holds_phrase_run(text: &str, phrase: &str) -> bool {
    let phrase = normalize(phrase);
    let phrase_words: Vec<&str> = phrase.split(' ').filter(|w| !w.is_empty()).collect();
    if phrase_words.len() < PHRASE_RUN {
        return false;
    }
    let text = normalize(text);
    let text_words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    if text_words.len() < PHRASE_RUN {
        return false;
    }
    phrase_words.windows(PHRASE_RUN).any(|run| text_words.windows(PHRASE_RUN).any(|seen| seen == run))
}

/// What the chat keeps for the warnings: the patterns (loaded once), the warnings dismissed with
/// "Got it" (one key per message and warning), the messages whose links were let through with
/// the link line's Open, and what each message matched. "Got it" and Open last while the app
/// runs (the session), as on the web: nothing about them is written anywhere.
#[derive(Debug, Default)]
pub struct WarningsUi {
    pub list: Option<Warnings>,
    /// Why the file could not be loaded, if it could not (logged once; warnings are then off).
    pub load_error: Option<String>,
    pub dismissed: std::collections::HashSet<String>,
    /// Messages (`message_key`) whose links open normally now: their Open was clicked.
    pub links_opened: std::collections::HashSet<String>,
    /// The entries each message matched, by a hash of the message and whether its author is a
    /// friend (engine/warnings.rs `shown_under`), so a conversation is matched once rather than
    /// every frame. Behind a lock because the chat reads it while drawing, from `&GuiState`.
    pub matched: std::sync::Mutex<std::collections::HashMap<u64, Vec<usize>>>,
}

/// One message, as "Got it" and Open remember it: its conversation, its author and its time.
pub fn message_key(channel: &str, sender_key: &str, timestamp_ms: u64) -> String {
    format!("{channel}\n{sender_key}\n{timestamp_ms}")
}

/// The key "Got it" records for one warning under one message: the message's key and the
/// warning's id.
pub fn dismiss_key(channel: &str, sender_key: &str, timestamp_ms: u64, warning_id: &str) -> String {
    format!("{}\n{warning_id}", message_key(channel, sender_key, timestamp_ms))
}

/// Every native unit test of 10g's proof list that needs neither the app state nor the screen.
/// Each was seen red once on purpose, recorded at the test.
#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped file, read from the tree (the same bytes the exe builds in).
    fn shipped() -> Warnings {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join(WARNINGS_FILE);
        parse_warnings(&std::fs::read(&path).expect("data/safety/warnings.json")).expect("the shipped warnings parse")
    }

    fn ids(list: Vec<&Warning>) -> Vec<&str> {
        list.into_iter().map(|w| w.id.as_str()).collect()
    }

    /// THE CROSS-CLIENT TEST (10g's proof): the native matcher over the shared cases in
    /// scripts/tests/fixtures/warning-cases.json, against the shipped warnings file, must give
    /// exactly each case's `expect`, in order. The web client runs its own matcher over the same
    /// file in a Node test, so the two cannot drift. The copy built into the exe is the same file.
    /// Seen red 2026-10-10 twice: with the `applies_to` filter taken out ("I'm an admin. Please
    /// verify your wallet..." from a friend gave ["recovery_phrase", "staff_claim"]), and with
    /// `is_alphanumeric` swapped for `is_ascii_alphanumeric` (the case "gift" + U+0902 + " card"
    /// gave ["money"], because the combining mark became a separator).
    #[test]
    fn the_shared_cases_hold_against_the_shipped_file() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("scripts/tests/fixtures/warning-cases.json")).expect("the shared cases"),
        )
        .expect("the shared cases are JSON");
        let cases = cases["cases"].as_array().expect("a cases list");
        assert!(cases.len() >= 10, "the shared cases file lost its cases");
        let shipped = shipped();
        let built_in = parse_warnings(crate::embedded_data::WARNINGS_JSON.as_bytes()).expect("the built-in copy parses");
        for case in cases {
            let text = case["text"].as_str().expect("text");
            let from = match case["from"].as_str() {
                Some("friend") => Sender::Friend,
                Some("stranger") => Sender::Stranger,
                other => panic!("a case's `from` must be friend or stranger, not {other:?}"),
            };
            let expect: Vec<&str> = case["expect"].as_array().expect("expect").iter().map(|v| v.as_str().expect("an id")).collect();
            assert_eq!(ids(shipped.matching(text, from)), expect, "{text:?} from a {from:?}");
            assert_eq!(ids(built_in.matching(text, from)), expect, "the built-in copy: {text:?} from a {from:?}");
        }
    }

    /// 10g's own examples: "I'm an admin!!" matches "i'm an admin" (punctuation inside a phrase),
    /// "admin" never matches inside "administer" (a near miss), Unicode text is lower-cased and
    /// split on its own letters, and a run of separators is one space.
    /// Seen red 2026-10-10 with `is_alphanumeric` swapped for `is_ascii_alphanumeric`: "ÜBERWEISUNG
    /// für Grüße" normalised to "berweisung f r gr e".
    #[test]
    fn normalising_and_matching_follow_the_rule() {
        assert_eq!(normalize("I'm an admin!!"), "i m an admin");
        assert_eq!(normalize("  Guaranteed   return,  double-your-money! "), "guaranteed return double your money");
        assert_eq!(normalize("ÜBERWEISUNG für Grüße"), "überweisung für grüße");
        assert_eq!(normalize("\u{039F}\u{0394}\u{039F}\u{03A3} 42"), "\u{03BF}\u{03B4}\u{03BF}\u{03C2} 42", "Greek lower-cases with the final sigma rule");
        assert_eq!(normalize("!!!"), "");

        let list = parse_warnings(
            r#"{"warnings":[
                {"id":"staff","applies_to":["strangers"],"title":"t","explain":"e","advice":"a","any":["i'm an admin"]},
                {"id":"pay","applies_to":["strangers","friends"],"title":"t","explain":"e","advice":"a","any":["Überweisung"]}
            ]}"#
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(ids(list.matching("I'm an admin!!", Sender::Stranger)), ["staff"]);
        assert_eq!(ids(list.matching("IM...AN ADMIN", Sender::Stranger)), Vec::<&str>::new(), "\"im\" is not \"i m\"");
        assert_eq!(ids(list.matching("Administer the group, I'm an administrator", Sender::Stranger)), Vec::<&str>::new(), "the near miss");
        assert_eq!(ids(list.matching("Bitte ÜBERWEISUNG heute", Sender::Stranger)), ["pay"], "Unicode letters");
        assert_eq!(ids(list.matching("Überweisungen", Sender::Stranger)), Vec::<&str>::new(), "a longer word is not the word");
    }

    /// Who an entry applies to (10g): a `strangers`-only entry never shows on a friend's message;
    /// one for both shows on either; and several matching entries come in the file's order.
    /// Seen red 2026-10-10 with the `applies_to` filter taken out: "file order, each once" got
    /// ["money", "staff", "none"] (the entry that applies to nobody showed).
    #[test]
    fn friends_and_strangers_get_the_entries_that_apply_to_them() {
        let list = parse_warnings(
            br#"[
                {"id":"money","applies_to":["strangers","friends"],"title":"t","explain":"e","advice":"a","any":["gift card"]},
                {"id":"staff","applies_to":["strangers"],"title":"t","explain":"e","advice":"a","any":["i am an admin"]},
                {"id":"none","applies_to":[],"title":"t","explain":"e","advice":"a","any":["gift card"]}
            ]"#,
        )
        .unwrap();
        let text = "I am an admin, buy me a gift card";
        assert_eq!(ids(list.matching(text, Sender::Stranger)), ["money", "staff"], "file order, each once");
        assert_eq!(ids(list.matching(text, Sender::Friend)), ["money"]);
        assert_eq!(ids(list.matching("", Sender::Stranger)), Vec::<&str>::new());
    }

    /// A phrase that normalises to nothing matches nothing; an entry left with no phrase is
    /// dropped, and a file left with no entry is an error rather than a list that warns on nothing.
    /// Seen red 2026-10-10 with the empty-phrase filter taken out: the file whose only phrase is
    /// "!!!" parsed instead of being refused.
    #[test]
    fn an_empty_phrase_matches_nothing() {
        let list = parse_warnings(
            br#"[{"id":"a","applies_to":["strangers"],"title":"t","explain":"e","advice":"a","any":["!!!","gift card"]}]"#,
        )
        .unwrap();
        assert!(list.matching("hello", Sender::Stranger).is_empty());
        assert!(parse_warnings(br#"[{"id":"a","applies_to":["strangers"],"title":"t","explain":"e","advice":"a","any":["!!!"]}]"#).is_err());
        assert!(parse_warnings(b"[]").is_err());
    }

    /// The guard's rule (10g): four consecutive words of the phrase in its order stop the send,
    /// wherever they stand and however they are separated; three do not, nor four out of order,
    /// nor four that skip a word of the phrase; the full phrase does.
    /// Seen red 2026-10-10 with PHRASE_RUN set to 3: "three words" (the three-word text was stopped).
    #[test]
    fn the_guard_stops_four_words_in_order_and_nothing_less() {
        let phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident \
                      account accuse achieve acid acoustic acquire across act action actor actress actual";
        assert!(holds_phrase_run(phrase, phrase), "the full phrase");
        assert!(holds_phrase_run("my words are: absorb abstract absurd abuse, ok?", phrase), "four in order");
        assert!(holds_phrase_run("ACT-ACTION-ACTOR-ACTRESS", phrase), "any separators, any case");
        assert!(holds_phrase_run("acquire\nacross\nact\naction\nactor", phrase), "five on separate lines");
        assert!(!holds_phrase_run("absorb abstract absurd", phrase), "three words");
        assert!(!holds_phrase_run("abstract absorb absurd abuse", phrase), "four out of order");
        assert!(!holds_phrase_run("absorb abstract abuse access", phrase), "four that skip a word of the phrase");
        assert!(!holds_phrase_run("absorb abstract absurd and abuse", phrase), "a word between");
        assert!(!holds_phrase_run("act action actor", "act action actor"), "a phrase too short to have a run");
        assert!(!holds_phrase_run("", phrase));
    }
}
