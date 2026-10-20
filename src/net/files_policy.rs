//! Files in messages: who a person takes files from, whose pictures show by themselves, whether
//! the attach button shows in private conversations, the remembered "Remove location and camera
//! details" switch and how much of opened private files this device keeps (docs/design/
//! files-showcase.md, "Files on the sender's device and in messages, exact", parts 1, 3 and 4;
//! the operator's answers 4, 6 and 9 of 2026-10-11).
//!
//! THE RECEIVING APP ENFORCES IT. The relay cannot and must not: with sealed sender it cannot tell
//! which direct message carries a file, and making it able to would tell it who sends files to
//! whom. So a file marker from someone this setting does not allow is simply not fetched; its card
//! asks, and Accept fetches that one file. The setting is also published in the person's profile
//! (`accepts_files`: "nobody", "friends" or "anyone"), so a sender's app can say so before
//! anything is sent.
//!
//! No socket and no GUI here, so every rule is unit tested (files_policy_tests.rs).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Accept files from: Nobody / Friends / Anyone I talk to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AcceptFrom {
    /// "Send but not receive": every file asks first.
    Nobody,
    /// Friends' files are fetched by themselves; anyone else's asks first. The default: the same
    /// line the click-to-load rule for pictures already draws.
    #[default]
    Friends,
    /// Anyone whose message reaches this person (who can reach them is Settings > Safety's job).
    Anyone,
}

impl AcceptFrom {
    /// Every choice, in the order the setting lists them.
    pub const ALL: [AcceptFrom; 3] = [AcceptFrom::Nobody, AcceptFrom::Friends, AcceptFrom::Anyone];

    /// The word the profile carries (`accepts_files`), the relay's own three.
    pub fn word(self) -> &'static str {
        match self {
            AcceptFrom::Nobody => "nobody",
            AcceptFrom::Friends => "friends",
            AcceptFrom::Anyone => "anyone",
        }
    }

    /// The choice a profile's word names; None for anything else.
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.word() == word)
    }

    /// What the setting shows.
    pub fn label(self) -> &'static str {
        match self {
            AcceptFrom::Nobody => "Nobody",
            AcceptFrom::Friends => "Friends",
            AcceptFrom::Anyone => "Anyone I talk to",
        }
    }
}

/// Show pictures without asking from: Friends / Nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShowPictures {
    /// Friends' pictures show by themselves; anyone else's wait for a click (the rule before this
    /// setting existed).
    #[default]
    Friends,
    /// Every picture waits for a click, friends' too.
    Nobody,
}

/// The default size cap of kept private files on this device: 1 GB.
pub const KEPT_CAP_DEFAULT_MB: u64 = 1024;
/// The range the Settings > Data slider offers, in MB.
pub const KEPT_CAP_RANGE_MB: (u64, u64) = (0, 64 * 1024);

/// The person's choices for files in messages, kept in the app's config (`AppConfig::
/// files_in_messages`). A config written before these existed reads as the defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesInMessages {
    pub accept_from: AcceptFrom,
    pub show_pictures_from: ShowPictures,
    /// Show the attach button in private conversations ("receive only" is off).
    pub attach_button: bool,
    /// The attach step's switch, "Remove location and camera details", as last chosen on this
    /// device (on by default). The protected setup always removes and shows no switch.
    pub remove_details: bool,
    /// How much of the private files the person opened this device keeps, in MB (0 keeps none).
    pub kept_cap_mb: u64,
    /// "Accept files from" waits to go to the server the app is on: a choice made on this device
    /// (one made offline still goes), or this device's choice for a server that never got it
    /// (`accept_step`). Only while this is set does a profile update carry `accepts_files`;
    /// otherwise the server keeps its word. It counts as saved when that server's copy carries it.
    pub accept_unsent: bool,
    /// The person changed "Accept files from" on this device at least once (BUG-200). Such a device
    /// publishes its choice to a server that never got it; a device that only ever took a server's
    /// word (false) takes that server's word instead.
    pub accept_chosen: bool,
    /// For each server and person (`accept_seen_key`), the word this device last saw that person's
    /// profile there hold once settled: confirmed after a publish, taken, or agreeing with this
    /// device's setting (BUG-200). A server holding a different word now is one where another
    /// device changed it since. Keyed by the person too, so after a restore from a different
    /// recovery phrase no word counts as seen for the new one, and the old one's stay for a switch
    /// back.
    pub accept_seen: BTreeMap<String, String>,
}

