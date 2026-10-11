//! Tests for files_policy.rs: who files are taken from, whose pictures show, the sender's line, and
//! the setting as the config keeps it.

use super::*;

/// Accept files from: our own always; Nobody takes none; Friends only friends'; Anyone all.
/// Seen red 2026-10-11 with `accepts` ignoring `own`: "our own file is always taken (Nobody)".
#[test]
fn who_files_are_taken_from() {
    for from in AcceptFrom::ALL {
        assert!(accepts(from, true, false), "our own file is always taken ({from:?})");
    }
    assert!(!accepts(AcceptFrom::Nobody, false, true), "Nobody: not even a friend's");
    assert!(accepts(AcceptFrom::Friends, false, true), "Friends: a friend's");
    assert!(!accepts(AcceptFrom::Friends, false, false), "Friends: not a stranger's");
    assert!(accepts(AcceptFrom::Anyone, false, false), "Anyone: a stranger's too");
}

/// Show pictures without asking from: Friends shows friends' only; Nobody none; the protected setup
/// is always Nobody, whatever the setting.
#[test]
fn whose_pictures_show_by_themselves() {
    assert!(shows_picture(ShowPictures::Friends, false, false, true));
    assert!(!shows_picture(ShowPictures::Friends, false, false, false));
    assert!(!shows_picture(ShowPictures::Nobody, false, false, true), "Nobody: a friend's waits too");
    assert!(!shows_picture(ShowPictures::Friends, true, false, true), "the protected setup: always Nobody");
    assert!(shows_picture(ShowPictures::Nobody, true, true, false), "our own always shows");
}

/// The card's words and the sender's line, word for word as the design gives them.
#[test]
fn the_words() {
    assert_eq!(not_accepted_line("Ann", AcceptFrom::Friends), "Ann sent a file. You accept files only from friends. Accept this one?");
    assert!(sender_line("Ann", "friends", false).unwrap().starts_with("Ann accepts files only from friends."));
    assert_eq!(sender_line("Ann", "friends", true), None, "a friend's file is taken");
    assert_eq!(sender_line("Ann", "anyone", false), None);
    assert!(sender_line("Ann", "nobody", true).unwrap().starts_with("Ann does not accept files from anyone."));
    assert_eq!(sender_line("Ann", "Friends", false), None, "only the relay's exact words count");
}

/// The words the profile carries are the relay's own three, both ways.
#[test]
fn the_profile_words_are_the_relays() {
    for (word, from) in [("nobody", AcceptFrom::Nobody), ("friends", AcceptFrom::Friends), ("anyone", AcceptFrom::Anyone)] {
        assert_eq!(from.word(), word);
        assert_eq!(AcceptFrom::from_word(word), Some(from));
        assert!(crate::relay::storage::profile::accepts_files_word(word).is_some(), "the relay knows {word}");
    }
    assert_eq!(AcceptFrom::default().word(), crate::relay::storage::profile::ACCEPTS_FILES_DEFAULT, "the same default as the relay");
}

/// The setting in the config: the defaults when it is not there (a config from before), and every
/// field kept through a write and a read.
#[test]
fn the_setting_reads_and_writes() {
    let old: FilesInMessages = serde_json::from_str("{}").unwrap();
    assert_eq!(old, FilesInMessages::default());
    assert!(old.attach_button && old.remove_details && old.kept_cap_mb == KEPT_CAP_DEFAULT_MB);
    let chosen = FilesInMessages { accept_from: AcceptFrom::Nobody, show_pictures_from: ShowPictures::Nobody, attach_button: false, remove_details: false, kept_cap_mb: 3 };
    let json = serde_json::to_string(&chosen).unwrap();
    assert!(json.contains("\"accept_from\":\"nobody\""), "{json}");
    assert_eq!(serde_json::from_str::<FilesInMessages>(&json).unwrap(), chosen);
}
