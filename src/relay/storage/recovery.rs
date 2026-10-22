//! Recovery through trusted people, the server's part (2026-10-21; the protocol is
//! relay/core/recovery.rs, the design docs/design/recovery-through-trusted-people.md).
//!
//! What this server keeps, and only this:
//! - `recovery_pieces`: each `recovery_share_v1` a person posted, one row per piece, with the
//!   signed object as posted (so every app can check it), its setup, slot and shape, and the
//!   hash of the slot's answer token. The piece is sealed to a trusted person and the note to
//!   the person themselves: nothing here names a trusted person, so no social graph is kept.
//!   Not in `signed_objects`, not listed by `/api/v2/objects`, never passed to other servers:
//!   a setup lives on the one server it was given to, so taking it back takes it away.
//! - `recovery_floors`: per person, the time of the newest setup that stood or was taken
//!   back. A piece signed at or before it is refused, so an old setup cannot be put back by
//!   posting its objects again.
//! - `recovery_asks`: a new device asking for help: the account, the one-time public key, the
//!   hash of the secret that closes the ask, and when it ends ([`ASK_MS`] after it opened).
//! - `recovery_answers`: a trusted person's piece, sealed to the one-time key, one per slot.
//!
//! Limits: a setup has 2 to 10 pieces (`SharePayload::parse`); a person has at most one setup
//! standing and one being sent; an account has at most [`ASKS_PER_PERSON`] asks open, the server
//! at most [`ASKS_OPEN_MAX`]; an ask holds at most one answer per slot. Asks and their answers go
//! when they end (storage/expiry.rs, and every read leaves an ended one out); a setup that was
//! never finished goes a day after it began.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Storage;
use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::object::Object;
use crate::relay::core::pq_crypto::KYBER_EK_LEN;
use crate::relay::core::recovery::{
    ANSWER_LEN, ASK_MS, SHARE_OBJECT_TYPE, SharePayload, TOKEN_LEN, account_id_ok, ask_code, close_hash, object_json,
    token_hash,
};

/// The most asks one account may have open at once.
pub const ASKS_PER_PERSON: usize = 3;
/// The most asks this server keeps open at once, everyone's together.
pub const ASKS_OPEN_MAX: usize = 1_000;
/// How far ahead of this server's clock a setup's signed time may be.
pub const SETUP_CLOCK_AHEAD_MS: u64 = 10 * 60 * 1000;
/// How old a setup's signed time may be when it arrives: a setup is posted as it is made.
pub const SETUP_MAX_AGE_MS: u64 = 86_400_000;

/// Why the server said no, for the HTTP answer.
#[derive(Debug)]
pub enum RecoveryRefusal {
    /// Malformed or not allowed by the protocol (400).
    Bad(String),
    /// Nothing to act on (404).
    NotFound(String),
    /// Wrong token or close secret (403).
    NotAllowed(String),
    /// A limit is reached (429).
    Full(String),
    /// The database failed (500).
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for RecoveryRefusal {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e)
    }
}

impl std::fmt::Display for RecoveryRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bad(s) | Self::NotFound(s) | Self::NotAllowed(s) | Self::Full(s) => f.write_str(s),
            Self::Db(e) => write!(f, "storage: {e}"),
        }
    }
}

/// One piece of a setup as served: its slot and the signed object as posted.
#[derive(Debug, Clone, Serialize)]
pub struct ServedPieceRow {
    pub slot: usize,
    pub object: serde_json::Value,
}

/// A setup that stands: every piece arrived.
#[derive(Debug, Clone, Serialize)]
pub struct StandingSetup {
    pub set_id: String,
    pub needed: usize,
    pub total: usize,
    /// The objects' signed time, Unix ms.
    pub created_at: i64,
    pub pieces: Vec<ServedPieceRow>,
}

/// A setup still arriving.
#[derive(Debug, Clone, Serialize)]
pub struct SendingSetup {
    pub set_id: String,
    pub have: usize,
    pub total: usize,
    pub created_at: i64,
}

/// What `GET /api/v2/recovery/setup/{account}` answers.
#[derive(Debug, Clone, Serialize)]
pub struct RecoverySetupView {
    pub holder_did: String,
    pub active: Option<StandingSetup>,
    pub sending: Option<SendingSetup>,
}

/// One answer an ask holds.
#[derive(Debug, Clone, Serialize)]
pub struct AnswerRow {
    pub slot: usize,
    pub sealed_b64: String,
}

