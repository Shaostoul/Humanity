//! The release check's tests. Most were src/release_update.rs's (the desktop updater's), moved here
//! with the verification on 2026-11-02 and unchanged; the ones over the REAL signed pairs of
//! v0.1547.0 and v0.1543.0 are new: they are the only proof that the keys compiled into every
//! build are the ones the operator signs with.

use super::*;
use base64::Engine;

/// The real pair of one release, as `just sign-release` uploaded it to GitHub (read 2026-11-02
/// into tests/fixtures/release/, kept byte for byte by .gitattributes: a line-ending rewrite would
/// change the signed bytes).
fn real_pair(version: &str) -> (Vec<u8>, Vec<u8>) {
    let dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/release").join(version);
    let manifest = std::fs::read(dir.join("release-manifest.json")).expect("the manifest fixture");
    let sig = std::fs::read(dir.join("release-manifest.json.sig.json")).expect("the signature fixture");
    (manifest, sig)
}

/// Build a throwaway keypair set and sign some bytes, returning (pubs, sig).
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
    let dil = crate::pq_crypto::DilithiumKeypair::from_seed(&[9u8; 32]);
    let dil_pub = dil.public_key();
    let dil_sig = dil.sign(bytes);

    let pubs = SigningPubkeys { ed25519: hex::encode(ed_pub.as_bytes()), dilithium: hex::encode(&dil_pub) };
    let sig = ManifestSignature { alg: HYBRID_ALG.to_string(), ed25519: b64().encode(ed_sig), dilithium: b64().encode(dil_sig) };
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
    // Corrupt only the ed25519 signature: must reject even though dilithium is fine.
    let mut raw = b64().decode(sig.ed25519.as_bytes()).unwrap();
    raw[0] ^= 0xFF;
    sig.ed25519 = b64().encode(raw);
    assert!(verify_manifest_bytes_with(bytes, &sig, &pubs).is_err());
}

#[test]
fn both_signatures_required_dilithium_half_broken() {
    let bytes = b"hello release";
    let (pubs, mut sig) = make_signed(bytes);
    // Corrupt only the dilithium signature: must reject even though ed25519 is fine.
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
    let other_dil = crate::pq_crypto::DilithiumKeypair::from_seed(&[2u8; 32]);
    let wrong = SigningPubkeys { ed25519: hex::encode(other_ed.as_bytes()), dilithium: hex::encode(other_dil.public_key()) };
    assert!(verify_manifest_bytes_with(bytes, &sig, &wrong).is_err());
}

#[test]
fn unknown_alg_is_rejected() {
    let bytes = b"x";
    let (pubs, mut sig) = make_signed(bytes);
    sig.alg = "rot13".to_string();
    assert!(verify_manifest_bytes_with(bytes, &sig, &pubs).is_err());
}

/// THE UPDATER'S WHOLE CHECK: the right artifact and version pass; a binary whose hash is not the
/// manifest's, an artifact the manifest does not list, another version, or a tampered manifest are
/// each refused. (It was a hand-copy of the check's tail in release_update.rs; it calls the real
/// function now that it takes a key set.)
#[test]
fn full_artifact_check_matches_hash_and_version() {
    let manifest = ReleaseManifest {
        version: "0.418.0".into(),
        artifacts: vec![ManifestArtifact { name: "HumanityOS-windows-x64.exe".into(), sha256: sha256_hex(b"fake-binary-bytes"), size: 17 }],
    };
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let (pubs, sig) = make_signed(&manifest_bytes);
    let sig_json = serde_json::to_vec(&sig).unwrap();
    let got = sha256_hex(b"fake-binary-bytes");
    let check = |m: &[u8], name: &str, ver: &str, hash: &str| verify_release_artifact_with(m, &sig_json, name, ver, hash, &pubs);

    assert!(check(&manifest_bytes, "HumanityOS-windows-x64.exe", "0.418.0", &got).is_ok());
    assert!(check(&manifest_bytes, "HumanityOS-windows-x64.exe", "v0.418.0", &got).is_ok(), "a leading v is the same version");
    assert!(check(&manifest_bytes, "HumanityOS-windows-x64.exe", "0.418.0", &sha256_hex(b"different-bytes")).is_err(), "a tampered binary");
    assert!(check(&manifest_bytes, "HumanityOS-linux-x64", "0.418.0", &got).is_err(), "an artifact the manifest does not list");
    assert!(check(&manifest_bytes, "HumanityOS-windows-x64.exe", "0.419.0", &got).is_err(), "another version");
    let mut tampered = manifest_bytes.clone();
    *tampered.last_mut().unwrap() = b' ';
    assert!(check(&tampered, "HumanityOS-windows-x64.exe", "0.418.0", &got).is_err(), "a manifest changed after signing");
}

#[test]
fn embedded_pubkeys_parse_and_are_provisioned() {
    // The committed file parses, and since the operator ran --gen-release-key it holds both keys:
    // an empty file here would make every updater install unchecked and the server mark nothing.
    let p = embedded_pubkeys();
    assert!(p.is_provisioned(), "data/release/signing_pubkeys.json must hold both release keys");
    assert_eq!(p.ed25519.len(), 64, "a 32-byte Ed25519 key in hex");
}

#[test]
fn file_hash_signature_roundtrip() {
    // The chain the launcher's sidecar check uses (file bytes -> sha256 hex -> hybrid sign ->
    // verify), with explicit keys.
    let file_bytes = b"a pretend HumanityOS.exe payload";
    let hash = sha256_hex(file_bytes);
    let (pubs, sig) = make_signed(hash.as_bytes());
    assert!(verify_manifest_bytes_with(hash.as_bytes(), &sig, &pubs).is_ok());
    // A different file (different hash) must not verify against this sig.
    let other_hash = sha256_hex(b"a tampered payload");
    assert!(verify_manifest_bytes_with(other_hash.as_bytes(), &sig, &pubs).is_err());
}