impl Default for FilesInMessages {
    fn default() -> Self {
        FilesInMessages {
            accept_from: AcceptFrom::default(),
            show_pictures_from: ShowPictures::default(),
            attach_button: true,
            remove_details: true,
            kept_cap_mb: KEPT_CAP_DEFAULT_MB,
            accept_unsent: false,
            accept_chosen: false,
            accept_seen: BTreeMap::new(),
        }
    }
}

impl AcceptFrom {
    /// How open a choice is: Nobody 0, Friends 1, Anyone 2 (the protected setup never takes a more
    /// open one from elsewhere, and never widens one without being turned off first).
    pub fn openness(self) -> u8 {
        match self {
            AcceptFrom::Nobody => 0,
            AcceptFrom::Friends => 1,
            AcceptFrom::Anyone => 2,
        }
    }
}

/// Whether this device may take the word a server holds for it (`accepts_files` in its own
/// profile) in place of its own setting: None to keep its own (a choice of its own is waiting to
/// go, the words agree, the word is unknown, or the server's is more open while the protected setup
/// is on), or the choice to take. `accept_step` decides when taking is the thing to do.
pub fn adopt_from_server(mine: &FilesInMessages, server_word: &str, protected: bool) -> Option<AcceptFrom> {
    let theirs = AcceptFrom::from_word(server_word)?;
    if mine.accept_unsent || theirs == mine.accept_from || (protected && theirs.openness() > mine.accept_from.openness()) {
        return None;
    }
    Some(theirs)
}

/// The key `accept_seen` keeps a word under: the server's normalised address and the start of the
/// person's public key (16 hex characters are plenty to tell this device's identities apart, and
/// keep the config small). Empty when either is missing: nothing is remembered then.
pub fn accept_seen_key(server: &str, person: &str) -> String {
    let who: String = person.chars().take(16).collect();
    if server.is_empty() || who.is_empty() {
        return String::new();
    }
    format!("{server} {who}")
}

/// What this device does about "Accept files from" when the server it is on holds a word for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptStep {
    /// The server holds this device's word: settled. A waiting choice counts as saved, and the word
    /// is recorded as seen there.
    Settled,
    /// This device's choice goes to this server and waits until the server's copy carries it.
    Send,
    /// Take the server's word, and record it as seen there.
    Take(AcceptFrom),
    /// Nothing: a word that is not one of the three, or a more open one while the protected setup
    /// is on.
    Keep,
}

/// The rule (BUG-200), once the own copy of the server the app is on is in and holds
/// `server_word` (`seen_key`: that server and person's `accept_seen_key`):
/// 1. the server holds this device's word: settled;
/// 2. a choice of this device's waits (`accept_unsent`): it goes;
/// 3. the server's word differs from the one this device last saw there: another device changed
///    it there, so take it (never a more open one under the protected setup);
/// 4. a device that never chose takes the server's word;
/// 5. one that chose publishes its choice where the server still holds what this device last saw
///    there, or holds no profile for the person (`stored` false: its word is the relay's default,
///    "friends", never a choice made elsewhere);
/// 6. a server this device has never seen that holds a profile: its word may be the person's newer
///    choice from another device, or an older one, and nothing tells which, so the TIGHTER of the
///    two wins (this device's goes if it is tighter, the server's is taken if that is). Publishing
///    there unconditionally put a looser word over a newer, tighter one (the review of
///    2026-10-12).
pub fn accept_step(mine: &FilesInMessages, seen_key: &str, server_word: &str, stored: bool, protected: bool) -> AcceptStep {
    let Some(theirs) = AcceptFrom::from_word(server_word) else { return AcceptStep::Keep };
    if theirs == mine.accept_from {
        return AcceptStep::Settled;
    }
    if mine.accept_unsent {
        return AcceptStep::Send;
    }
    let take = || adopt_from_server(mine, server_word, protected).map_or(AcceptStep::Keep, AcceptStep::Take);
    let seen = mine.accept_seen.get(seen_key);
    if seen.is_some_and(|seen| seen != server_word) || !mine.accept_chosen {
        return take();
    }
    if seen.is_some() || !stored || mine.accept_from.openness() < theirs.openness() {
        return AcceptStep::Send;
    }
    take()
}

