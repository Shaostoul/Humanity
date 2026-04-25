//! AI-as-citizen status tracking (Phase 8 PR 1).
//!
//! Per the strategic plan (decision 2): AI agents are first-class participants
//! under the same rules as humans, with mandatory transparency, and humans
//! always retain the right to refuse AI interaction.
//!
//! Rules enforced here:
//! - DIDs declaring `subject_class = "ai_agent"` must have a `controlled_by_v1` VC
//!   linking to a human operator's DID.
//! - AI agents cannot vote in governance (Accord — votes require sentient consent).
//! - All authored objects from an AI carry the `ai_agent` flag transparently.
//!
//! Future PRs will extend this with `interaction_filter` profile preferences and
//! the `ai_introduction_v1` per-conversation disclosure check.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Storage;
use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::object::Object;

/// Subject class for a DID. `subject_class_v1` objects are self-asserted; lying
/// about class is grounds for federation dispute (Phase 3).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum SubjectClass {
    Human,
    AiAgent,
    Institution,
    Unknown,
}

impl SubjectClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::AiAgent => "ai_agent",
            Self::Institution => "institution",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "human" => Self::Human,
            "ai_agent" => Self::AiAgent,
            "institution" => Self::Institution,
            _ => Self::Unknown,
        }
    }

    pub fn is_ai(&self) -> bool {
        matches!(self, Self::AiAgent)
    }
}

/// AI status for a DID.
#[derive(Debug, Clone, Serialize)]
pub struct AiStatus {
    pub did: String,
    pub subject_class: String,
    pub operator_did: Option<String>,
    pub last_updated: i64,
}

