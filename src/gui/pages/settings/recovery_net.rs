//! The work behind Settings > Account > "Recovery through trusted people" (2026-10-21): every
//! call to the server and every lock and unlock, run on a thread by the section
//! (`recovery.rs`), never on the frame. The protocol is relay/core/recovery.rs, the server's
//! routes relay/api_v2_recovery.rs, the design docs/design/recovery-through-trusted-people.md.
//!
//! Each function takes what it needs by value (the server's web address, the 32 bytes the
//! recovery phrase writes down, names) and answers with words a person can read on failure.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine};

use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, derive_kyber_seed, DilithiumKeypair, KyberKeypair};
use crate::relay::core::recovery::{self as proto, OneTimeAsk, ServedPiece, TrustedPerson};

/// One friend who could keep a piece: their key, account id, name and Kyber768 key (base64)
/// when it is known.
#[derive(Debug, Clone, PartialEq)]
pub struct Friend {
    pub key: String,
    pub did: String,
    pub name: String,
    pub kyber_b64: Option<String>,
}

/// The person's own setup as this device reads it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MySetup {
    /// The standing setup: how many must help, and who keeps each piece (from the notes to
    /// self), in slot order. Empty when none stands.
    pub needed: usize,
    pub keepers: Vec<String>,
    /// A setup still arriving: (have, total).
    pub sending: Option<(usize, usize)>,
    /// When a request for help naming this account opened, if one is open.
    pub asked_since: Option<i64>,
}

/// Someone asking for help whose piece I keep.
#[derive(Debug, Clone, PartialEq)]
pub struct Asking {
    pub did: String,
    pub name: String,
    pub since: i64,
}

/// A code looked up, and my piece for its account, ready to send.
#[derive(Debug, Clone)]
pub struct CodeLookup {
    pub code: String,
    pub holder_did: String,
    pub name: String,
    pub one_time_key: Vec<u8>,
    pub slot: usize,
    pub share: [u8; proto::SHARE_LEN],
    pub token: [u8; proto::TOKEN_LEN],
    pub needed: usize,
}

/// An account found by name or id that has trusted people on this server.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub did: String,
    pub name: String,
    pub needed: usize,
    pub total: usize,
}

/// A request for help this device opened: kept in memory only.
pub struct LiveAsk {
    pub found: Found,
    pub code: String,
    pub one_time: OneTimeAsk,
}

/// What a look at the request found.
#[derive(Debug, Clone, PartialEq)]
pub enum Poll {
    /// Still waiting: this many pieces arrived of this many needed.
    Waiting { arrived: usize, needed: usize },
    /// Enough arrived, and they rebuild the account: its 32 bytes.
    Rebuilt([u8; proto::SECRET_LEN]),
    /// Enough arrived, but they do not rebuild this account.
    DoNotFit { arrived: usize },
    /// The request is no longer open (it ended or was closed).
    Ended,
}

// ── HTTP ──

const TIMEOUT: Duration = Duration::from_secs(10);

fn read(resp: ureq::Response) -> Result<(u16, serde_json::Value), String> {
    let code = resp.status();
    let text = resp.into_string().map_err(|e| format!("the server's answer could not be read ({e})"))?;
    Ok((code, serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)))
}

fn answer(r: Result<ureq::Response, ureq::Error>) -> Result<(u16, serde_json::Value), String> {
    match r {
        Ok(resp) => read(resp),
        Err(ureq::Error::Status(_, resp)) => read(resp),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    }
}

fn get(url: &str) -> Result<(u16, serde_json::Value), String> {
    answer(ureq::get(url).timeout(TIMEOUT).call())
}

fn post(url: &str, body: &serde_json::Value) -> Result<(u16, serde_json::Value), String> {
    answer(ureq::post(url).timeout(TIMEOUT).set("Content-Type", "application/json").send_string(&body.to_string()))
}

fn said(v: &serde_json::Value, code: u16) -> String {
    v.get("error").and_then(|e| e.as_str()).map(String::from).unwrap_or_else(|| format!("the server answered with an error ({code})"))
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// The keys the 32 bytes derive.
fn keys_of(secret: &[u8]) -> (DilithiumKeypair, KyberKeypair, String) {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(secret));
    let kyber = KyberKeypair::from_seed(&derive_kyber_seed(secret)).expect("a Kyber key from 64 bytes");
    let did = did_for_pubkey(&dil.public_key());
    (dil, kyber, did)
}

/// A request signed the way the relay's `check_signed` reads it: `purpose\ntimestamp`.
fn signed(secret: &[u8], purpose: &str) -> serde_json::Value {
    let (dil, _, _) = keys_of(secret);
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    serde_json::json!({
        "key": hex::encode(dil.public_key()),
        "timestamp": ts,
        "sig": crate::net::identity::pq_sign_chat(secret, purpose, ts),
    })
}

