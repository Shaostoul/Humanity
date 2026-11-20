//! The recovery phrase, and the keys made from the 32 bytes it writes down.
//!
//! The 24-word BIP39 phrase encodes 32 bytes of entropy directly (not BIP39's PBKDF2 `to_seed()`),
//! exactly as the website reads it (web/chat/crypto.js `seedFromMnemonic`). Those 32 bytes, the
//! "seed" in the code, are the identity's root secret. Every key is DERIVED from them through
//! BLAKE3 `derive_key`, each under its own label, and no key IS them:
//! - the Dilithium3 / ML-DSA-65 chat identity (`hum/dilithium3/v1`) and the Kyber768 / ML-KEM-768
//!   private-message key (`hum/kyber768/v1`): crates/humanity-protocol/src/pq_crypto.rs;
//! - the Ed25519 Solana wallet key (`hum/ed25519-wallet/v1`, [`WALLET_KEY_DOMAIN`]): here.
//!
//! Until 2026-11-19 the 32 bytes themselves were the Ed25519 wallet key, so exporting the wallet
//! key to any wallet tool handed over the root of the chat identity and of every past private
//! message (docs/reference/research/2026-11-16-pqc-review/, "Checked in the code" item 1).

/// How many words a HumanityOS recovery phrase has (256 bits and an 8-bit check).
pub const PHRASE_WORDS: usize = 24;

/// What the restore screens say when the words do not add up: every word is a real one, but the
/// phrase's built-in check (the last 8 bits, from the other 256) does not match, which is what one
/// mistyped or swapped word does. The website says the same (web/chat/crypto.js
/// `seedFromMnemonic`).
pub const PHRASE_DOES_NOT_ADD_UP: &str =
    "One of the words is not right: the 24 words do not add up. Check each word against your written copy, in order.";

/// The 32-byte secret a recovery phrase writes down, read the way the website reads it: any case,
/// any spacing (`normalizeMnemonicInput`). Refused, with words a person can act on, when it is not
/// 24 words, when a word is not on the list, or when the words do not add up
/// (`PHRASE_DOES_NOT_ADD_UP`).
///
/// Until 2026-10-16 the desktop app fell back to decoding the words WITHOUT their check when the
/// check failed, so one mistyped word that happened to be another real word restored a different,
/// empty identity instead of saying a word was wrong. The website never did. The fallback's reason,
/// that the website might write a different check, does not hold: it writes the standard BIP39
/// check (the first byte of SHA-256 of the 32 bytes), pinned by the test vectors below and by
/// scripts/check-bip39-wordlists.js.
pub fn seed_from_phrase(text: &str) -> Result<[u8; 32], String> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.len() != PHRASE_WORDS {
        return Err(format!("Expected {PHRASE_WORDS} words, got {}. Check for extra spaces or missing words.", words.len()));
    }
    let list = bip39::Language::English;
    if let Some(unknown) = words.iter().find(|w| list.find_word(w).is_none()) {
        return Err(format!("Unknown word: \"{unknown}\". Check spelling."));
    }
    let mnemonic = bip39::Mnemonic::parse_in(list, words.join(" ")).map_err(|e| match e {
        bip39::Error::InvalidChecksum => PHRASE_DOES_NOT_ADD_UP.to_string(),
        other => format!("This recovery phrase could not be read ({other})."),
    })?;
    mnemonic.to_entropy().try_into().map_err(|_| "This recovery phrase does not hold a 32-byte key.".to_string())
}

// ── The wallet key (Ed25519, the Solana wallet) ─────────────────────────────

/// The BLAKE3 `derive_key` label the wallet's Ed25519 key is made under (2026-11-19).
///
/// Why this label: it follows the `hum/<algorithm>/v1` pattern of the other two
/// (`hum/dilithium3/v1`, `hum/kyber768/v1`) and also names the ROLE, because Ed25519 is the one
/// algorithm here with a job that is not the identity: should a second Ed25519 use ever appear,
/// it gets its own label instead of silently sharing the wallet's key. `v1` moves only with a
/// deliberate change of the derivation, which changes every wallet address again.
///
/// The website uses the same string (web/shared/wallet.js `WALLET_KEY_DOMAIN`); the known-answer
/// tests on both sides (`wallet_key_known_answer_for_the_frozen_phrase` below and
/// scripts/tests/wallet-key-web.test.js) pin the same key for the frozen test phrase.
pub const WALLET_KEY_DOMAIN: &str = "hum/ed25519-wallet/v1";