/// Read a CBOR text payload field (helper).
fn read_text(object: &Object, field: &str) -> Option<String> {
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    if let ciborium::Value::Map(entries) = value {
        for (k, v) in entries {
            if let ciborium::Value::Text(name) = k {
                if name == field {
                    if let ciborium::Value::Text(s) = v {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

impl Storage {
    /// Index a `subject_class_v1` object: DID self-declares its class.
    /// Idempotent on (did) — last write wins.
    pub fn index_subject_class(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "subject_class_v1" {
            return Ok(false);
        }
        let did = did_for_pubkey(&object.author_public_key);
        let class = match read_text(object, "class") {
            Some(c) => c,
            None => return Ok(false),
        };

        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO ai_status (did, subject_class, operator_did, last_updated)
                 VALUES (?1, ?2, NULL, ?3)
                 ON CONFLICT(did) DO UPDATE SET
                   subject_class = excluded.subject_class,
                   last_updated  = excluded.last_updated",
                params![did, class, now],
            )?;
            Ok(true)
        })
    }

    /// Index a `controlled_by_v1` object: AI agent declares its operator.
    /// The author of this object IS the AI agent (subject = author for self-claims).
    pub fn index_controlled_by(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "controlled_by_v1" {
            return Ok(false);
        }
        let agent_did = did_for_pubkey(&object.author_public_key);
        let operator_did = match read_text(object, "operator_did") {
            Some(d) => d,
            None => return Ok(false),
        };

        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            // Upsert: if there's no row yet, set class to ai_agent; otherwise just
            // record the operator (don't override an explicit class declaration).
            conn.execute(
                "INSERT INTO ai_status (did, subject_class, operator_did, last_updated)
                 VALUES (?1, 'ai_agent', ?2, ?3)
                 ON CONFLICT(did) DO UPDATE SET
                   operator_did = excluded.operator_did,
                   last_updated = excluded.last_updated",
                params![agent_did, operator_did, now],
            )?;
            Ok(true)
        })
    }

    /// Get the AI status for a DID. Returns Unknown class if no record exists.
    pub fn get_ai_status(&self, did: &str) -> Result<AiStatus, rusqlite::Error> {
        self.with_conn(|conn| {
            let row = conn
                .query_row(
                    "SELECT subject_class, operator_did, last_updated
                     FROM ai_status WHERE did = ?1",
                    params![did],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()?;

            Ok(match row {
                Some((class, op, ts)) => AiStatus {
                    did: did.to_string(),
                    subject_class: class,
                    operator_did: op,
                    last_updated: ts,
                },
                None => AiStatus {
                    did: did.to_string(),
                    subject_class: "unknown".to_string(),
                    operator_did: None,
                    last_updated: 0,
                },
            })
        })
    }

    /// Convenience: is the given DID an AI agent? Defaults to false (assume human)
    /// if no class has been declared.
    pub fn is_ai_agent(&self, did: &str) -> bool {
        self.get_ai_status(did)
            .map(|s| s.subject_class == "ai_agent")
            .unwrap_or(false)
    }

    /// Check if an AI agent has the required `controlled_by_v1` binding.
    /// Returns true for non-AI DIDs (no requirement).
    pub fn ai_has_operator(&self, did: &str) -> bool {
        match self.get_ai_status(did) {
            Ok(s) if s.subject_class == "ai_agent" => s.operator_did.is_some(),
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::encoding::{cbor_map, cbor_text};
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_ai_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn make_class_declaration(kp: &DilithiumKeypair, class: &str) -> Object {
        let payload = cbor_map(vec![("class", cbor_text(class))]);
        ObjectBuilder::new("subject_class_v1")
            .created_at(1)
            .payload_cbor(&payload)
            .unwrap()
            .sign(kp)
            .unwrap()
    }

    fn make_controlled_by(agent: &DilithiumKeypair, operator_did: &str) -> Object {
        let payload = cbor_map(vec![("operator_did", cbor_text(operator_did))]);
        ObjectBuilder::new("controlled_by_v1")
            .created_at(2)
            .payload_cbor(&payload)
            .unwrap()
            .sign(agent)
            .unwrap()
    }

    #[test]
    fn unknown_did_is_not_ai() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());
        assert!(!db.is_ai_agent(&did));
    }

    #[test]
    fn declaring_ai_class_persists() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        let decl = make_class_declaration(&kp, "ai_agent");
        db.put_signed_object(&decl, None).unwrap();

        assert!(db.is_ai_agent(&did));
        let status = db.get_ai_status(&did).unwrap();
        assert_eq!(status.subject_class, "ai_agent");
    }

    #[test]
    fn human_class_is_persisted_too() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        let decl = make_class_declaration(&kp, "human");
        db.put_signed_object(&decl, None).unwrap();

        let status = db.get_ai_status(&did).unwrap();
        assert_eq!(status.subject_class, "human");
        assert!(!db.is_ai_agent(&did));
    }

    #[test]
    fn ai_without_operator_fails_compliance() {
        let db = make_test_storage();
        let agent = DilithiumKeypair::generate().unwrap();
        let agent_did = did_for_pubkey(&agent.public_key());

        let decl = make_class_declaration(&agent, "ai_agent");
        db.put_signed_object(&decl, None).unwrap();

        assert!(db.is_ai_agent(&agent_did));
        assert!(!db.ai_has_operator(&agent_did));
    }

    #[test]
    fn ai_with_controlled_by_passes_compliance() {
        let db = make_test_storage();
        let operator = DilithiumKeypair::generate().unwrap();
        let operator_did = did_for_pubkey(&operator.public_key());

        let agent = DilithiumKeypair::generate().unwrap();
        let agent_did = did_for_pubkey(&agent.public_key());

        // Agent declares class + operator binding
        db.put_signed_object(&make_class_declaration(&agent, "ai_agent"), None).unwrap();
        db.put_signed_object(&make_controlled_by(&agent, &operator_did), None).unwrap();

        assert!(db.is_ai_agent(&agent_did));
        assert!(db.ai_has_operator(&agent_did));
        let status = db.get_ai_status(&agent_did).unwrap();
        assert_eq!(status.operator_did.as_deref(), Some(operator_did.as_str()));
    }

    #[test]
    fn humans_pass_operator_check_trivially() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());
        // No class declaration at all
        assert!(db.ai_has_operator(&did));

        // Or explicit human declaration
        db.put_signed_object(&make_class_declaration(&kp, "human"), None).unwrap();
        assert!(db.ai_has_operator(&did));
    }
}
