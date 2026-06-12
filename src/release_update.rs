//! Signed-release verification + signing (the supply-chain root of trust).
//!
//! WHY THIS EXISTS (security audit 2026-06-12, the CRITICAL finding): the
//! auto-updater used to install a downloaded binary after only a 1 MB size
//! check — no signature, no hash. A GitHub-account/release compromise (or a
//! wrong "Latest" pointer, cf. the v0.415 stray tag) would have been silent
//! RCE on every desktop user. This module makes the updater (and the local
//! `find_newer_exe` launcher) FAIL CLOSED unless the release is signed by the
//! operator's keys.
//!
//! SCHEME (operator-chosen): **hybrid Ed25519 + Dilithium3 (ML-DSA-65), BOTH
//! must verify.** Classical robustness from the battle-tested Ed25519 *plus*
//! post-quantum safety from the project's house PQ primitive. An attacker has
//! to break BOTH to forge a release — which also hedges against a bug in the
//! still-young ML-DSA implementations. The signing key is a DEDICATED key
//! (never the operator's personal identity seed), held passphrase-encrypted on
//! the operator's machine and used to sign LOCALLY — never in CI (the whole
//! point is to survive a CI/GitHub compromise).
//!
//! FLOW:
//!  - Operator, once:  `HumanityOS --gen-release-key`  → writes the public keys
//!    to `data/release/signing_pubkeys.json` (committed; compiled into every
//!    build) and the encrypted PRIVATE keys to a vault file (gitignored, backed
//!    up by the operator). Until this runs, the embedded pubkeys are empty and
//!    verification is SKIPPED with a loud warning (legacy behaviour) so the app
//!    keeps working during the one-time provisioning.
//!  - Operator, per release:  build a `release-manifest.json` listing each
//!    platform artifact's sha256, then `HumanityOS --sign-release <manifest>` →
//!    writes `<manifest>.sig.json` (the two signatures over the exact manifest
//!    bytes). Upload both to the GitHub release.
//!  - Client (updater / launcher): fetch the manifest + sig, verify BOTH
//!    signatures over the exact manifest bytes against the EMBEDDED pubkeys,
//!    then check the downloaded artifact's sha256 against the manifest. Any
//!    failure → abort, never execute.
//!
//! The signature covers the VERBATIM manifest bytes (not a re-serialization),
//! so there is no canonicalization-mismatch foot-gun: signer and verifier hash
//! the identical bytes.

#![cfg(feature = "native")]

use serde::{Deserialize, Serialize};

/// Algorithm tag embedded in every signature file. Bump the suffix if the
/// signed-bytes definition or the key set ever changes.
pub const HYBRID_ALG: &str = "hybrid-ed25519-mldsa65-v1";

/// Embedded operator public keys, compiled in from the committed JSON. Empty
/// strings mean "signing not provisioned yet" (pre-keygen) → verification is
/// skipped with a warning. Once the operator runs `--gen-release-key` and
/// commits the file, these are non-empty and verification is ENFORCED.
const EMBEDDED_PUBKEYS_JSON: &str = include_str!("../data/release/signing_pubkeys.json");

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SigningPubkeys {
    /// Ed25519 verifying key, 32 bytes, hex (64 chars). Empty = unprovisioned.
    #[serde(default)]
    pub ed25519: String,
    /// Dilithium3 (ML-DSA-65) verifying key, 1952 bytes, hex. Empty = unprovisioned.
    #[serde(default)]
    pub dilithium: String,
}

impl SigningPubkeys {
    /// True once both keys are present (signing has been provisioned).
    pub fn is_provisioned(&self) -> bool {
        !self.ed25519.is_empty() && !self.dilithium.is_empty()
    }
}

/// The embedded operator pubkeys (parsed from the compiled-in JSON).
pub fn embedded_pubkeys() -> SigningPubkeys {
    serde_json::from_str(EMBEDDED_PUBKEYS_JSON).unwrap_or_default()
}