/// The wallet's 32-byte Ed25519 private key (RFC 8032's secret seed) for an identity's 32 bytes:
/// BLAKE3 `derive_key(WALLET_KEY_DOMAIN, seed)`. One way: the wallet key gives nothing back about
/// the seed, so a wallet key handed to another wallet tool exposes the wallet and nothing else.
/// None unless `seed` is 32 bytes.
pub fn wallet_secret_from_seed(seed: &[u8]) -> Option<[u8; 32]> {
    let seed: &[u8; 32] = seed.try_into().ok()?;
    Some(blake3::derive_key(WALLET_KEY_DOMAIN, seed))
}

/// The wallet's Ed25519 signing key (`wallet_secret_from_seed`).
pub fn wallet_signing_key(seed: &[u8]) -> Option<ed25519_dalek::SigningKey> {
    wallet_secret_from_seed(seed).map(|secret| ed25519_dalek::SigningKey::from_bytes(&secret))
}

/// The wallet's Ed25519 public key, 64 lowercase hex characters.
pub fn wallet_pubkey_hex(seed: &[u8]) -> Option<String> {
    wallet_signing_key(seed).map(|k| hex::encode(k.verifying_key().as_bytes()))
}

/// The wallet's Solana address: its Ed25519 public key in base58, the form Solana writes
/// addresses in (web/shared/wallet.js `publicKeyToSolanaAddress`).
pub fn wallet_address(seed: &[u8]) -> Option<String> {
    wallet_signing_key(seed).map(|k| bs58::encode(k.verifying_key().as_bytes()).into_string())
}

// ── Full-PQ identity (Dilithium3 id+signing, Kyber768 DM) ───────────────────
// Both keys derive from the 32-byte seed (each under its own label, as the wallet key does),
// byte-identical to the web client (crypto.js attachPqIdentity) and the
// relay — locked by pq_crypto::{dilithium,kyber}_cross_language_kat +
// scripts/pq-kat.mjs.

/// The post-quantum identity presented to the relay.
pub struct PqIdentity {
    /// Dilithium3 public key hex — THE chat identity (`public_key`).
    pub dilithium_hex: String,
    /// Kyber768 public key base64 — advertised at identify for DMs.
    pub kyber_public_b64: String,
}

/// Derive the full-PQ identity from the 32-byte BIP39 seed.
pub fn derive_pq_identity(seed32: &[u8]) -> Result<PqIdentity, String> {
    let dil_seed = humanity_protocol::pq_crypto::derive_dilithium_seed(seed32);
    let dil = humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    let dm = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed32)?;
    Ok(PqIdentity {
        dilithium_hex: hex::encode(dil.public_key()),
        kyber_public_b64: dm.public_base64(),
    })
}

/// Dilithium3 chat signature (hex) over `content\ntimestamp` — the EXACT
/// preimage the web client signs and the relay verifies (Inc3).
pub fn pq_sign_chat(seed32: &[u8], content: &str, timestamp: u64) -> String {
    let dil_seed = humanity_protocol::pq_crypto::derive_dilithium_seed(seed32);
    let dil = humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    hex::encode(dil.sign(format!("{content}\n{timestamp}").as_bytes()))
}

/// The Dilithium3 keypair itself, for signing full Humanity Network OBJECTS
/// (market provider/offering, votes, profiles) via
/// `relay::core::object::ObjectBuilder::sign`. Same derivation as the chat
/// identity: the merchant IS the chat identity, so an offering published
/// in-app is owned by the same key the importer would use.
pub fn pq_object_keypair(seed32: &[u8]) -> humanity_protocol::pq_crypto::DilithiumKeypair {
    let dil_seed = humanity_protocol::pq_crypto::derive_dilithium_seed(seed32);
    humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&dil_seed)
}

