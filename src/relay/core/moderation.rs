//! Signed moderation objects: `mod_action_v1` and `space_policy_v1`.
//!
//! Rung 1 of `docs/design/signed_moderation_logs.md`. This module is the
//! ENCODING half only: it builds and signs the two object types and validates
//! their payloads. It does not enforce anything yet, and deliberately so, since
//! a rung that claimed enforcement it did not have would be the exact false
//! promise the design was revised to avoid.
//!
//! ## Why the client signs, not the relay
//!
//! The 2026-05 design was accepted and then nothing was built for four months,
//! because it required every moderation action to be signed by an authorized
//! key and never said who holds that key when the RELAY is the executor. A
//! moderator types `/ban bob` over a socket; the relay does not have their
//! secret key and must never have it. A log signed by the party whose behaviour
//! it exists to constrain proves only that the relay agrees with itself.
//!
//! So the moderator's CLIENT builds and signs these objects, exactly as it
//! already builds and signs `vote_v1`, and the relay verifies and applies. That
//! is why these builders take a seed: they run on whichever side holds the
//! identity. Native calls them directly; the web has a byte-identical twin in
//! `web/shared/pq-object.js`, locked by `just mod-kat`.
//!
//! ## Payload shapes
//!
//! Key ORDER here is not load-bearing. `to_canonical_bytes` sorts map keys, and
//! the JS `cborMap` sorts by the same rule, so the two languages agree
//! regardless. The order below simply reads well.

use super::encoding::{cbor_int, cbor_map, cbor_text};
use super::object::{Object, ObjectBuilder};
use super::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use ciborium::Value;

/// Actions a `mod_action_v1` may carry. Anything else is refused at build time
/// rather than becoming an object nobody can interpret later.
pub const MOD_ACTIONS: &[&str] = &[
    "ban", "unban", "mute", "unmute", "hide", "unhide", "grant_role", "revoke_role", "revoke",
];

/// What a `target` names.
pub const TARGET_KINDS: &[&str] = &["identity", "object"];

/// Why a payload was refused. Plain enough to show a moderator directly.
#[derive(Debug, PartialEq, Eq)]
pub enum ModError {
    UnknownAction(String),
    UnknownTargetKind(String),
    EmptyTarget,
    EmptyReason,
    RoleMissing,
    RoleUnexpected,
    Encode(String),
}

impl std::fmt::Display for ModError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModError::UnknownAction(a) => write!(f, "unknown moderation action '{a}'"),
            ModError::UnknownTargetKind(k) => {
                write!(f, "target_kind must be 'identity' or 'object', got '{k}'")
            }
            ModError::EmptyTarget => write!(f, "target is required"),
            ModError::EmptyReason => write!(
                f,
                "a reason is required: an unexplained moderation action is what the appeals \
                 requirement exists to prevent"
            ),
            ModError::RoleMissing => write!(f, "grant_role and revoke_role require a role"),
            ModError::RoleUnexpected => {
                write!(f, "role is only meaningful for grant_role and revoke_role")
            }
            ModError::Encode(e) => write!(f, "encode failed: {e}"),
        }
    }
}

/// The fields of one moderation action, before encoding.
#[derive(Debug, Clone)]
pub struct ModAction<'a> {
    pub action: &'a str,
    /// Dilithium public key hex for a person, or an object id hex for content.
    pub target: &'a str,
    pub target_kind: &'a str,
    /// Plain language, shown to the target and in the public log. Required.
    pub reason: &'a str,
    /// Identifier of the published rule being applied, or "" if none.
    pub rule: &'a str,
    /// Milliseconds since epoch, or 0 for no expiry.
    pub expires_at: u64,
    /// For grant_role / revoke_role only.
    pub role: &'a str,
}