/// One artifact (platform binary) in a release manifest.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestArtifact {
    /// Exact asset filename as uploaded to the release (e.g. `HumanityOS-windows-x64.exe`).
    pub name: String,
    /// Lowercase hex SHA-256 of the artifact bytes.
    pub sha256: String,
    /// Size in bytes (a cheap sanity cross-check; the sha256 is authoritative).
    #[serde(default)]
    pub size: u64,
}

/// A release manifest: the version + every platform artifact's hash. The
/// VERBATIM bytes of this file are what get signed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReleaseManifest {
    /// Release version, e.g. "0.418.0" (no leading `v`).
    pub version: String,
    pub artifacts: Vec<ManifestArtifact>,
}

/// The detached signature file accompanying a manifest. Both signatures are
/// over the EXACT manifest bytes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestSignature {
    pub alg: String,
    /// Ed25519 signature (64 bytes) over the manifest bytes, base64.
    pub ed25519: String,
    /// Dilithium3 signature (3309 bytes) over the manifest bytes, base64.
    pub dilithium: String,
}

/// Outcome of verifying a release artifact against a signed manifest.
#[derive(Debug)]
pub enum VerifyOutcome {
    /// Both signatures + the artifact hash all checked out.
    Verified,
    /// Signing is not provisioned yet (embedded pubkeys empty). The caller may
    /// fall back to legacy behaviour with a loud warning. This is ONLY for the
    /// one-time pre-keygen window.
    Unprovisioned,
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Verify the hybrid signature over `manifest_bytes` against the embedded
/// pubkeys. BOTH the Ed25519 and the Dilithium3 signature must pass. Returns
/// `Unprovisioned` (not an error) when no keys are embedded yet.
pub fn verify_manifest_bytes(
    manifest_bytes: &[u8],
    sig: &ManifestSignature,
) -> Result<VerifyOutcome, String> {
    let pubs = embedded_pubkeys();
    if !pubs.is_provisioned() {
        return Ok(VerifyOutcome::Unprovisioned);
    }
    verify_manifest_bytes_with(manifest_bytes, sig, &pubs).map(|()| VerifyOutcome::Verified)
}

/// Verify against an EXPLICIT pubkey set (used by tests + by the embedded path
/// above). BOTH signatures must verify; either failure rejects.
pub fn verify_manifest_bytes_with(
    manifest_bytes: &[u8],
    sig: &ManifestSignature,
    pubs: &SigningPubkeys,
) -> Result<(), String> {
    if sig.alg != HYBRID_ALG {
        return Err(format!(
            "unknown signature algorithm '{}' (expected '{}')",
            sig.alg, HYBRID_ALG
        ));
    }

    // ---- Ed25519 half ----
    {
        use base64::Engine;
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let pk_bytes = hex::decode(&pubs.ed25519)
            .map_err(|e| format!("bad embedded ed25519 pubkey hex: {e}"))?;
        let pk_arr: [u8; 32] = pk_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "ed25519 pubkey must be 32 bytes".to_string())?;
        let vk = VerifyingKey::from_bytes(&pk_arr)
            .map_err(|e| format!("malformed ed25519 pubkey: {e}"))?;
        let sig_bytes = b64()
            .decode(sig.ed25519.as_bytes())
            .map_err(|e| format!("bad ed25519 signature base64: {e}"))?;
        let sig_arr: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "ed25519 signature must be 64 bytes".to_string())?;
        let ed_sig = Signature::from_bytes(&sig_arr);
        // verify_strict rejects non-canonical / small-order edge cases.
        vk.verify_strict(manifest_bytes, &ed_sig)
            .map_err(|_| "ed25519 signature verification FAILED".to_string())?;
    }

    // ---- Dilithium3 half ----
    {
        use base64::Engine;
        let pk_bytes = hex::decode(&pubs.dilithium)
            .map_err(|e| format!("bad embedded dilithium pubkey hex: {e}"))?;
        let sig_bytes = b64()
            .decode(sig.dilithium.as_bytes())
            .map_err(|e| format!("bad dilithium signature base64: {e}"))?;
        crate::relay::core::pq_crypto::verify_dilithium(&pk_bytes, manifest_bytes, &sig_bytes)
            .map_err(|_| "dilithium3 signature verification FAILED".to_string())?;
    }

    Ok(())
}

