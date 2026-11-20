//! CROSS-IMPLEMENTATION TESTS, BOTH WAYS (2026-11-19, the post-quantum review's plan item 1b,
//! docs/reference/research/2026-11-16-pqc-review/).
//!
//! The known-answer tests above only prove the two implementations agree on KEY GENERATION from
//! one fixed seed. These prove they agree on what the keys are used for, each side checking what
//! the OTHER side made:
//!
//! - ML-KEM-768: noble (the website's vendored bundle) encapsulated to the key this crate derives,
//!   and this crate decapsulates its ciphertext to the same shared secret; and the reverse, where
//!   this crate encapsulated and noble decapsulates (scripts/tests/pq-cross.test.js). Both also
//!   encapsulate from the same fixed randomness and must write the same ciphertext byte for byte
//!   (FIPS 203 ML-KEM.Encaps_internal), which holds the encapsulation itself to one answer, not
//!   only to "the other side can open it".
//! - ML-DSA-65: noble's hedged signature (what the website makes) verifies here; this crate's
//!   signature verifies in noble; and the deterministic variant (FIPS 204, rnd all zeros, empty
//!   context) is the same bytes on both sides.
//!
//! The shared fixture is scripts/tests/fixtures/pq-cross-v1.json. Its `rust` half is written by
//! `write_the_rust_half` below, its `noble` half by scripts/pq-cross-fixtures.mjs, which
//! encapsulates to the key in the `rust` half (so write the Rust half first):
//!
//!   node scripts/lib-tests.js --crate humanity-protocol write_the_rust_half --ignored
//!   node scripts/pq-cross-fixtures.mjs
//!
//! Regenerate only on purpose (a library upgrade is exactly when NOT to: the old fixture is what
//! the new version must still match). These tests pin today's behaviour; they change nothing.

use super::*;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};

/// The shared fixture, from the repository root.
const FIXTURE: &str = "scripts/tests/fixtures/pq-cross-v1.json";

/// The master seeds are `[byte; 32]`, the recovery phrase's 32 bytes of entropy in a real
/// identity. 7 is the known-answer tests' seed, so the keys here are the ones they pin.
const KEM_MASTER_BYTE: u8 = 7;
const DSA_MASTER_BYTE: u8 = 7;
/// The fixed randomness `m` of the deterministic encapsulation (32 bytes).
const KEM_M: [u8; 32] = [0x42; 32];
/// The message both sides sign.
const DSA_MESSAGE: &str = "hum/pq-cross/v1\nsigned on one side, verified on the other";

/// The fixture, or its copy in the folder HOS_PQ_CROSS_DIR names (how a changed copy is run to
/// see these tests red; scripts/tests/pq-cross.test.js reads the same variable).
fn fixture() -> Value {
    let path = match std::env::var_os("HOS_PQ_CROSS_DIR") {
        Some(dir) => std::path::PathBuf::from(dir).join("pq-cross-v1.json"),
        None => std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(FIXTURE),
    };
    serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("the fixture is JSON")
}

fn b64(v: &Value) -> Vec<u8> {
    B64.decode(v.as_str().expect("a base64 string")).expect("valid base64")
}

fn unhex(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().expect("a hex string")).expect("valid hex")
}

fn kem_keypair() -> KyberKeypair {
    KyberKeypair::from_seed(&derive_kyber_seed(&[KEM_MASTER_BYTE; 32])).expect("the keypair")
}

fn dsa_keypair() -> DilithiumKeypair {
    DilithiumKeypair::from_seed(&derive_dilithium_seed(&[DSA_MASTER_BYTE; 32]))
}

/// ML-KEM-768 encapsulation from fixed randomness `m` (FIPS 203 ML-KEM.Encaps_internal), to the
/// encapsulation key's BYTES, parsed the way `encapsulate_to` parses them. `encapsulate_deterministic`
/// is public in ml-kem 0.3.3 (documented only under its `hazmat` feature, which is not turned on):
/// used here, in tests, and nowhere else.
fn encapsulate_from(ek_bytes: &[u8], m: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    let key_arr: Key<EncapsulationKey<MlKem768>> = Array::try_from(ek_bytes).expect("a 1184-byte key");
    let ek = EncapsulationKey::<MlKem768>::new(&key_arr).expect("a valid key");
    let (ct, ss) = ek.encapsulate_deterministic(&Array::from(*m));
    (ct.as_slice().to_vec(), ss.as_slice().to_vec())
}