// ── The real signed releases, against the keys compiled into every build ──

/// A REAL PAIR VERIFIES AGAINST THE COMPILED-IN KEYS, for the updater and for the server alike:
/// v0.1547.0 and v0.1543.0 as the operator signed them. If these fail, the keys in
/// data/release/signing_pubkeys.json are not the ones he signs with, and every desktop app would
/// refuse every update.
///
/// Seen red 2026-11-02 with the first byte of the Ed25519 key in signing_pubkeys.json changed (2f to
/// 3f): "v0.1547.0 is the operator's: Err(\"malformed ed25519 pubkey: signature error: Cannot
/// decompress Edwards point\")".
#[test]
fn the_real_signed_releases_verify_against_the_compiled_in_keys() {
    for version in ["v0.1547.0", "v0.1543.0"] {
        let (manifest, sig) = real_pair(version);
        let got = check_signed_release(&manifest, &sig);
        assert!(got.is_ok(), "{version} is the operator's: {got:?}");
        let got = got.unwrap();
        assert_eq!(format!("v{}", got.version), version);
        assert_eq!(got.manifest_sha256, sha256_hex(&manifest));
        let windows = serde_json::from_slice::<ReleaseManifest>(&manifest).unwrap().artifacts[0].sha256.clone();
        assert!(
            matches!(verify_release_artifact(&manifest, &sig, "HumanityOS-windows-x64.exe", version, &windows), Ok(VerifyOutcome::Verified)),
            "the updater accepts {version}'s Windows binary by its signed hash"
        );
    }
}

/// THE SERVER REFUSES WHAT THE PROJECT'S KEYS DID NOT MAKE: a real manifest with one byte changed,
/// one release's signature over another's manifest, a pair signed by another key, and either half
/// of a real signature broken. Each is an error, so the server keeps nothing.
///
/// Seen red 2026-11-02 with the signature check taken out of check_signed_release_with (it parsed the
/// manifest and returned): "a tampered manifest was accepted".
#[test]
fn the_server_refuses_a_tampered_or_foreign_pair() {
    let (manifest, sig) = real_pair("v0.1547.0");
    let (other_manifest, _) = real_pair("v0.1543.0");

    let mut tampered = manifest.clone();
    let at = tampered.windows(8).position(|w| w == b"0.1547.0").unwrap();
    tampered[at + 7] = b'1';
    assert!(check_signed_release(&tampered, &sig).is_err(), "a tampered manifest was accepted");
    assert!(check_signed_release(&other_manifest, &sig).is_err(), "one release's signature over another's manifest");

    // A pair made by a key that is not the project's: valid in itself, refused here.
    let (_, foreign) = make_signed(&manifest);
    let foreign_json = serde_json::to_vec(&foreign).unwrap();
    assert!(check_signed_release(&manifest, &foreign_json).is_err(), "a signature from another key");

    // Either half broken alone.
    let real: ManifestSignature = serde_json::from_slice(&sig).unwrap();
    for half in ["ed25519", "dilithium"] {
        let mut broken = real.clone();
        let field = if half == "ed25519" { &mut broken.ed25519 } else { &mut broken.dilithium };
        let mut raw = b64().decode(field.as_bytes()).unwrap();
        raw[3] ^= 0x01;
        *field = b64().encode(raw);
        assert!(check_signed_release(&manifest, &serde_json::to_vec(&broken).unwrap()).is_err(), "the {half} half broken");
    }
    assert!(check_signed_release(&manifest, b"not json").is_err());
    assert!(check_signed_release(b"{}", &sig).is_err());
}

/// THE SERVER ONLY KEEPS A PLAIN VERSION, AND A BUILD WITHOUT KEYS MARKS NOTHING: a version made
/// of anything but digits and dots is refused even when the signature is good (it goes into a page
/// and a URL), a body past the limits is refused before it is parsed, and an empty key set refuses
/// a real pair.
#[test]
fn only_plain_versions_and_never_without_keys() {
    for good in ["0.1547.0", "1.0", "0.4.2", "10.20.30.40"] {
        assert!(is_plain_version(good), "{good}");
    }
    let long = "9.".repeat(20) + "9";
    for bad in ["", "1", ".1", "1.", "1..2", "0.1547.0-rc1", "0.1547.0/../x", "v0.1", "1.0 ", "1.0<b>", long.as_str()] {
        assert!(!is_plain_version(bad), "{bad:?}");
    }
    let odd = br#"{"version":"0.1.0<script>","artifacts":[]}"#;
    let (pubs, sig) = make_signed(odd);
    let sig_json = serde_json::to_vec(&sig).unwrap();
    assert!(check_signed_release_with(odd, &sig_json, &pubs).is_err(), "a well-signed odd version");
    let plain = br#"{"version":"v0.2.0","artifacts":[]}"#;
    let (pubs, sig) = make_signed(plain);
    let sig_json = serde_json::to_vec(&sig).unwrap();
    assert_eq!(check_signed_release_with(plain, &sig_json, &pubs).unwrap().version, "0.2.0", "a leading v is dropped");

    let big = vec![b' '; MANIFEST_MAX_BYTES + 1];
    assert!(check_signed_release_with(&big, &sig_json, &pubs).unwrap_err().contains("more than"));
    let (manifest, real_sig) = real_pair("v0.1547.0");
    assert!(check_signed_release_with(&manifest, &real_sig, &SigningPubkeys::default()).is_err(), "no keys, nothing signed");
}
