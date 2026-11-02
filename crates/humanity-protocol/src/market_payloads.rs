//! Payload validators for the Market's signed-object kinds (v0.1140):
//! `provider_v1` (schemas/provider.toml) and `offering_v1`
//! (schemas/offering.toml). The schemas are the source of truth; this module
//! is their mechanical enforcement at the storage chokepoint
//! (`put_signed_object`), which covers BOTH ingest paths -- REST submission
//! and federation gossip -- so no invalid market object can enter any
//! server's directory from any direction.
//!
//! Design notes, mirrored from the schemas:
//! - Settlement `mode` may ONLY be "directory" today; `wallet` and `escrow`
//!   are reserved and MUST be rejected, as must the reserved settlement
//!   payload keys, so their meaning stays fixed for the increment that
//!   ships them (the money-transmitter firewall).
//! - `updated_at` more than 5 minutes in the future is rejected, or one bad
//!   clock pins a stale offering at the top of the Market forever.
//! - Category MEMBERSHIP (data/market/categories.json) is checked when the
//!   caller supplies the loaded set; format is always checked. The file is
//!   a federation-shared vocabulary, not a schema constant.
//! - item_ref existence in data/items.csv is NOT checked here: relays do
//!   not ship the game data tree. Clients and curators enforce taxonomy;
//!   the relay enforces shape.

use ciborium::Value;
use std::collections::HashSet;

// ── CBOR access helpers ─────────────────────────────────────────────────────

fn as_map(v: &Value) -> Result<&Vec<(Value, Value)>, String> {
    match v {
        Value::Map(m) => Ok(m),
        _ => Err("payload is not a CBOR map".to_string()),
    }
}

fn get<'a>(m: &'a [(Value, Value)], key: &str) -> Option<&'a Value> {
    m.iter().find_map(|(k, v)| match k {
        Value::Text(t) if t == key => Some(v),
        _ => None,
    })
}

fn text<'a>(m: &'a [(Value, Value)], key: &str) -> Result<Option<&'a str>, String> {
    match get(m, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(t)) => Ok(Some(t.as_str())),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn req_text<'a>(m: &'a [(Value, Value)], key: &str) -> Result<&'a str, String> {
    text(m, key)?.ok_or_else(|| format!("{key} is required"))
}

fn integer(m: &[(Value, Value)], key: &str) -> Result<Option<i128>, String> {
    match get(m, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Integer(i)) => Ok(Some(i128::from(*i))),
        Some(_) => Err(format!("{key} must be an integer")),
    }
}

fn float(m: &[(Value, Value)], key: &str) -> Result<Option<f64>, String> {
    match get(m, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Float(f)) => Ok(Some(*f)),
        Some(Value::Integer(i)) => Ok(Some(i128::from(*i) as f64)),
        Some(_) => Err(format!("{key} must be a number")),
    }
}

fn array<'a>(m: &'a [(Value, Value)], key: &str) -> Result<Option<&'a Vec<Value>>, String> {
    match get(m, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) => Ok(Some(a)),
        Some(_) => Err(format!("{key} must be an array")),
    }
}

fn table<'a>(m: &'a [(Value, Value)], key: &str) -> Result<Option<&'a Vec<(Value, Value)>>, String> {
    match get(m, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Map(t)) => Ok(Some(t)),
        Some(_) => Err(format!("{key} must be a table")),
    }
}

fn text_len(m: &[(Value, Value)], key: &str, min: usize, max: usize) -> Result<Option<()>, String> {
    if let Some(t) = text(m, key)? {
        let n = t.trim().chars().count();
        if n < min || n > max {
            return Err(format!("{key} must be {min} to {max} characters (got {n})"));
        }
        return Ok(Some(()));
    }
    Ok(None)
}

fn str_array(
    m: &[(Value, Value)],
    key: &str,
    max_entries: usize,
    each_min: usize,
    each_max: usize,
) -> Result<Vec<String>, String> {
    let Some(a) = array(m, key)? else { return Ok(Vec::new()) };
    if a.len() > max_entries {
        return Err(format!("{key} has {} entries (max {max_entries})", a.len()));
    }
    let mut out = Vec::with_capacity(a.len());
    for v in a {
        match v {
            Value::Text(t) => {
                let n = t.chars().count();
                if n < each_min || n > each_max {
                    return Err(format!("{key} entries must be {each_min} to {each_max} characters"));
                }
                out.push(t.clone());
            }
            _ => return Err(format!("{key} entries must be strings")),
        }
    }
    Ok(out)
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn is_key_charset(s: &str, min: usize, max: usize) -> bool {
    let n = s.chars().count();
    n >= min
        && n <= max
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

fn is_https(s: &str) -> bool {
    s.starts_with("https://")
}

fn enum_check(value: &str, key: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("{key} \"{value}\" is not one of {allowed:?}"))
    }
}

