//! Rung 2 of `docs/design/signed_moderation_logs.md`: the relay verifies a
//! signed moderation action against the space's declared authority and applies
//! it. This is where the promise stops being an encoder nobody calls.
//!
//! ## The rule
//!
//! An action that arrives without a valid signed object is not applied. If the
//! relay can act without a signature, the log is decoration. The object arrives
//! through the ordinary signed-object path, so `put_signed_object` has already
//! verified the Dilithium signature by the time anything here runs: what is left
//! is deciding whether the SIGNER was allowed to do it, and then doing it.
//!
//! ## Where authority comes from
//!
//! A `space_policy_v1` names an owner and a set of moderator keys, and it is
//! itself signed, so the authority set is as checkable as the actions taken
//! under it. When a space has published one, it decides.
//!
//! When a space has NOT published one, this falls back to the server's own role
//! table and records the outcome as `ServerRole` rather than `Declared`. That is
//! a deliberate, and weaker, position: it means the ACTION is attributable from
//! the first day (it is signed by a named key, and anyone can fetch and verify
//! it), while the AUTHORITY only becomes independently checkable once a policy
//! exists. Refusing everything until a policy is published would have been
//! purer and would have shipped a moderation system that moderates nothing.
//! The distinction is carried in the outcome rather than hidden, so a reader
//! can tell the two apart.
//!
//! ## What is deliberately NOT applied
//!
//! `hide` and `unhide` are refused, because this relay has no content-hiding
//! mechanism to call: there is no hidden or quarantined column anywhere in
//! storage. Accepting and storing an action whose effect never happens is
//! exactly the decoration this design exists to avoid, so it is refused loudly
//! instead. `expires_at` is likewise not enforced yet, and an action carrying
//! one is refused rather than silently becoming permanent.

use rusqlite::params;

use super::Storage;
use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::encoding::from_canonical_bytes;
use crate::relay::core::object::Object;

/// Which authority allowed an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModAuthority {
    /// A published, signed `space_policy_v1` named this key.
    Declared,
    /// No policy exists for this space, so the server's role table decided.
    /// The action is still signed and attributable; the authority is not yet
    /// independently checkable.
    ServerRole,
}

/// What happened to a `mod_action_v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModOutcome {
    Applied { action: String, target: String, authority: ModAuthority },
    /// Signed, stored, but the signer may not moderate here.
    NotAuthorized,
    /// Well-formed but this relay cannot carry out the effect.
    Unsupported(String),
    /// A moderator may not act on themselves, matching both other paths.
    SelfTarget,
    /// A non-admin may not act on an admin, matching both other paths.
    ProtectedTarget,
    /// The action names a space this relay does not host. It is stored,
    /// because it is a signed statement someone made and may be another
    /// server's log, but it has no effect here.
    ForeignSpace,
    /// Not a mod_action_v1, or its payload does not parse.
    NotApplicable,
}

fn payload_text_bytes(payload: &[u8], key: &str) -> Option<String> {
    let value = from_canonical_bytes(payload).ok()?;
    let ciborium::Value::Map(entries) = value else { return None };
    for (k, v) in entries {
        if let (ciborium::Value::Text(k), ciborium::Value::Text(v)) = (k, v) {
            if k == key {
                return Some(v);
            }
        }
    }
    None
}

fn payload_text(object: &Object, key: &str) -> Option<String> {
    payload_text_bytes(&object.payload, key)
}

fn payload_int(object: &Object, key: &str) -> Option<i64> {
    let value = from_canonical_bytes(&object.payload).ok()?;
    let ciborium::Value::Map(entries) = value else { return None };
    for (k, v) in entries {
        if let ciborium::Value::Text(k) = k {
            if k == key {
                if let ciborium::Value::Integer(i) = v {
                    return i128::from(i).try_into().ok();
                }
            }
        }
    }
    None
}