/// The standing setup of `did` as served, each piece checked. `None` when none stands.
fn standing(base: &str, did: &str) -> Result<Option<(usize, usize, Vec<ServedPiece>)>, String> {
    let (code, v) = get(&format!("{base}/api/v2/recovery/setup/{}", enc(did)))?;
    if code == 404 {
        return Ok(None);
    }
    if code != 200 {
        return Err(said(&v, code));
    }
    Ok(active_of(&v, did))
}

/// The standing setup in a setup answer: how many must help, how many pieces, and the pieces
/// whose signature verifies and whose signer is `did`.
fn active_of(v: &serde_json::Value, did: &str) -> Option<(usize, usize, Vec<ServedPiece>)> {
    let active = v.get("active").filter(|a| !a.is_null())?;
    let needed = active.get("needed").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let total = active.get("total").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let pieces = active
        .get("pieces")
        .and_then(|p| p.as_array())
        .map(|list| list.iter().filter_map(|p| p.get("object").and_then(|o| proto::served_piece(o).ok())).filter(|p| p.holder_did == did).collect())
        .unwrap_or_default();
    Some((needed, total, pieces))
}

/// The open requests for help: (account, opened at).
fn open_asks(base: &str) -> Result<Vec<(String, i64)>, String> {
    let (code, v) = get(&format!("{base}/api/v2/recovery/asks"))?;
    if code != 200 {
        return Err(said(&v, code));
    }
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| Some((x.get("holder_did")?.as_str()?.to_string(), x.get("opened_at")?.as_i64()?)))
                .collect()
        })
        .unwrap_or_default())
}

// ── Your trusted people ──

/// My setup: who keeps each piece (read from my notes to self), and whether someone asked for
/// help with my account.
pub fn load_my_setup(base: &str, secret: &[u8]) -> Result<MySetup, String> {
    let (_, kyber, did) = keys_of(secret);
    let mut out = MySetup::default();
    let (code, v) = get(&format!("{base}/api/v2/recovery/setup/{}", enc(&did)))?;
    if code == 200 {
        if let Some(s) = v.get("sending").filter(|s| !s.is_null()) {
            out.sending = Some((
                s.get("have").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                s.get("total").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
            ));
        }
        if let Some((needed, _, pieces)) = active_of(&v, &did) {
            out.needed = needed;
            out.keepers = pieces
                .iter()
                .map(|p| match proto::open_note(&kyber, p) {
                    Some(n) => format!("{} ({})", n.name, n.did),
                    None => "someone (this piece's note could not be read)".to_string(),
                })
                .collect();
        }
    } else if code != 404 {
        return Err(said(&v, code));
    }
    out.asked_since = open_asks(base)?.into_iter().filter(|(d, _)| *d == did).map(|(_, t)| t).min();
    Ok(out)
}

/// Fill in each chosen friend's Kyber768 key from the server when this device does not have it.
pub fn with_keys(base: &str, mut people: Vec<Friend>) -> Result<Vec<TrustedPerson>, String> {
    let mut out = Vec::new();
    for f in people.iter_mut() {
        if f.kyber_b64.is_none() {
            let (code, v) = get(&format!("{base}/api/members/{}", enc(&f.key)))?;
            if code == 200 {
                f.kyber_b64 = v.get("kyber_public").and_then(|k| k.as_str()).filter(|k| !k.is_empty()).map(String::from);
            }
        }
        let kyber = f
            .kyber_b64
            .as_deref()
            .and_then(|k| B64.decode(k.trim()).ok())
            .ok_or_else(|| format!("{}'s key is not known yet: it arrives once they have been online.", f.name))?;
        out.push(TrustedPerson { did: f.did.clone(), name: f.name.clone(), kyber_public: kyber });
    }
    Ok(out)
}

/// Lock the pieces and send them: one signed object per trusted person.
pub fn send_setup(base: &str, secret: &[u8], people: Vec<Friend>, needed: usize) -> Result<String, String> {
    let names: Vec<String> = people.iter().map(|p| p.name.clone()).collect();
    let trusted = with_keys(base, people)?;
    let (dil, kyber, _) = keys_of(secret);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let (_, objects) = proto::build_setup(secret, &dil, &kyber.public_key(), &trusted, needed, now)?;
    for (i, o) in objects.iter().enumerate() {
        let (code, v) = post(&format!("{base}/api/v2/objects"), &proto::object_json(o))?;
        if code != 200 {
            return Err(format!(
                "piece {} of {} was not taken ({}). Your earlier setup, if you had one, still stands.",
                i + 1,
                objects.len(),
                said(&v, code)
            ));
        }
    }
    Ok(format!("Done. {} each keep a piece now. Tell them, so they know what it is if you ever call.", list(&names)))
}

