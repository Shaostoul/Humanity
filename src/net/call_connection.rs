//! How this person's calls and voice rooms connect (docs/design/blocking-and-safe-mode.md 10p,
//! the operator's question of 2026-10-22: "what if someone is okay using google servers?").
//!
//! Three modes, one setting, both apps (Settings > Privacy here, Settings > Safety in the web
//! chat):
//!
//! - **Private** (the default): only the server's call relay is offered to the other side, so
//!   nobody in the call sees this device's network address (step E, 10f, unchanged).
//! - **Direct**: this device's own addresses are offered too (its host address, and the
//!   server-reflexive one the connected server's own STUN responder reports), so two people who
//!   both chose Direct can connect device to device. The relay is still offered, so a Direct
//!   person and a Private person connect through it.
//! - **Direct, also asking a public service**: as Direct, and a public STUN server the person
//!   picks from `data/net/call_connection.json` (Google's, say) is asked for the address as
//!   well. That service sees the address; nothing on the list is ever asked in the other modes.
//!
//! Each person's choice governs only their OWN address: what decides whether two people connect
//! directly is that both offered a direct address, and the relay always carries the rest. The
//! protected setup forces Private (`effective`).
//!
//! The words both apps show come from the same data file, so they cannot drift apart. This file
//! has no socket and no GUI: src/net/webrtc.rs acts on the mode, src/engine/call_relay.rs hands
//! it over every frame, and src/gui/pages/settings/calls.rs draws the choice.

use std::sync::OnceLock;

use serde::Deserialize;

/// Where the words and the public STUN list live, next to the exe like the rest of `data/`.
pub const DATA_FILE: &str = "data/net/call_connection.json";

/// How this person's calls and voice rooms connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallConnection {
    /// Only the server's call relay is offered: nobody in the call sees this device's address.
    #[default]
    Private,
    /// This device's own addresses are offered too, found with the server's own STUN responder.
    Direct,
    /// As Direct, and a public STUN server the person chose is asked as well.
    DirectPublicStun,
}

impl CallConnection {
    /// Every mode, in the order they are offered.
    pub const ALL: [CallConnection; 3] = [CallConnection::Private, CallConnection::Direct, CallConnection::DirectPublicStun];

    /// The saved value (AppConfig `call_connection`, the web's `hos_call_connection`).
    pub fn id(self) -> &'static str {
        match self {
            CallConnection::Private => "private",
            CallConnection::Direct => "direct",
            CallConnection::DirectPublicStun => "direct_public_stun",
        }
    }

    /// A saved value read back. Anything else, an empty value or a mode a newer app wrote
    /// included, is Private: the safe default.
    pub fn from_id(id: &str) -> CallConnection {
        match id.trim() {
            "direct" => CallConnection::Direct,
            "direct_public_stun" => CallConnection::DirectPublicStun,
            _ => CallConnection::Private,
        }
    }

    /// Whether this device's own network address is offered to the other side.
    pub fn shows_my_address(self) -> bool {
        !matches!(self, CallConnection::Private)
    }

    /// Whether a public STUN server is asked as well as the server's own.
    pub fn asks_public_stun(self) -> bool {
        matches!(self, CallConnection::DirectPublicStun)
    }
}

/// The mode that applies: the person's choice, except that the protected setup is always Private
/// (design 7.4: "Hide my address" is on and cannot be turned off there).
pub fn effective(chosen: CallConnection, protected: bool) -> CallConnection {
    if protected {
        CallConnection::Private
    } else {
        chosen
    }
}

/// One mode's words.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModeWords {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub detail: String,
}

/// One public STUN server a person may opt into.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PublicStun {
    pub id: String,
    /// Who runs it, as the person sees it ("Google").
    pub name: String,
    /// `stun:host:port`.
    pub url: String,
}

/// The data file, read.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Words {
    #[serde(default)]
    pub heading: String,
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub protected_line: String,
    #[serde(default)]
    pub direct_without_server: String,
    #[serde(default)]
    pub public_stun_label: String,
    #[serde(default)]
    pub modes: Vec<ModeWords>,
    #[serde(default)]
    pub public_stun: Vec<PublicStun>,
}

impl Words {
    /// Read the file's text. `None` when it is not the file's shape.
    pub fn parse(text: &str) -> Option<Words> {
        serde_json::from_str(text).ok()
    }

    /// The words for `mode`; a mode the file does not describe gets its id as its label, so the
    /// choice can still be made and seen.
    pub fn mode(&self, mode: CallConnection) -> ModeWords {
        self.modes.iter().find(|m| m.id == mode.id()).cloned().unwrap_or_else(|| ModeWords {
            id: mode.id().to_string(),
            label: mode.id().replace('_', " "),
            ..Default::default()
        })
    }

    /// The public STUN server the person picked: the one with id `chosen`, else the first listed.
    /// `None` when the list is empty.
    pub fn public_stun(&self, chosen: &str) -> Option<&PublicStun> {
        self.public_stun.iter().find(|s| s.id == chosen).or_else(|| self.public_stun.first())
    }

    /// The line said when a Direct call cannot use the server's relay.
    pub fn direct_without_server_line(&self) -> &str {
        if self.direct_without_server.is_empty() {
            DIRECT_WITHOUT_SERVER_FALLBACK
        } else {
            &self.direct_without_server
        }
    }
}