/// The fixture's `rust` half, as this code makes it now. The encapsulation through
/// `encapsulate_to` is randomized (the apps' own path), so each run writes a different
/// ciphertext; the rest is deterministic.
fn rust_half_now() -> Value {
    let kp = kem_keypair();
    let ek = kp.public_key();
    let (ct, ss) = encapsulate_to(&ek).expect("encapsulate");
    let (dct, dss) = encapsulate_from(&ek, &KEM_M);
    let sig = dsa_keypair().sign(DSA_MESSAGE.as_bytes());
    json!({
        "written_by": "crates/humanity-protocol/src/pq_cross_tests.rs write_the_rust_half (RustCrypto ml-kem 0.3.3, ml-dsa 0.1.1)",
        "kem": {
            "recipient_master_byte": KEM_MASTER_BYTE,
            "ek_b64": B64.encode(&ek),
            "ct_b64": B64.encode(&ct),
            "ss_hex": hex::encode(ss),
            "deterministic": {
                "m_hex": hex::encode(KEM_M),
                "ct_blake3": blake3::hash(&dct).to_hex().to_string(),
                "ss_hex": hex::encode(&dss),
            },
        },
        "dsa": {
            "signer_master_byte": DSA_MASTER_BYTE,
            "message": DSA_MESSAGE,
            "sig_b64": B64.encode(&sig),
        },
    })
}

/// WRITES the `rust` half of scripts/tests/fixtures/pq-cross-v1.json, keeping the `noble` half
/// as it is. Run it only on purpose (see the module's notes), then scripts/pq-cross-fixtures.mjs.
#[test]
#[ignore = "writes the rust half of scripts/tests/fixtures/pq-cross-v1.json; run: node scripts/lib-tests.js --crate humanity-protocol write_the_rust_half --ignored"]
fn write_the_rust_half() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(FIXTURE);
    let mut fx: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}));
    fx["about"] = json!(
        "Post-quantum cross-implementation fixture: each half is made by one implementation and checked by the other. \
         rust: crates/humanity-protocol/src/pq_cross_tests.rs; noble: scripts/pq-cross-fixtures.mjs. \
         Checked by those Rust tests and scripts/tests/pq-cross.test.js."
    );
    fx["rust"] = rust_half_now();
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&fx).unwrap())).expect("the fixture is written");
}

/// NOBLE ENCAPSULATED TO OUR KEY, AND WE DECAPSULATE ITS CIPHERTEXT TO ITS SHARED SECRET: the
/// direction every website-to-desktop DM takes. The ciphertext is noble's randomized
/// encapsulation (pq.js `pqDmSeal`'s own call), to the encapsulation key in the fixture's `rust`
/// half, which is this crate's derivation from the master seed.
///
/// Seen red 2026-11-19 with one byte of `noble.kem.ct_b64` flipped: "decapsulating noble's
/// ciphertext gives noble's shared secret" failed (ML-KEM's implicit rejection returns a different
/// secret instead of an error, so the comparison is the check).
#[test]
fn noble_encapsulates_and_rust_decapsulates() {
    let fx = fixture();
    let kp = kem_keypair();
    let noble = &fx["noble"]["kem"];
    assert_eq!(
        noble["ek_blake3"].as_str(),
        Some(blake3::hash(&kp.public_key()).to_hex().as_str()),
        "noble encapsulated to the key this crate derives"
    );
    let ct = b64(&noble["ct_b64"]);
    assert_eq!(ct.len(), KYBER_CIPHERTEXT_LEN, "an ML-KEM-768 ciphertext");
    let ss = kp.decapsulate(&ct).expect("decapsulate");
    assert_eq!(ss.to_vec(), unhex(&noble["ss_hex"]), "decapsulating noble's ciphertext gives noble's shared secret");
}