/// The words when the protected setup will not send a file that could not be checked (the web's
/// rule: the setup's promise is that location is always removed, so not even to a DM).
pub const PROTECTED_UNCHECKED: &str = "This file could not be read all the way through, so where it was taken may still be in it. The protected setup does not send such a file.";

/// Whether a file from someone is fetched without asking: always our own; otherwise as the
/// setting says, `friend` being whether they are a friend (the pictures rule's own test).
pub fn accepts(from: AcceptFrom, own: bool, friend: bool) -> bool {
    own || match from {
        AcceptFrom::Nobody => false,
        AcceptFrom::Friends => friend,
        AcceptFrom::Anyone => true,
    }
}

/// Whether a picture (one already accepted) shows without a click: always our own; otherwise only
/// a friend's, only when the setting says friends, and never while the protected setup is on (its
/// pictures rule is always Nobody).
pub fn shows_picture(show: ShowPictures, protected: bool, own: bool, friend: bool) -> bool {
    own || (!protected && show == ShowPictures::Friends && friend)
}

/// The card for a file this setting did not accept, word for word as the design gives it.
pub fn not_accepted_line(sender: &str, from: AcceptFrom) -> String {
    match from {
        AcceptFrom::Friends => format!("{sender} sent a file. You accept files only from friends. Accept this one?"),
        AcceptFrom::Nobody | AcceptFrom::Anyone => format!("{sender} sent a file. You do not accept files in messages. Accept this one?"),
    }
}

/// What a sender's attach step says about the person a DM is going to, from the word their
/// profile publishes, when it applies to this sender: None when the file will be taken (or the
/// word is not one of the three). `friend` is whether the sender is their friend.
pub fn sender_line(name: &str, word: &str, friend: bool) -> Option<String> {
    match AcceptFrom::from_word(word)? {
        AcceptFrom::Anyone => None,
        AcceptFrom::Friends if friend => None,
        AcceptFrom::Friends => Some(format!("{name} accepts files only from friends. Their app will ask them before it downloads this one.")),
        AcceptFrom::Nobody => Some(format!("{name} does not accept files in messages. Their app will ask them before it downloads this one.")),
    }
}

/// The line a recipient sees under a file whose sender sent it unchecked (answer 5).
pub const UNCHECKED_LINE: &str = "The sender's app could not check this file for hidden location.";

/// The question in a private conversation when a file could not be checked (answer 5).
pub const SEND_ANYWAY: &str = "This file could not be read all the way through, so where it was taken may still be in it. Send it anyway?";

/// Why such a file is not sent to a public place: the server would refuse it too, and only keeping
/// everything by choice sends it.
pub const NOT_SENT_PUBLIC: &str = "This file could not be read all the way through, so where it was taken may still be in it. It is not sent to a public place: the server would refuse it too. To send it exactly as it is, turn off \"Remove location and camera details\".";

/// The switch's words.
pub const SWITCH_LABEL: &str = "Remove location and camera details";

/// What the attach step says under the switch while the protected setup is on.
pub const PROTECTED_ALWAYS: &str = "The protected setup always removes location and camera details.";

/// While the file is being looked at.
pub const CHECKING: &str = "Checking the file for hidden details...";

/// When the person keeps everything and something was found.
pub const KEPT_AS_IS: &str = "It goes exactly as it is, with these details in it.";

/// What a paste says while sending files is off in private conversations.
pub const ATTACH_OFF: &str = "Sending files is off in private conversations (Settings > Privacy > Files in messages).";

#[cfg(test)]
#[path = "files_policy_tests.rs"]
mod tests;
