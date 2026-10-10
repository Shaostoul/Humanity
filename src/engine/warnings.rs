//! Warnings, and the recovery-phrase guard (step F of docs/design/blocking-and-safe-mode.md,
//! sections 6.3 and 10g, 2026-10-10): the native client's half that touches the app state. The
//! matching rule and the guard's test are src/net/warnings.rs; what is drawn under a message is
//! src/gui/pages/chat/warnings.rs; the switch is in Settings > Safety (src/gui/pages/safety.rs).
//!
//! WHERE WARNINGS SHOW (10g): under a received direct message (`dm:<key>`) and under a message
//! in a P2P group (`p2pgroup:<id>`), from someone else, when the entry applies to its sender:
//! `friends` is a mutual follow in this server's DM store (`DmStore::is_friend`, the test the
//! passes are given on), everyone else is `strangers`. Never on public channel posts in this step,
//! never on our own messages, never on a message carrying an encrypted file, and never with
//! Settings > Safety "Warnings on messages" Off. The stranger link line is a rule of its own
//! (`links_held`) and stays with the switch Off. "Got it" and Open last for the session.
//!
//! THE GUARD (10g): before anything this person wrote is sent, `guard_stops` checks it against
//! their own recovery phrase, derived here from the seed the app already holds in memory
//! (`GuiState::private_key_bytes` through `identity::mnemonic_from_seed`, the derivation the
//! Settings page and onboarding use to show it). The
//! phrase is never kept: it is derived for the check and dropped. A locked identity (no seed in
//! memory) cannot be checked, so the send goes as before and nothing is said.

use std::hash::{Hash, Hasher};

use crate::gui::{ChatMessage, GuiState};
use crate::net::warnings::{self, Sender, Warning, GUARD_LINE};

/// Load the warnings (`data/safety/warnings.json`, or the copy built into the exe) the first
/// time the chat draws. A file that cannot be read is logged once and warnings stay off.
pub(crate) fn ensure_loaded(gs: &mut GuiState) {
    if gs.warnings.list.is_some() || gs.warnings.load_error.is_some() {
        return;
    }
    match warnings::load_warnings(&crate::data_dir()) {
        Ok(list) => gs.warnings.list = Some(list),
        Err(e) => {
            log::warn!("{e}; message warnings are off for this run");
            gs.warnings.load_error = Some(e);
        }
    }
}

/// Who `key` is, as the warnings see it: a friend is a mutual follow (you follow them and they
/// follow you) in this server's DM store; everyone else, and everyone while the store is closed,
/// is a stranger.
pub(crate) fn sender_of(gs: &GuiState, key: &str) -> Sender {
    if gs.dm_store.as_ref().is_some_and(|s| s.is_friend(key)) {
        Sender::Friend
    } else {
        Sender::Stranger
    }
}

/// Whether step F looks at `msg` at all: a direct message or P2P group message from someone
/// else. Not a channel post, our own message, a line with no author (a system line), or a
/// message carrying an encrypted file (its marker is not words anyone wrote; the web skips it
/// too).
fn looked_at(gs: &GuiState, msg: &ChatMessage) -> bool {
    let kind = msg.channel.starts_with("dm:") || msg.channel.starts_with("p2pgroup:");
    let from_someone_else = !msg.sender_key.is_empty() && msg.sender_key != gs.profile_public_key;
    kind && from_someone_else && crate::net::dm_pq::parse_file_marker(&msg.content).is_none()
}

/// Whether the warnings look at `msg`, and who sent it: a message `looked_at` covers, with
/// "Warnings on messages" On. None otherwise.
pub(crate) fn checked_sender(gs: &GuiState, msg: &ChatMessage) -> Option<Sender> {
    (gs.settings.warnings_on_messages && looked_at(gs, msg)).then(|| sender_of(gs, &msg.sender_key))
}

/// The warnings to show under `msg`, in the file's order, less the ones dismissed with "Got it".
/// What matched is remembered per message (keyed by its conversation, author, time, text and
/// whether its author is a friend), so a long conversation is not re-matched every frame.
pub(crate) fn shown_under<'a>(gs: &'a GuiState, msg: &ChatMessage) -> Vec<&'a Warning> {
    let (Some(from), Some(list)) = (checked_sender(gs, msg), gs.warnings.list.as_ref()) else { return Vec::new() };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&msg.channel, &msg.sender_key, msg.timestamp_ms, &msg.content, from == Sender::Friend).hash(&mut h);
    let key = h.finish();
    let ids: Vec<usize> = {
        let mut seen = gs.warnings.matched.lock().unwrap_or_else(|p| p.into_inner());
        if seen.len() > 4096 {
            seen.clear(); // a bound, not a policy: re-matching is only a few microseconds a message
        }
        seen.entry(key).or_insert_with(|| list.matching_indices(&msg.content, from)).clone()
    };
    ids.into_iter()
        .filter_map(|i| list.get(i))
        .filter(|w| !gs.warnings.dismissed.contains(&dismiss_key(msg, &w.id)))
        .collect()
}