/// The federation-shared category vocabulary, loaded once from
/// `data/market/categories.json` relative to the working directory (the VPS
/// relay runs from /opt/Humanity; the native host node from the app dir,
/// which ships data/). Fail-OPEN: a missing or unparsable file returns None
/// and category MEMBERSHIP is not enforced (the snake_case shape rule still
/// is), so a stripped-down node never rejects valid goods over a lost file.
pub fn shared_categories() -> Option<&'static HashSet<String>> {
    static CATS: std::sync::OnceLock<Option<HashSet<String>>> = std::sync::OnceLock::new();
    CATS.get_or_init(|| {
        let text = std::fs::read_to_string("data/market/categories.json").ok()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        let set: HashSet<String> = v
            .get("categories")?
            .as_array()?
            .iter()
            .filter_map(|c| c.get("id").and_then(|i| i.as_str()).map(str::to_string))
            .collect();
        (!set.is_empty()).then_some(set)
    })
    .as_ref()
}

fn now_ms() -> i128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i128
}

/// updated_at: > 0, and no more than 5 minutes in the future.
fn check_updated_at(m: &[(Value, Value)]) -> Result<i128, String> {
    let ts = integer(m, "updated_at")?.ok_or("updated_at is required")?;
    if ts <= 0 {
        return Err("updated_at must be > 0".to_string());
    }
    if ts > now_ms() + 300_000 {
        return Err("updated_at is more than 5 minutes in the future".to_string());
    }
    Ok(ts)
}

/// One location/service-area entry (the shared shape).
fn check_location(loc: &[(Value, Value)], ctx: &str) -> Result<(), String> {
    let mode = req_text(loc, "mode")?;
    enum_check(mode, &format!("{ctx}.mode"), LOCATION_MODES)?;
    let label = req_text(loc, "label")?;
    let n = label.trim().chars().count();
    if n < 1 || n > 120 {
        return Err(format!("{ctx}.label must be 1 to 120 characters"));
    }
    let lat = float(loc, "lat")?;
    let lon = float(loc, "lon")?;
    if let Some(la) = lat {
        if !(-90.0..=90.0).contains(&la) {
            return Err(format!("{ctx}.lat out of range"));
        }
    }
    if let Some(lo) = lon {
        if !(-180.0..=180.0).contains(&lo) {
            return Err(format!("{ctx}.lon out of range"));
        }
    }
    match mode {
        "radius" => {
            let r = float(loc, "radius_km")?.ok_or(format!("{ctx}.radius_km required for radius mode"))?;
            if !(r > 0.0 && r <= 20000.0) {
                return Err(format!("{ctx}.radius_km must be > 0 and <= 20000"));
            }
            if lat.is_none() || lon.is_none() {
                return Err(format!("{ctx}: radius mode requires lat and lon"));
            }
        }
        "region" => {
            let rc = req_text(loc, "region_code")?;
            let ok = rc.len() >= 2
                && rc.len() <= 6
                && rc.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-');
            if !ok {
                return Err(format!("{ctx}.region_code must be an uppercase ISO 3166 code"));
            }
        }
        _ => {}
    }
    if let Some(t) = text(loc, "locality")? {
        if t.chars().count() > 120 {
            return Err(format!("{ctx}.locality max 120 characters"));
        }
    }
    Ok(())
}

// ── Shared enum vocabularies (single source: the validator AND the in-app
//    publish form read these, so a new enum value reaches both at once) ──────

