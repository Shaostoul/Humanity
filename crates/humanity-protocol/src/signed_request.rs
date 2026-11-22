//! The bytes a person's key signs FOR A SERVER (2026-11-20, round 2 on 2026-11-21; docs/BUGS.md
//! under "signed requests"): a REQUEST (an account export, a bug vote, a vault sync, an admin action,
//! a channel read) and the sign-in proof a socket answers its server's challenge with (IDENTIFY).
//!
//! WHY IT EXISTS.
//!
//! Round 1 (2026-11-20). Requests used to be signed over `purpose\ntimestamp`, the very shape a chat
//! message is signed over (`text\ntimestamp`, the same key and the same verify), and chat
//! signatures are broadcast to the room. So a person who posted the chat line `account_export`
//! handed everyone in that room a working credential for `POST /api/account/export`. A request is
//! now signed under its own domain line, and a chat text that begins with a signing domain line
//! (`hum/<name>/v<n>` then a newline, [`starts_with_signing_domain`]) is never signed by our clients
//! and is refused by the relay, so no chat signature equals any `hum/...` preimage.
//!
//! Round 2 (2026-11-21). Neither a request nor a sign-in named the server it was for. So any server
//! a person sent a request to could replay it at another server inside the five-minute window (the
//! desktop signs a history read for every channel a server lists, so a server listing a channel
//! named like another server's members-only room read that room as the person); and a server could
//! open a socket to another server as the person, pass that server's sign-in challenge through to the
//! person's app, and hand back the signature, becoming the person there (slash commands, link codes,
//! moderation). Now EVERY signature a person's key makes for a server names that server, the way a
//! passkey is bound to its site (WebAuthn's relying party):
//!
//! - the AUDIENCE is the address the person's app connected to, host name and port (the port only
//!   when it is not the scheme's default), lowercase: [`audience_of`] on the desktop (the URL it
//!   opened), `location.host` on the website (web/chat/pq.js `pqAudience`);
//! - a request is signed over [`request_preimage`]: [`REQUEST_DOMAIN`], the audience, the purpose,
//!   the time in milliseconds, each on its own line;
//! - a sign-in is signed over [`identify_preimage`]: [`IDENTIFY_DOMAIN`], the audience, the server's
//!   nonce, the key. The app takes the audience only from the address it opened, never from anything
//!   the server says, and signs only a nonce of the server's shape ([`is_identify_nonce`]);
//! - each relay admits the audience only when it is the address the request or socket was sent to,
//!   one of its own names, and a name the client may use from where it is (a loopback or LAN name
//!   only from a client there: round 3), refusing any other in words that name only the audience
//!   signed for (`src/relay/audience.rs` `OwnNames::admit`).
//!
//! WHAT THIS DOES NOT STOP: a server that IS the audience receives the signature and can use it
//! there, once (the relay keeps each request's signature for one use, and a sign-in nonce is fresh
//! per socket). It cannot carry it anywhere else. And on plain http someone who can answer for the
//! name (a DNS or network attacker) is that name: only https (a certificate binding the name) makes
//! the audience mean the server, as for passkeys.
//!
//! The relay's checks are `src/relay/signed_request.rs` and `src/relay/relay.rs` (identify); the
//! desktop's signers `src/net/identity.rs` (`pq_sign_request`, `identify_response`); the website's
//! `web/chat/pq.js`. All are held to `scripts/tests/fixtures/signed-request-v2.json` (this file's
//! tests and `scripts/tests/signed-request-web.test.js`).

/// The first line of every signed request. A new shape gets a new version, never a reuse.
pub const REQUEST_DOMAIN: &str = "hum/request/v2";

/// The first line of every sign-in proof (the answer to a server's identify challenge).
pub const IDENTIFY_DOMAIN: &str = "hum/identify/v2";

/// The longest audience: a 253-byte host name, or a bracketed IPv6 address, then `:` and a port.
pub const MAX_AUDIENCE_BYTES: usize = 262;

/// The longest purpose a relay checks (2026-11-21, D4): a purpose names its target and carries a
/// BLAKE3 hash of any body ([`body_hash`]), never the body, so it stays short. The relay refuses a
/// longer one before it verifies anything or remembers anything.
pub const MAX_PURPOSE_BYTES: usize = 2_048;