/// Take every piece off the server.
pub fn withdraw(base: &str, secret: &[u8]) -> Result<String, String> {
    let (code, v) = post(&format!("{base}/api/v2/recovery/withdraw"), &signed(secret, proto::WITHDRAW_PURPOSE))?;
    if code != 200 {
        return Err(said(&v, code));
    }
    Ok("Taken back: the server no longer keeps any piece of your recovery phrase.".into())
}

/// Close every request for help that names my account ("that was not me").
pub fn not_me(base: &str, secret: &[u8]) -> Result<String, String> {
    let (code, v) = post(&format!("{base}/api/v2/recovery/not-me"), &signed(secret, proto::NOT_ME_PURPOSE))?;
    if code != 200 {
        return Err(said(&v, code));
    }
    Ok("Stopped. Every request for help with your account on this server is closed.".into())
}

// ── Help someone get back in ──

/// My piece of `did`'s standing setup, opened, with the setup's `needed`.
fn my_piece(base: &str, kyber: &KyberKeypair, did: &str) -> Result<Option<(ServedPiece, proto::OpenedPiece, usize)>, String> {
    let Some((needed, _, pieces)) = standing(base, did)? else { return Ok(None) };
    Ok(pieces.into_iter().find_map(|p| proto::open_piece(kyber, &p).map(|o| (p, o, needed))))
}

/// Everyone asking for help whose piece I keep, named from `names` (account id -> name).
pub fn who_is_asking(base: &str, secret: &[u8], names: &HashMap<String, String>) -> Result<Vec<Asking>, String> {
    let (_, kyber, me) = keys_of(secret);
    let mut out: Vec<Asking> = Vec::new();
    for (did, since) in open_asks(base)? {
        if did == me || out.iter().any(|a| a.did == did) {
            continue;
        }
        if my_piece(base, &kyber, &did)?.is_some() {
            out.push(Asking { name: names.get(&did).cloned().unwrap_or_else(|| did.clone()), did, since });
        }
    }
    Ok(out)
}

/// Look up the code a person read me: the request, a check that its key is the one the code
/// names, and my piece for its account.
pub fn look_up_code(base: &str, secret: &[u8], typed: &str, names: &HashMap<String, String>) -> Result<CodeLookup, String> {
    let code = proto::normalize_code(typed).ok_or("That is not a code: it is 16 letters and numbers, like ABCD-EFGH-JKMN-PQRS.")?;
    let (status, v) = get(&format!("{base}/api/v2/recovery/ask/{code}"))?;
    if status == 404 {
        return Err("There is no open request for help with that code on this server. Check the code with them.".into());
    }
    if status != 200 {
        return Err(said(&v, status));
    }
    let holder = v.get("holder_did").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let key = v.get("one_time_key_b64").and_then(|x| x.as_str()).and_then(|k| B64.decode(k).ok()).unwrap_or_default();
    if proto::ask_code(&holder, &key) != code {
        return Err("This request does not match the code they read you, so nothing can be sent. Tell them, and do not send anything for this code.".into());
    }
    let (_, kyber, _) = keys_of(secret);
    let (piece, opened, needed) =
        my_piece(base, &kyber, &holder)?.ok_or("You do not keep a piece for the account that made this request, so there is nothing to send.")?;
    Ok(CodeLookup {
        code,
        name: names.get(&holder).cloned().unwrap_or_else(|| holder.clone()),
        holder_did: holder,
        one_time_key: key,
        slot: piece.payload.slot,
        share: opened.share,
        token: opened.token,
        needed,
    })
}

/// Send my piece to the device that asked, sealed to its one-time key.
pub fn send_piece(base: &str, l: &CodeLookup) -> Result<String, String> {
    let sealed = proto::seal_answer(&l.one_time_key, &l.code, &l.holder_did, l.slot, &l.share)?;
    let body = serde_json::json!({
        "code": l.code,
        "slot": l.slot,
        "token_b64": B64.encode(l.token),
        "sealed_b64": B64.encode(&sealed),
    });
    let (code, v) = post(&format!("{base}/api/v2/recovery/answer"), &body)?;
    if code != 200 {
        return Err(said(&v, code));
    }
    let n = v.get("answered").and_then(|x| x.as_u64()).unwrap_or(0);
    Ok(format!("Sent to {}'s new device. It needs {} pieces in all; {n} have arrived.", l.name, l.needed))
}

