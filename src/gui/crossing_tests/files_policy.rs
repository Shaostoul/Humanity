//! From net/files_policy_tests.rs: the profile's files words are the relay's, so the app and the
//! server agree on them.

use crate::net::files_policy::AcceptFrom;

/// The words the profile carries are the relay's own three, both ways. Seen red 2026-10-11 with
/// Anyone written "everyone": the first assertion of the loop.
#[test]
fn the_profile_words_are_the_relays() {
    for (word, from) in [("nobody", AcceptFrom::Nobody), ("friends", AcceptFrom::Friends), ("anyone", AcceptFrom::Anyone)] {
        assert_eq!(from.word(), word);
        assert_eq!(AcceptFrom::from_word(word), Some(from));
        assert!(crate::relay::storage::profile::accepts_files_word(word).is_some(), "the relay knows {word}");
    }
    assert_eq!(AcceptFrom::default().word(), crate::relay::storage::profile::ACCEPTS_FILES_DEFAULT, "the same default as the relay");
}