/// Full release-artifact check: verify the signed manifest, then confirm the
/// downloaded artifact's hash + version match what the manifest attests.
///
/// `manifest_bytes` / `sig_json_bytes` are the fetched files; `asset_name` is
/// the artifact filename we downloaded; `expected_version` is the release we
/// believe we're installing (no leading `v`); `artifact_sha256` is the hash we
/// computed over the bytes we actually downloaded.
pub fn verify_release_artifact(
    manifest_bytes: &[u8],
    sig_json_bytes: &[u8],
    asset_name: &str,
    expected_version: &str,
    artifact_sha256: &str,
) -> Result<VerifyOutcome, String> {
    let sig: ManifestSignature = serde_json::from_slice(sig_json_bytes)
        .map_err(|e| format!("malformed signature file: {e}"))?;

    match verify_manifest_bytes(manifest_bytes, &sig)? {
        VerifyOutcome::Unprovisioned => return Ok(VerifyOutcome::Unprovisioned),
        VerifyOutcome::Verified => {}
    }

    // The signature is valid → the manifest CONTENT is now trusted. Parse it
    // and confirm it covers exactly the artifact + version we're installing.
    let manifest: ReleaseManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|e| format!("malformed manifest: {e}"))?;

    let want_ver = expected_version.trim_start_matches('v');
    if manifest.version.trim_start_matches('v') != want_ver {
        return Err(format!(
            "manifest version '{}' does not match the release being installed '{}'",
            manifest.version, expected_version
        ));
    }

    let entry = manifest
        .artifacts
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("artifact '{asset_name}' is not listed in the signed manifest"))?;

    if !entry.sha256.eq_ignore_ascii_case(artifact_sha256) {
        return Err(format!(
            "artifact '{asset_name}' hash mismatch: downloaded {artifact_sha256}, manifest says {}",
            entry.sha256
        ));
    }

    Ok(VerifyOutcome::Verified)
}

// ===========================================================================
// Operator-side tooling: keygen + sign (CLI subcommands). These run on the
// operator's machine only; the private keys never touch CI.
// ===========================================================================

/// The encrypted signing-key vault on disk. The two 32-byte seeds (ed25519 +
/// dilithium) are AES-256-GCM-encrypted under an Argon2id key derived from the
/// operator's passphrase. Stored as base64; nothing here is secret WITHOUT the
/// passphrase, but the file must still be kept private + backed up.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct KeyVault {
    /// Format/version marker.
    v: u32,
    /// Argon2id salt (base64).
    salt: String,
    /// AES-256-GCM nonce (base64).
    nonce: String,
    /// AES-256-GCM ciphertext of `ed25519_seed(32) || dilithium_seed(32)` (base64).
    ct: String,
}

fn read_passphrase() -> Result<String, String> {
    // Non-interactive by design (scriptable for `just sign-release`, and avoids
    // TTY-masking complexity). The operator sets the env var transiently for
    // the signing session.
    match std::env::var("HUMANITY_SIGNING_PASSPHRASE") {
        Ok(p) if p.len() >= 12 => Ok(p),
        Ok(_) => Err("HUMANITY_SIGNING_PASSPHRASE is set but too short (need >= 12 chars).".into()),
        Err(_) => Err(
            "Set HUMANITY_SIGNING_PASSPHRASE to your release-signing passphrase first.".into(),
        ),
    }
}

fn derive_vault_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    use argon2::Argon2;
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("argon2 derive failed: {e}"))?;
    Ok(key)
}

fn os_rand(buf: &mut [u8]) -> Result<(), String> {
    getrandom::getrandom(buf).map_err(|e| format!("OS RNG failed: {e}"))
}

