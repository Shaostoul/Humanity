// Child of chat/protected.rs (#[path]): what a message row leaves out while the protected setup
// hides a non-friend's pictures and files (the 2026-10-10 batch review, item 4). Each test was
// seen failing once on purpose, recorded at the test.

use super::*;

/// A signed-in app with the protected setup on (pictures from non-friends not shown) and its
/// preset loaded. No DM store, so nobody counts as a friend.
fn protected_app() -> GuiState {
    let mut gs = GuiState::default();
    gs.profile_public_key = "me".into();
    assert!(crate::engine::protected::ensure_preset(&mut gs), "the preset loads");
    let preset = gs.protected.preset.clone().unwrap();
    let pin = crate::net::protected::PinVerifier::with_salt("1234", &[5u8; 16], 1_000);
    gs.protected.setup = crate::net::protected::ProtectedSetup::applied(&preset, pin, "me", Vec::new());
    gs
}

fn from(sender: &str, content: &str) -> ChatMessage {
    ChatMessage { sender_name: sender.into(), sender_key: sender.into(), content: content.into(), timestamp_ms: 3, channel: "general".into(), ..Default::default() }
}

/// PICTURES AND FILES (10h; the web hides audio, video and document links too,
/// web/shared/protected.js): from someone who is not a friend, every link to a picture, a sound,
/// a video or a document is left out of the text and the preset's line is shown; a message with
/// no such link is untouched; our own, and anyone's with the setup off, are untouched.
/// Seen red 2026-10-10 with `withheld` asking only about pictures and private files (the old
/// rule): "a sound, a video and a document" got None.
#[test]
fn a_non_friends_sounds_videos_and_documents_are_left_out_too() {
    let mut gs = protected_app();
    let line = picture_hidden_line(&gs).to_string();
    assert_eq!(line, "A picture or file from someone who is not a friend is not shown.", "the preset's line");

    let media = "listen https://example.org/song.mp3 and read /uploads/7f3a.pdf, then https://example.org/clip.webm?t=3";
    let rest = withheld(&gs, &from("stranger", media)).expect("a sound, a video and a document");
    for gone in ["song.mp3", "7f3a.pdf", "clip.webm"] {
        assert!(!rest.contains(gone), "{gone} is left out: {rest}");
    }
    assert!(rest.starts_with("listen") && rest.contains("then"), "the words stay: {rest}");
    for each in ["https://example.org/a.ogg", "https://example.org/b.wav", "https://example.org/c.mp4", "/uploads/d.txt", "/uploads/e.md", "/uploads/f.json", "https://example.org/g.zip", "https://example.org/h.tar.gz"] {
        assert!(withheld(&gs, &from("stranger", each)).is_some_and(|r| r.is_empty()), "{each} is left out");
    }
    let picture = withheld(&gs, &from("stranger", "look https://example.org/a.png")).expect("a picture");
    assert_eq!(picture, "look");
    assert!(withheld(&gs, &from("stranger", "see https://example.org/about and bring snacks")).is_none(), "no picture, no file: untouched");
    assert!(withheld(&gs, &from("me", media)).is_none(), "our own: untouched");

    gs.protected.setup = Default::default();
    assert!(withheld(&gs, &from("stranger", media)).is_none(), "setup off: untouched");
}

/// The link finder the rule uses: a web address or a server path ending in one of the web's
/// kinds, with a query kept and trailing punctuation dropped; anything else is not a file link.
/// Seen red 2026-10-10 with the trailing-punctuation trim taken out: "a link in brackets" kept
/// its ")" and was not seen as a .pdf.
#[test]
fn the_file_link_finder_matches_the_webs_list() {
    use crate::net::protected::{file_urls, strip_file_urls};
    assert_eq!(file_urls("(see https://example.org/plan.pdf)"), ["https://example.org/plan.pdf"], "a link in brackets");
    assert_eq!(file_urls("https://example.org/a.MP3?x=1#t"), ["https://example.org/a.MP3?x=1#t"], "any case, a query kept");
    assert!(file_urls("https://example.org/a.mp3x https://example.org/pdf notes.pdf").is_empty(), "not a file link");
    assert_eq!(strip_file_urls("one https://example.org/a.mp3 two\n/uploads/b.zip\nthree"), "one two\nthree");
}