/// The "Got it" key of warning `id` under `msg`.
pub(crate) fn dismiss_key(msg: &ChatMessage, id: &str) -> String {
    warnings::dismiss_key(&msg.channel, &msg.sender_key, msg.timestamp_ms, id)
}

/// The key Open remembers `msg` by.
pub(crate) fn message_key(msg: &ChatMessage) -> String {
    warnings::message_key(&msg.channel, &msg.sender_key, msg.timestamp_ms)
}

/// Whether the links in `msg` wait for the link line's Open (10g): a direct message from a
/// stranger whose Open has not been clicked this session. The line says "<name> is not your
/// friend. Links open only when you choose." (`net::warnings::link_line`). This is its own rule,
/// like a stranger's pictures waiting for a click (6.2 lists it as a row of its own), so it holds
/// whatever "Warnings on messages" says.
pub(crate) fn links_held(gs: &GuiState, msg: &ChatMessage) -> bool {
    msg.channel.starts_with("dm:")
        && looked_at(gs, msg)
        && sender_of(gs, &msg.sender_key) == Sender::Stranger
        && !gs.warnings.links_opened.contains(&message_key(msg))
}

// ── The recovery-phrase guard ───────────────────────────────────────────────────────────────

/// Whether any of `texts` holds a run of four or more of this identity's recovery-phrase words,
/// in the phrase's order (net/warnings.rs `holds_phrase_run`). False when the identity is
/// locked, or its key is not a recovery-phrase seed: then the guard cannot run.
pub(crate) fn holds_own_phrase(gs: &GuiState, texts: &[&str]) -> bool {
    // Derived for this check only; the String is dropped when the function returns.
    let Some(phrase) = gs.private_key_bytes.as_deref().and_then(crate::net::identity::mnemonic_from_seed) else { return false };
    texts.iter().any(|t| warnings::holds_phrase_run(t, &phrase))
}

/// THE GUARD, called on every send path before anything leaves (10g): true when the send must
/// stop, after saying `GUARD_LINE` once. There is no "send anyway": the caller keeps the person's
/// text where it was so they can remove the phrase and send the rest.
pub(crate) fn guard_stops(gs: &mut GuiState, texts: &[&str]) -> bool {
    if !holds_own_phrase(gs, texts) {
        return false;
    }
    if !gs.pending_notices.iter().any(|n| n == GUARD_LINE) {
        gs.pending_notices.push(GUARD_LINE.to_string());
    }
    true
}

