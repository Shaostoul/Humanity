//! BIP39 recovery phrase identity recovery.
//!
//! Matches the web client's key derivation: the 24-word BIP39 mnemonic
//! directly encodes the 32-byte Ed25519 seed as its entropy (not using
//! PBKDF2-based `to_seed()`). This ensures the same mnemonic produces
//! the same public key on both web and native.

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

/// Derive an Ed25519 keypair from a 24-word recovery phrase (`seed_from_phrase`).
///
/// Returns `(public_key_hex, private_key_bytes)` on success.
/// The private_key_bytes are the raw 32-byte Ed25519 seed.
pub fn derive_keypair_from_mnemonic(mnemonic_str: &str) -> Result<(String, Vec<u8>), String> {
    let secret_key_bytes = seed_from_phrase(mnemonic_str)?;
    // Derive the Ed25519 public key from the seed
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_key_bytes);
    let public_key = signing_key.verifying_key();
    let public_key_hex = hex::encode(public_key.as_bytes());
    Ok((public_key_hex, secret_key_bytes.to_vec()))
}

// ── Full-PQ identity (Dilithium3 id+signing, Kyber768 DM) ───────────────────
// Both keys derive from the SAME 32-byte BIP39 seed the Ed25519 key uses,
// byte-identical to the web client (crypto.js attachPqIdentity) and the
// relay — locked by pq_crypto::{dilithium,kyber}_cross_language_kat +
// scripts/pq-kat.mjs. Ed25519 is kept ONLY as the seed/Solana wallet.

/// The post-quantum identity presented to the relay.
pub struct PqIdentity {
    /// Dilithium3 public key hex — THE chat identity (`public_key`).
    pub dilithium_hex: String,
    /// Kyber768 public key base64 — advertised at identify for DMs.
    pub kyber_public_b64: String,
}

/// Derive the full-PQ identity from the 32-byte BIP39 seed.
pub fn derive_pq_identity(seed32: &[u8]) -> Result<PqIdentity, String> {
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    let dm = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed32)?;
    Ok(PqIdentity {
        dilithium_hex: hex::encode(dil.public_key()),
        kyber_public_b64: dm.public_base64(),
    })
}

/// Dilithium3 chat signature (hex) over `content\ntimestamp` — the EXACT
/// preimage the web client signs and the relay verifies (Inc3).
pub fn pq_sign_chat(seed32: &[u8], content: &str, timestamp: u64) -> String {
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    hex::encode(dil.sign(format!("{content}\n{timestamp}").as_bytes()))
}

/// The Dilithium3 keypair itself, for signing full Humanity Network OBJECTS
/// (market provider/offering, votes, profiles) via
/// `relay::core::object::ObjectBuilder::sign`. Same derivation as the chat
/// identity: the merchant IS the chat identity, so an offering published
/// in-app is owned by the same key the importer would use.
pub fn pq_object_keypair(seed32: &[u8]) -> crate::relay::core::pq_crypto::DilithiumKeypair {
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed)
}

/// Raw-bytes Dilithium3 signature (Vec<u8>, 3309 bytes). Used by Inc3b
/// identify-challenge response, where the preimage is a domain-separated
/// string ("hum/identify/v1\n" + nonce + "\n" + pubkey) — not chat's
/// `content\ntimestamp` shape. Caller base64-encodes for the wire.
pub fn pq_sign_raw(seed32: &[u8], message: &[u8]) -> Vec<u8> {
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    dil.sign(message)
}

/// Generate a fresh 32-byte BIP39 seed (256-bit entropy) from the OS
/// CSPRNG. This 32-byte value IS the identity — it deterministically
/// re-derives Ed25519 (Solana wallet), Dilithium3 (the chat identity),
/// and Kyber768 (DM). Persist it (encrypted) and back it up as the
/// 24-word phrase via `mnemonic_from_seed`.
pub fn generate_new_seed() -> Vec<u8> {
    let mut entropy = [0u8; 32];
    getrandom::getrandom(&mut entropy).expect("OS RNG failed");
    entropy.to_vec()
}

/// Render a 32-byte seed as its 24-word BIP39 mnemonic, for backup and
/// in-app display. Exact inverse of `derive_keypair_from_mnemonic`'s
/// entropy path: re-entering this phrase restores the same identity.
pub fn mnemonic_from_seed(seed: &[u8]) -> Option<String> {
    if seed.len() != 32 {
        return None;
    }
    bip39::Mnemonic::from_entropy(seed).ok().map(|m| m.to_string())
}

