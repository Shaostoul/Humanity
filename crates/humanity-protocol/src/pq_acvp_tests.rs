//! NIST'S OFFICIAL TEST VECTORS against the pinned RustCrypto crates (2026-11-19, the post-quantum
//! review's plan item 1b, docs/reference/research/2026-11-16-pqc-review/).
//!
//! The known-answer and cross tests hold our two implementations to EACH OTHER; these hold this
//! crate's (ml-kem 0.3.3, ml-dsa 0.1.1, exactly as Cargo.toml pins them) to THE STANDARD: NIST's
//! ACVP vectors (FIPS 203, FIPS 204, the "internal projection" files, which carry the expected
//! answers), cut down to the parameter sets HumanityOS uses, ML-KEM-768 and ML-DSA-65, by
//! scripts/acvp-extract.mjs into scripts/tests/fixtures/acvp/ (each fixture names its ACVP-Server
//! commit and the source file's SHA-256; NIST's notice is NOTICE.md beside them). The website's
//! library meets the same vectors in scripts/tests/pq-acvp.test.js.
//!
//! Every group in a fixture is run: a group this file cannot place fails the test instead of being
//! skipped. The groups no implementation of ours can run (HashML-DSA, pre-hash) are not in the
//! fixtures; each fixture's `left_out` names them with the reason.
//!
//! Where FIPS 204 or 203 defines an interface the app does not call (the expanded secret-key
//! encodings, `Sign_internal`, a precomputed mu), the crate's own public function for it is used,
//! some of them deprecated or documented only for testing; nothing here changes what the app does.

#![allow(deprecated)] // ml-dsa's from_expanded/to_expanded and ml-kem's ExpandedKeyEncoding: the vectors' key format.

use super::*;
use ml_dsa::{signature::rand_core, ExpandedSigningKey, SigningKey};
use ml_kem::ExpandedKeyEncoding;
use serde_json::Value;

/// One fixture from scripts/tests/fixtures/acvp/ (or the folder HOS_ACVP_DIR names, as
/// scripts/tests/pq-acvp.test.js reads it: how a changed copy is run to see these tests red), its
/// provenance checked.
fn vectors(name: &str) -> Value {
    let dir = std::env::var_os("HOS_ACVP_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/acvp"));
    let path = dir.join(name);
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("JSON");
    let src = &v["source"];
    assert_eq!(src["repository"], "https://github.com/usnistgov/ACVP-Server", "{name}: from NIST's ACVP-Server");
    assert!(src["commit"].as_str().is_some_and(|c| c.len() == 40), "{name}: names its commit");
    assert!(src["sha256"].as_str().is_some_and(|c| c.len() == 64), "{name}: names its source file's SHA-256");
    assert!(!v["testGroups"].as_array().expect("groups").is_empty(), "{name}: has groups");
    v
}

fn groups<'a>(v: &'a Value, parameter_set: &str) -> impl Iterator<Item = &'a Value> + 'a {
    let ps = parameter_set.to_string();
    v["testGroups"].as_array().expect("groups").iter().inspect(move |g| {
        assert_eq!(g["parameterSet"].as_str(), Some(ps.as_str()), "only {ps} groups in this fixture");
    })
}

fn tests(g: &Value) -> &Vec<Value> {
    g["tests"].as_array().expect("tests")
}

fn bytes(t: &Value, field: &str) -> Vec<u8> {
    hex::decode(t[field].as_str().unwrap_or_else(|| panic!("tcId {}: no {field}", t["tcId"]))).expect("hex")
}

fn array<const N: usize>(t: &Value, field: &str) -> [u8; N] {
    bytes(t, field).try_into().unwrap_or_else(|_| panic!("tcId {}: {field} is not {N} bytes", t["tcId"]))
}

/// A stand-in random-number generator that hands out exactly the vector's `rnd` (32 bytes) and
/// nothing more: how a hedged signature from given randomness is made through ml-dsa's public
/// `sign_randomized` and `sign_mu_randomized`.
struct GivenRandomness {
    left: Vec<u8>,
}