/// Every native item of 10g's proof list that needs the app state but no screen and no socket.
/// Each was seen red once on purpose, recorded at the test.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::dm_store::DmStore;

    const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

    /// An app signed in as `me` with the seed `seed`, its own DM store (a unique file), and the
    /// shipped warnings loaded.
    fn app(me: &str, seed: &[u8], tag: &str) -> GuiState {
        let mut gs = GuiState::default();
        gs.profile_public_key = me.to_string();
        gs.private_key_bytes = Some(seed.to_vec());
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let mut store = DmStore::load(seed, me, &format!("wss://{tag}-{nanos}.example"));
        store.set_pass_server(SERVER);
        gs.dm_store = Some(store);
        gs.warnings.list = Some(warnings::parse_warnings(crate::embedded_data::WARNINGS_JSON.as_bytes()).unwrap());
        gs
    }

    fn tidy(gs: &GuiState) {
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    fn msg(channel: &str, from: &str, ts: u64, text: &str) -> ChatMessage {
        ChatMessage { sender_name: "Dana".into(), sender_key: from.into(), content: text.into(), timestamp_ms: ts, channel: channel.into(), ..Default::default() }
    }

    fn ids(list: Vec<&Warning>) -> Vec<&str> {
        list.into_iter().map(|w| w.id.as_str()).collect()
    }

    /// The friends and strangers rule (10g), on both kinds of conversation: a stranger's DM gets
    /// every entry for strangers; once the follow is mutual, only the entries for friends; a
    /// follow one way is still a stranger; a P2P group message is checked the same way; a public
    /// channel post, our own message and a line with no author never are.
    /// Seen red 2026-10-10 with the `applies_to` filter taken out (net/warnings.rs): "a friend: money
    /// and the phrase only" got all four.
    #[test]
    fn warnings_follow_the_friends_and_strangers_rule() {
        let mut gs = app("me", &[3u8; 32], "warn-rule");
        let text = "I'm an admin. Verify your wallet: buy me a gift card and let's talk on Telegram.";
        let from_dana = msg("dm:dana", "dana", 1, text);
        assert_eq!(ids(shown_under(&gs, &from_dana)), ["money", "recovery_phrase", "staff_claim", "move_elsewhere"], "a stranger");

        gs.dm_store.as_mut().unwrap().set_following("dana", true);
        assert_eq!(ids(shown_under(&gs, &from_dana)), ["money", "recovery_phrase", "staff_claim", "move_elsewhere"], "a one-way follow is not a friend");
        gs.dm_store.as_mut().unwrap().set_follower("dana", true);
        assert_eq!(ids(shown_under(&gs, &from_dana)), ["money", "recovery_phrase"], "a friend: money and the phrase only");

        let in_group = msg("p2pgroup:g1", "eve", 2, text);
        assert_eq!(ids(shown_under(&gs, &in_group)), ["money", "recovery_phrase", "staff_claim", "move_elsewhere"], "a group member who is no friend");
        let in_group_from_dana = msg("p2pgroup:g1", "dana", 3, text);
        assert_eq!(ids(shown_under(&gs, &in_group_from_dana)), ["money", "recovery_phrase"], "a friend in a group");

        assert!(shown_under(&gs, &msg("general", "eve", 4, text)).is_empty(), "not on public channel posts");
        assert!(shown_under(&gs, &msg("dm:dana", "me", 5, text)).is_empty(), "never on our own messages");
        assert!(shown_under(&gs, &msg("dm:dana", "", 6, text)).is_empty(), "never on a line with no author");
        tidy(&gs);
    }

    /// "Got it" hides that one warning under that one message: the others under it stay, and the
    /// same warning under another message stays.
    /// Seen red 2026-10-10 with the dismissed filter taken out of `shown_under`: the first message
    /// gave ["money", "staff_claim"].
    #[test]
    fn got_it_hides_one_warning_under_one_message() {
        let mut gs = app("me", &[4u8; 32], "warn-got-it");
        let first = msg("dm:dana", "dana", 10, "Buy me a gift card. I'm an admin.");
        let second = msg("dm:dana", "dana", 11, "A gift card, please.");
        gs.warnings.dismissed.insert(dismiss_key(&first, "money"));
        assert_eq!(ids(shown_under(&gs, &first)), ["staff_claim"]);
        assert_eq!(ids(shown_under(&gs, &second)), ["money"]);
        tidy(&gs);
    }

    /// The switch (10g): Off hides every warning, and On shows them again. The link line is its
    /// own rule (6.2, like click-to-load pictures), so it stays with the switch Off.
    /// Seen red 2026-10-10 with the switch left out of `checked_sender`: "Off: no warnings".
    #[test]
    fn the_switch_off_hides_the_warnings_but_not_the_link_line() {
        let mut gs = app("me", &[5u8; 32], "warn-switch");
        let m = msg("dm:dana", "dana", 20, "Act now and send me a gift card: https://example.com/pay");
        assert!(!shown_under(&gs, &m).is_empty() && links_held(&gs, &m), "On: warnings and the link line");
        gs.settings.warnings_on_messages = false;
        assert!(shown_under(&gs, &m).is_empty(), "Off: no warnings");
        assert!(links_held(&gs, &m), "Off: the link line stays");
        gs.settings.warnings_on_messages = true;
        assert!(!shown_under(&gs, &m).is_empty());
        tidy(&gs);
    }

    /// The link line (10g): a stranger's direct message holds its links for the line's Open, until
    /// Open is clicked (remembered for the session); a friend's does not, nor a group message (the
    /// rule is for direct messages), nor our own, nor a message carrying an encrypted file, which
    /// gets no warnings either.
    /// Seen red 2026-10-10 three ways: with the `dm:` test left out of `links_held` ("a group
    /// message"), with the encrypted-file test left out of `looked_at` ("an encrypted file:
    /// neither"), and with the Open set ignored ("after Open, its links open as normal").
    #[test]
    fn a_strangers_direct_message_holds_its_links() {
        let mut gs = app("me", &[6u8; 32], "warn-links");
        let m = msg("dm:dana", "dana", 30, "see https://example.com");
        assert!(links_held(&gs, &m), "a stranger");
        assert_eq!(warnings::link_line("Dana"), "Dana is not your friend. Links open only when you choose.");
        assert!(!links_held(&gs, &msg("p2pgroup:g1", "dana", 31, "see https://example.com")), "a group message");
        assert!(!links_held(&gs, &msg("dm:dana", "me", 32, "see https://example.com")), "our own");
        let file = crate::net::dm_pq::build_file_marker(&crate::net::dm_pq::DmAttachment {
            url: "/uploads/gift-card.enc".into(),
            k: "k".into(),
            n: "n".into(),
            name: "gift card offer, act now.png".into(),
            mime: "image/png".into(),
            size: 10,
        });
        let with_file = msg("dm:dana", "dana", 33, &file);
        assert!(!links_held(&gs, &with_file) && shown_under(&gs, &with_file).is_empty(), "an encrypted file: neither");

        gs.warnings.links_opened.insert(message_key(&m));
        assert!(!links_held(&gs, &m), "after Open, its links open as normal");
        assert!(links_held(&gs, &msg("dm:dana", "dana", 34, "and https://example.org")), "only that message's");
        gs.warnings.links_opened.clear();
        let store = gs.dm_store.as_mut().unwrap();
        store.set_following("dana", true);
        store.set_follower("dana", true);
        assert!(!links_held(&gs, &m), "a friend's links open as before");
        tidy(&gs);
    }

    /// The guard (10g): four consecutive words of this identity's own phrase stop the send with
    /// the sentence, said once; three words, or four out of order, do not; the full phrase does;
    /// and with the identity locked (no seed in memory) the guard cannot run, says nothing, and
    /// lets the send go.
    /// Seen red 2026-10-10 twice: with PHRASE_RUN set to 3 ("three words go"), and with a locked
    /// identity made to stop every send instead ("locked: the guard cannot run, the send goes").
    #[test]
    fn the_guard_uses_this_identitys_own_phrase() {
        let seed = [7u8; 32];
        let mut gs = app("me", &seed, "warn-guard");
        let phrase = crate::net::identity::mnemonic_from_seed(&seed).unwrap();
        let words: Vec<&str> = phrase.split(' ').collect();
        assert_eq!(words.len(), 24);
        let four = format!("here: {}", words[5..9].join(" "));
        let three = words[5..8].join(" ");
        let shuffled = [words[6], words[5], words[7], words[8]].join(" ");

        assert!(!guard_stops(&mut gs, &[&three]), "three words go");
        assert!(!guard_stops(&mut gs, &["hello", &shuffled]), "four out of order go");
        assert!(gs.pending_notices.is_empty(), "and nothing is said");
        assert!(guard_stops(&mut gs, &["hello", &four]), "four in order stop");
        assert!(guard_stops(&mut gs, &[&phrase]), "the full phrase stops");
        assert_eq!(gs.pending_notices, [GUARD_LINE.to_string()], "the sentence, once");

        let other = crate::net::identity::mnemonic_from_seed(&[8u8; 32]).unwrap();
        assert!(!holds_own_phrase(&gs, &[&other]), "someone else's phrase is not ours");

        gs.pending_notices.clear();
        gs.private_key_bytes = None;
        assert!(!guard_stops(&mut gs, &[&phrase]), "locked: the guard cannot run, the send goes");
        assert!(gs.pending_notices.is_empty(), "and says nothing");
        tidy(&gs);
    }

    /// A contact request carries our name, and only our name (10c), so a name holding four words
    /// of our phrase stops the request with the sentence, before any pass is minted or sealed.
    /// Seen red 2026-10-10 with the guard taken out of `contact_request_puts`: the request went on
    /// and failed later, "The request could not be sealed."
    #[test]
    fn a_contact_request_whose_name_holds_the_phrase_is_stopped() {
        let seed = [10u8; 32];
        let mut gs = app("me", &seed, "warn-request");
        gs.peer_kyber_keys.insert("ann".into(), "a-key".into());
        let phrase = crate::net::identity::mnemonic_from_seed(&seed).unwrap();
        gs.user_name = phrase.split(' ').skip(10).take(4).collect::<Vec<_>>().join("_");
        let refused = crate::engine::reach::contact_request_puts(&gs, "ann").err();
        assert_eq!(refused.as_deref(), Some(GUARD_LINE));
        gs.user_name = "Ann".into();
        let other = crate::engine::reach::contact_request_puts(&gs, "ann").err();
        assert_ne!(other.as_deref(), Some(GUARD_LINE), "an ordinary name goes on to the request itself");
        tidy(&gs);
    }
}