/// The line for a Direct call the server cannot relay, when the data file is missing.
const DIRECT_WITHOUT_SERVER_FALLBACK: &str =
    "This server cannot carry calls yet, so this one connects device to device where your networks allow it. The people in it can see your network address.";

/// The data file, read once for the run (`Words::default()` when it is missing or damaged: the
/// choice still works, Private needs no words, and the Direct modes fall back to their ids).
pub fn words() -> &'static Words {
    static WORDS: OnceLock<Words> = OnceLock::new();
    WORDS.get_or_init(|| {
        std::fs::read_to_string(DATA_FILE).ok().and_then(|t| Words::parse(&t)).unwrap_or_else(|| {
            log::warn!("{DATA_FILE} is missing or damaged: the call connection choice shows its ids only");
            Words::default()
        })
    })
}

/// `stun:host:port` (or `stun:host`, port 3478) to `host:port`. `None` for anything else (a
/// `turn:` or `stuns:` address, an empty host).
pub fn stun_host_port(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("stun:")?;
    let addr = rest.split('?').next().unwrap_or(rest);
    crate::net::call_relay::with_port(addr, 3478)
}

/// The public STUN servers to ask for `mode`, as `host:port`: the one the person picked in the
/// third mode, none in the other two.
pub fn public_stun_hosts(mode: CallConnection, chosen: &str, words: &Words) -> Vec<String> {
    if !mode.asks_public_stun() {
        return Vec::new();
    }
    words.public_stun(chosen).and_then(|s| stun_host_port(&s.url)).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> Words {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(DATA_FILE)).expect("the data file");
        Words::parse(&text).expect("the data file reads")
    }

    /// The saved value round-trips, and anything unknown is Private. Seen red 2026-10-22 with
    /// `from_id` answering Direct for an unknown id: "an unknown mode is Private".
    #[test]
    fn ids_round_trip_and_unknown_is_private() {
        for m in CallConnection::ALL {
            assert_eq!(CallConnection::from_id(m.id()), m);
        }
        assert_eq!(CallConnection::from_id(""), CallConnection::Private, "nothing saved yet is Private");
        assert_eq!(CallConnection::from_id("relay_via_mars"), CallConnection::Private, "an unknown mode is Private");
        assert!(!CallConnection::Private.shows_my_address());
        assert!(CallConnection::Direct.shows_my_address() && !CallConnection::Direct.asks_public_stun());
        assert!(CallConnection::DirectPublicStun.shows_my_address() && CallConnection::DirectPublicStun.asks_public_stun());
        assert_eq!(CallConnection::default(), CallConnection::Private, "the default is Private");
    }

    /// The protected setup is always Private, whatever was chosen.
    #[test]
    fn the_protected_setup_is_always_private() {
        for m in CallConnection::ALL {
            assert_eq!(effective(m, true), CallConnection::Private);
            assert_eq!(effective(m, false), m);
        }
    }

    /// A public STUN server is asked only in the third mode, and only the one picked (the first
    /// listed when the pick is unknown). Seen red 2026-10-22 with `public_stun_hosts` not checking
    /// the mode: "Private asks no public service".
    #[test]
    fn a_public_service_is_asked_only_in_the_third_mode() {
        let words = Words {
            public_stun: vec![
                PublicStun { id: "one".into(), name: "One".into(), url: "stun:stun.one.example:5349".into() },
                PublicStun { id: "two".into(), name: "Two".into(), url: "stun:stun.two.example".into() },
            ],
            ..Default::default()
        };
        assert!(public_stun_hosts(CallConnection::Private, "two", &words).is_empty(), "Private asks no public service");
        assert!(public_stun_hosts(CallConnection::Direct, "two", &words).is_empty(), "Direct asks no public service");
        assert_eq!(public_stun_hosts(CallConnection::DirectPublicStun, "two", &words), vec!["stun.two.example:3478".to_string()]);
        assert_eq!(public_stun_hosts(CallConnection::DirectPublicStun, "", &words), vec!["stun.one.example:5349".to_string()], "the first when none was picked");
        assert!(public_stun_hosts(CallConnection::DirectPublicStun, "x", &Words::default()).is_empty(), "an empty list asks nobody");
        assert_eq!(stun_host_port("turn:relay.example:3478"), None);
        assert_eq!(stun_host_port("stun:[2001:db8::1]:3478").as_deref(), Some("[2001:db8::1]:3478"));
    }

    /// The shipped data file describes all three modes, in order, with words for each, and every
    /// public service on it is a STUN address with a name.
    #[test]
    fn the_data_file_describes_every_mode() {
        let w = file();
        let ids: Vec<&str> = w.modes.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, CallConnection::ALL.iter().map(|m| m.id()).collect::<Vec<_>>());
        for m in &w.modes {
            assert!(!m.label.is_empty() && !m.summary.is_empty() && !m.detail.is_empty(), "{m:?}");
        }
        assert!(!w.heading.is_empty() && !w.intro.is_empty() && !w.protected_line.is_empty() && !w.direct_without_server.is_empty());
        assert!(!w.public_stun.is_empty(), "at least one public service to choose");
        for s in &w.public_stun {
            assert!(!s.id.is_empty() && !s.name.is_empty(), "{s:?}");
            assert!(stun_host_port(&s.url).is_some(), "a STUN address: {s:?}");
        }
        assert_eq!(w.mode(CallConnection::Direct).id, "direct");
    }
}