/// A sign-in nonce: 32 random bytes in lowercase hex (relay.rs makes them).
pub const IDENTIFY_NONCE_HEX_LEN: usize = 64;

/// The bytes signed for a request: `hum/request/v2`, the audience, the purpose, the time (Unix
/// milliseconds), joined by `\n`. A purpose may itself hold several lines (`push_unsubscribe\n
/// {endpoint}`): it names what the request acts on, so a signature for one target is not good for
/// another. The audience comes first, on a line of its own, so nothing in the purpose can move it.
pub fn request_preimage(audience: &str, purpose: &str, timestamp: u64) -> String {
    format!("{REQUEST_DOMAIN}\n{audience}\n{purpose}\n{timestamp}")
}

/// The bytes signed to answer a server's identify challenge: `hum/identify/v2`, the audience, the
/// server's nonce, the person's public key (hex), joined by `\n`.
pub fn identify_preimage(audience: &str, nonce: &str, public_key: &str) -> String {
    format!("{IDENTIFY_DOMAIN}\n{audience}\n{nonce}\n{public_key}")
}

/// Whether `nonce` has the shape of a relay's identify challenge: exactly 64 lowercase hex
/// characters. An app signs nothing else as a sign-in, so a server cannot hand it other bytes to sign.
pub fn is_identify_nonce(nonce: &str) -> bool {
    nonce.len() == IDENTIFY_NONCE_HEX_LEN && nonce.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The audience for a URL the app opened: its host name, lowercase, then `:port` when the port is
/// not the scheme's default (80 for http and ws, 443 for https and wss). `None` for anything that is
/// not a usable address. `https://United-Humanity.us/ws` gives `united-humanity.us`;
/// `http://127.0.0.1:3210` gives `127.0.0.1:3210`; `wss://example.org:443/x` gives `example.org`.
/// A URL with no scheme is read as `host[:port]` and keeps any port it names.
///
/// ONE READING (2026-11-21, round 3, C3): the URL is read twice, by this strict hand reading and by
/// the WHATWG URL parser (`url::Url`) that the desktop's HTTP clients and the browser SEND by, and
/// `None` unless both name the same host and port. The two differ exactly where a signature could
/// name one server while the request goes to another: `https://evil.example\@victim.example/` is
/// victim.example by hand (the `@`) and evil.example to the sender (a backslash ends a special
/// URL's host); `127.1`, `0x7f.0.0.1` and `2130706433` are all 127.0.0.1 to the sender; IPv6 has
/// one written form (`[::1]`, never `[0:0:0:0:0:0:0:1]`). A host with a trailing dot is a second
/// spelling of a name and gives none ([`canonical_audience`]).
pub fn audience_of(url: &str) -> Option<String> {
    let url = url.trim();
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => (String::new(), url),
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host).to_ascii_lowercase();
    let (host, port) = split_host_port(&authority)?;
    let default_port = match scheme.as_str() {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        _ => None,
    };
    let joined = match port {
        Some(p) if Some(p) != default_port => format!("{host}:{p}"),
        _ => host.to_string(),
    };
    let audience = canonical_audience(&joined)?;
    // The sender's reading. A URL with no scheme is read as http would read its host.
    let sent = url::Url::parse(&if scheme.is_empty() { format!("http://{rest}") } else { url.to_string() }).ok()?;
    let hand_port = port.or(if scheme.is_empty() { Some(80) } else { default_port });
    (sent.host_str() == Some(host) && sent.port_or_known_default() == hand_port).then_some(audience)
}

/// An audience as written by a relay's settings or an HTTP `Host` header, checked and lowercased:
/// a host name of letters, digits, `.` and `-` (or a bracketed IPv6 address), then optionally `:`
/// and a port from 1 to 65535. `None` for anything else (a space, a newline, a path, an empty host).
pub fn canonical_audience(value: &str) -> Option<String> {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() || v.len() > MAX_AUDIENCE_BYTES {
        return None;
    }
    let (host, port) = split_host_port(&v)?;
    let host_ok = if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        !inner.is_empty() && inner.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.')
    } else {
        !host.is_empty()
            && host.len() <= 253
            && host.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
            && !host.starts_with(['.', '-'])
            && !host.ends_with('.')
            && !host.contains("..")
    };
    if !host_ok {
        return None;
    }
    Some(match port {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    })
}