impl rand_core::TryRng for GivenRandomness {
    type Error = core::convert::Infallible;
    fn try_next_u32(&mut self) -> core::result::Result<u32, Self::Error> {
        unreachable!("ML-DSA asks for rnd as bytes")
    }
    fn try_next_u64(&mut self) -> core::result::Result<u64, Self::Error> {
        unreachable!("ML-DSA asks for rnd as bytes")
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> core::result::Result<(), Self::Error> {
        assert!(dst.len() <= self.left.len(), "asked for more randomness than the vector gives");
        let rest = self.left.split_off(dst.len());
        dst.copy_from_slice(&self.left);
        self.left = rest;
        Ok(())
    }
}

impl rand_core::TryCryptoRng for GivenRandomness {}

/// ML-KEM-768 KEY GENERATION (FIPS 203 ML-KEM.KeyGen_internal from d and z): our own
/// `KyberKeypair::from_seed` (the seed is d || z, the order ml-kem and noble both take) gives the
/// vector's encapsulation key, and the decapsulation key encodes to the vector's.
///
/// Seen red 2026-11-19 with one hex digit of the first test's `ek` changed in the fixture: "tcId
/// 26: the encapsulation key" failed.
#[test]
fn acvp_ml_kem_768_key_generation() {
    let v = vectors("ML-KEM-768-keyGen.json");
    let mut n = 0;
    for g in groups(&v, "ML-KEM-768") {
        assert_eq!(g["testType"], "AFT");
        for t in tests(g) {
            let mut seed = [0u8; KYBER_SEED_LEN];
            seed[..32].copy_from_slice(&bytes(t, "d"));
            seed[32..].copy_from_slice(&bytes(t, "z"));
            let kp = KyberKeypair::from_seed(&seed).expect("keygen");
            assert_eq!(kp.public_key(), bytes(t, "ek"), "tcId {}: the encapsulation key", t["tcId"]);
            assert_eq!(kp.decap_key.to_expanded_bytes().to_vec(), bytes(t, "dk"), "tcId {}: the decapsulation key", t["tcId"]);
            n += 1;
        }
    }
    assert_eq!(n, 25, "every key-generation vector ran");
}

/// ML-KEM-768 ENCAPSULATION, DECAPSULATION AND THE KEY CHECKS (FIPS 203 sections 6.2, 6.3, 7.2,
/// 7.3): encapsulation from the vector's `m` gives its ciphertext and shared secret;
/// decapsulation gives the vector's shared secret, for valid ciphertexts and for modified ones
/// (implicit rejection: the secret derived from z, not an error); an encapsulation key whose
/// coefficients are out of range is refused, by ml-kem and by our own `encapsulate_to`; a
/// decapsulation key whose stored hash of the encapsulation key is wrong is refused.
///
/// Seen red 2026-11-19 with one hex digit of the first decapsulation test's `k` changed in the
/// fixture: "tcId 86 (valid decapsulation): the shared secret" failed.
#[test]
fn acvp_ml_kem_768_encapsulation_and_decapsulation() {
    let v = vectors("ML-KEM-768-encapDecap.json");
    let mut ran = std::collections::BTreeMap::<String, usize>::new();
    for g in groups(&v, "ML-KEM-768") {
        let function = g["function"].as_str().expect("a function").to_string();
        for t in tests(g) {
            let id = format!("tcId {} ({})", t["tcId"], t["reason"].as_str().unwrap_or("AFT"));
            match function.as_str() {
                "encapsulation" => {
                    let ek_bytes = bytes(t, "ek");
                    let key: Key<EncapsulationKey<MlKem768>> = Array::try_from(ek_bytes.as_slice()).expect("1184 bytes");
                    let ek = EncapsulationKey::<MlKem768>::new(&key).expect("a valid key");
                    let (c, k) = ek.encapsulate_deterministic(&Array::from(array::<32>(t, "m")));
                    assert_eq!(c.as_slice(), bytes(t, "c").as_slice(), "{id}: the ciphertext");
                    assert_eq!(k.as_slice(), bytes(t, "k").as_slice(), "{id}: the shared secret");
                }
                "decapsulation" => {
                    let dk_bytes = bytes(t, "dk");
                    let dk = DecapsulationKey::<MlKem768>::from_expanded_bytes(&Array::try_from(dk_bytes.as_slice()).expect("2400 bytes"))
                        .expect("a valid decapsulation key");
                    let c = Array::try_from(bytes(t, "c").as_slice()).expect("1088 bytes");
                    assert_eq!(dk.decapsulate(&c).as_slice(), bytes(t, "k").as_slice(), "{id}: the shared secret");
                }
                "encapsulationKeyCheck" => {
                    let ek_bytes = bytes(t, "ek");
                    let key: Key<EncapsulationKey<MlKem768>> = Array::try_from(ek_bytes.as_slice()).expect("1184 bytes");
                    let want = t["testPassed"].as_bool().expect("testPassed");
                    assert_eq!(EncapsulationKey::<MlKem768>::new(&key).is_ok(), want, "{id}: ml-kem's encapsulation key check");
                    assert_eq!(encapsulate_to(&ek_bytes).is_ok(), want, "{id}: our encapsulate_to's check");
                }
                "decapsulationKeyCheck" => {
                    let dk_bytes = bytes(t, "dk");
                    let want = t["testPassed"].as_bool().expect("testPassed");
                    let got = DecapsulationKey::<MlKem768>::from_expanded_bytes(&Array::try_from(dk_bytes.as_slice()).expect("2400 bytes")).is_ok();
                    assert_eq!(got, want, "{id}: the decapsulation key check");
                }
                other => panic!("an encapDecap group this test cannot place: function {other}"),
            }
            *ran.entry(function.clone()).or_default() += 1;
        }
    }
    let want: std::collections::BTreeMap<String, usize> =
        [("decapsulation", 10), ("decapsulationKeyCheck", 10), ("encapsulation", 25), ("encapsulationKeyCheck", 10)]
            .into_iter()
            .map(|(f, n)| (f.to_string(), n))
            .collect();
    assert_eq!(ran, want, "every encapsulation, decapsulation and key-check vector ran");
}

/// ML-DSA-65 KEY GENERATION (FIPS 204 ML-DSA.KeyGen_internal from the 32-byte seed): our own
/// `DilithiumKeypair::from_seed` gives the vector's public key, and the signing key encodes to the
/// vector's secret key.
///
/// Seen red 2026-11-19 with one hex digit of the first test's `pk` changed in the fixture:
/// "tcId 26: the public key" failed.
#[test]
fn acvp_ml_dsa_65_key_generation() {
    let v = vectors("ML-DSA-65-keyGen.json");
    let mut n = 0;
    for g in groups(&v, "ML-DSA-65") {
        assert_eq!(g["testType"], "AFT");
        for t in tests(g) {
            let seed = array::<32>(t, "seed");
            assert_eq!(DilithiumKeypair::from_seed(&seed).public_key(), bytes(t, "pk"), "tcId {}: the public key", t["tcId"]);
            let sk = SigningKey::<MlDsa65>::from_seed(&Array::from(seed));
            assert_eq!(sk.expanded_key().to_expanded().to_vec(), bytes(t, "sk"), "tcId {}: the secret key", t["tcId"]);
            n += 1;
        }
    }
    assert_eq!(n, 25, "every key-generation vector ran");
}

/// ML-DSA-65 SIGNING (FIPS 204 ML-DSA.Sign, its deterministic variant, and Sign_internal, with and
/// without a precomputed mu), deterministic and hedged with the vector's own `rnd`: each makes
/// the vector's signature byte for byte. The pure external interface with a context is what the
/// app's signatures are (with an empty context); the deterministic external group includes an
/// empty context.
///
/// Seen red 2026-11-19 with one hex digit of the first hedged external test's `signature` changed
/// in the fixture: "external hedged tcId 211: the signature" failed.
#[test]
fn acvp_ml_dsa_65_signing() {
    let v = vectors("ML-DSA-65-sigGen.json");
    let mut ran = std::collections::BTreeMap::<String, usize>::new();
    for g in groups(&v, "ML-DSA-65") {
        let deterministic = g["deterministic"].as_bool().expect("deterministic");
        let interface = g["signatureInterface"].as_str().expect("an interface");
        let external_mu = g["externalMu"].as_bool().expect("externalMu");
        assert_eq!(g["preHash"].as_str(), Some(if interface == "external" { "pure" } else { "none" }), "pre-hash groups are left out of the fixture");
        let kind = format!("{interface}{} {}", if external_mu { " mu" } else { "" }, if deterministic { "deterministic" } else { "hedged" });
        for t in tests(g) {
            let sk_bytes = bytes(t, "sk");
            let sk = ExpandedSigningKey::<MlDsa65>::from_expanded(&Array::try_from(sk_bytes.as_slice()).expect("4032 bytes"));
            let rnd = if deterministic { [0u8; 32] } else { array::<32>(t, "rnd") };
            let mut given = GivenRandomness { left: rnd.to_vec() };
            let sig = match (interface, external_mu, deterministic) {
                ("external", false, true) => sk.sign_deterministic(&bytes(t, "message"), &bytes(t, "context")).expect("a context of at most 255 bytes"),
                ("external", false, false) => sk.sign_randomized(&bytes(t, "message"), &bytes(t, "context"), &mut given).expect("signed"),
                ("internal", true, true) => sk.sign_mu_deterministic(&Array::try_from(bytes(t, "mu").as_slice()).expect("64 bytes of mu")),
                ("internal", true, false) => sk.sign_mu_randomized(&Array::try_from(bytes(t, "mu").as_slice()).expect("64 bytes of mu"), &mut given).expect("signed"),
                ("internal", false, _) => {
                    let m = bytes(t, "message");
                    sk.sign_internal(&[m.as_slice()], &Array::from(rnd))
                }
                other => panic!("a sigGen group this test cannot place: {other:?}"),
            };
            assert_eq!(sig.encode().to_vec(), bytes(t, "signature"), "{kind} tcId {}: the signature", t["tcId"]);
            *ran.entry(kind.clone()).or_default() += 1;
        }
    }
    let want: std::collections::BTreeMap<String, usize> = [
        "external deterministic", "external hedged", "internal deterministic", "internal hedged", "internal mu deterministic", "internal mu hedged",
    ]
    .into_iter()
    .map(|k| (k.to_string(), 15))
    .collect();
    assert_eq!(ran, want, "every signing vector ran");
}

/// ML-DSA-65 VERIFICATION (FIPS 204 ML-DSA.Verify, Verify_internal, and with a precomputed mu):
/// the valid signatures verify and the modified ones (message, commitment, z, hint) do not. For
/// the external vectors with an empty context, our own `verify_dilithium` (what the relay and the
/// desktop app check every signature with) gives the same answer.
///
/// Seen red 2026-11-19 with the first test's `testPassed` flipped in the fixture: "tcId 31
/// (modified signature - z): ml-dsa's verdict" failed.
#[test]
fn acvp_ml_dsa_65_verification() {
    let v = vectors("ML-DSA-65-sigVer.json");
    let (mut ran, mut ours, mut refused) = (0, 0, 0);
    for g in groups(&v, "ML-DSA-65") {
        let interface = g["signatureInterface"].as_str().expect("an interface");
        let external_mu = g["externalMu"].as_bool().expect("externalMu");
        for t in tests(g) {
            let id = format!("tcId {} ({})", t["tcId"], t["reason"].as_str().unwrap_or(""));
            let want = t["testPassed"].as_bool().expect("testPassed");
            let pk_bytes = bytes(t, "pk");
            let vk = DilithiumVerifyingKeyInner::<MlDsa65>::decode(&EncodedVerifyingKey::<MlDsa65>::try_from(pk_bytes.as_slice()).expect("1952 bytes"));
            let sig_bytes = bytes(t, "signature");
            let sig = DilithiumSignatureInner::<MlDsa65>::decode(&EncodedSignature::<MlDsa65>::try_from(sig_bytes.as_slice()).expect("3309 bytes"));
            let got = match (interface, external_mu) {
                ("external", false) => sig.as_ref().is_some_and(|s| vk.verify_with_context(&bytes(t, "message"), &bytes(t, "context"), s)),
                ("internal", true) => sig.as_ref().is_some_and(|s| vk.verify_mu(&Array::try_from(bytes(t, "mu").as_slice()).expect("64 bytes of mu"), s)),
                ("internal", false) => sig.as_ref().is_some_and(|s| vk.verify_internal(&bytes(t, "message"), s)),
                other => panic!("a sigVer group this test cannot place: {other:?}"),
            };
            assert_eq!(got, want, "{id}: ml-dsa's verdict");
            if interface == "external" && bytes(t, "context").is_empty() {
                assert_eq!(verify_dilithium(&pk_bytes, &bytes(t, "message"), &sig_bytes).is_ok(), want, "{id}: our verify_dilithium");
                ours += 1;
            }
            refused += usize::from(!want);
            ran += 1;
        }
    }
    assert_eq!(ran, 45, "every verification vector ran");
    assert!(ours >= 1, "our own verify_dilithium met at least one vector");
    assert!(refused >= 30, "the modified signatures and messages are among them: {refused}");
}
