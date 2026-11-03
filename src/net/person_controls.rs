//! Per-person controls (docs/design/per-person-controls.md, 2026-11-03): what a person chose for
//! ONE other person, on that person's profile card ("Your settings for <name>"). The operator,
//! verbatim: "We need to add the various controls users can use on other users to the profile
//! page. Like we need to set file sharing limits, allowed to call during quiet hours, etc."
//!
//! Each control is an override of a global setting, never a second copy of it: with nothing
//! chosen for anyone the app behaves exactly as before. Rung 1 keeps two new ones here:
//!
//! - **Notifications from them** (`notices`): Everything (the default), only calls, mentions and
//!   replies, or Nothing. THE notification rule reads it as the candidate's `from_level`
//!   (`notifications::decide`, the data file's `person_level` per kind).
//! - **Files from them** (`files`, `files_mb`): use my setting (the default), always ask, up to N
//!   MB without asking, or never. [`file_verdict`] says what a file from them gets, read by the
//!   file's plan (`gui/pages/chat/attach_view.rs`) and a big file's question
//!   (`net/file_offer.rs`).
//!
//! The always-allow list for quiet hours and the friend's ticks are per-person controls too, but
//! they already had their own homes (`notifications::allow`, the DM store's choices); the card
//! draws them from there.
//!
//! **Kept on this device, by identity** (`config.json` `person_controls`), only what differs from
//! the defaults. A choice names a person, so it is a social edge: no server ever holds it, and no
//! sender is told it (it is never published in the profile's `accepts_files`). Rung 2 of the
//! design carries it between the person's own devices as notes to oneself.
//!
//! Pure: no socket, no GUI. The website's twin is `web/shared/person-controls.js`; both answer
//! `scripts/tests/fixtures/person-controls-cases.json` the same way.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Notifications from them (THE rule reads it as the candidate's `from_level`; the engine turns it
/// into `notifications::Level`, whose ids these are, so net never reaches up into notifications).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notices {
    /// Everything, as the person's other settings allow (the default).
    #[default]
    All,
    /// Only calls, mentions and replies.
    Mentions,
    /// Nothing at all, calls included.
    Nothing,
}

impl Notices {
    /// The id `config.json`, the website and the data file's `person_levels` write.
    pub fn id(self) -> &'static str {
        match self {
            Notices::All => "all",
            Notices::Mentions => "mentions",
            Notices::Nothing => "nothing",
        }
    }

    /// The level an id names; anything unknown is Everything (no choice).
    pub fn from_id(id: &str) -> Notices {
        match id {
            "mentions" => Notices::Mentions,
            "nothing" => Notices::Nothing,
            _ => Notices::All,
        }
    }
}

/// "Up to N MB without asking": N until the person changes it.
pub const FILES_MB_DEFAULT: u32 = 10;
/// The sizes the card offers, in MB: one up to the largest private file a server can allow
/// (`attach_send::private_ceiling`, 1 GB).
pub const FILES_MB_RANGE: (u32, u32) = (1, 1024);

/// Files from them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesChoice {
    /// Files in messages decides, as for anyone (the default).
    #[default]
    Mine,
    /// Every file from them waits for Accept, a friend's too.
    Ask,
    /// A file up to `files_mb` is taken without asking; a bigger one asks first.
    UpTo,
    /// Nothing from them is fetched, and there is no Accept.
    Never,
}

impl FilesChoice {
    /// Every choice, in the order the card lists them.
    pub const ALL: [FilesChoice; 4] = [FilesChoice::Mine, FilesChoice::Ask, FilesChoice::UpTo, FilesChoice::Never];

    /// The id `config.json` and the website write.
    pub fn id(self) -> &'static str {
        match self {
            FilesChoice::Mine => "mine",
            FilesChoice::Ask => "ask",
            FilesChoice::UpTo => "up_to",
            FilesChoice::Never => "never",
        }
    }

    /// The choice an id names; anything unknown is the default.
    pub fn from_id(id: &str) -> FilesChoice {
        FilesChoice::ALL.into_iter().find(|c| c.id() == id).unwrap_or_default()
    }

    /// Its words on the card, and its hint (keys in `data/i18n/en.json`).
    pub fn label_key(self) -> &'static str {
        match self {
            FilesChoice::Mine => "person.files_mine",
            FilesChoice::Ask => "person.files_ask",
            FilesChoice::UpTo => "person.files_up_to",
            FilesChoice::Never => "person.files_never",
        }
    }

    pub fn hint_key(self) -> &'static str {
        match self {
            FilesChoice::Mine => "person.files_mine_hint",
            FilesChoice::Ask => "person.files_ask_hint",
            FilesChoice::UpTo => "person.files_up_to_hint",
            FilesChoice::Never => "person.files_never_hint",
        }
    }
}

/// One person's controls. A field a saved record lacks (or holds something unknown) is its
/// default, so an older or hand-edited `config.json` reads safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Controls {
    /// Notifications from them: all (the default), mentions (calls, mentions and replies) or
    /// nothing.
    #[serde(deserialize_with = "level_or_all")]
    pub notices: Notices,
    #[serde(deserialize_with = "files_or_mine")]
    pub files: FilesChoice,
    /// The size for [`FilesChoice::UpTo`], in MB (read through [`Controls::mb`], which keeps it
    /// in [`FILES_MB_RANGE`]). Kept while another choice is picked, so it is there again.
    #[serde(deserialize_with = "mb_or_default")]
    pub files_mb: u32,
}