/// Build the canonical payload for a `mod_action_v1`, validating as it goes.
///
/// `reason` being mandatory is a design position, not an oversight: a schema
/// that permits an empty reason will collect them, and an action nobody can
/// explain cannot be appealed.
pub fn mod_action_payload(a: &ModAction<'_>) -> Result<Value, ModError> {
    if !MOD_ACTIONS.contains(&a.action) {
        return Err(ModError::UnknownAction(a.action.to_string()));
    }
    if !TARGET_KINDS.contains(&a.target_kind) {
        return Err(ModError::UnknownTargetKind(a.target_kind.to_string()));
    }
    if a.target.trim().is_empty() {
        return Err(ModError::EmptyTarget);
    }
    if a.reason.trim().is_empty() {
        return Err(ModError::EmptyReason);
    }
    let role_action = matches!(a.action, "grant_role" | "revoke_role");
    if role_action && a.role.trim().is_empty() {
        return Err(ModError::RoleMissing);
    }
    if !role_action && !a.role.trim().is_empty() {
        return Err(ModError::RoleUnexpected);
    }
    Ok(cbor_map(vec![
        ("action", cbor_text(a.action)),
        ("target", cbor_text(a.target)),
        ("target_kind", cbor_text(a.target_kind)),
        ("reason", cbor_text(a.reason)),
        ("rule", cbor_text(a.rule)),
        ("expires_at", cbor_int(a.expires_at)),
        ("role", cbor_text(a.role)),
    ]))
}

/// Build + sign a `mod_action_v1`.
///
/// `references` carries the superseded object id for a `revoke`; it is empty
/// for every other action. `space_id` scopes the action to one space.
pub fn build_mod_action_v1(
    seed32: &[u8],
    space_id: &str,
    a: &ModAction<'_>,
    created_at: u64,
    references: &[String],
) -> Result<Object, ModError> {
    let payload = mod_action_payload(a)?;
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed32));
    let mut b = ObjectBuilder::new("mod_action_v1")
        .space_id(space_id)
        .created_at(created_at)
        .payload_cbor(&payload)
        .map_err(|e| ModError::Encode(format!("{e:?}")))?;
    for r in references {
        b = b.reference(r);
    }
    b.sign(&kp).map_err(|e| ModError::Encode(format!("{e:?}")))
}

/// A space's declared authority and rules, before encoding.
#[derive(Debug, Clone)]
pub struct SpacePolicy<'a> {
    /// Public key hex of the space owner.
    pub owner: &'a str,
    /// Public key hex of each key that may sign a `mod_action_v1` here.
    pub moderators: &'a [String],
    pub rules_url: &'a str,
    /// BLAKE3 of the rules text at publication time. This is what makes "the
    /// rules were published before you participated" checkable rather than
    /// merely asserted.
    pub rules_hash: &'a str,
    pub appeals: &'a str,
    /// Actions a keyless path (a bot, a console) may still perform. Anything
    /// not listed here requires a signed object.
    pub unsigned_allowed: &'a [String],
}

/// Build the canonical payload for a `space_policy_v1`.
pub fn space_policy_payload(p: &SpacePolicy<'_>) -> Result<Value, ModError> {
    if p.owner.trim().is_empty() {
        return Err(ModError::EmptyTarget);
    }
    for a in p.unsigned_allowed {
        if !MOD_ACTIONS.contains(&a.as_str()) {
            return Err(ModError::UnknownAction(a.clone()));
        }
    }
    let arr = |v: &[String]| Value::Array(v.iter().map(|s| cbor_text(s)).collect());
    Ok(cbor_map(vec![
        ("owner", cbor_text(p.owner)),
        ("moderators", arr(p.moderators)),
        ("rules_url", cbor_text(p.rules_url)),
        ("rules_hash", cbor_text(p.rules_hash)),
        ("appeals", cbor_text(p.appeals)),
        ("unsigned_allowed", arr(p.unsigned_allowed)),
    ]))
}

/// Build + sign a `space_policy_v1`. Must be signed by the CURRENT owner; the
/// newest valid policy signed by the current owner wins. Changing the owner key
/// is how a space forks, which the Accord treats as legitimate, so the
/// mechanism is deliberately available rather than prevented.
pub fn build_space_policy_v1(
    seed32: &[u8],
    space_id: &str,
    p: &SpacePolicy<'_>,
    created_at: u64,
) -> Result<Object, ModError> {
    let payload = space_policy_payload(p)?;
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed32));
    ObjectBuilder::new("space_policy_v1")
        .space_id(space_id)
        .created_at(created_at)
        .payload_cbor(&payload)
        .map_err(|e| ModError::Encode(format!("{e:?}")))?
        .sign(&kp)
        .map_err(|e| ModError::Encode(format!("{e:?}")))
}

