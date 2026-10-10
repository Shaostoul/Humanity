//! Friendship passes at the relay (v2, 2026-10-09, docs/design/blocking-and-safe-mode.md 10b).
//!
//! Two jobs, both small:
//!
//! - [`friend_pass`]: the one check every contact path makes (dm_put, trade_request,
//!   voice_call, a dc_offer, a profile request). It rebuilds the pass's signed words from the
//!   relay's OWN facts (its did:hum, the person being reached as issuer, the sender's signed-in
//!   socket key as grantee), so a pass given on another server, by someone else or to someone
//!   else does not count, and then asks the withdrawal list whether the issuer has taken it back.
//! - [`handle_cert_revoke`]: `{"type":"cert_revoke","serial":"<32 lowercase hex>"}` from the
//!   issuer's own signed-in socket withdraws the pass with that serial. The issuer is the socket
//!   key, so no further signature is needed and nobody can withdraw a pass they did not give.
//!   The answer, `cert_revoked {serial}`, goes to the issuer's own devices, which stop resending
//!   it; a withdrawal sent while offline waits in the client until one is answered.
//!
//! Until step B ("who can reach me") ships, a pass only does what v1's certificate did: a valid
//! one lifts the stranger's daily budget on DMs and trade requests and keeps a trade note whole.
//! voice_call and dc_offer read it and refuse nothing new.

use std::sync::Arc;

use crate::relay::core::pq_crypto::{friend_cert_serial_ok, verify_friend_cert, FriendPass};
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::FriendCertWithdrawal;

/// The pass `cert` that `issuer` (the person being reached) gave `grantee` (the sender: always
/// the signed-in socket's key, never a key the message names), if it is good here and not
/// withdrawn. None for no pass, a pass that does not check out, or one its issuer took back.
pub fn friend_pass(state: &Arc<RelayState>, issuer: &str, grantee: &str, cert: Option<&str>) -> Option<FriendPass> {
    let cert = cert.filter(|c| !c.is_empty())?;
    let server = state.db.server_did().ok()?;
    let pass = verify_friend_cert(&server, issuer, grantee, cert).ok()?;
    if state.db.friend_cert_withdrawn(issuer, &pass.serial) {
        return None;
    }
    Some(pass)
}

/// For tests: mint the pass `issuer_hex` (whose BIP39 seed is `issuer_seed`) gives `grantee` on
/// THIS relay, with a fresh serial and the defaults two new friends get. Returns (pass, serial).
#[cfg(test)]
pub(crate) fn test_pass(state: &Arc<RelayState>, issuer_seed: &[u8], issuer_hex: &str, grantee: &str) -> (String, String) {
    use crate::relay::core::pq_crypto::{build_friend_cert, new_friend_cert_serial, FRIEND_PASS_DEFAULT_MAY};
    let server = state.db.server_did().expect("the relay's own did:hum");
    let serial = new_friend_cert_serial().expect("randomness");
    let pass = build_friend_cert(issuer_seed, &server, issuer_hex, grantee, &serial, &FRIEND_PASS_DEFAULT_MAY).expect("a pass");
    (pass, serial)
}

/// Today, in Unix days: all a withdrawal row keeps of when it was made.
fn today() -> i64 {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() / 86_400) as i64
}

/// `cert_revoke {serial}`: the signed-in `my_key` takes back the pass it gave with `serial`.
pub async fn handle_cert_revoke(state: &Arc<RelayState>, my_key: &str, raw: &serde_json::Value) {
    let tell = |message: &str| {
        let _ = state.broadcast_tx.send(RelayMessage::Private { to: my_key.to_string(), message: message.to_string() });
    };
    let serial = raw.get("serial").and_then(|v| v.as_str()).unwrap_or("");
    if !friend_cert_serial_ok(serial) {
        tell("Withdrawal refused: that is not a friendship pass serial.");
        return;
    }
    // A bot has no seed, so it never gave anyone a pass.
    if my_key.starts_with("bot_") {
        return;
    }
    match state.db.withdraw_friend_cert(my_key, serial, today()) {
        Ok(FriendCertWithdrawal::Recorded | FriendCertWithdrawal::AlreadyWithdrawn) => {
            let _ = state.broadcast_tx.send(RelayMessage::CertRevoked { to: my_key.to_string(), serial: serial.to_string() });
        }
        Ok(FriendCertWithdrawal::TooMany) => {
            tell("Withdrawal refused: this server already keeps the most friendship withdrawals it holds for one account.");
        }
        Err(e) => tracing::error!("friend passes: could not record a withdrawal: {e}"),
    }
}