impl Storage {
    /// The current `space_policy_v1` for a space, as (owner_hex, moderator_hex).
    ///
    /// Newest wins. A policy is only honoured if it was signed by the CURRENT
    /// owner, which is what makes changing the owner key a fork rather than a
    /// takeover: a stranger publishing a policy naming themselves is ignored
    /// because the newest policy chain does not lead to them.
    pub fn current_space_policy(
        &self,
        space_id: Option<&str>,
    ) -> Result<Option<(String, Vec<String>)>, rusqlite::Error> {
        let rows: Vec<(Vec<u8>, Vec<u8>)> = self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT payload, author_pubkey FROM signed_objects
                 WHERE object_type = 'space_policy_v1'
                   AND (space_id = ?1 OR (?1 IS NULL AND space_id IS NULL))
                 ORDER BY received_at DESC",
            )?;
            let v: Vec<(Vec<u8>, Vec<u8>)> = stmt
                .query_map(params![space_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            Ok::<Vec<(Vec<u8>, Vec<u8>)>, rusqlite::Error>(v)
        })?;

        for (payload, author) in rows {
            let Some(owner) = payload_text_bytes(&payload, "owner") else { continue };
            // The policy must be signed by the owner it names. A policy naming
            // someone else as owner is somebody trying to hand themselves a
            // space, and is ignored.
            if hex::encode(&author) != owner {
                continue;
            }
            // And the owner must actually be an admin of THIS server.
            //
            // Without this the check above is trivially satisfiable: a stranger
            // signs a policy naming THEMSELVES owner, and it is self-consistent
            // by construction. Since POST /api/v2/objects is unauthenticated,
            // that let anyone on the internet declare a space and then moderate
            // this relay through it. That attack is now a test
            // (a_stranger_cannot_invent_a_space_and_moderate_this_server) and it
            // failed before this line existed.
            //
            // Anchoring to the existing admin set is the bootstrap: the space is
            // the server, so its owner is the server's admin. A policy chain
            // where policy N+1 is signed by policy N's owner, which would let
            // ownership move without the role table, is rung 3 work.
            let owner_role = self.get_role(&owner).unwrap_or_default();
            if owner_role != "admin" && owner_role != "owner" {
                continue;
            }
            let mods = {
                let value = from_canonical_bytes(&payload).ok();
                let mut out = Vec::new();
                if let Some(ciborium::Value::Map(entries)) = value {
                    for (k, v) in entries {
                        if let (ciborium::Value::Text(k), ciborium::Value::Array(items)) = (k, v) {
                            if k == "moderators" {
                                for it in items {
                                    if let ciborium::Value::Text(m) = it {
                                        out.push(m);
                                    }
                                }
                            }
                        }
                    }
                }
                out
            };
            return Ok(Some((owner, mods)));
        }
        Ok(None)
    }

    /// Verify a `mod_action_v1` against the space's authority and apply it.
    ///
    /// Called from the signed-object ingest, so the signature is already
    /// verified. Returns what happened; the caller logs it.
    pub fn apply_mod_action(&self, object: &Object) -> Result<ModOutcome, rusqlite::Error> {
        if object.object_type != "mod_action_v1" {
            return Ok(ModOutcome::NotApplicable);
        }
        let (Some(action), Some(target), Some(kind)) = (
            payload_text(object, "action"),
            payload_text(object, "target"),
            payload_text(object, "target_kind"),
        ) else {
            return Ok(ModOutcome::NotApplicable);
        };

        // ── the space must be OURS ──
        //
        // space_id is chosen freely by whoever submits the object, and objects
        // also arrive by federation gossip from peers. Without this, an action
        // scoped to any other space took effect here, which means a peer server
        // could moderate a server it does not run.
        //
        // This server's space is its own DID. An action for anything else is
        // stored and ignored: it may be a perfectly valid entry in somebody
        // else's moderation log, and keeping it is how a federated reader can
        // audit that log. It just has no authority here.
        let this_space = self.server_did().ok();
        if object.space_id != this_space {
            return Ok(ModOutcome::ForeignSpace);
        }

        let signer_hex = hex::encode(&object.author_public_key);

        // ── authority ──
        let policy = self.current_space_policy(object.space_id.as_deref())?;
        let (authorized, authority) = match &policy {
            Some((owner, mods)) => (
                *owner == signer_hex || mods.iter().any(|m| *m == signer_hex),
                ModAuthority::Declared,
            ),
            None => {
                let role = self.get_role(&signer_hex).unwrap_or_default();
                (
                    role == "admin" || role == "mod" || role == "moderator" || role == "owner",
                    ModAuthority::ServerRole,
                )
            }
        };
        if !authorized {
            return Ok(ModOutcome::NotAuthorized);
        }
        let signer_is_admin = match &policy {
            Some((owner, _)) => *owner == signer_hex,
            None => {
                let r = self.get_role(&signer_hex).unwrap_or_default();
                r == "admin" || r == "owner"
            }
        };

        // ── effects this relay cannot carry out ──
        // Refused rather than stored-and-ignored: an action whose effect never
        // happens is the decoration this whole design exists to avoid.
        if action == "hide" || action == "unhide" {
            return Ok(ModOutcome::Unsupported(
                "content hiding is not implemented on this relay".into(),
            ));
        }
        if payload_int(object, "expires_at").unwrap_or(0) != 0 {
            return Ok(ModOutcome::Unsupported(
                "timed actions are not enforced yet, so this would become permanent".into(),
            ));
        }
        if kind != "identity" {
            return Ok(ModOutcome::Unsupported(format!(
                "target_kind '{kind}' has no effect on this relay"
            )));
        }

        // ── the same two guards both other moderation paths use ──
        // Divergence between the paths is the bug that was fixed on the typed
        // path today; a third path with different rules would reintroduce it.
        if matches!(action.as_str(), "ban" | "mute") && target == signer_hex {
            return Ok(ModOutcome::SelfTarget);
        }
        if !signer_is_admin {
            let target_role = self.get_role(&target).unwrap_or_default();
            if target_role == "admin" || target_role == "owner" {
                return Ok(ModOutcome::ProtectedTarget);
            }
        }

        // ── apply ──
        let name = self.name_for_key(&target).ok().flatten().unwrap_or_default();
        match action.as_str() {
            "ban" => self.ban_user(&target, &name)?,
            "unban" => self.unban_user(&target)?,
            "mute" => self.mute_user(&target, &name)?,
            "unmute" => self.unmute_user(&target)?,
            "grant_role" => {
                let Some(role) = payload_text(object, "role").filter(|r| !r.trim().is_empty())
                else {
                    return Ok(ModOutcome::NotApplicable);
                };
                // Only an admin or the space owner may hand out roles, which is
                // the one place a plain moderator must not reach: otherwise a
                // moderator promotes themselves.
                if !signer_is_admin {
                    return Ok(ModOutcome::NotAuthorized);
                }
                self.set_role(&target, &role)?
            }
            "revoke_role" => {
                if !signer_is_admin {
                    return Ok(ModOutcome::NotAuthorized);
                }
                self.set_role(&target, "member")?
            }
            // A revoke cancels an earlier action; undoing its effect needs the
            // referenced action replayed in reverse, which is rung 5's
            // deterministic replay. Refused rather than silently doing nothing.
            "revoke" => {
                return Ok(ModOutcome::Unsupported(
                    "revoking a prior action needs deterministic replay (rung 5)".into(),
                ));
            }
            other => {
                return Ok(ModOutcome::Unsupported(format!("unknown action '{other}'")));
            }
        }

        Ok(ModOutcome::Applied {
            action,
            target: did_for_pubkey(&hex::decode(&target).unwrap_or_default()),
            authority,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::moderation::{
        build_mod_action_v1, build_space_policy_v1, ModAction, SpacePolicy,
    };
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

    fn db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_modapply_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn key_of(seed: &[u8; 32]) -> String {
        let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
        hex::encode(kp.public_key())
    }

    fn action(a: &'static str, target: &'static str) -> ModAction<'static> {
        ModAction {
            action: a,
            target,
            target_kind: "identity",
            reason: "sustained harassment after a warning",
            rule: "respect-every-person",
            expires_at: 0,
            role: "",
        }
    }

    /// The load-bearing property: a signed action from an authorized moderator
    /// actually takes effect.
    #[test]
    fn an_authorized_moderator_action_applies() {
        let db = db();
        let space = db.server_did().unwrap();
        let mod_seed = [1u8; 32];
        let mod_key = key_of(&mod_seed);
        let target = key_of(&[2u8; 32]);
        db.register_name("Mod", &mod_key).unwrap();
        db.set_role(&mod_key, "mod").unwrap();
        db.register_name("Target", &target).unwrap();

        let obj = build_mod_action_v1(&mod_seed, &space, &action("mute", Box::leak(target.clone().into_boxed_str())), 1, &[]).unwrap();
        let out = db.apply_mod_action(&obj).unwrap();
        assert!(matches!(out, ModOutcome::Applied { authority: ModAuthority::ServerRole, .. }), "got {out:?}");
        assert!(db.is_muted(&target).unwrap(), "the mute must actually be in force");
    }

    /// And a stranger's signed action does NOT, which is the whole point: the
    /// object is well-formed and correctly signed, and it is still refused.
    #[test]
    fn an_unauthorized_signer_is_refused() {
        let db = db();
        let space = db.server_did().unwrap();
        let stranger_seed = [9u8; 32];
        let target = key_of(&[2u8; 32]);
        db.register_name("Nobody", &key_of(&stranger_seed)).unwrap();
        db.register_name("Target", &target).unwrap();

        let obj = build_mod_action_v1(&stranger_seed, &space, &action("ban", Box::leak(target.clone().into_boxed_str())), 1, &[]).unwrap();
        assert_eq!(db.apply_mod_action(&obj).unwrap(), ModOutcome::NotAuthorized);
        assert!(!db.is_banned(&target).unwrap(), "nothing may have happened");
    }

    /// A published policy overrides the server's role table, in BOTH directions:
    /// it can authorize someone the server has never heard of, and it can refuse
    /// someone the server calls a moderator.
    #[test]
    fn a_published_policy_decides_who_may_moderate() {
        let db = db();
        let space = db.server_did().unwrap();
        let owner_seed = [3u8; 32];
        let owner = key_of(&owner_seed);
        let outsider_seed = [4u8; 32];
        let outsider = key_of(&outsider_seed);
        let server_mod_seed = [5u8; 32];
        let server_mod = key_of(&server_mod_seed);
        let target = key_of(&[2u8; 32]);

        db.register_name("Target", &target).unwrap();
        // The owner must be an admin of this server: that is the bootstrap that
        // stops a stranger declaring a space and moderating through it.
        db.register_name("Owner", &owner).unwrap();
        db.set_role(&owner, "admin").unwrap();
        // The server thinks this person is a moderator...
        db.register_name("ServerMod", &server_mod).unwrap();
        db.set_role(&server_mod, "mod").unwrap();

        // ...but the published policy names someone else entirely.
        let mods = vec![outsider.clone()];
        let allowed: Vec<String> = vec![];
        let policy = build_space_policy_v1(
            &owner_seed,
            &space,
            &SpacePolicy {
                owner: &owner,
                moderators: &mods,
                rules_url: "https://example.invalid/rules",
                rules_hash: "",
                appeals: "open an issue",
                unsigned_allowed: &allowed,
            },
            1,
        )
        .unwrap();
        db.put_signed_object(&policy, None).unwrap();

        // The outsider, unknown to the role table, is authorized by the policy.
        let ok = build_mod_action_v1(&outsider_seed, &space, &action("mute", Box::leak(target.clone().into_boxed_str())), 2, &[]).unwrap();
        assert!(
            matches!(db.apply_mod_action(&ok).unwrap(), ModOutcome::Applied { authority: ModAuthority::Declared, .. }),
            "a policy-named moderator must be allowed"
        );

        // The server's own "mod" is NOT in the policy, so is refused.
        let refused = build_mod_action_v1(&server_mod_seed, &space, &action("ban", Box::leak(target.clone().into_boxed_str())), 3, &[]).unwrap();
        assert_eq!(
            db.apply_mod_action(&refused).unwrap(),
            ModOutcome::NotAuthorized,
            "a published policy must override the role table"
        );
    }

    /// A policy naming someone ELSE as owner is somebody trying to hand
    /// themselves a space, and is ignored.
    #[test]
    fn a_policy_not_signed_by_its_named_owner_is_ignored() {
        let db = db();
        let space = db.server_did().unwrap();
        let real_owner = key_of(&[3u8; 32]);
        let usurper_seed = [7u8; 32];
        let target = key_of(&[2u8; 32]);
        db.register_name("Target", &target).unwrap();

        let mods = vec![key_of(&usurper_seed)];
        let allowed: Vec<String> = vec![];
        // Signed by the usurper, but NAMES the real owner.
        let forged = build_space_policy_v1(
            &usurper_seed,
            &space,
            &SpacePolicy {
                owner: &real_owner,
                moderators: &mods,
                rules_url: "",
                rules_hash: "",
                appeals: "",
                unsigned_allowed: &allowed,
            },
            1,
        )
        .unwrap();
        db.put_signed_object(&forged, None).unwrap();

        assert!(
            db.current_space_policy(Some(&space)).unwrap().is_none(),
            "a policy must be signed by the owner it names"
        );
    }

    /// Effects this relay cannot carry out are refused, not stored and ignored.
    #[test]
    fn unsupported_effects_are_refused_loudly() {
        let db = db();
        let space = db.server_did().unwrap();
        let mod_seed = [1u8; 32];
        let mod_key = key_of(&mod_seed);
        let target = key_of(&[2u8; 32]);
        db.register_name("Mod", &mod_key).unwrap();
        db.set_role(&mod_key, "admin").unwrap();
        db.register_name("Target", &target).unwrap();
        let t: &'static str = Box::leak(target.clone().into_boxed_str());

        // hide: no content-hiding mechanism exists anywhere in storage.
        let mut hide = action("hide", t);
        hide.target_kind = "object";
        let obj = build_mod_action_v1(&mod_seed, &space, &hide, 1, &[]).unwrap();
        assert!(matches!(db.apply_mod_action(&obj).unwrap(), ModOutcome::Unsupported(_)));

        // A timed action would silently become permanent.
        let mut timed = action("mute", t);
        timed.expires_at = 1_751_328_000_000;
        let obj = build_mod_action_v1(&mod_seed, &space, &timed, 1, &[]).unwrap();
        assert!(matches!(db.apply_mod_action(&obj).unwrap(), ModOutcome::Unsupported(_)));
        assert!(!db.is_muted(&target).unwrap(), "a refused action must not apply");
    }

    /// The same two guards the other two moderation paths use.
    #[test]
    fn self_target_and_protected_target_are_refused() {
        let db = db();
        let space = db.server_did().unwrap();
        let mod_seed = [1u8; 32];
        let mod_key = key_of(&mod_seed);
        db.register_name("Mod", &mod_key).unwrap();
        db.set_role(&mod_key, "mod").unwrap();

        let boss = key_of(&[6u8; 32]);
        db.register_name("Boss", &boss).unwrap();
        db.set_role(&boss, "admin").unwrap();

        let me: &'static str = Box::leak(mod_key.clone().into_boxed_str());
        let obj = build_mod_action_v1(&mod_seed, &space, &action("ban", me), 1, &[]).unwrap();
        assert_eq!(db.apply_mod_action(&obj).unwrap(), ModOutcome::SelfTarget);

        let b: &'static str = Box::leak(boss.clone().into_boxed_str());
        let obj = build_mod_action_v1(&mod_seed, &space, &action("mute", b), 1, &[]).unwrap();
        assert_eq!(db.apply_mod_action(&obj).unwrap(), ModOutcome::ProtectedTarget);
        assert!(!db.is_muted(&boss).unwrap());
    }

    /// THE ATTACK. A complete stranger declares their own space, names
    /// themselves its owner, and bans someone on this server.
    ///
    /// POST /api/v2/objects is unauthenticated and signature-verified, so
    /// anyone on the internet can submit both objects. If this passes, the
    /// authority check is worthless.
    #[test]
    fn a_stranger_cannot_invent_a_space_and_moderate_this_server() {
        let db = db();
        let space = db.server_did().unwrap();
        let attacker_seed = [42u8; 32];
        let attacker = key_of(&attacker_seed);
        let victim = key_of(&[2u8; 32]);
        db.register_name("Victim", &victim).unwrap();
        // The attacker has NO role here at all.
        assert_eq!(db.get_role(&attacker).unwrap_or_default(), "");

        // Step 1: publish a policy for a space of their own invention, naming
        // themselves owner. It is correctly signed, so it is self-consistent.
        let mods: Vec<String> = vec![];
        let allowed: Vec<String> = vec![];
        let policy = build_space_policy_v1(
            &attacker_seed,
            "a-space-i-just-made-up",
            &SpacePolicy {
                owner: &attacker,
                moderators: &mods,
                rules_url: "",
                rules_hash: "",
                appeals: "",
                unsigned_allowed: &allowed,
            },
            1,
        )
        .unwrap();
        db.put_signed_object(&policy, None).unwrap();

        // Step 2: ban the victim inside that space.
        let ban = build_mod_action_v1(
            &attacker_seed,
            "a-space-i-just-made-up",
            &action("ban", Box::leak(victim.clone().into_boxed_str())),
            2,
            &[],
        )
        .unwrap();
        let out = db.apply_mod_action(&ban).unwrap();

        assert_ne!(
            out,
            ModOutcome::Applied {
                action: "ban".into(),
                target: did_for_pubkey(&hex::decode(&victim).unwrap()),
                authority: ModAuthority::Declared
            },
            "a stranger's invented space must not grant authority over this server"
        );
        assert!(
            !db.is_banned(&victim).unwrap(),
            "THE VICTIM MUST NOT BE BANNED by a stranger who declared their own space"
        );
    }

    /// A peer server cannot moderate a server it does not run.
    ///
    /// Objects arrive by federation gossip as well as by direct POST, and both
    /// land in put_signed_object and therefore here. An action scoped to the
    /// PEER's space is stored, because it is a real entry in that peer's log and
    /// keeping it is how a federated reader audits them, but it must not take
    /// effect locally.
    #[test]
    fn an_action_scoped_to_another_space_is_stored_but_not_applied() {
        let db = db();
        let admin_seed = [1u8; 32];
        let admin = key_of(&admin_seed);
        let victim = key_of(&[2u8; 32]);
        db.register_name("Admin", &admin).unwrap();
        db.set_role(&admin, "admin").unwrap();
        db.register_name("Victim", &victim).unwrap();

        // Signed by a REAL admin of this server, but scoped to another space.
        let obj = build_mod_action_v1(
            &admin_seed,
            "did:hum:SomeOtherServersSpace",
            &action("ban", Box::leak(victim.clone().into_boxed_str())),
            1,
            &[],
        )
        .unwrap();
        assert_eq!(db.apply_mod_action(&obj).unwrap(), ModOutcome::ForeignSpace);
        assert!(!db.is_banned(&victim).unwrap(), "another space must not reach us");

        // The object itself is still storable: it is somebody else's record.
        assert!(db.put_signed_object(&obj, Some("peer.example")).unwrap());
    }

    /// A plain moderator may not hand out roles, or they promote themselves.
    #[test]
    fn only_an_admin_may_grant_a_role() {
        let db = db();
        let space = db.server_did().unwrap();
        let mod_seed = [1u8; 32];
        let mod_key = key_of(&mod_seed);
        let target = key_of(&[2u8; 32]);
        db.register_name("Mod", &mod_key).unwrap();
        db.set_role(&mod_key, "mod").unwrap();
        db.register_name("Target", &target).unwrap();

        let mut grant = action("grant_role", Box::leak(target.clone().into_boxed_str()));
        grant.role = "admin";
        let obj = build_mod_action_v1(&mod_seed, &space, &grant, 1, &[]).unwrap();
        assert_eq!(db.apply_mod_action(&obj).unwrap(), ModOutcome::NotAuthorized);
        assert_ne!(db.get_role(&target).unwrap_or_default(), "admin");
    }
}