/// Encode a payload to canonical bytes. Exposed for the KAT.
pub fn payload_bytes(v: &Value) -> Result<Vec<u8>, super::error::Error> {
    super::encoding::to_canonical_bytes(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action() -> ModAction<'static> {
        ModAction {
            action: "mute",
            target: "aa".repeat(32).leak(),
            target_kind: "identity",
            reason: "repeated harassment in #general after a warning",
            rule: "respect-every-person",
            expires_at: 1_751_328_000_000,
            role: "",
        }
    }

    #[test]
    fn a_reason_is_mandatory() {
        let mut a = action();
        a.reason = "   ";
        assert_eq!(mod_action_payload(&a).unwrap_err(), ModError::EmptyReason);
        // And the message says WHY, because a moderator sees it.
        assert!(ModError::EmptyReason.to_string().contains("appeals"));
    }

    #[test]
    fn unknown_actions_and_target_kinds_are_refused() {
        let mut a = action();
        a.action = "vaporize";
        assert!(matches!(mod_action_payload(&a), Err(ModError::UnknownAction(_))));
        let mut a = action();
        a.target_kind = "vibes";
        assert!(matches!(mod_action_payload(&a), Err(ModError::UnknownTargetKind(_))));
    }

    /// `role` must be present for exactly the two role actions. Both directions
    /// are checked: a role action without a role is unapplyable, and a role on
    /// a ban is a field the verifier would silently ignore.
    #[test]
    fn role_is_required_for_role_actions_and_refused_otherwise() {
        let mut a = action();
        a.action = "grant_role";
        assert_eq!(mod_action_payload(&a).unwrap_err(), ModError::RoleMissing);
        a.role = "mod";
        assert!(mod_action_payload(&a).is_ok());

        let mut b = action();
        b.role = "mod";
        assert_eq!(mod_action_payload(&b).unwrap_err(), ModError::RoleUnexpected);
    }

    #[test]
    fn a_signed_action_verifies_and_carries_its_space() {
        let obj = build_mod_action_v1(&[3u8; 32], "united-humanity.us", &action(), 1_751_328_000_000, &[])
            .expect("build");
        assert_eq!(obj.object_type, "mod_action_v1");
        assert_eq!(obj.space_id.as_deref(), Some("united-humanity.us"));
        obj.verify_signature().expect("must verify");
    }

    /// A revoke references the object it cancels, which is how supersession is
    /// expressed without mutating an append-only log.
    #[test]
    fn a_revoke_references_the_action_it_cancels() {
        let mut a = action();
        a.action = "revoke";
        a.reason = "issued in error, the report was about a different member";
        let prior = "ab".repeat(32);
        let obj = build_mod_action_v1(&[3u8; 32], "s", &a, 1, std::slice::from_ref(&prior)).unwrap();
        assert_eq!(obj.references, vec![prior]);
    }

    #[test]
    fn a_policy_signs_and_rejects_an_unknown_unsigned_action() {
        let mods = vec!["bb".repeat(32)];
        let allowed = vec!["mute".to_string()];
        let p = SpacePolicy {
            owner: &"cc".repeat(32),
            moderators: &mods,
            rules_url: "https://united-humanity.us/rules",
            rules_hash: "dd".repeat(32).leak(),
            appeals: "Open an issue on the public tracker.",
            unsigned_allowed: &allowed,
        };
        let obj = build_space_policy_v1(&[5u8; 32], "united-humanity.us", &p, 7).unwrap();
        assert_eq!(obj.object_type, "space_policy_v1");
        obj.verify_signature().expect("must verify");

        let bad_allowed = vec!["vaporize".to_string()];
        let mut bad = p.clone();
        bad.unsigned_allowed = &bad_allowed;
        assert!(matches!(space_policy_payload(&bad), Err(ModError::UnknownAction(_))));
    }

    /// Key insertion order must not change the bytes, because the JS twin sorts
    /// and this side must too. If this ever fails, the two languages have
    /// diverged and every client-signed moderation action becomes unverifiable.
    #[test]
    fn payload_bytes_do_not_depend_on_insertion_order() {
        let a = mod_action_payload(&action()).unwrap();
        let reordered = cbor_map(vec![
            ("role", cbor_text("")),
            ("expires_at", cbor_int(1_751_328_000_000)),
            ("rule", cbor_text("respect-every-person")),
            ("reason", cbor_text("repeated harassment in #general after a warning")),
            ("target_kind", cbor_text("identity")),
            ("target", cbor_text(&"aa".repeat(32))),
            ("action", cbor_text("mute")),
        ]);
        assert_eq!(
            payload_bytes(&a).unwrap(),
            payload_bytes(&reordered).unwrap(),
            "canonical encoding must sort keys"
        );
    }
}

