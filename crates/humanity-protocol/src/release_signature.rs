//! Checking that the project's release keys made a release (the supply-chain root of trust).
//!
//! WHY IT LIVES HERE (2026-11-02, docs/design/front-door.md section 3.1): the desktop updater
//! checked a release's signatures before it installed it, and that check was native-only, in the
//! GUI's `release_update`. The website's front door marks a signed release "Signed", and the
//! server learns which releases are signed by checking the signatures ITSELF (`POST
//! /api/release-signature`, src/relay/front_door.rs): `just sign-release` sends it the two files,
//! and the server keeps them only when both signatures verify against the same public keys the
//! desktop app has compiled in. So the route needs no secret, and a forged "Signed" mark is
//! impossible rather than merely unlikely. The pure verification moved down to this crate, which
//! the server and the app both link; the download, the install and the signing commands stay in
//! the app (`src/release_update.rs`, `src/updater.rs`).
//!
//! THE SCHEME (operator-chosen, security audit 2026-06-12): **hybrid Ed25519 + Dilithium3
//! (ML-DSA-65), BOTH must verify** over the VERBATIM bytes of `release-manifest.json` (the version
//! and every platform binary's SHA-256), so there is no canonicalisation to get wrong: signer and
//! checker read the identical bytes. The public keys are `data/release/signing_pubkeys.json`,
//! compiled into every build; the signing key is a dedicated key, passphrase-locked on the
//! operator's machine, never in CI. docs/reference/cryptography.md, "Release signing", is the
//! canonical row.
//!
//! What is here: the file shapes, the compiled-in keys, [`verify_manifest_bytes`] (the two
//! signatures), [`verify_release_artifact`] (the updater's whole check: signatures, version, the
//! downloaded binary's hash) and [`check_signed_release`] (the server's: signatures and a version
//! it can show). Nothing here touches a file, the network or a clock.

use serde::{Deserialize, Serialize};

/// Algorithm tag carried in every signature file. Bump the suffix if the signed-bytes definition
/// or the key set ever changes.
pub const HYBRID_ALG: &str = "hybrid-ed25519-mldsa65-v1";

/// The project's release public keys, compiled in from the committed file. Empty strings mean
/// "signing not provisioned yet" (before `--gen-release-key`): the updater then installs with a
/// loud warning, and the server marks nothing signed.
const EMBEDDED_PUBKEYS_JSON: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/release/signing_pubkeys.json"));

/// The most bytes a release manifest may be before the server will even parse it. The real ones
/// are under 1 KB (four platform binaries, 2026-11-02); 64 KB leaves room for many more platforms.
pub const MANIFEST_MAX_BYTES: usize = 64 * 1024;

/// The most bytes a signature file may be: the real ones are 4.6 KB (a 3,309-byte Dilithium3
/// signature and a 64-byte Ed25519 one, both in base64).
pub const SIGNATURE_MAX_BYTES: usize = 16 * 1024;

/// The longest version the server will keep, in characters ("0.1547.0" is 8).
pub const VERSION_MAX_CHARS: usize = 32;

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

/// The project's release public keys (parsed from the compiled-in JSON).
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

/// A release manifest: the version and every platform artifact's hash. The VERBATIM bytes of this
/// file are what get signed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReleaseManifest {
    /// Release version, e.g. "0.418.0" (no leading `v`).
    pub version: String,
    pub artifacts: Vec<ManifestArtifact>,
}

/// The detached signature file accompanying a manifest. Both signatures are over the EXACT
/// manifest bytes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestSignature {
    pub alg: String,
    /// Ed25519 signature (64 bytes) over the manifest bytes, base64.
    pub ed25519: String,
    /// Dilithium3 signature (3309 bytes) over the manifest bytes, base64.
    pub dilithium: String,
}