/// Which of these friends (account id, name) I keep a piece for.
pub fn pieces_i_keep(base: &str, secret: &[u8], friends: &[(String, String)]) -> Result<Vec<String>, String> {
    let (_, kyber, _) = keys_of(secret);
    let mut out = Vec::new();
    for (did, name) in friends {
        if my_piece(base, &kyber, did)?.is_some() {
            out.push(name.clone());
        }
    }
    Ok(out)
}

// ── Lost your device? ──

/// Find an account by the name it used here, or its account id, among those with trusted
/// people on this server.
pub fn find_account(base: &str, typed: &str) -> Result<Vec<Found>, String> {
    let typed = typed.trim();
    let mut candidates: Vec<(String, String)> = Vec::new();
    if proto::account_id_ok(typed) {
        candidates.push((typed.to_string(), typed.to_string()));
    } else if !typed.is_empty() {
        let (code, v) = get(&format!("{base}/api/members?search={}&limit=20", enc(typed)))?;
        if code != 200 {
            return Err(said(&v, code));
        }
        for m in v.get("members").and_then(|x| x.as_array()).into_iter().flatten() {
            let (Some(name), Some(key)) = (m.get("name").and_then(|x| x.as_str()), m.get("public_key").and_then(|x| x.as_str())) else { continue };
            if !name.eq_ignore_ascii_case(typed) {
                continue;
            }
            if let Ok(bytes) = hex::decode(key) {
                candidates.push((did_for_pubkey(&bytes), name.to_string()));
            }
        }
        if candidates.is_empty() {
            return Err(format!("No one named \"{typed}\" is listed on this server. Check the spelling, or type your account ID."));
        }
    }
    let mut out = Vec::new();
    for (did, name) in candidates {
        if let Some((needed, total, _)) = standing(base, &did)? {
            out.push(Found { did, name, needed, total });
        }
    }
    if out.is_empty() {
        return Err("This account has no trusted people on this server, so there is no one to ask here.".into());
    }
    Ok(out)
}

/// Open a request for help for `found` with a fresh one-time key.
pub fn open_ask(base: &str, found: Found) -> Result<LiveAsk, String> {
    let one_time = OneTimeAsk::new()?;
    let body = serde_json::json!({
        "holder_did": found.did,
        "one_time_key_b64": B64.encode(&one_time.public),
        "close_hash": proto::close_hash(&one_time.close_secret),
    });
    let (status, v) = post(&format!("{base}/api/v2/recovery/ask"), &body)?;
    if status != 200 {
        return Err(said(&v, status));
    }
    let code = proto::ask_code(&found.did, &one_time.public);
    if v.get("code").and_then(|c| c.as_str()) != Some(code.as_str()) {
        return Err("The server filed this request under another code, so it was not used.".into());
    }
    Ok(LiveAsk { found, code, one_time })
}

/// Look at the request: the pieces that arrived, opened, and the account rebuilt once enough
/// fit together.
pub fn poll_ask(base: &str, live: &Arc<LiveAsk>) -> Result<Poll, String> {
    let (status, v) = get(&format!("{base}/api/v2/recovery/ask/{}", live.code))?;
    if status == 404 {
        return Ok(Poll::Ended);
    }
    if status != 200 {
        return Err(said(&v, status));
    }
    let needed = v.get("needed").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let shares: Vec<[u8; proto::SHARE_LEN]> = v
        .get("answers")
        .and_then(|a| a.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let slot = a.get("slot")?.as_u64()? as usize;
                    let sealed = B64.decode(a.get("sealed_b64")?.as_str()?).ok()?;
                    proto::open_answer(&live.one_time.keys, &live.code, &live.found.did, slot, &sealed)
                })
                .collect()
        })
        .unwrap_or_default();
    if needed >= proto::MIN_NEEDED && shares.len() >= needed {
        return Ok(match proto::rebuild(&shares, needed, &live.found.did) {
            Some(secret) => Poll::Rebuilt(secret),
            None => Poll::DoNotFit { arrived: shares.len() },
        });
    }
    Ok(Poll::Waiting { arrived: shares.len(), needed })
}

/// Close the request (with the secret only this device has). Errors are not worth showing: an
/// open request ends by itself.
pub fn close_ask(base: &str, live: &Arc<LiveAsk>) {
    let body = serde_json::json!({ "code": live.code, "close_secret_b64": B64.encode(live.one_time.close_secret) });
    let _ = post(&format!("{base}/api/v2/recovery/close"), &body);
}

/// "A, B and C".
pub fn list(names: &[String]) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        n => format!("{} and {}", names[..n - 1].join(", "), names[n - 1]),
    }
}