/// Split `host[:port]` (a bracketed IPv6 address keeps its brackets). `None` when the port is not a
/// number from 1 to 65535.
fn split_host_port(authority: &str) -> Option<(&str, Option<u16>)> {
    let (host, port_text) = if authority.starts_with('[') {
        let end = authority.find(']')?;
        let (host, after) = authority.split_at(end + 1);
        match after {
            "" => (host, None),
            _ => (host, Some(after.strip_prefix(':')?)),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        }
    };
    let port = match port_text {
        None => None,
        Some(p) if !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) => {
            match p.parse::<u32>() {
                Ok(n) if (1..=65_535).contains(&n) => Some(n as u16),
                _ => return None,
            }
        }
        Some(_) => return None,
    };
    Some((host, port))
}

/// Whether `key` is a person's key in its one canonical form (2026-11-21, D3): a Dilithium3 public
/// key, 1,952 bytes, written as 3,904 LOWERCASE hex characters. A relay takes a key only so: hex
/// decoding ignores letter case, so `AB..` and `ab..` are the same key, and a relay that remembered
/// or limited by the text gave each spelling a fresh slot (one signature replayed once per spelling,
/// a per-key rate limit dodged by upper-casing, a bug vote counted again).
pub fn is_canonical_key(key: &str) -> bool {
    key.len() == crate::pq_crypto::DILITHIUM_PK_LEN * 2 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The BLAKE3 hash of a request's body, lowercase hex: what a purpose names instead of the body
/// itself (a vault blob, a system profile, a review's words), so the signature covers the body and
/// the purpose stays short.
pub fn body_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Whether `text` begins with a signing domain line: `hum/`, a name of lowercase letters, digits,
/// `-` or `.`, `/v`, a version number, then a newline. Every domain-separated preimage starts so,
/// and a chat message is signed over its text then `\n{timestamp}`; so a chat text that starts so
/// could be signed into the very bytes of a request, a report or a DM's inner signature. Our clients
/// refuse to sign such a chat text and the relay refuses to accept or broadcast one. A text that
/// merely mentions `hum/request/v2` later on, or on a line of its own with nothing after it, is not
/// refused: neither can equal a preimage, which always has the domain line first and more after it.
pub fn starts_with_signing_domain(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("hum/") else { return false };
    let Some((first_line, _)) = rest.split_once('\n') else { return false };
    let Some((name, version)) = first_line.split_once('/') else { return false };
    let name_ok = !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.');
    let version_ok = version
        .strip_prefix('v')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    name_ok && version_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words every side signs, pinned in one file the website's test reads too.
    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/scripts/tests/fixtures/signed-request-v2.json")))
            .expect("the fixture is JSON")
    }

    #[test]
    fn preimages_match_the_shared_fixture() {
        let fx = fixture();
        assert_eq!(fx["domain"].as_str(), Some(REQUEST_DOMAIN));
        assert_eq!(fx["identify_domain"].as_str(), Some(IDENTIFY_DOMAIN));
        let cases = fx["requests"].as_array().expect("requests");
        assert!(cases.len() >= 3);
        for c in cases {
            let (aud, purpose, ts) = (c["audience"].as_str().unwrap(), c["purpose"].as_str().unwrap(), c["timestamp"].as_u64().unwrap());
            assert_eq!(request_preimage(aud, purpose, ts), c["preimage"].as_str().unwrap(), "purpose {purpose:?}");
        }
        for c in fx["identify"].as_array().expect("identify") {
            let (aud, nonce, key) = (c["audience"].as_str().unwrap(), c["nonce"].as_str().unwrap(), c["key"].as_str().unwrap());
            assert_eq!(identify_preimage(aud, nonce, key), c["preimage"].as_str().unwrap());
        }
    }

    /// The audience each URL a person's app may open gives, and the addresses that give none.
    /// Red evidence (2026-11-21, `audience_of` made to keep the default port): this failed at
    /// `https://united-humanity.us:443/ws`, left "united-humanity.us:443", right "united-humanity.us".
    /// Round 3 (2026-11-21, `audience_of` without its `url::Url` reading): this failed at
    /// `https://evil.example\@victim.example/`, left Some("victim.example"), right None.
    #[test]
    fn audiences_match_the_shared_fixture() {
        let fx = fixture();
        for c in fx["audiences"].as_array().expect("audiences") {
            let url = c["url"].as_str().unwrap();
            assert_eq!(audience_of(url).as_deref(), c["audience"].as_str(), "url {url:?}");
        }
    }

    #[test]
    fn reserved_chat_texts_match_the_shared_fixture() {
        let fx = fixture();
        for t in fx["chat_refused"].as_array().unwrap() {
            assert!(starts_with_signing_domain(t.as_str().unwrap()), "should be refused: {t}");
        }
        for t in fx["chat_allowed"].as_array().unwrap() {
            assert!(!starts_with_signing_domain(t.as_str().unwrap()), "should be allowed: {t}");
        }
    }

    /// The point of round 1: the text a chat message would need to carry for its signature to be a
    /// request's is a text the chat refuses. Red evidence (2026-11-20, `request_preimage` put back to
    /// the old `{purpose}\n{timestamp}`): this failed at "a chat text produces the request's bytes"
    /// for `account_export`.
    #[test]
    fn no_allowed_chat_text_signs_a_request() {
        let ts = 1_800_000_000_000u64;
        for purpose in ["account_export", "bug_vote", "vault_sync", "push_unsubscribe\nhttps://example.test/x"] {
            let preimage = request_preimage("example.org", purpose, ts);
            // A chat message is signed over `text\n{ts}`: the only text that yields these bytes.
            let chat_text = preimage.strip_suffix(&format!("\n{ts}")).expect("the time ends the preimage");
            assert!(starts_with_signing_domain(chat_text), "a chat text produces the request's bytes: {chat_text:?}");
        }
        let sign_in = identify_preimage("example.org", &"a".repeat(64), "ab");
        assert!(starts_with_signing_domain(&sign_in), "a sign-in's words open with a signing domain line");
    }

    /// The point of round 2: the same request or sign-in for two servers is two different texts,
    /// whatever the purpose or nonce holds (the audience line comes before them).
    #[test]
    fn two_audiences_never_share_a_preimage() {
        let ts = 1_800_000_000_000u64;
        assert_ne!(request_preimage("a.example", "chat_read\nmessages\nx", ts), request_preimage("b.example", "chat_read\nmessages\nx", ts));
        // A purpose cannot smuggle a second audience line in front of the real one.
        assert_ne!(request_preimage("a.example", "b.example\nx", ts), request_preimage("b.example", "x", ts));
        let nonce = "0".repeat(64);
        assert_ne!(identify_preimage("a.example", &nonce, "ab"), identify_preimage("b.example", &nonce, "ab"));
    }

    #[test]
    fn only_the_relays_nonce_shape_is_signed() {
        assert!(is_identify_nonce(&"0123456789abcdef".repeat(4)));
        for bad in ["", "abc", &"A".repeat(64), &"g".repeat(64), &"a".repeat(63), &"a".repeat(65), &format!("{}\nb.example", "a".repeat(55))] {
            assert!(!is_identify_nonce(bad), "{bad:?}");
        }
    }

    /// D3: one spelling per key. Red evidence (2026-11-21, `is_canonical_key` made case-blind):
    /// "an upper-case spelling is not canonical" failed.
    #[test]
    fn a_key_has_one_spelling() {
        let key = "ab".repeat(crate::pq_crypto::DILITHIUM_PK_LEN);
        assert!(is_canonical_key(&key));
        assert!(!is_canonical_key(&key.to_ascii_uppercase()), "an upper-case spelling is not canonical");
        assert!(!is_canonical_key(&format!("A{}", &key["a".len()..])), "one upper-case letter is not canonical");
        assert!(!is_canonical_key(&key[2..]), "a short key is not a key");
        assert!(!is_canonical_key(&format!(" {}", &key["a".len()..])));
    }

    #[test]
    fn body_hashes_are_blake3_hex() {
        assert_eq!(body_hash(b""), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
        assert_eq!(body_hash(b"abc").len(), 64);
    }
}