/// Outcome of verifying against the compiled-in keys.
#[derive(Debug)]
pub enum VerifyOutcome {
    /// Both signatures (and, for an artifact, its hash) checked out.
    Verified,
    /// Signing is not provisioned yet (the compiled-in keys are empty). The updater may fall back
    /// to installing with a loud warning; this is ONLY for the one-time pre-keygen window.
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

/// Verify the hybrid signature over `manifest_bytes` against the compiled-in keys. BOTH the
/// Ed25519 and the Dilithium3 signature must pass. Returns `Unprovisioned` (not an error) when no
/// keys are compiled in yet.
pub fn verify_manifest_bytes(manifest_bytes: &[u8], sig: &ManifestSignature) -> Result<VerifyOutcome, String> {
    let pubs = embedded_pubkeys();
    if !pubs.is_provisioned() {
        return Ok(VerifyOutcome::Unprovisioned);
    }
    verify_manifest_bytes_with(manifest_bytes, sig, &pubs).map(|()| VerifyOutcome::Verified)
}

/// Verify against an EXPLICIT key set (the tests, and the compiled-in path above). BOTH signatures
/// must verify; either failure rejects.
pub fn verify_manifest_bytes_with(manifest_bytes: &[u8], sig: &ManifestSignature, pubs: &SigningPubkeys) -> Result<(), String> {
    if sig.alg != HYBRID_ALG {
        return Err(format!("unknown signature algorithm '{}' (expected '{}')", sig.alg, HYBRID_ALG));
    }

    // ---- Ed25519 half ----
    {
        use base64::Engine;
        use ed25519_dalek::{Signature, VerifyingKey};
        let pk_bytes = hex::decode(&pubs.ed25519).map_err(|e| format!("bad embedded ed25519 pubkey hex: {e}"))?;
        let pk_arr: [u8; 32] = pk_bytes.as_slice().try_into().map_err(|_| "ed25519 pubkey must be 32 bytes".to_string())?;
        let vk = VerifyingKey::from_bytes(&pk_arr).map_err(|e| format!("malformed ed25519 pubkey: {e}"))?;
        let sig_bytes = b64().decode(sig.ed25519.as_bytes()).map_err(|e| format!("bad ed25519 signature base64: {e}"))?;
        let sig_arr: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| "ed25519 signature must be 64 bytes".to_string())?;
        let ed_sig = Signature::from_bytes(&sig_arr);
        // verify_strict rejects non-canonical / small-order edge cases.
        vk.verify_strict(manifest_bytes, &ed_sig).map_err(|_| "ed25519 signature verification FAILED".to_string())?;
    }

    // ---- Dilithium3 half ----
    {
        use base64::Engine;
        let pk_bytes = hex::decode(&pubs.dilithium).map_err(|e| format!("bad embedded dilithium pubkey hex: {e}"))?;
        let sig_bytes = b64().decode(sig.dilithium.as_bytes()).map_err(|e| format!("bad dilithium signature base64: {e}"))?;
        crate::pq_crypto::verify_dilithium(&pk_bytes, manifest_bytes, &sig_bytes)
            .map_err(|_| "dilithium3 signature verification FAILED".to_string())?;
    }

    Ok(())
}

/// The updater's full check: verify the signed manifest against the compiled-in keys, then
/// confirm the downloaded artifact's hash and version match what the manifest attests.
///
/// `manifest_bytes` / `sig_json_bytes` are the fetched files; `asset_name` is the artifact filename
/// downloaded; `expected_version` is the release being installed (no leading `v`);
/// `artifact_sha256` is the hash computed over the bytes actually downloaded.
pub fn verify_release_artifact(
    manifest_bytes: &[u8],
    sig_json_bytes: &[u8],
    asset_name: &str,
    expected_version: &str,
    artifact_sha256: &str,
) -> Result<VerifyOutcome, String> {
    let pubs = embedded_pubkeys();
    if !pubs.is_provisioned() {
        return Ok(VerifyOutcome::Unprovisioned);
    }
    verify_release_artifact_with(manifest_bytes, sig_json_bytes, asset_name, expected_version, artifact_sha256, &pubs)
        .map(|()| VerifyOutcome::Verified)
}