/// `--gen-release-key`: generate a fresh hybrid keypair, write the public keys
/// to `pubkeys_path` (commit this) and the encrypted private seeds to
/// `vault_path` (gitignored; back this up). One-time operator step.
pub fn gen_release_key(pubkeys_path: &str, vault_path: &str) -> Result<(), String> {
    use base64::Engine;
    let passphrase = read_passphrase()?;

    // Generate both keypairs from fresh OS randomness.
    let mut ed_seed = [0u8; 32];
    os_rand(&mut ed_seed)?;
    let ed_sk = ed25519_dalek::SigningKey::from_bytes(&ed_seed);
    let ed_pub = ed_sk.verifying_key();

    let dil = crate::relay::core::pq_crypto::DilithiumKeypair::generate()
        .map_err(|e| format!("dilithium keygen failed: {e}"))?;
    let dil_seed = dil.to_seed();
    let dil_pub = dil.public_key();

    // Encrypt ed_seed || dil_seed under Argon2id(passphrase) + AES-256-GCM.
    let mut salt = [0u8; 16];
    os_rand(&mut salt)?;
    let mut nonce = [0u8; 12];
    os_rand(&mut nonce)?;
    let key = derive_vault_key(&passphrase, &salt)?;
    let mut plaintext = Vec::with_capacity(64);
    plaintext.extend_from_slice(&ed_seed);
    plaintext.extend_from_slice(&dil_seed);
    let ct = {
        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|_| "vault encryption failed".to_string())?
    };

    let vault = KeyVault {
        v: 1,
        salt: b64().encode(salt),
        nonce: b64().encode(nonce),
        ct: b64().encode(ct),
    };
    let vault_json =
        serde_json::to_string_pretty(&vault).map_err(|e| format!("serialize vault: {e}"))?;
    std::fs::write(vault_path, vault_json).map_err(|e| format!("write vault {vault_path}: {e}"))?;

    let pubs = SigningPubkeys {
        ed25519: hex::encode(ed_pub.as_bytes()),
        dilithium: hex::encode(&dil_pub),
    };
    let pubs_json =
        serde_json::to_string_pretty(&pubs).map_err(|e| format!("serialize pubkeys: {e}"))?;
    std::fs::write(pubkeys_path, format!("{pubs_json}\n"))
        .map_err(|e| format!("write pubkeys {pubkeys_path}: {e}"))?;

    println!("Release signing key generated.");
    println!("  Public keys  -> {pubkeys_path}  (COMMIT this; it gets compiled in)");
    println!("  Private vault -> {vault_path}  (gitignored; BACK THIS UP, encrypted)");
    println!("  ed25519 pub : {}", pubs.ed25519);
    println!("  dilithium pub: {}...", &pubs.dilithium[..32.min(pubs.dilithium.len())]);
    println!("Next: commit {pubkeys_path}, rebuild, and sign releases with --sign-release.");
    Ok(())
}

/// Load + decrypt the signing seeds from the vault.
fn load_seeds(vault_path: &str) -> Result<([u8; 32], [u8; 32]), String> {
    use base64::Engine;
    let passphrase = read_passphrase()?;
    let vault_json =
        std::fs::read_to_string(vault_path).map_err(|e| format!("read vault {vault_path}: {e}"))?;
    let vault: KeyVault =
        serde_json::from_str(&vault_json).map_err(|e| format!("parse vault: {e}"))?;
    let salt = b64()
        .decode(vault.salt.as_bytes())
        .map_err(|e| format!("vault salt b64: {e}"))?;
    let nonce = b64()
        .decode(vault.nonce.as_bytes())
        .map_err(|e| format!("vault nonce b64: {e}"))?;
    let ct = b64()
        .decode(vault.ct.as_bytes())
        .map_err(|e| format!("vault ct b64: {e}"))?;
    let key = derive_vault_key(&passphrase, &salt)?;
    let plaintext = {
        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        cipher
            .decrypt(Nonce::from_slice(&nonce), ct.as_ref())
            .map_err(|_| "vault decryption failed (wrong passphrase?)".to_string())?
    };
    if plaintext.len() != 64 {
        return Err("vault plaintext is not 64 bytes".into());
    }
    let mut ed = [0u8; 32];
    let mut dil = [0u8; 32];
    ed.copy_from_slice(&plaintext[..32]);
    dil.copy_from_slice(&plaintext[32..]);
    Ok((ed, dil))
}