/// Raw-bytes Dilithium3 signature (Vec<u8>, 3309 bytes). Used by Inc3b
/// identify-challenge response, where the preimage is a domain-separated
/// string ("hum/identify/v1\n" + nonce + "\n" + pubkey) — not chat's
/// `content\ntimestamp` shape. Caller base64-encodes for the wire.
pub fn pq_sign_raw(seed32: &[u8], message: &[u8]) -> Vec<u8> {
    let dil_seed = humanity_protocol::pq_crypto::derive_dilithium_seed(seed32);
    let dil = humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    dil.sign(message)
}

/// Generate a fresh 32-byte BIP39 seed (256-bit entropy) from the OS
/// CSPRNG. This 32-byte value IS the identity: Dilithium3 (the chat identity),
/// Kyber768 (DM) and the Ed25519 wallet key are each derived from it under their
/// own label, and none of them is it. Persist it (encrypted) and back it up as the
/// 24-word phrase via `mnemonic_from_seed`.
pub fn generate_new_seed() -> Vec<u8> {
    let mut entropy = [0u8; 32];
    getrandom::getrandom(&mut entropy).expect("OS RNG failed");
    entropy.to_vec()
}

/// Render a 32-byte seed as its 24-word BIP39 mnemonic, for backup and
/// in-app display. Exact inverse of `seed_from_phrase`: re-entering this
/// phrase restores the same identity.
pub fn mnemonic_from_seed(seed: &[u8]) -> Option<String> {
    if seed.len() != 32 {
        return None;
    }
    bip39::Mnemonic::from_entropy(seed).ok().map(|m| m.to_string())
}

/// Build the JSON backup payload the web client's `importIdentityBackup` consumes
/// (crypto.js `importIdentityFromJSON`): `{name, privateKey, note}`, where `privateKey`
/// is the 32-byte seed in 64 hex characters. Encoding THIS as a QR lets a phone scan a
/// native identity and become the same account. Returns None if the seed is not 32 bytes.
///
/// No `publicKey` field (2026-11-19): it carried the Ed25519 public key of the seed itself,
/// back when the seed was also the wallet key. The importer requires only `name` and
/// `privateKey`; without `publicKey` it works out its own storage key from the seed
/// (`ed25519FromSeed`), so the desktop app no longer makes any Ed25519 key from the seed
/// except the wallet's, under its own label.
pub fn device_link_payload_json(seed: &[u8], name: &str) -> Option<String> {
    if seed.len() != 32 {
        return None;
    }
    let display = if name.trim().is_empty() { "user" } else { name.trim() };
    Some(format!(
        "{{\"name\":{},\"privateKey\":\"{}\",\"note\":\"Keep this safe. Anyone with it can impersonate you.\"}}",
        serde_json::to_string(display).ok()?,
        hex::encode(seed)
    ))
}

/// The device-link QR must encode a proper https URL with the payload in the
/// URL FRAGMENT (after `#`), NOT raw JSON. Root cause of a 2026-07-12 seed leak:
/// a QR containing raw `{"name":...}` text, when scanned by a phone's SYSTEM
/// camera, was handed to a browser which ran it as a WEB SEARCH -- sending the
/// seed to the search engine's servers. A well-formed https URL instead makes the
/// camera NAVIGATE to the chat page, and the fragment is never sent to any server
/// (browsers do not transmit `#...`), so the chat page reads it locally, imports,
/// and clears it. The seed still rides the QR image, but no third party ever
/// receives it. `<base>` lets self-hosters point at their own origin.
pub fn device_link_url(seed: &[u8], name: &str) -> Option<String> {
    device_link_url_for(seed, name, "https://united-humanity.us")
}