pub const PROVIDER_KINDS: &[&str] = &[
    "individual",
    "household",
    "mutual_aid",
    "cooperative",
    "nonprofit",
    "public_institution",
    "faith_community",
    "business",
];
pub const CONTACT_CHANNELS: &[&str] = &["in_app_dm", "email", "phone", "website", "in_person"];
pub const LOCATION_MODES: &[&str] = &["fixed_location", "radius", "region", "remote"];
pub const PRICE_MODES: &[&str] = &[
    "free",
    "fixed",
    "range",
    "sliding_scale",
    "pay_what_you_can",
    "donation",
    "trade",
    "inquire",
];
pub const PRICE_UNITS: &[&str] = &[
    "each", "kg", "g", "liter", "meter", "dozen", "box", "hour", "day", "week", "month",
    "session", "visit", "job", "person", "lb", "oz", "ton", "gallon", "quart", "foot",
    "yard", "sq_ft", "sq_meter", "acre", "cord", "bushel", "bale",
];
pub const FULFILLMENT_MODES: &[&str] = &[
    "pickup_at_provider",
    "delivery_local",
    "shipping",
    "mail_in",
    "at_recipient_location",
    "remote",
    "digital_delivery",
];
pub const SETTLEMENT_CONTACTS: &[&str] =
    &["in_app_dm", "email", "phone", "website", "in_person", "provider_checkout"];
pub const GOOD_CONDITIONS: &[&str] =
    &["new", "like_new", "used_good", "used_fair", "refurbished", "for_parts"];
pub const GOOD_AVAILABILITY: &[&str] =
    &["in_stock", "one_off", "made_to_order", "unlimited", "while_supplies_last"];
pub const SERVICE_ACTIONS: &[&str] = &[
    "repair", "maintain", "install", "build", "make_to_order", "process", "transport",
    "store", "lend", "instruct", "consult", "care", "host", "other",
];
pub const SCHEDULE_KINDS: &[&str] =
    &["walk_in", "by_appointment", "recurring", "on_call", "waitlist", "one_time_event"];