/// The Ed25519 public key hex (64 chars) for a 32-byte seed. This is the SAME
/// `publicKey` field the web backup uses (web `myIdentity.publicKeyHex`), not
/// the long Dilithium identity key. Used to build a device-link payload the web
/// importer accepts (it validates `publicKey.length === 64`).
pub fn ed25519_pubkey_hex_from_seed(seed: &[u8]) -> Option<String> {
    if seed.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&arr);
    Some(hex::encode(signing_key.verifying_key().as_bytes()))
}

/// Build the exact JSON backup payload the web client's `importIdentityBackup`
/// consumes (crypto.js `importIdentityFromJSON`): `{name, publicKey, privateKey}`
/// where both keys are 64-char hex (Ed25519 pubkey + 32-byte seed). Encoding
/// THIS as a QR lets a phone scan a native identity and become the same account.
/// Returns None if the seed is not a 32-byte BIP39 seed.
pub fn device_link_payload_json(seed: &[u8], name: &str) -> Option<String> {
    let pubkey = ed25519_pubkey_hex_from_seed(seed)?;
    let display = if name.trim().is_empty() { "user" } else { name.trim() };
    // Match the web export field-for-field so the importer's validation passes.
    Some(format!(
        "{{\"name\":{},\"publicKey\":\"{}\",\"privateKey\":\"{}\",\"note\":\"Keep this safe. Anyone with it can impersonate you.\"}}",
        serde_json::to_string(display).ok()?,
        pubkey,
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
        let (_pk, recovered) =
            derive_keypair_from_mnemonic(&phrase).expect("recover");
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
        // client's importIdentityFromJSON accepts: name + 64-char publicKey +
        // 64-char privateKey, where publicKey is the Ed25519 pubkey (NOT the
        // long Dilithium key) and privateKey is the 32-byte seed hex. If this
        // drifts, a scanned native identity silently fails to import on web.
        let seed = [9u8; 32];
        let json = device_link_payload_json(&seed, "Alice").expect("payload");
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(v["name"], "Alice");
        let pubkey = v["publicKey"].as_str().unwrap();
        let privkey = v["privateKey"].as_str().unwrap();
        assert_eq!(pubkey.len(), 64, "web importer requires publicKey.length === 64");
        assert_eq!(privkey.len(), 64, "web importer requires privateKey.length === 64");
        // privateKey is the seed; publicKey is the Ed25519 pubkey for that seed,
        // identical to the recovery path derive_keypair_from_mnemonic uses.
        assert_eq!(privkey, hex::encode(seed));
        let phrase = mnemonic_from_seed(&seed).unwrap();
        let (derived_pub, _) = derive_keypair_from_mnemonic(&phrase).unwrap();
        assert_eq!(pubkey, derived_pub, "QR pubkey must match the recovery-derived pubkey");
        assert_eq!(pubkey, ed25519_pubkey_hex_from_seed(&seed).unwrap());
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
        let (_pk, recovered) = derive_keypair_from_mnemonic(&phrase).expect("recover");
        assert_eq!(recovered, seed);
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
            assert!(derive_keypair_from_mnemonic(&words.join(" ")).is_err(), "and nothing is restored from it");
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
        // Generate a random mnemonic and verify we can derive a keypair.
        // bip39 0.7+ requires explicit entropy: 32 bytes = 256 bits = 24 words.
        let mut entropy = [0u8; 32];
        getrandom::getrandom(&mut entropy).expect("OS RNG failed");
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy)
            .expect("Failed to generate mnemonic");
        let phrase = mnemonic.to_string();

        let (pubkey_hex, privkey) = derive_keypair_from_mnemonic(&phrase)
            .expect("Failed to derive keypair");

        assert_eq!(pubkey_hex.len(), 64, "Public key hex should be 64 chars");
        assert_eq!(privkey.len(), 32, "Private key should be 32 bytes");

        // Derive again — should produce the same key
        let (pubkey_hex2, _) = derive_keypair_from_mnemonic(&phrase)
            .expect("Failed to derive keypair second time");
        assert_eq!(pubkey_hex, pubkey_hex2, "Same mnemonic should produce same key");
    }

    #[test]
    fn test_invalid_mnemonic() {
        let result = derive_keypair_from_mnemonic("not a valid mnemonic phrase");
        assert!(result.is_err());
    }
}