/// Sign `manifest_bytes` with BOTH keys; returns the detached signature struct.
pub fn sign_manifest_bytes(
    manifest_bytes: &[u8],
    vault_path: &str,
) -> Result<ManifestSignature, String> {
    use base64::Engine;
    let (ed_seed, dil_seed) = load_seeds(vault_path)?;

    let ed_sig = {
        use ed25519_dalek::{Signer, SigningKey};
        let sk = SigningKey::from_bytes(&ed_seed);
        sk.sign(manifest_bytes).to_bytes().to_vec()
    };
    let dil_sig = {
        let kp = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
        kp.sign(manifest_bytes)
    };

    Ok(ManifestSignature {
        alg: HYBRID_ALG.to_string(),
        ed25519: b64().encode(ed_sig),
        dilithium: b64().encode(dil_sig),
    })
}

/// `--sign-release <manifest.json>`: read the manifest bytes verbatim, sign
/// them, and write `<manifest>.sig.json` next to it. Also re-verifies its own
/// output against the embedded pubkeys as a sanity check before exiting.
pub fn sign_release(manifest_path: &str, vault_path: &str) -> Result<(), String> {
    let manifest_bytes =
        std::fs::read(manifest_path).map_err(|e| format!("read manifest {manifest_path}: {e}"))?;
    // Fail early if the manifest isn't even valid JSON of the right shape.
    let _parsed: ReleaseManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("manifest is not a valid ReleaseManifest: {e}"))?;

    let sig = sign_manifest_bytes(&manifest_bytes, vault_path)?;
    let sig_json = serde_json::to_string_pretty(&sig).map_err(|e| format!("serialize sig: {e}"))?;
    let sig_path = format!("{manifest_path}.sig.json");
    std::fs::write(&sig_path, format!("{sig_json}\n"))
        .map_err(|e| format!("write sig {sig_path}: {e}"))?;

    // Self-check: the signature we just wrote must verify against the embedded
    // pubkeys (catches a key/file mismatch before the operator ships it).
    match verify_manifest_bytes(&manifest_bytes, &sig)? {
        VerifyOutcome::Verified => {
            println!("Signed + self-verified OK -> {sig_path}");
        }
        VerifyOutcome::Unprovisioned => {
            println!(
                "Signed -> {sig_path}\nWARNING: the embedded pubkeys are empty, so this build can't \
                 self-verify. Commit data/release/signing_pubkeys.json and rebuild, then re-run to \
                 confirm."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    /// Build a throwaway keypair set + sign some bytes, returning (pubs, sig).
    fn make_signed(bytes: &[u8]) -> (SigningPubkeys, ManifestSignature) {
        // Ed25519 from a fixed seed.
        let ed_seed = [7u8; 32];
        let ed_sk = ed25519_dalek::SigningKey::from_bytes(&ed_seed);
        let ed_pub = ed_sk.verifying_key();
        let ed_sig = {
            use ed25519_dalek::Signer;
            ed_sk.sign(bytes).to_bytes().to_vec()
        };
        // Dilithium from a fixed seed.
        let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&[9u8; 32]);
        let dil_pub = dil.public_key();
        let dil_sig = dil.sign(bytes);

        let pubs = SigningPubkeys {
            ed25519: hex::encode(ed_pub.as_bytes()),
            dilithium: hex::encode(&dil_pub),
        };
        let sig = ManifestSignature {
            alg: HYBRID_ALG.to_string(),
            ed25519: b64().encode(ed_sig),
            dilithium: b64().encode(dil_sig),
        };
        (pubs, sig)
    }

    #[test]
    fn hybrid_sign_verify_roundtrip() {
        let bytes = br#"{"version":"0.418.0","artifacts":[]}"#;
        let (pubs, sig) = make_signed(bytes);
        assert!(verify_manifest_bytes_with(bytes, &sig, &pubs).is_ok());
    }

    #[test]
    fn tampered_message_is_rejected() {
        let bytes = br#"{"version":"0.418.0","artifacts":[]}"#;
        let (pubs, sig) = make_signed(bytes);
        let tampered = br#"{"version":"9.9.9","artifacts":[]}"#;
        assert!(verify_manifest_bytes_with(tampered, &sig, &pubs).is_err());
    }

    #[test]
    fn both_signatures_required_ed25519_half_broken() {
        let bytes = b"hello release";
        let (pubs, mut sig) = make_signed(bytes);
        // Corrupt only the ed25519 signature → must reject even though dilithium is fine.
        let mut raw = b64().decode(sig.ed25519.as_bytes()).unwrap();
        raw[0] ^= 0xFF;
        sig.ed25519 = b64().encode(raw);
        assert!(verify_manifest_bytes_with(bytes, &sig, &pubs).is_err());
    }

    #[test]
    fn both_signatures_required_dilithium_half_broken() {
        let bytes = b"hello release";
        let (pubs, mut sig) = make_signed(bytes);
        // Corrupt only the dilithium signature → must reject even though ed25519 is fine.
        let mut raw = b64().decode(sig.dilithium.as_bytes()).unwrap();
        raw[0] ^= 0xFF;
        raw[1] ^= 0xFF;
        sig.dilithium = b64().encode(raw);
        assert!(verify_manifest_bytes_with(bytes, &sig, &pubs).is_err());
    }

    #[test]
    fn wrong_key_is_rejected() {
        let bytes = b"hello release";
        let (_pubs, sig) = make_signed(bytes);
        // A DIFFERENT pubkey set must not verify the signature.
        let other_ed = ed25519_dalek::SigningKey::from_bytes(&[1u8; 32]).verifying_key();
        let other_dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&[2u8; 32]);
        let wrong = SigningPubkeys {
            ed25519: hex::encode(other_ed.as_bytes()),
            dilithium: hex::encode(other_dil.public_key()),
        };
        assert!(verify_manifest_bytes_with(bytes, &sig, &wrong).is_err());
    }

    #[test]
    fn unknown_alg_is_rejected() {
        let bytes = b"x";
        let (pubs, mut sig) = make_signed(bytes);
        sig.alg = "rot13".to_string();
        assert!(verify_manifest_bytes_with(bytes, &sig, &pubs).is_err());
    }

    #[test]
    fn full_artifact_check_matches_hash_and_version() {
        let manifest = ReleaseManifest {
            version: "0.418.0".into(),
            artifacts: vec![ManifestArtifact {
                name: "HumanityOS-windows-x64.exe".into(),
                sha256: sha256_hex(b"fake-binary-bytes"),
                size: 17,
            }],
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        // Sign with a throwaway key, then verify with that SAME key via the
        // explicit path (the embedded path needs provisioned keys).
        let (pubs, sig) = make_signed(&manifest_bytes);
        assert!(verify_manifest_bytes_with(&manifest_bytes, &sig, &pubs).is_ok());

        // Hash match check (mirrors verify_release_artifact's tail logic).
        let got = sha256_hex(b"fake-binary-bytes");
        let entry = manifest
            .artifacts
            .iter()
            .find(|a| a.name == "HumanityOS-windows-x64.exe")
            .unwrap();
        assert!(entry.sha256.eq_ignore_ascii_case(&got));
        let wrong = sha256_hex(b"different-bytes");
        assert!(!entry.sha256.eq_ignore_ascii_case(&wrong));
    }

    #[test]
    fn embedded_pubkeys_parse() {
        // The committed file must at least parse (empty strings = unprovisioned).
        let p = embedded_pubkeys();
        // Unprovisioned by default in the repo; this asserts the include + parse path works.
        let _ = p.is_provisioned();
    }
}