pub const WEEKDAYS: &[&str] =
    &["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

// ── provider_v1 ─────────────────────────────────────────────────────────────

pub fn validate_provider_v1(payload: &[u8]) -> Result<(), String> {
    let value: Value =
        ciborium::from_reader(payload).map_err(|e| format!("payload is not valid CBOR: {e}"))?;
    let m = as_map(&value)?;

    if let Some(pr) = text(m, "provider_ref")? {
        if !is_hex64(pr) {
            return Err("provider_ref must be 64 lowercase hex characters".to_string());
        }
    }
    text_len(m, "display_name", 1, 80)?.ok_or("display_name is required")?;
    if let Some(slug) = text(m, "slug")? {
        let ok = slug.len() >= 3
            && slug.len() <= 40
            && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && !slug.starts_with(|c: char| c.is_ascii_digit());
        if !ok {
            return Err("slug must be snake_case, 3 to 40 characters, not starting with a digit".to_string());
        }
    }
    let kind = req_text(m, "kind")?;
    enum_check(kind, "kind", PROVIDER_KINDS)?;
    text_len(m, "description", 1, 600)?.ok_or("description is required")?;
    let backed_by = req_text(m, "backed_by")?;
    enum_check(backed_by, "backed_by", &["identity", "group"])?;
    match text(m, "group_ref")? {
        Some(g) => {
            if !is_hex64(g) {
                return Err("group_ref must be 64 lowercase hex characters".to_string());
            }
        }
        None => {
            if backed_by == "group" {
                return Err("group_ref is required when backed_by is \"group\"".to_string());
            }
        }
    }
    if let Some(did) = text(m, "identity_did")? {
        if !did.starts_with("did:hum:") {
            return Err("identity_did must start with did:hum:".to_string());
        }
    }
    let status = req_text(m, "status")?;
    enum_check(status, "status", &["active", "paused", "closed"])?;
    check_updated_at(m)?;
    for lang in str_array(m, "languages", 12, 2, 2)? {
        if !lang.chars().all(|c| c.is_ascii_lowercase()) {
            return Err("languages entries must be lowercase ISO 639-1 codes".to_string());
        }
    }
    if let Some(uri) = text(m, "logo_uri")? {
        if !is_https(uri) || uri.len() > 512 {
            return Err("logo_uri must be an https:// URI up to 512 characters".to_string());
        }
    }
    // Contact: required table with a required preferred channel.
    let contact = table(m, "contact")?.ok_or("contact is required")?;
    let preferred = req_text(contact, "preferred")?;
    enum_check(preferred, "contact.preferred", CONTACT_CHANNELS)?;
    if let Some(w) = text(contact, "website")? {
        if !is_https(w) || w.len() > 512 {
            return Err("contact.website must be an https:// URI up to 512 characters".to_string());
        }
    }
    // Service areas: required, 1+, each the shared location shape.
    let areas = array(m, "service_areas")?.ok_or("service_areas is required")?;
    if areas.is_empty() || areas.len() > 32 {
        return Err("service_areas must have 1 to 32 entries".to_string());
    }
    for (i, a) in areas.iter().enumerate() {
        match a {
            Value::Map(am) => check_location(am, &format!("service_areas[{i}]"))?,
            _ => return Err("service_areas entries must be tables".to_string()),
        }
    }
    // Verification: optional table; when present, claimed_status is bounded.
    if let Some(ver) = table(m, "verification")? {
        let cs = req_text(ver, "claimed_status")?;
        enum_check(
            cs,
            "verification.claimed_status",
            &["unverified", "self_attested", "domain_proven", "manually_verified", "revoked"],
        )?;
    }
    Ok(())
}

// ── offering_v1 ─────────────────────────────────────────────────────────────

/// `references` is the ENVELOPE references list; the payload's provider_ref
/// must appear there so gossip and dedup see the dependency without decoding.
/// `categories`: the loaded ids from data/market/categories.json when the
/// relay has them (membership check), None for format-only.
pub fn validate_offering_v1(
    payload: &[u8],
    references: &[String],
    categories: Option<&HashSet<String>>,
) -> Result<(), String> {
    let value: Value =
        ciborium::from_reader(payload).map_err(|e| format!("payload is not valid CBOR: {e}"))?;
    let m = as_map(&value)?;

    let provider_ref = req_text(m, "provider_ref")?;
    if !is_hex64(provider_ref) {
        return Err("provider_ref must be 64 lowercase hex characters".to_string());
    }
    if !references.iter().any(|r| r == provider_ref) {
        return Err("envelope references must contain the provider_ref".to_string());
    }
    let offering_key = req_text(m, "offering_key")?;
    if !is_key_charset(offering_key, 1, 64) {
        return Err("offering_key must be 1 to 64 characters from [a-z0-9._-]".to_string());
    }
    let kind = req_text(m, "kind")?;
    enum_check(kind, "kind", &["good", "service"])?;
    let reality = req_text(m, "reality")?;
    enum_check(reality, "reality", &["real", "sim"])?;
    text_len(m, "title", 1, 80)?.ok_or("title is required")?;
    text_len(m, "description", 1, 2000)?.ok_or("description is required")?;

    let item_ref = text(m, "item_ref")?;
    let proposed = table(m, "proposed_item")?;
    if item_ref.is_some() && proposed.is_some() {
        return Err("item_ref and proposed_item are mutually exclusive".to_string());
    }
    if let Some(ir) = item_ref {
        if !is_key_charset(ir, 1, 64) || ir.contains('.') || ir.contains('-') {
            return Err("item_ref must be snake_case".to_string());
        }
    }
    if let Some(p) = proposed {
        let pid = req_text(p, "id")?;
        let ok = pid.len() >= 3
            && pid.len() <= 48
            && pid.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !ok {
            return Err("proposed_item.id must be snake_case, 3 to 48 characters".to_string());
        }
        text_len(p, "name", 1, 40)?.ok_or("proposed_item.name is required")?;
    }

    let category = req_text(m, "category")?;
    let cat_ok = !category.is_empty()
        && category.len() <= 48
        && category.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !cat_ok {
        return Err("category must be lowercase snake_case".to_string());
    }
    if let Some(cats) = categories {
        if !cats.contains(category) {
            return Err(format!(
                "category \"{category}\" is not in data/market/categories.json (the federation-shared vocabulary)"
            ));
        }
    }

    for t in str_array(m, "tags", 12, 1, 32)? {
        if !t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-')) {
            return Err("tags must be lowercase [a-z0-9_-]".to_string());
        }
    }
    for img in str_array(m, "images", 8, 1, 512)? {
        if !is_https(&img) {
            return Err("images entries must be https:// URIs".to_string());
        }
    }
    for r in str_array(m, "restrictions", 8, 1, 64)? {
        enum_check(
            &r,
            "restrictions",
            &[
                "age_18_plus",
                "age_21_plus",
                "id_required",
                "license_required",
                "prescription_required",
                "membership_required",
                "local_pickup_only",
                "hazardous_material",
                "perishable",
            ],
        )?;
    }
    let status = req_text(m, "status")?;
    enum_check(status, "status", &["active", "paused", "sold_out", "withdrawn"])?;

    let updated_at = check_updated_at(m)?;
    if let Some(ttl) = integer(m, "ttl_days")? {
        if !(1..=365).contains(&ttl) {
            return Err("ttl_days must be 1 to 365".to_string());
        }
    }
    if let Some(exp) = integer(m, "expires_at")? {
        if exp <= updated_at {
            return Err("expires_at must be after updated_at".to_string());
        }
        // A farther value than +365d is clamped by readers, never honoured;
        // accept it here (the schema says clamp, not reject).
    }
    if let Some(lang) = text(m, "language")? {
        if lang.len() != 2 || !lang.chars().all(|c| c.is_ascii_lowercase()) {
            return Err("language must be a lowercase ISO 639-1 code".to_string());
        }
    }

    let fulfillment = str_array(m, "fulfillment", 6, 1, 32)?;
    if fulfillment.is_empty() {
        return Err("fulfillment requires 1 to 6 entries".to_string());
    }
    for f in &fulfillment {
        enum_check(f, "fulfillment", FULFILLMENT_MODES)?;
    }

    check_price(m, reality)?;
    check_settlement(m)?;

    if let Some(loc) = table(m, "location")? {
        check_location(loc, "location")?;
    }

    // Branch discipline: the matching branch is required, the other must be
    // absent -- a service has no stock and a good has no calendar.
    let good = table(m, "good")?;
    let service = table(m, "service")?;
    match kind {
        "good" => {
            if service.is_some() {
                return Err("kind \"good\" must not carry a service table".to_string());
            }
            check_good(good.ok_or("kind \"good\" requires the good table")?)?;
        }
        "service" => {
            if good.is_some() {
                return Err("kind \"service\" must not carry a good table".to_string());
            }
            check_service(service.ok_or("kind \"service\" requires the service table")?)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn check_price(m: &[(Value, Value)], reality: &str) -> Result<(), String> {
    let price = table(m, "price")?.ok_or("price is required")?;
    let mode = req_text(price, "mode")?;
    enum_check(mode, "price.mode", PRICE_MODES)?;
    let amount = float(price, "amount")?;
    let amount_max = float(price, "amount_max")?;
    match mode {
        "free" | "trade" | "donation" | "inquire" => {
            if amount.is_some() {
                return Err(format!("price.amount must be absent when mode is {mode}"));
            }
        }
        "fixed" | "range" => {
            let a = amount.ok_or(format!("price.amount required for mode {mode}"))?;
            if a < 0.0 {
                return Err("price.amount must be >= 0".to_string());
            }
            if mode == "range" {
                let mx = amount_max.ok_or("price.amount_max required for mode range")?;
                if mx <= a {
                    return Err("price.amount_max must be > price.amount".to_string());
                }
            }
        }
        _ => {} // sliding_scale, pay_what_you_can: amounts optional
    }
    if amount.is_some() {
        let cur = text(price, "currency")?.ok_or("price.currency required when amount is present")?;
        let cur_ok = cur.len() >= 2
            && cur.len() <= 8
            && cur.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if !cur_ok {
            return Err("price.currency must be an uppercase code".to_string());
        }
        // Sim offerings price in CR only; real offerings never in CR.
        if reality == "sim" && cur != "CR" {
            return Err("sim offerings must price in CR".to_string());
        }
        if reality == "real" && cur == "CR" {
            return Err("real offerings cannot price in CR".to_string());
        }
    }
    if let Some(u) = text(price, "unit")? {
        enum_check(u, "price.unit", PRICE_UNITS)?;
    }
    str_array(price, "accepts", 12, 1, 48)?;
    if let Some(n) = text(price, "notes")? {
        if n.chars().count() > 300 {
            return Err("price.notes max 300 characters".to_string());
        }
    }
    Ok(())
}

fn check_settlement(m: &[(Value, Value)]) -> Result<(), String> {
    let s = table(m, "settlement")?.ok_or("settlement is required")?;
    let mode = req_text(s, "mode")?;
    // wallet and escrow are RESERVED: reject until those addons ship.
    if mode != "directory" {
        return Err(format!(
            "settlement.mode \"{mode}\" is not available: only \"directory\" is active (wallet/escrow are reserved)"
        ));
    }
    // The reserved keys must not appear at all.
    for reserved in ["wallet_address", "wallet_chain", "escrow_policy_ref", "dispute_policy_ref"] {
        if get(s, reserved).is_some() {
            return Err(format!("settlement.{reserved} is a reserved field and must not appear"));
        }
    }
    let via = req_text(s, "contact_via")?;
    enum_check(via, "settlement.contact_via", SETTLEMENT_CONTACTS)?;
    match text(s, "checkout_uri")? {
        Some(uri) => {
            if !is_https(uri) || uri.len() > 512 {
                return Err("settlement.checkout_uri must be an https:// URI up to 512 characters".to_string());
            }
        }
        None => {
            if via == "provider_checkout" {
                return Err("settlement.checkout_uri required when contact_via is provider_checkout".to_string());
            }
        }
    }
    if let Some(i) = text(s, "instructions")? {
        if i.chars().count() > 500 {
            return Err("settlement.instructions max 500 characters".to_string());
        }
    }
    Ok(())
}

fn check_good(g: &[(Value, Value)]) -> Result<(), String> {
    let condition = req_text(g, "condition")?;
    enum_check(condition, "good.condition", GOOD_CONDITIONS)?;
    for p in str_array(g, "provenance", 4, 1, 32)? {
        enum_check(
            &p,
            "good.provenance",
            &["handmade", "homegrown", "home_processed", "salvaged", "second_hand", "surplus", "factory_new"],
        )?;
    }
    let avail = req_text(g, "availability_mode")?;
    enum_check(avail, "good.availability_mode", GOOD_AVAILABILITY)?;
    let qty = integer(g, "quantity_available")?;
    match avail {
        "in_stock" => {
            let q = qty.ok_or("good.quantity_available required for in_stock")?;
            if q < 0 {
                return Err("good.quantity_available must be >= 0".to_string());
            }
        }
        "one_off" => {
            if qty != Some(1) {
                return Err("good.quantity_available must be exactly 1 for one_off".to_string());
            }
        }
        "unlimited" | "while_supplies_last" => {
            if qty.is_some() {
                return Err(format!("good.quantity_available must be absent for {avail}"));
            }
        }
        "made_to_order" => {
            let lead = integer(g, "lead_time_days")?
                .ok_or("good.lead_time_days required for made_to_order")?;
            if lead < 0 {
                return Err("good.lead_time_days must be >= 0".to_string());
            }
        }
        _ => unreachable!(),
    }
    if let Some(u) = text(g, "unit")? {
        enum_check(u, "good.unit", &["each", "kg", "g", "liter", "meter", "dozen", "box"])?;
    }
    if let Some(minq) = integer(g, "min_quantity")? {
        if minq < 1 {
            return Err("good.min_quantity must be >= 1".to_string());
        }
        if let Some(q) = qty {
            if minq > q {
                return Err("good.min_quantity must be <= quantity_available".to_string());
            }
        }
    }
    if let Some(variants) = array(g, "variants")? {
        if variants.len() > 100 {
            return Err("good.variants max 100 entries".to_string());
        }
        let mut seen = HashSet::new();
        for v in variants {
            let vm = match v {
                Value::Map(vm) => vm,
                _ => return Err("good.variants entries must be tables".to_string()),
            };
            let key = req_text(vm, "variant_key")?;
            if !is_key_charset(key, 1, 48) {
                return Err("variant_key must be 1 to 48 characters from [a-z0-9._-]".to_string());
            }
            if !seen.insert(key.to_string()) {
                return Err(format!("variant_key \"{key}\" appears twice"));
            }
            text_len(vm, "label", 1, 60)?.ok_or("variant label is required")?;
        }
    }
    if let Some(ship) = table(g, "shipping")? {
        let ships_to = str_array(ship, "ships_to", 32, 1, 12)?;
        if ships_to.is_empty() {
            return Err("good.shipping.ships_to requires 1 to 32 entries".to_string());
        }
    }
    Ok(())
}

fn check_service(s: &[(Value, Value)]) -> Result<(), String> {
    let action = req_text(s, "action")?;
    enum_check(action, "service.action", SERVICE_ACTIONS)?;
    if let Some(d) = integer(s, "duration_minutes")? {
        if d <= 0 {
            return Err("service.duration_minutes must be > 0".to_string());
        }
    }
    let avail = table(s, "availability")?.ok_or("service.availability is required")?;
    let sk = req_text(avail, "schedule_kind")?;
    enum_check(sk, "service.availability.schedule_kind", SCHEDULE_KINDS)?;
    let recurring = array(avail, "recurring")?;
    if matches!(sk, "recurring" | "walk_in") && recurring.map_or(true, |r| r.is_empty()) {
        return Err(format!("service.availability.recurring required for {sk}"));
    }
    if let Some(rec) = recurring {
        if rec.len() > 21 {
            return Err("service.availability.recurring max 21 entries".to_string());
        }
        for r in rec {
            let rm = match r {
                Value::Map(rm) => rm,
                _ => return Err("recurring entries must be tables".to_string()),
            };
            let day = req_text(rm, "day")?;
            enum_check(day, "recurring.day", WEEKDAYS)?;
            let start = req_text(rm, "start_time")?;
            let end = req_text(rm, "end_time")?;
            let hhmm = |t: &str| {
                t.len() == 5
                    && t.as_bytes()[2] == b':'
                    && t[..2].parse::<u8>().map_or(false, |h| h <= 23)
                    && t[3..].parse::<u8>().map_or(false, |m| m <= 59)
            };
            if !hhmm(start) || !hhmm(end) {
                return Err("recurring times must be HH:MM".to_string());
            }
            if end <= start {
                return Err("recurring end_time must be later than start_time".to_string());
            }
        }
    }
    if sk == "one_time_event" && integer(avail, "starts_at")?.is_none() {
        return Err("service.availability.starts_at required for one_time_event".to_string());
    }
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cbor(v: ciborium::Value) -> Vec<u8> {
        let mut out = Vec::new();
        ciborium::into_writer(&v, &mut out).unwrap();
        out
    }

    fn t(s: &str) -> Value {
        Value::Text(s.to_string())
    }
    fn kv(k: &str, v: Value) -> (Value, Value) {
        (t(k), v)
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    fn provider_payload() -> Vec<(Value, Value)> {
        vec![
            kv("display_name", t("Rivertown Bikes")),
            kv("kind", t("business")),
            kv("description", t("Bicycle sales and repair on Main Street.")),
            kv("backed_by", t("identity")),
            kv("status", t("active")),
            kv("updated_at", Value::Integer(now().into())),
            kv("contact", Value::Map(vec![kv("preferred", t("in_person"))])),
            kv(
                "service_areas",
                Value::Array(vec![Value::Map(vec![
                    kv("mode", t("fixed_location")),
                    kv("label", t("12 Main St, Rivertown")),
                ])]),
            ),
        ]
    }

    fn offering_payload() -> Vec<(Value, Value)> {
        vec![
            kv("provider_ref", t(&"a".repeat(64))),
            kv("offering_key", t("flashlight_800l")),
            kv("kind", t("good")),
            kv("reality", t("real")),
            kv("title", t("LED Flashlight 800L")),
            kv("description", t("Aluminium body, 800 lumen, USB-C rechargeable.")),
            kv("category", t("tools")),
            kv("status", t("active")),
            kv("updated_at", Value::Integer(now().into())),
            kv("fulfillment", Value::Array(vec![t("pickup_at_provider")])),
            kv(
                "price",
                Value::Map(vec![
                    kv("mode", t("fixed")),
                    kv("amount", Value::Float(24.99)),
                    kv("currency", t("USD")),
                ]),
            ),
            kv(
                "settlement",
                Value::Map(vec![kv("mode", t("directory")), kv("contact_via", t("in_person"))]),
            ),
            kv(
                "good",
                Value::Map(vec![
                    kv("condition", t("new")),
                    kv("availability_mode", t("in_stock")),
                    kv("quantity_available", Value::Integer(12.into())),
                ]),
            ),
        ]
    }

    fn refs() -> Vec<String> {
        vec!["a".repeat(64)]
    }

    #[test]
    fn a_valid_provider_and_offering_pass() {
        assert_eq!(validate_provider_v1(&cbor(Value::Map(provider_payload()))), Ok(()));
        assert_eq!(
            validate_offering_v1(&cbor(Value::Map(offering_payload())), &refs(), None),
            Ok(())
        );
    }

    #[test]
    fn reserved_settlement_modes_and_fields_are_rejected() {
        let mut p = offering_payload();
        p.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "settlement"));
        p.push(kv(
            "settlement",
            Value::Map(vec![kv("mode", t("wallet")), kv("contact_via", t("in_app_dm"))]),
        ));
        let err = validate_offering_v1(&cbor(Value::Map(p)), &refs(), None).unwrap_err();
        assert!(err.contains("reserved"), "wallet mode must be rejected: {err}");

        let mut p2 = offering_payload();
        p2.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "settlement"));
        p2.push(kv(
            "settlement",
            Value::Map(vec![
                kv("mode", t("directory")),
                kv("contact_via", t("in_app_dm")),
                kv("wallet_address", t("somewhere")),
            ]),
        ));
        let err2 = validate_offering_v1(&cbor(Value::Map(p2)), &refs(), None).unwrap_err();
        assert!(err2.contains("reserved field"), "reserved keys must be rejected: {err2}");
    }

    #[test]
    fn envelope_must_reference_the_provider_and_branches_stay_disciplined() {
        let err = validate_offering_v1(&cbor(Value::Map(offering_payload())), &[], None).unwrap_err();
        assert!(err.contains("references"), "missing envelope reference: {err}");

        // A good carrying a service table is two shapes in one object.
        let mut p = offering_payload();
        p.push(kv("service", Value::Map(vec![kv("action", t("repair"))])));
        let err2 = validate_offering_v1(&cbor(Value::Map(p)), &refs(), None).unwrap_err();
        assert!(err2.contains("must not carry"), "{err2}");
    }

    #[test]
    fn price_and_clock_rules_hold() {
        // free + amount is a contradiction.
        let mut p = offering_payload();
        p.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "price"));
        p.push(kv(
            "price",
            Value::Map(vec![kv("mode", t("free")), kv("amount", Value::Float(1.0))]),
        ));
        assert!(validate_offering_v1(&cbor(Value::Map(p)), &refs(), None).is_err());

        // sim reality must price in CR.
        let mut p2 = offering_payload();
        p2.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "reality"));
        p2.push(kv("reality", t("sim")));
        let err = validate_offering_v1(&cbor(Value::Map(p2)), &refs(), None).unwrap_err();
        assert!(err.contains("CR"), "{err}");

        // A far-future clock is rejected.
        let mut p3 = offering_payload();
        p3.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "updated_at"));
        p3.push(kv("updated_at", Value::Integer((now() + 600_000).into())));
        let err3 = validate_offering_v1(&cbor(Value::Map(p3)), &refs(), None).unwrap_err();
        assert!(err3.contains("future"), "{err3}");
    }

    #[test]
    fn category_membership_checked_when_the_vocabulary_is_loaded() {
        let cats: HashSet<String> = ["tools", "food"].iter().map(|s| s.to_string()).collect();
        assert!(validate_offering_v1(&cbor(Value::Map(offering_payload())), &refs(), Some(&cats)).is_ok());
        let mut p = offering_payload();
        p.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "category"));
        p.push(kv("category", t("weapons")));
        let err = validate_offering_v1(&cbor(Value::Map(p)), &refs(), Some(&cats)).unwrap_err();
        assert!(err.contains("vocabulary"), "{err}");
    }

    #[test]
    fn provider_group_backing_requires_group_ref_and_quantity_rules_hold() {
        let mut p = provider_payload();
        p.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "backed_by"));
        p.push(kv("backed_by", t("group")));
        let err = validate_provider_v1(&cbor(Value::Map(p))).unwrap_err();
        assert!(err.contains("group_ref"), "{err}");

        // while_supplies_last must NOT carry a count (the food-pantry rule).
        let mut o = offering_payload();
        o.retain(|(k, _)| !matches!(k, Value::Text(t) if t == "good"));
        o.push(kv(
            "good",
            Value::Map(vec![
                kv("condition", t("new")),
                kv("availability_mode", t("while_supplies_last")),
                kv("quantity_available", Value::Integer(5.into())),
            ]),
        ));
        let err2 = validate_offering_v1(&cbor(Value::Map(o)), &refs(), None).unwrap_err();
        assert!(err2.contains("absent"), "{err2}");
    }
}