/// FROM THE SAME RANDOMNESS, NOBLE AND THIS CRATE WRITE THE SAME CIPHERTEXT AND SECRET: the
/// encapsulation itself agrees (FIPS 203 ML-KEM.Encaps_internal), byte for byte.
///
/// Seen red 2026-11-19 with one hex digit of `noble.kem.deterministic.ss_hex` changed: "the same
/// shared secret" failed.
#[test]
fn the_same_randomness_encapsulates_to_the_same_bytes_on_both_sides() {
    let fx = fixture();
    let d = &fx["noble"]["kem"]["deterministic"];
    let m: [u8; 32] = unhex(&d["m_hex"]).try_into().expect("32 bytes of m");
    let (ct, ss) = encapsulate_from(&kem_keypair().public_key(), &m);
    assert_eq!(d["ct_blake3"].as_str(), Some(blake3::hash(&ct).to_hex().as_str()), "the same ciphertext");
    assert_eq!(ss, unhex(&d["ss_hex"]), "the same shared secret");
}

/// NOBLE'S SIGNATURES VERIFY HERE, AND OUR DETERMINISTIC SIGNATURE IS NOBLE'S BYTE FOR BYTE.
/// `sig_hedged_b64` is noble's default (hedged) signing, what the website's `pqSignMessage` makes
/// for every chat message, DM, card and pass; `sig_deterministic_b64` is noble with
/// `extraEntropy: false`, FIPS 204's deterministic variant, which `DilithiumKeypair::sign` uses.
/// A changed message or a flipped byte does not verify.
///
/// Seen red 2026-11-19 with one byte of `noble.dsa.sig_hedged_b64` flipped: "noble's hedged
/// signature verifies under RustCrypto ml-dsa" failed.
#[test]
fn noble_signatures_verify_in_rust() {
    let fx = fixture();
    let noble = &fx["noble"]["dsa"];
    assert_eq!(noble["signer_master_byte"].as_u64(), Some(DSA_MASTER_BYTE as u64));
    let kp = dsa_keypair();
    let pk = kp.public_key();
    let msg = noble["message"].as_str().expect("the message").as_bytes();
    let hedged = b64(&noble["sig_hedged_b64"]);
    let det = b64(&noble["sig_deterministic_b64"]);
    verify_dilithium(&pk, msg, &hedged).expect("noble's hedged signature verifies under RustCrypto ml-dsa");
    verify_dilithium(&pk, msg, &det).expect("noble's deterministic signature verifies under RustCrypto ml-dsa");
    assert_eq!(kp.sign(msg), det, "the deterministic signature is the same bytes on both sides");
    assert!(verify_dilithium(&pk, b"another message", &hedged).is_err(), "another message");
    let mut bad = hedged.clone();
    bad[100] ^= 0x01;
    assert!(verify_dilithium(&pk, msg, &bad).is_err(), "a flipped byte");
}

/// THE RUST HALF IS STILL WHAT THIS CODE MAKES: the key, the deterministic ciphertext and secret,
/// and the signature are reproduced exactly, and the randomized ciphertext still decapsulates to
/// its secret. Run on every upgrade of ml-kem or ml-dsa, this is what says the new version still
/// gives the old answers (scripts/tests/pq-cross.test.js does the same for noble's half).
///
/// Seen red 2026-11-19 with one byte of `rust.dsa.sig_b64` flipped: "the signature" failed.
#[test]
fn the_rust_half_is_what_this_code_makes_today() {
    let fx = fixture();
    let (old, now) = (&fx["rust"], rust_half_now());
    assert_eq!(old["kem"]["ek_b64"], now["kem"]["ek_b64"], "the encapsulation key");
    assert_eq!(old["kem"]["deterministic"], now["kem"]["deterministic"], "the deterministic encapsulation");
    assert_eq!(old["dsa"], now["dsa"], "the signature");
    let ss = kem_keypair().decapsulate(&b64(&old["kem"]["ct_b64"])).expect("decapsulate");
    assert_eq!(ss.to_vec(), unhex(&old["kem"]["ss_hex"]), "the randomized ciphertext opens to its secret");
}
