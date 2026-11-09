// Child of api.rs (#[path]). BUG-280 (2026-11-07): `GET /api/bugs` gave every report's reporter
// key and name, their browser's name and window size and the page they were on to anyone, signed in
// or not. Now anyone gets what was reported; who sent it and from where go to an admin's or a
// moderator's signed read alone.
//
// Seen red on 2026-11-07 against get_bugs before the fix (everything to everyone): the anonymous
// list carried "reporter_key", "reporter_name", "browser_info" and "page_url" ("nobody's key, name,
// browser or page" failed). The signed read is new.

use super::*;
use crate::relay::storage::Storage;
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

fn fresh_state() -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp("bug_reports")))
}

fn person(seed_byte: u8) -> (DilithiumKeypair, String) {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed_byte; 32]));
    let key = hex::encode(dil.public_key());
    (dil, key)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

fn public(st: &Arc<RelayState>) -> serde_json::Value {
    let q = BugsQuery { status: None, severity: None, category: None, limit: None, offset: None };
    let Json(v) = tokio::runtime::Runtime::new().unwrap().block_on(get_bugs(State(st.clone()), Query(q)));
    v
}

/// `POST /api/bugs/details` signed by `dil` in the name of `key`.
fn details(st: &Arc<RelayState>, dil: &DilithiumKeypair, key: &str) -> Result<serde_json::Value, StatusCode> {
    let ts = now_ms();
    let sig = hex::encode(dil.sign(format!("{BUG_DETAILS_PURPOSE}\n{ts}").as_bytes()));
    let req: BugDetailsRequest = serde_json::from_value(serde_json::json!({ "key": key, "timestamp": ts, "sig": sig, "limit": 10 })).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(bug_details(State(st.clone()), Json(req)))
        .map(|Json(v)| v)
        .map_err(|(code, _)| code)
}

const PRIVATE: [&str; 4] = ["reporter_key", "reporter_name", "browser_info", "page_url"];

fn file_one(st: &Arc<RelayState>) {
    st.db
        .create_bug("The map is blank", "Opened Maps, nothing drawn", "", "", "", "medium", "other",
            "reporter_key_hex", "Rowan", "Mozilla/5.0 (Linux; Android 14; Pixel 8) | 412x915",
            "https://united-humanity.us/maps?from=bugs", "0.1549.0")
        .unwrap();
}

#[test]
fn anyone_gets_the_report_but_not_who_sent_it_or_from_where() {
    let st = fresh_state();
    file_one(&st);
    let v = public(&st);
    let bug = &v["bugs"][0];
    assert_eq!(bug["title"], "The map is blank", "the report itself is public");
    for field in PRIVATE {
        assert!(bug.get(field).is_none(), "nobody's key, name, browser or page: {field} in {bug}");
    }
    assert_eq!(v["details"], false);
}

#[test]
fn an_admin_or_a_moderator_reads_the_details_with_a_signature() {
    let st = fresh_state();
    file_one(&st);
    let (ada, ada_key) = person(0x31);
    st.db.set_role(&ada_key, "admin").unwrap();
    let v = details(&st, &ada, &ada_key).expect("an admin's signed read");
    assert_eq!(v["details"], true);
    assert_eq!(v["bugs"][0]["reporter_name"], "Rowan");
    assert_eq!(v["bugs"][0]["browser_info"], "Mozilla/5.0 (Linux; Android 14; Pixel 8) | 412x915");

    let (mo, mo_key) = person(0x32);
    st.db.set_role(&mo_key, "mod").unwrap();
    assert_eq!(details(&st, &mo, &mo_key).unwrap()["details"], true, "a moderator too");

    // A member's signed read is refused, and so is a signature made by someone else.
    let (bea, bea_key) = person(0x33);
    assert_eq!(details(&st, &bea, &bea_key).unwrap_err(), StatusCode::FORBIDDEN, "a member sees no details");
    assert_eq!(details(&st, &bea, &ada_key).unwrap_err(), StatusCode::UNAUTHORIZED, "someone else's key, my signature");
}