pub fn device_link_url_for(seed: &[u8], name: &str, base: &str) -> Option<String> {
    let json = device_link_payload_json(seed, name)?;
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
    let base = base.trim_end_matches('/');
    Some(format!("{base}/chat/index.html#devicelink={b64}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_seed_mnemonic_roundtrips_and_derives_pq() {
        // Generate → phrase → recover must yield the SAME 32-byte seed,
        // and that seed must derive a stable PQ identity. This is what
        // makes "Generate New Identity" + later recovery consistent.
        let seed = generate_new_seed();
        assert_eq!(seed.len(), 32);
        let phrase = mnemonic_from_seed(&seed).expect("mnemonic");
        assert_eq!(phrase.split_whitespace().count(), 24);
        let recovered = seed_from_phrase(&phrase).expect("recover").to_vec();
        assert_eq!(recovered, seed, "phrase must restore the exact seed");
        let a = derive_pq_identity(&seed).unwrap();
        let b = derive_pq_identity(&recovered).unwrap();
        assert_eq!(a.dilithium_hex, b.dilithium_hex);
        assert_eq!(a.kyber_public_b64, b.kyber_public_b64);
        // Two generated seeds must differ (distinct identities).
        assert_ne!(generate_new_seed(), generate_new_seed());
    }

    #[test]
    fn device_link_payload_matches_web_importer_contract() {
        // The native "Link a device" QR must produce EXACTLY the JSON the web
        // client's importIdentityFromJSON accepts: a name and the 64-char
        // privateKey (the 32-byte seed hex), the two fields it requires. If this
        // drifts, a scanned native identity silently fails to import on web.
        let seed = [9u8; 32];
        let json = device_link_payload_json(&seed, "Alice").expect("payload");
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(v["name"], "Alice");
        let privkey = v["privateKey"].as_str().unwrap();
        assert_eq!(privkey.len(), 64, "web importer requires privateKey.length === 64");
        assert_eq!(privkey, hex::encode(seed));
        let phrase = mnemonic_from_seed(&seed).unwrap();
        assert_eq!(seed_from_phrase(&phrase).map(hex::encode), Ok(privkey.to_string()), "the QR carries what the phrase restores");
        // No Ed25519 key of the seed itself rides along any more (2026-11-19): that key was the
        // wallet's until the wallet got its own label, and the importer works out its own.
        assert!(v.get("publicKey").is_none(), "no publicKey field: {json}");
        // Empty name falls back to "user" like the web export (importer rejects
        // an empty name).
        let j2 = device_link_payload_json(&seed, "   ").unwrap();
        let v2: serde_json::Value = serde_json::from_str(&j2).unwrap();
        assert_eq!(v2["name"], "user");
        // A non-32-byte seed yields no payload rather than a bad QR.
        assert!(device_link_payload_json(&[0u8; 16], "x").is_none());
    }

    /// The word list both apps use, fingerprinted: SHA-256 of the 2048 words joined by newlines.
    /// scripts/check-bip39-wordlists.js holds the website's list (web/chat/bip39-english.js) to the
    /// SAME number, so the two lists are equal word for word without a second copy in Rust. The guard
    /// for the 2026-07-12 incident where a non-canonical list shipped and native-shown words like
    /// "occur" were "unknown" on the web restore. Regenerate the web list with
    /// `node scripts/gen-wordlist.js`.
    pub(crate) const WORDLIST_SHA256: &str = "187db04a869dd9bc7be80d21a86497d692c0db6abd3aa8cb6be5d618ff757fae";

    #[test]
    fn bip39_wordlist_is_canonical_and_matches_web_kat() {
        use sha2::Digest as _;
        let list = bip39::Language::English.word_list();
        assert_eq!(list.len(), 2048);
        let fingerprint = hex::encode(sha2::Sha256::digest(list.join("\n").as_bytes()));
        assert_eq!(fingerprint, WORDLIST_SHA256, "the bip39 crate's list changed: regenerate the web list and both pins");
        // Cross-client known-answer: the canonical phrase for seed [7;32] must be
        // IDENTICAL to what the web client produces from the same shared list
        // (scripts/check-bip39-wordlists.js round-trips + prints this exact string).
        // Equal phrases for equal seeds is the guarantee that a phrase written on
        // native restores on web and vice-versa.
        let seed = [7u8; 32];
        let phrase = mnemonic_from_seed(&seed).expect("mnemonic");
        assert_eq!(
            phrase,
            "alpha deal scrub asthma idea logic bright thought alpha deal scrub asthma idea logic bright thought alpha deal scrub asthma idea logic bright truly",
            "native BIP39 phrase for [7;32] diverged from the web KAT -- the wordlists are out of sync"
        );
        // And it reads back to the same seed.
        assert_eq!(seed_from_phrase(&phrase), Ok(seed));
    }

    /// THE WALLET KEY, PINNED FOR THE FROZEN TEST PHRASE (2026-11-19, the PQC review's item 2).
    /// Seed [7;32] is the phrase "alpha deal scrub asthma ... bright truly" above, the same frozen
    /// seed every cross-language test uses (pq_crypto.rs `dilithium_cross_language_kat`). The SAME
    /// three values are asserted on the website's side (scripts/tests/wallet-key-web.test.js),
    /// computed there by an independent stack (the vendored noble BLAKE3 and WebCrypto's Ed25519),
    /// so a phrase restored on either app shows the same wallet address.
    ///
    /// Seen red 2026-11-19 against the old relationship (`wallet_secret_from_seed` returning the 32
    /// bytes themselves): left Some("0707...07"), right Some("d883ea7c...6e73"), and failed again
    /// alone. (The old wallet address for this seed was GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB.)
    #[test]
    fn wallet_key_known_answer_for_the_frozen_phrase() {
        let seed = [7u8; 32];
        assert_eq!(
            wallet_secret_from_seed(&seed).map(hex::encode).as_deref(),
            Some("d883ea7c52a0db010de32a89fef61181bda081710736b236c1184ff224496e73"),
            "BLAKE3 derive_key({WALLET_KEY_DOMAIN}) drift: the website will show another address"
        );
        assert_eq!(
            wallet_pubkey_hex(&seed).as_deref(),
            Some("a697fa9ba1a4794c49a86dd87a3798b61775c9e436848e3331df855b84f56e5e")
        );
        assert_eq!(wallet_address(&seed).as_deref(), Some("CDK69dGcrdKp2LFhmeKGnCg3mFv6ZhpPLHvqqgQB5vGd"));
        // The address is the public key in base58, nothing else.
        let pk = hex::decode(wallet_pubkey_hex(&seed).unwrap()).unwrap();
        assert_eq!(wallet_address(&seed), Some(bs58::encode(pk).into_string()));
        // Not 32 bytes: no wallet, rather than a key from a wrong-sized input.
        assert!(wallet_address(&[7u8; 16]).is_none());
        assert!(wallet_secret_from_seed(&[7u8; 64]).is_none());
    }

    /// THE WALLET SECRET IS NOT THE ROOT (2026-11-19): for any seed, the wallet's private key is
    /// neither the 32 bytes the recovery phrase writes down nor the seed of the chat identity or
    /// the private-message key, and its public key is not the Ed25519 key of the seed itself (the
    /// wallet address before 2026-11-19). So a wallet key exported to another wallet tool cannot
    /// unlock the identity.
    ///
    /// Seen red 2026-11-19 against the old relationship: "the wallet secret is the recovery
    /// phrase's own 32 bytes" failed for the first seed ([7;32] on both sides), and again alone.
    #[test]
    fn the_wallet_secret_is_not_the_recovery_phrase_entropy() {
        for seed in [[7u8; 32], [0u8; 32], [0xffu8; 32], {
            let mut s = [0u8; 32];
            for (i, b) in s.iter_mut().enumerate() { *b = i as u8; }
            s
        }] {
            let secret = wallet_secret_from_seed(&seed).unwrap();
            assert_ne!(secret, seed, "the wallet secret is the recovery phrase's own 32 bytes");
            assert_ne!(secret, humanity_protocol::pq_crypto::derive_dilithium_seed(&seed), "the wallet secret is the chat identity's seed");
            assert_ne!(&secret[..], &humanity_protocol::pq_crypto::derive_kyber_seed(&seed)[..32], "the wallet secret is the message key's seed");
            let old_wallet = ed25519_dalek::SigningKey::from_bytes(&seed).verifying_key();
            assert_ne!(wallet_signing_key(&seed).unwrap().verifying_key(), old_wallet, "the wallet address is still the one made from the 32 bytes themselves");
        }
    }

    /// THE STANDARD CHECK, BOTH WAYS: official BIP39 test vectors (the trezor python-mnemonic set)
    /// for 32-byte entropies. The website's generator writes the same phrases
    /// (scripts/check-bip39-wordlists.js holds it to these exact strings), so every phrase it ever
    /// made passes the desktop app's strict check, and the checksum-less fallback had no reason.
    #[test]
    fn official_bip39_vectors_read_and_write_the_same_on_both_apps() {
        for (byte, phrase) in [
            (0x00u8, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"),
            (0x7f, "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title"),
            (0x80, "letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless"),
            (0xff, "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote"),
        ] {
            assert_eq!(mnemonic_from_seed(&[byte; 32]).as_deref(), Some(phrase), "written for {byte:#04x}");
            assert_eq!(seed_from_phrase(phrase), Ok([byte; 32]), "read for {byte:#04x}");
        }
    }

    /// ONE WRONG WORD IS REFUSED, NOT RESTORED AS SOMEONE ELSE (2026-10-16): a phrase with one word
    /// swapped for another real word fails its check and is refused in words a person can act on;
    /// before, the desktop app decoded it anyway and restored a different, empty identity. Case and
    /// spacing do not matter (as on the website); an unknown word and a wrong count are named.
    ///
    /// Seen red 2026-10-16 with the old fallback put back: "one swapped word is refused" got
    /// Ok(a different seed).
    #[test]
    fn a_phrase_with_one_wrong_word_is_refused_not_restored_as_someone_else() {
        let seed = [7u8; 32];
        let phrase = mnemonic_from_seed(&seed).unwrap();
        for (at, swap) in [(0, "abandon"), (4, "ability"), (10, "zoo")] {
            let mut words: Vec<&str> = phrase.split(' ').collect();
            words[at] = swap;
            assert_eq!(seed_from_phrase(&words.join(" ")), Err(PHRASE_DOES_NOT_ADD_UP.to_string()), "one swapped word is refused (word {})", at + 1);
        }
        // Two words in the wrong order: the same.
        let mut words: Vec<&str> = phrase.split(' ').collect();
        words.swap(0, 1);
        assert_eq!(seed_from_phrase(&words.join(" ")), Err(PHRASE_DOES_NOT_ADD_UP.to_string()), "two words swapped");

        let shouted = format!("  {}\n", phrase.to_uppercase().replace(' ', "   "));
        assert_eq!(seed_from_phrase(&shouted), Ok(seed), "any case, any spacing, as on the website");
        let typo = phrase.replacen("alpha", "alpah", 1);
        assert_eq!(seed_from_phrase(&typo), Err("Unknown word: \"alpah\". Check spelling.".to_string()));
        let short: Vec<&str> = phrase.split(' ').take(12).collect();
        assert_eq!(seed_from_phrase(&short.join(" ")), Err("Expected 24 words, got 12. Check for extra spaces or missing words.".to_string()));
    }

    #[test]
    fn pq_identity_deterministic_from_seed() {
        // Same seed → same Dilithium id + Kyber DM key (the property that
        // makes web↔native interop work; cross-language locked by KAT).
        let seed = [7u8; 32];
        let a = derive_pq_identity(&seed).unwrap();
        let b = derive_pq_identity(&seed).unwrap();
        assert_eq!(a.dilithium_hex, b.dilithium_hex);
        assert_eq!(a.kyber_public_b64, b.kyber_public_b64);
        assert_eq!(a.dilithium_hex.len(), 1952 * 2, "ML-DSA-65 pubkey hex");
        // Signature preimage matches the web/relay contract.
        let sig = pq_sign_chat(&seed, "hi", 123);
        assert_eq!(sig.len(), 3309 * 2, "ML-DSA-65 sig hex");
    }

    #[test]
    fn test_mnemonic_roundtrip() {
        // A random phrase restores its own 32 bytes, and the same wallet each time.
        // bip39 0.7+ requires explicit entropy: 32 bytes = 256 bits = 24 words.
        let mut entropy = [0u8; 32];
        getrandom::getrandom(&mut entropy).expect("OS RNG failed");
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy)
            .expect("Failed to generate mnemonic");
        let phrase = mnemonic.to_string();

        let seed = seed_from_phrase(&phrase).expect("Failed to read the phrase");
        assert_eq!(seed, entropy);
        let wallet = wallet_pubkey_hex(&seed).expect("a wallet key");
        assert_eq!(wallet.len(), 64, "Public key hex should be 64 chars");
        assert_eq!(wallet_pubkey_hex(&seed_from_phrase(&phrase).unwrap()), Some(wallet), "Same phrase, same wallet");
    }

    #[test]
    fn test_invalid_mnemonic() {
        assert!(seed_from_phrase("not a valid mnemonic phrase").is_err());
    }
}