/// [`verify_release_artifact`] against an EXPLICIT key set, so its tests can sign with a key of
/// their own.
pub fn verify_release_artifact_with(
    manifest_bytes: &[u8],
    sig_json_bytes: &[u8],
    asset_name: &str,
    expected_version: &str,
    artifact_sha256: &str,
    pubs: &SigningPubkeys,
) -> Result<(), String> {
    let sig: ManifestSignature = serde_json::from_slice(sig_json_bytes).map_err(|e| format!("malformed signature file: {e}"))?;
    verify_manifest_bytes_with(manifest_bytes, &sig, pubs)?;

    // The signature is valid, so the manifest CONTENT is now trusted. Parse it and confirm it
    // covers exactly the artifact and version being installed.
    let manifest: ReleaseManifest = serde_json::from_slice(manifest_bytes).map_err(|e| format!("malformed manifest: {e}"))?;

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
    Ok(())
}

/// What the server keeps of a release whose signatures it checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRelease {
    /// The manifest's version without a leading `v`: digits and dots only, so it can be matched
    /// against an announcement and put into a page and a URL as it is.
    pub version: String,
    /// Lowercase hex SHA-256 of the manifest's exact bytes.
    pub manifest_sha256: String,
}

/// Whether `v` is a version the server may show and match: one to [`VERSION_MAX_CHARS`]
/// characters of digits and dots, at least two numbers ("0.1547.0", "1.0"), no empty part.
pub fn is_plain_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= VERSION_MAX_CHARS
        && v.contains('.')
        && v.split('.').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The server's check of a release someone sent it (`POST /api/release-signature`): both
/// signatures over the manifest's exact bytes verify against the compiled-in keys, the manifest
/// parses, and its version is plain. Anyone may send a pair; only the project's keys make one that
/// passes. A build with no keys compiled in refuses every pair (it cannot tell).
pub fn check_signed_release(manifest_bytes: &[u8], sig_json_bytes: &[u8]) -> Result<SignedRelease, String> {
    check_signed_release_with(manifest_bytes, sig_json_bytes, &embedded_pubkeys())
}

/// [`check_signed_release`] against an EXPLICIT key set (its tests).
pub fn check_signed_release_with(manifest_bytes: &[u8], sig_json_bytes: &[u8], pubs: &SigningPubkeys) -> Result<SignedRelease, String> {
    if !pubs.is_provisioned() {
        return Err("this build has no release keys compiled in, so it cannot check a release's signatures".into());
    }
    if manifest_bytes.len() > MANIFEST_MAX_BYTES {
        return Err(format!("the manifest is {} bytes, more than the {MANIFEST_MAX_BYTES} allowed", manifest_bytes.len()));
    }
    if sig_json_bytes.len() > SIGNATURE_MAX_BYTES {
        return Err(format!("the signature file is {} bytes, more than the {SIGNATURE_MAX_BYTES} allowed", sig_json_bytes.len()));
    }
    let sig: ManifestSignature = serde_json::from_slice(sig_json_bytes).map_err(|e| format!("malformed signature file: {e}"))?;
    verify_manifest_bytes_with(manifest_bytes, &sig, pubs)?;
    let manifest: ReleaseManifest = serde_json::from_slice(manifest_bytes).map_err(|e| format!("malformed manifest: {e}"))?;
    let version = manifest.version.trim_start_matches('v').to_string();
    if !is_plain_version(&version) {
        return Err(format!("the manifest's version '{}' is not plain digits and dots", manifest.version));
    }
    Ok(SignedRelease { version, manifest_sha256: sha256_hex(manifest_bytes) })
}

#[cfg(test)]
#[path = "release_signature_tests.rs"]
mod tests;