impl Default for Controls {
    fn default() -> Self {
        Controls { notices: Notices::All, files: FilesChoice::Mine, files_mb: FILES_MB_DEFAULT }
    }
}

/// A saved word, or nothing usable (a number, a word from elsewhere): read as the default.
fn word<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_str().unwrap_or_default().to_string())
}

fn level_or_all<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Notices, D::Error> {
    Ok(Notices::from_id(&word(d)?))
}

fn mb_or_default<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_u64().map(|n| n.min(u64::from(u32::MAX)) as u32).unwrap_or(FILES_MB_DEFAULT))
}

fn files_or_mine<'de, D: serde::Deserializer<'de>>(d: D) -> Result<FilesChoice, D::Error> {
    Ok(FilesChoice::from_id(&word(d)?))
}

impl Controls {
    /// "Up to N MB": N, kept in range.
    pub fn mb(&self) -> u32 {
        self.files_mb.clamp(FILES_MB_RANGE.0, FILES_MB_RANGE.1)
    }

    /// Nothing differs from the defaults (the size counts only while Up to is picked).
    pub fn is_default(&self) -> bool {
        self.notices == Notices::All && self.files == FilesChoice::Mine
    }
}

/// Every identity's choices: my identity key -> their identity key -> their controls (both
/// lower-case hex). Only people with something chosen are kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PeopleControls(pub BTreeMap<String, BTreeMap<String, Controls>>);

impl PeopleControls {
    /// What identity `me` chose for `key` (the defaults when nothing).
    pub fn get(&self, me: &str, key: &str) -> Controls {
        self.0.get(&me.to_ascii_lowercase()).and_then(|m| m.get(&key.to_ascii_lowercase())).copied().unwrap_or_default()
    }

    /// Keep `c` as identity `me`'s choice for `key`; the defaults are kept as no entry. Never for
    /// myself, and nothing without both keys.
    pub fn set(&mut self, me: &str, key: &str, c: Controls) {
        let (me, key) = (me.to_ascii_lowercase(), key.to_ascii_lowercase());
        if me.is_empty() || key.is_empty() || me == key {
            return;
        }
        if c.is_default() {
            if let Some(m) = self.0.get_mut(&me) {
                m.remove(&key);
                if m.is_empty() {
                    self.0.remove(&me);
                }
            }
        } else {
            self.0.entry(me).or_default().insert(key, c);
        }
    }

    /// The level THE rule is handed for a notice from `key` (`Candidate::from_level`): None when
    /// nothing is chosen (Everything), so a candidate with no sender and one with no choice read
    /// the same.
    pub fn level_of(&self, me: &str, key: &str) -> Option<Notices> {
        if key.is_empty() {
            return None;
        }
        Some(self.get(me, key).notices).filter(|l| *l != Notices::All)
    }
}

/// What a file from someone gets under their controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileVerdict {
    /// Nothing chosen for them: Files in messages decides, as for anyone.
    Mine,
    /// Taken as from someone files are accepted from (a size within "Up to N MB"). Pictures still
    /// follow "Show pictures without asking from".
    Accept,
    /// "Always ask": it waits for Accept.
    AskAlways,
    /// Over "Up to N MB" (this N): it waits for Accept.
    AskOver(u32),
    /// "Never": nothing is fetched and there is no Accept.
    Never,
}

impl FileVerdict {
    /// The id the shared cases use.
    pub fn id(self) -> &'static str {
        match self {
            FileVerdict::Mine => "mine",
            FileVerdict::Accept => "accept",
            FileVerdict::AskAlways => "ask_always",
            FileVerdict::AskOver(_) => "ask_over",
            FileVerdict::Never => "never",
        }
    }
}

/// THE FILES RULE for one person: what a file of `size` bytes from them gets. Under the protected
/// setup "Up to N MB" only narrows (`protected`: a size within the limit falls back to the
/// global rule, which the protected setup already holds), so nothing here widens what it holds
/// back. The protected setup's own rule (leaving out non-friends' files) runs before this and
/// nothing overrides it.
pub fn file_verdict(c: &Controls, size: u64, protected: bool) -> FileVerdict {
    match c.files {
        FilesChoice::Mine => FileVerdict::Mine,
        FilesChoice::Ask => FileVerdict::AskAlways,
        FilesChoice::Never => FileVerdict::Never,
        FilesChoice::UpTo => {
            let limit = u64::from(c.mb()) * 1024 * 1024;
            if size > limit {
                FileVerdict::AskOver(c.mb())
            } else if protected {
                FileVerdict::Mine
            } else {
                FileVerdict::Accept
            }
        }
    }
}

/// The line a file from `name` shows under its verdict: the question for the two asks, the
/// refusal for Never, nothing otherwise (the global rule's own words apply).
pub fn verdict_line(v: FileVerdict, name: &str) -> Option<String> {
    use humanity_core::i18n::tf;
    match v {
        FileVerdict::AskAlways => Some(tf("person.files_ask_line", &[("name", &name)])),
        FileVerdict::AskOver(mb) => Some(tf("person.files_over_line", &[("name", &name), ("mb", &mb)])),
        FileVerdict::Never => Some(tf("person.files_never_line", &[("name", &name)])),
        FileVerdict::Mine | FileVerdict::Accept => None,
    }
}

#[cfg(test)]
#[path = "person_controls_tests.rs"]
mod tests;