#[cfg(test)]
mod cross_language_kat {
    use super::*;
    use crate::relay::core::hash::Hash;
    use crate::relay::core::pq_crypto::DILITHIUM_SIG_LEN;

    // CROSS-LANGUAGE KAT for the two moderation object types.
    //
    // The moderator's CLIENT signs a moderation action, so the web builder in
    // web/shared/pq-object.js must produce byte-identical objects to this
    // encoder. If it does not, a moderation action signed in a browser is
    // unverifiable by the relay, and the audit log the whole design exists to
    // provide would silently contain only the actions taken from one client.
    //
    // Same shape as object.rs::vote_v1_cross_language_kat: the goldens below
    // are DUPLICATED in scripts/mod-object-kat.mjs on purpose, so editing the
    // encoding must break BOTH or neither. Regenerate with the ignored
    // mod_kat_print_fixture test.

    // Deterministic inputs. The master seed matches the other KATs.
    const KAT_SEED: [u8; 32] = [7u8; 32];
    const KAT_CREATED_AT: u64 = 1_751_328_000_000;
    const KAT_SPACE: &str = "united-humanity.us";
    const KAT_TARGET: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const KAT_REASON: &str = "repeated harassment in #general after a warning";
    const KAT_RULE: &str = "respect-every-person";