/// An ask as `GET /api/v2/recovery/ask/{code}` answers it.
#[derive(Debug, Clone, Serialize)]
pub struct AskView {
    pub code: String,
    pub holder_did: String,
    pub one_time_key_b64: String,
    pub opened_at: i64,
    pub expires_at: i64,
    /// The standing setup's shape (0 and 0 when it no longer has one).
    pub needed: usize,
    pub total: usize,
    /// Answers for the standing setup only.
    pub answers: Vec<AnswerRow>,
}

/// An open ask as the list of them shows it: no code and no key, only whose and when.
#[derive(Debug, Clone, Serialize)]
pub struct OpenAsk {
    pub holder_did: String,
    pub opened_at: i64,
    pub expires_at: i64,
}

fn b64(b: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(b)
}

/// The standing setup of `holder`: the newest set with every piece, on a connection already open.
fn standing(conn: &rusqlite::Connection, holder: &str) -> rusqlite::Result<Option<StandingSetup>> {
    let mut stmt = conn.prepare(
        "SELECT set_id, needed, total, set_time, COUNT(*) FROM recovery_pieces WHERE holder_did = ?1
         GROUP BY set_id ORDER BY set_time DESC",
    )?;
    let sets: Vec<(String, i64, i64, i64, i64)> = stmt
        .query_map(params![holder], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let Some((set_id, needed, total, set_time, _)) = sets.into_iter().find(|s| s.4 == s.2) else { return Ok(None) };
    let mut stmt = conn.prepare("SELECT slot, object_json FROM recovery_pieces WHERE holder_did = ?1 AND set_id = ?2 ORDER BY slot")?;
    let pieces = stmt
        .query_map(params![holder, set_id], |r| {
            let text: String = r.get(1)?;
            Ok(ServedPieceRow { slot: r.get::<_, i64>(0)? as usize, object: serde_json::from_str(&text).unwrap_or_default() })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(StandingSetup { set_id, needed: needed as usize, total: total as usize, created_at: set_time, pieces }))
}

impl Storage {
    /// Keep one piece of a setup (`put_signed_object` hands every `recovery_share_v1` here; its
    /// signature is already checked). `Ok(true)` when it is new, `Ok(false)` when this slot of
    /// this setup was already here. When the piece completes its setup, every other setup of
    /// the person goes, with the answers asks hold for them, and the floor moves to its time.
    pub fn put_recovery_piece(&self, object: &Object, now_ms: u64) -> Result<bool, RecoveryRefusal> {
        if object.object_type != SHARE_OBJECT_TYPE {
            return Err(RecoveryRefusal::Bad(format!("not a {SHARE_OBJECT_TYPE}")));
        }
        let p = SharePayload::parse(&object.payload).map_err(RecoveryRefusal::Bad)?;
        if !object.references.is_empty() || object.space_id.is_some() || object.channel_id.is_some() {
            return Err(RecoveryRefusal::Bad("a piece belongs to no space, channel or other object".into()));
        }
        let at = object.created_at.ok_or_else(|| RecoveryRefusal::Bad("a piece carries the time it was made".into()))?;
        if at > now_ms + SETUP_CLOCK_AHEAD_MS {
            return Err(RecoveryRefusal::Bad("this piece's time is ahead of the server's clock: check this device's clock".into()));
        }
        if at + SETUP_MAX_AGE_MS < now_ms {
            return Err(RecoveryRefusal::Bad("this piece was made more than a day ago: a setup is sent as it is made".into()));
        }
        let holder = did_for_pubkey(&object.author_public_key);
        let json = object_json(object).to_string();
        let (set_id, slot, needed, total, at) = (p.set_id.clone(), p.slot as i64, p.needed as i64, p.total as i64, at as i64);
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let floor: Option<i64> =
                tx.query_row("SELECT floor_ms FROM recovery_floors WHERE holder_did = ?1", params![holder], |r| r.get(0)).optional()?;
            if floor.is_some_and(|f| at <= f) {
                return Err(RecoveryRefusal::Bad(
                    "a newer setup stands for this account, or it was taken back after this piece was made".into(),
                ));
            }
            let shape: Option<(i64, i64, i64)> = tx
                .query_row(
                    "SELECT needed, total, set_time FROM recovery_pieces WHERE holder_did = ?1 AND set_id = ?2 LIMIT 1",
                    params![holder, set_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            match shape {
                Some(s) if s != (needed, total, at) => {
                    return Err(RecoveryRefusal::Bad("this piece does not match the rest of its setup".into()));
                }
                Some(_) => {}
                None => {
                    // A new setup: any other setup of this person still arriving goes (a person
                    // has at most the one standing and the one being sent).
                    let standing_id = standing(&tx, &holder)?.map(|s| s.set_id).unwrap_or_default();
                    tx.execute(
                        "DELETE FROM recovery_pieces WHERE holder_did = ?1 AND set_id != ?2 AND set_id != ?3",
                        params![holder, set_id, standing_id],
                    )?;
                }
            }
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO recovery_pieces
                    (holder_did, set_id, slot, needed, total, set_time, token_hash, object_json, received_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![holder, set_id, slot, needed, total, at, p.token_hash.to_vec(), json, now_ms as i64],
            )? > 0;
            let have: i64 = tx.query_row(
                "SELECT COUNT(*) FROM recovery_pieces WHERE holder_did = ?1 AND set_id = ?2",
                params![holder, set_id],
                |r| r.get(0),
            )?;
            if inserted && have == total {
                // It stands: it replaces whatever stood before, and nothing older can come back.
                tx.execute("DELETE FROM recovery_pieces WHERE holder_did = ?1 AND set_id != ?2", params![holder, set_id])?;
                tx.execute(
                    "INSERT INTO recovery_floors (holder_did, floor_ms) VALUES (?1, ?2)
                     ON CONFLICT(holder_did) DO UPDATE SET floor_ms = MAX(floor_ms, excluded.floor_ms)",
                    params![holder, at],
                )?;
                tx.execute(
                    "DELETE FROM recovery_answers WHERE set_id != ?1 AND code IN (SELECT code FROM recovery_asks WHERE holder_did = ?2)",
                    params![set_id, holder],
                )?;
            }
            tx.commit()?;
            Ok(inserted)
        })
    }

    /// A person's setup: the one standing and the one being sent, `None` when there is neither.
    pub fn recovery_setup(&self, holder: &str) -> Result<Option<RecoverySetupView>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let active = standing(conn, holder)?;
            let sending = conn
                .query_row(
                    "SELECT set_id, COUNT(*), total, set_time FROM recovery_pieces WHERE holder_did = ?1
                     GROUP BY set_id HAVING COUNT(*) < total ORDER BY set_time DESC LIMIT 1",
                    params![holder],
                    |r| {
                        Ok(SendingSetup {
                            set_id: r.get(0)?,
                            have: r.get::<_, i64>(1)? as usize,
                            total: r.get::<_, i64>(2)? as usize,
                            created_at: r.get(3)?,
                        })
                    },
                )
                .optional()?;
            Ok((active.is_some() || sending.is_some()).then(|| RecoverySetupView { holder_did: holder.to_string(), active, sending }))
        })
    }

    /// Take every piece of `holder` off this server (their signed withdrawal, made at `at_ms`),
    /// with every ask that names them. Returns the pieces removed.
    pub fn recovery_withdraw(&self, holder: &str, at_ms: u64) -> Result<usize, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let n = tx.execute("DELETE FROM recovery_pieces WHERE holder_did = ?1", params![holder])?;
            tx.execute(
                "DELETE FROM recovery_answers WHERE code IN (SELECT code FROM recovery_asks WHERE holder_did = ?1)",
                params![holder],
            )?;
            tx.execute("DELETE FROM recovery_asks WHERE holder_did = ?1", params![holder])?;
            tx.execute(
                "INSERT INTO recovery_floors (holder_did, floor_ms) VALUES (?1, ?2)
                 ON CONFLICT(holder_did) DO UPDATE SET floor_ms = MAX(floor_ms, excluded.floor_ms)",
                params![holder, at_ms as i64],
            )?;
            tx.commit()?;
            Ok(n)
        })
    }

    /// Open an ask: a new device asking `holder`'s trusted people for help, with its one-time
    /// Kyber768 public key and the hash of the secret that closes it. The code is computed here
    /// from the two, never taken from the asker. Asking again with the same key gives back the
    /// same ask.
    pub fn recovery_open_ask(&self, holder: &str, one_time_key: &[u8], close: &str, now_ms: u64) -> Result<AskView, RecoveryRefusal> {
        if !account_id_ok(holder) {
            return Err(RecoveryRefusal::Bad("that is not an account id".into()));
        }
        if one_time_key.len() != KYBER_EK_LEN {
            return Err(RecoveryRefusal::Bad(format!("a one-time key is {KYBER_EK_LEN} bytes")));
        }
        if close.len() != 64 || !close.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return Err(RecoveryRefusal::Bad("close_hash is 64 lowercase hex characters".into()));
        }
        let code = ask_code(holder, one_time_key);
        let now = now_ms as i64;
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            if standing(&tx, holder)?.is_none() {
                return Err(RecoveryRefusal::NotFound("This account has no trusted people on this server.".into()));
            }
            let known: bool = tx
                .query_row("SELECT 1 FROM recovery_asks WHERE code = ?1 AND expires_at > ?2", params![code, now], |_| Ok(true))
                .optional()?
                .unwrap_or(false);
            if !known {
                let mine: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM recovery_asks WHERE holder_did = ?1 AND expires_at > ?2",
                    params![holder, now],
                    |r| r.get(0),
                )?;
                if mine as usize >= ASKS_PER_PERSON {
                    return Err(RecoveryRefusal::Full(format!(
                        "{ASKS_PER_PERSON} requests for help with this account are open already. Each ends by itself {} days after it began, or when it is finished.",
                        ASK_MS / 86_400_000
                    )));
                }
                let all: i64 = tx.query_row("SELECT COUNT(*) FROM recovery_asks WHERE expires_at > ?1", params![now], |r| r.get(0))?;
                if all as usize >= ASKS_OPEN_MAX {
                    return Err(RecoveryRefusal::Full("This server has too many open requests for help right now. Try again later.".into()));
                }
                tx.execute("DELETE FROM recovery_answers WHERE code = ?1", params![code])?;
                tx.execute(
                    "INSERT OR REPLACE INTO recovery_asks (code, holder_did, one_time_key, close_hash, opened_at, expires_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![code, holder, one_time_key, close, now, now + ASK_MS as i64],
                )?;
            }
            tx.commit()?;
            Ok(())
        })?;
        self.recovery_ask(&code, now_ms)?.ok_or_else(|| RecoveryRefusal::NotFound("the request could not be read back".into()))
    }

    /// An open ask by its code, with the answers it holds for the standing setup.
    pub fn recovery_ask(&self, code: &str, now_ms: u64) -> Result<Option<AskView>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let row: Option<(String, Vec<u8>, i64, i64)> = conn
                .query_row(
                    "SELECT holder_did, one_time_key, opened_at, expires_at FROM recovery_asks WHERE code = ?1 AND expires_at > ?2",
                    params![code, now_ms as i64],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            let Some((holder, key, opened_at, expires_at)) = row else { return Ok(None) };
            let active = standing(conn, &holder)?;
            let (needed, total, set_id) = active.as_ref().map(|s| (s.needed, s.total, s.set_id.clone())).unwrap_or_default();
            let mut stmt = conn.prepare("SELECT slot, sealed FROM recovery_answers WHERE code = ?1 AND set_id = ?2 ORDER BY slot")?;
            let answers = stmt
                .query_map(params![code, set_id], |r| {
                    Ok(AnswerRow { slot: r.get::<_, i64>(0)? as usize, sealed_b64: b64(&r.get::<_, Vec<u8>>(1)?) })
                })?
                .collect::<rusqlite::Result<_>>()?;
            Ok(Some(AskView {
                code: code.to_string(),
                holder_did: holder,
                one_time_key_b64: b64(&key),
                opened_at,
                expires_at,
                needed,
                total,
                answers,
            }))
        })
    }

    /// Every open ask, newest first, at most `limit`: whose and when, never the code or the key.
    pub fn recovery_open_asks(&self, now_ms: u64, limit: usize) -> Result<Vec<OpenAsk>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT holder_did, opened_at, expires_at FROM recovery_asks WHERE expires_at > ?1 ORDER BY opened_at DESC LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![now_ms as i64, limit as i64], |r| {
                    Ok(OpenAsk { holder_did: r.get(0)?, opened_at: r.get(1)?, expires_at: r.get(2)? })
                })?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Keep a trusted person's answer to an ask: their piece sealed to the ask's one-time key.
    /// Accepted only with the slot's answer token (its hash is in the standing setup), which only
    /// the person the piece was sealed to can read. One answer per slot; a second replaces the
    /// first. Returns how many slots have answered.
    pub fn recovery_answer(&self, code: &str, slot: usize, token: &[u8], sealed: &[u8], now_ms: u64) -> Result<usize, RecoveryRefusal> {
        if sealed.len() != ANSWER_LEN {
            return Err(RecoveryRefusal::Bad(format!("an answer is {ANSWER_LEN} bytes")));
        }
        if token.len() != TOKEN_LEN {
            return Err(RecoveryRefusal::Bad(format!("a token is {TOKEN_LEN} bytes")));
        }
        let now = now_ms as i64;
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let holder: Option<String> = tx
                .query_row("SELECT holder_did FROM recovery_asks WHERE code = ?1 AND expires_at > ?2", params![code, now], |r| r.get(0))
                .optional()?;
            let holder = holder.ok_or_else(|| RecoveryRefusal::NotFound("There is no open request for help with that code.".into()))?;
            let set = standing(&tx, &holder)?
                .ok_or_else(|| RecoveryRefusal::NotFound("This account no longer has trusted people on this server.".into()))?;
            if slot >= set.total {
                return Err(RecoveryRefusal::Bad("no such piece in this setup".into()));
            }
            let hash: Vec<u8> = tx.query_row(
                "SELECT token_hash FROM recovery_pieces WHERE holder_did = ?1 AND set_id = ?2 AND slot = ?3",
                params![holder, set.set_id, slot as i64],
                |r| r.get(0),
            )?;
            if hash != token_hash(token) {
                return Err(RecoveryRefusal::NotAllowed("That piece is not part of this account's setup.".into()));
            }
            tx.execute(
                "INSERT OR REPLACE INTO recovery_answers (code, slot, set_id, sealed, answered_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![code, slot as i64, set.set_id, sealed, now],
            )?;
            let n: i64 = tx.query_row(
                "SELECT COUNT(*) FROM recovery_answers WHERE code = ?1 AND set_id = ?2",
                params![code, set.set_id],
                |r| r.get(0),
            )?;
            tx.commit()?;
            Ok(n as usize)
        })
    }

    /// Close an ask with the secret its device kept (its hash was filed when it opened).
    pub fn recovery_close_with_secret(&self, code: &str, secret: &[u8]) -> Result<(), RecoveryRefusal> {
        let filed: Option<String> = self
            .with_read_conn(|conn| conn.query_row("SELECT close_hash FROM recovery_asks WHERE code = ?1", params![code], |r| r.get(0)).optional())?;
        let filed = filed.ok_or_else(|| RecoveryRefusal::NotFound("There is no request for help with that code.".into()))?;
        if filed != close_hash(secret) {
            return Err(RecoveryRefusal::NotAllowed("Only the device that asked can close this request.".into()));
        }
        self.recovery_drop_ask(code)?;
        Ok(())
    }

    /// Close every ask that names `holder` (their own signed "that was not me"). Returns how
    /// many were open.
    pub fn recovery_close_all_for(&self, holder: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM recovery_answers WHERE code IN (SELECT code FROM recovery_asks WHERE holder_did = ?1)",
                params![holder],
            )?;
            let n = tx.execute("DELETE FROM recovery_asks WHERE holder_did = ?1", params![holder])?;
            tx.commit()?;
            Ok(n)
        })
    }

    fn recovery_drop_ask(&self, code: &str) -> Result<(), rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM recovery_answers WHERE code = ?1", params![code])?;
            tx.execute("DELETE FROM recovery_asks WHERE code = ?1", params![code])?;
            tx.commit()
        })
    }

    /// The expiry pass's part (storage/expiry.rs): asks that ended, with their answers, and
    /// setups never finished a day after they began. Returns the rows removed.
    pub fn recovery_expire(&self, now_ms: u64) -> Result<usize, rusqlite::Error> {
        let now = now_ms as i64;
        let stale = now - SETUP_MAX_AGE_MS as i64;
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let mut n = tx.execute(
                "DELETE FROM recovery_answers WHERE code IN (SELECT code FROM recovery_asks WHERE expires_at <= ?1)",
                params![now],
            )?;
            n += tx.execute("DELETE FROM recovery_asks WHERE expires_at <= ?1", params![now])?;
            n += tx.execute(
                "DELETE FROM recovery_pieces WHERE set_time < ?1 AND (holder_did, set_id) IN
                    (SELECT holder_did, set_id FROM recovery_pieces GROUP BY holder_did, set_id HAVING COUNT(*) < MAX(total))",
                params![stale],
            )?;
            tx.commit()?;
            Ok(n)
        })
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