    pub(super) fn kat_action() -> ModAction<'static> {
        ModAction {
            action: "mute",
            target: KAT_TARGET,
            target_kind: "identity",
            reason: KAT_REASON,
            rule: KAT_RULE,
            expires_at: KAT_CREATED_AT,
            role: "",
        }
    }

    pub(super) fn kat_policy_parts() -> (Vec<String>, Vec<String>) {
        (
            vec!["11".repeat(32), "22".repeat(32)],
            vec!["hide".to_string(), "unhide".to_string()],
        )
    }

    pub(super) fn kat_mod_action() -> Object {
        build_mod_action_v1(&KAT_SEED, KAT_SPACE, &kat_action(), KAT_CREATED_AT, &[]).unwrap()
    }

    pub(super) fn kat_space_policy() -> Object {
        let (mods, allowed) = kat_policy_parts();
        let p = SpacePolicy {
            owner: KAT_TARGET,
            moderators: &mods,
            rules_url: "https://united-humanity.us/rules",
            rules_hash: "33".repeat(32).leak(),
            appeals: "Open an issue at github.com/Shaostoul/Humanity/issues.",
            unsigned_allowed: &allowed,
        };
        build_space_policy_v1(&KAT_SEED, KAT_SPACE, &p, KAT_CREATED_AT).unwrap()
    }

    // GOLDEN — duplicated in scripts/mod-object-kat.mjs.
    const MOD_ACTION_PAYLOAD_HEX: &str = "a764726f6c65606472756c6574726573706563742d65766572792d706572736f6e66616374696f6e646d75746566726561736f6e782f7265706561746564206861726173736d656e7420696e202367656e6572616c2061667465722061207761726e696e67667461726765747840303132333435363738396162636465663031323334353637383961626364656630313233343536373839616263646566303132333435363738396162636465666a657870697265735f61741b00000197c34888006b7461726765745f6b696e64686964656e74697479";
    const MOD_ACTION_SIGNABLE_LEN: usize = 5684;
    const MOD_ACTION_SIGNABLE_BLAKE3: &str = "f4c5e89ab9c8edca0eb1f7e3a0c6cd6dfefe76c64b69484982e760d9da2b6c20";
    const MOD_ACTION_OBJECT_ID: &str = "8fc28ee60cb7b2bf9201bfacbe26a8b1a414a7c0cd7da60b45e65eefdea03dcf";

    const SPACE_POLICY_PAYLOAD_HEX: &str = "a6656f776e65727840303132333435363738396162636465663031323334353637383961626364656630313233343536373839616263646566303132333435363738396162636465666761707065616c7378364f70656e20616e206973737565206174206769746875622e636f6d2f5368616f73746f756c2f48756d616e6974792f6973737565732e6972756c65735f75726c782068747470733a2f2f756e697465642d68756d616e6974792e75732f72756c65736a6d6f64657261746f7273827840313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131317840323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232326a72756c65735f6861736878403333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333370756e7369676e65645f616c6c6f77656482646869646566756e68696465";
    const SPACE_POLICY_SIGNABLE_LEN: usize = 5904;
    const SPACE_POLICY_SIGNABLE_BLAKE3: &str = "d07496f0b69220adeab8640616d735d247ec2f8410c67df86a6484dd9de73b03";
    const SPACE_POLICY_OBJECT_ID: &str = "e56f6806d31ab08523cae62a5831535cb0faae634f122c9f38ea3d8aa4b00749";

    fn signable_of(obj: &Object) -> Vec<u8> {
        let mut unsigned = obj.clone();
        unsigned.signature = vec![0u8; DILITHIUM_SIG_LEN];
        unsigned.to_canonical_bytes().unwrap()
    }

    #[test]
    fn mod_action_v1_cross_language_kat() {
        let obj = kat_mod_action();
        assert_eq!(
            hex::encode(&obj.payload),
            MOD_ACTION_PAYLOAD_HEX,
            "mod_action_v1 payload encoding drifted"
        );
        let signable = signable_of(&obj);
        assert_eq!(signable.len(), MOD_ACTION_SIGNABLE_LEN, "signable length drifted");
        assert_eq!(
            Hash::digest(&signable).to_hex(),
            MOD_ACTION_SIGNABLE_BLAKE3,
            "signable bytes drifted: browser-signed moderation actions would be unverifiable"
        );
        assert_eq!(obj.object_id().unwrap().to_hex(), MOD_ACTION_OBJECT_ID, "object_id drifted");
        obj.verify_signature().expect("KAT mod_action must verify");
    }

    #[test]
    fn space_policy_v1_cross_language_kat() {
        let obj = kat_space_policy();
        assert_eq!(
            hex::encode(&obj.payload),
            SPACE_POLICY_PAYLOAD_HEX,
            "space_policy_v1 payload encoding drifted"
        );
        let signable = signable_of(&obj);
        assert_eq!(signable.len(), SPACE_POLICY_SIGNABLE_LEN, "signable length drifted");
        assert_eq!(
            Hash::digest(&signable).to_hex(),
            SPACE_POLICY_SIGNABLE_BLAKE3,
            "signable bytes drifted"
        );
        assert_eq!(obj.object_id().unwrap().to_hex(), SPACE_POLICY_OBJECT_ID, "object_id drifted");
        obj.verify_signature().expect("KAT space_policy must verify");
    }

    /// Fixture regenerator. Run:
    ///   cargo test --features relay --no-default-features --lib \
    ///     mod_kat_print_fixture -- --ignored --nocapture
    #[test]
    #[ignore]
    fn mod_kat_print_fixture() {
        for (label, obj) in [("MOD_ACTION", kat_mod_action()), ("SPACE_POLICY", kat_space_policy())] {
            let signable = signable_of(&obj);
            eprintln!("{label}_PAYLOAD_HEX={}", hex::encode(&obj.payload));
            eprintln!("{label}_SIGNABLE_LEN={}", signable.len());
            eprintln!("{label}_SIGNABLE_BLAKE3={}", Hash::digest(&signable).to_hex());
            eprintln!("{label}_OBJECT_ID={}", obj.object_id().unwrap().to_hex());
            eprintln!("{label}_SIG_HEX={}", hex::encode(&obj.signature));
        }
    }
}
