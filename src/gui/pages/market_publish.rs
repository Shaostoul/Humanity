//! In-app Market publishing (v0.1143): create or update your shop
//! (provider_v1) and publish offerings (offering_v1) without touching a
//! terminal. GUI-first rule: the importer (scripts/import-offerings.mjs)
//! remains the bulk path; this is the one-at-a-time human path. Same signed
//! objects, same validators, same identity: the merchant IS the chat
//! identity, so a shop created here is the same shop a nightly import
//! updates.
//!
//! Flow: form fields -> canonical-CBOR payload -> validated LOCALLY with the
//! exact relay validators (instant plain-language errors, nothing invalid
//! ever leaves the machine) -> ObjectBuilder signs with the Dilithium3
//! keypair derived from the seed -> worker-thread POST to /api/v2/objects.
//! Enum dropdowns read the SAME pub consts the validator enforces
//! (market_payloads), so a vocabulary change reaches both at once.

use egui::{RichText, ScrollArea};
use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver};

use crate::gui::i18n::{t, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::Hint;
use crate::gui::GuiState;
use crate::relay::core::market_payloads as vocab;

use super::market_directory::DirProvider;

// ── Page-local state ────────────────────────────────────────────────────────

#[derive(Debug)]
struct PublishOk {
    object_id: String,
    was_provider: bool,
}

struct PublishState {
    open: bool,
    // Shop (provider) form.
    p_display_name: String,
    p_kind_idx: usize,
    p_description: String,
    p_contact_idx: usize,
    p_website: String,
    p_area_mode_idx: usize,
    p_area_label: String,
    p_region_code: String,
    // Offering form.
    o_key: String,
    o_is_good: bool,
    o_title: String,
    o_description: String,
    o_category_id: String,
    o_price_mode_idx: usize,
    o_amount: String,
    o_amount_max: String,
    o_currency: String,
    o_unit_idx: usize, // 0 = no unit; i>0 -> PRICE_UNITS[i-1]
    o_fulfillment: Vec<bool>,
    o_contact_via_idx: usize,
    o_checkout_uri: String,
    o_instructions: String,
    // Good branch.
    g_condition_idx: usize,
    g_avail_idx: usize,
    g_quantity: String,
    g_lead_days: String,
    // Service branch.
    s_action_idx: usize,
    s_schedule_idx: usize,
    s_days: Vec<bool>,
    s_start: String,
    s_end: String,
    // Machinery.
    rx: Option<Receiver<Result<PublishOk, String>>>,
    status: String,
    error: String,
    /// Set after a successful publish; the directory consumes it to refetch.
    published: bool,
    /// Provider root created THIS session (before the refetch shows it).
    created_provider_root: Option<String>,
    prefilled_for: Option<String>,
}

impl Default for PublishState {
    fn default() -> Self {
        Self {
            open: false,
            p_display_name: String::new(),
            p_kind_idx: 0,
            p_description: String::new(),
            p_contact_idx: 0,
            p_website: String::new(),
            p_area_mode_idx: 3, // remote: the only mode with no extra fields
            p_area_label: String::new(),
            p_region_code: String::new(),
            o_key: String::new(),
            o_is_good: true,
            o_title: String::new(),
            o_description: String::new(),
            o_category_id: String::new(),
            o_price_mode_idx: 0, // free
            o_amount: String::new(),
            o_amount_max: String::new(),
            o_currency: "USD".to_string(),
            o_unit_idx: 0,
            o_fulfillment: vec![false; vocab::FULFILLMENT_MODES.len()],
            o_contact_via_idx: 0, // in_app_dm
            o_checkout_uri: String::new(),
            o_instructions: String::new(),
            g_condition_idx: 0,
            g_avail_idx: 0,
            g_quantity: "1".to_string(),
            g_lead_days: "7".to_string(),
            s_action_idx: 0,
            s_schedule_idx: 1, // by_appointment: valid with no recurring rows
            s_days: vec![false; 7],
            s_start: "09:00".to_string(),
            s_end: "17:00".to_string(),
            rx: None,
            status: String::new(),
            error: String::new(),
            published: false,
            created_provider_root: None,
            prefilled_for: None,
        }
    }
}

fn with_pub<R>(f: impl FnOnce(&mut PublishState) -> R) -> R {
    thread_local! {
        static STATE: RefCell<PublishState> = RefCell::new(PublishState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn is_open() -> bool {
    with_pub(|ps| ps.open)
}

/// Open the publish view, prefilling the shop form from the caller's existing
/// provider row (so "update shop" starts from what is live). Prefill happens
/// once per provider root; reopening keeps in-progress edits.
pub fn open(my_provider: Option<&DirProvider>) {
    with_pub(|ps| {
        ps.open = true;
        ps.error.clear();
        ps.status.clear();
        if let Some(p) = my_provider {
            if ps.prefilled_for.as_deref() != Some(p.root_id.as_str()) {
                ps.prefilled_for = Some(p.root_id.clone());
                ps.p_display_name = p.display_name.clone();
                ps.p_description = p.description.clone();
                ps.p_kind_idx = vocab::PROVIDER_KINDS
                    .iter()
                    .position(|k| *k == p.kind)
                    .unwrap_or(0);
                ps.p_contact_idx = vocab::CONTACT_CHANNELS
                    .iter()
                    .position(|c| *c == p.contact_preferred)
                    .unwrap_or(0);
                ps.p_website = p.website.clone().unwrap_or_default();
            }
        }
    });
}

/// True once after a successful publish: the directory refetches on it.
pub fn take_published() -> bool {
    with_pub(|ps| std::mem::take(&mut ps.published))
}

// ── Payload construction (pure; unit-tested below) ──────────────────────────

type CborMap = Vec<(ciborium::Value, ciborium::Value)>;

fn txt(s: &str) -> ciborium::Value {
    ciborium::Value::Text(s.to_string())
}
fn kv(k: &str, v: ciborium::Value) -> (ciborium::Value, ciborium::Value) {
    (txt(k), v)
}
fn int(n: i64) -> ciborium::Value {
    ciborium::Value::Integer(n.into())
}

/// The provider_v1 payload from the form. `provider_root` = Some(root) makes
/// this an UPDATE revision of an existing shop.
fn provider_payload(ps: &PublishState, provider_root: Option<&str>, now: i64) -> ciborium::Value {
    let mut m: CborMap = Vec::new();
    if let Some(root) = provider_root {
        m.push(kv("provider_ref", txt(root)));
    }
    m.push(kv("display_name", txt(ps.p_display_name.trim())));
    m.push(kv("kind", txt(vocab::PROVIDER_KINDS[ps.p_kind_idx])));
    m.push(kv("description", txt(ps.p_description.trim())));
    m.push(kv("backed_by", txt("identity")));
    m.push(kv("status", txt("active")));
    m.push(kv("updated_at", int(now)));
    let mut contact: CborMap =
        vec![kv("preferred", txt(vocab::CONTACT_CHANNELS[ps.p_contact_idx]))];
    if !ps.p_website.trim().is_empty() {
        contact.push(kv("website", txt(ps.p_website.trim())));
    }
    m.push(kv("contact", ciborium::Value::Map(contact)));
    let mode = vocab::LOCATION_MODES[ps.p_area_mode_idx];
    let mut area: CborMap = vec![
        kv("mode", txt(mode)),
        kv("label", txt(ps.p_area_label.trim())),
    ];
    if mode == "region" {
        area.push(kv("region_code", txt(ps.p_region_code.trim())));
    }
    m.push(kv(
        "service_areas",
        ciborium::Value::Array(vec![ciborium::Value::Map(area)]),
    ));
    ciborium::Value::Map(m)
}

/// The offering_v1 payload from the form.
fn offering_payload(
    ps: &PublishState,
    provider_root: &str,
    reality: &str,
    now: i64,
) -> ciborium::Value {
    let mut m: CborMap = vec![
        kv("provider_ref", txt(provider_root)),
        kv("offering_key", txt(ps.o_key.trim())),
        kv("kind", txt(if ps.o_is_good { "good" } else { "service" })),
        kv("reality", txt(reality)),
        kv("title", txt(ps.o_title.trim())),
        kv("category", txt(ps.o_category_id.as_str())),
        kv("status", txt("active")),
        kv("updated_at", int(now)),
    ];
    m.push(kv("description", txt(ps.o_description.trim())));
    let fulfillment: Vec<ciborium::Value> = vocab::FULFILLMENT_MODES
        .iter()
        .zip(&ps.o_fulfillment)
        .filter(|(_, on)| **on)
        .map(|(f, _)| txt(f))
        .collect();
    m.push(kv("fulfillment", ciborium::Value::Array(fulfillment)));

    // Price. Amount fields only ride along when the mode uses them, so a
    // mode switch can never leave a stale amount that the validator rejects.
    let mode = vocab::PRICE_MODES[ps.o_price_mode_idx];
    let mut price: CborMap = vec![kv("mode", txt(mode))];
    let wants_amount = matches!(mode, "fixed" | "range" | "sliding_scale" | "pay_what_you_can");
    let amount: Option<f64> = ps.o_amount.trim().parse().ok().filter(|_| wants_amount);
    if let Some(a) = amount {
        price.push(kv("amount", ciborium::Value::Float(a)));
        if matches!(mode, "range" | "sliding_scale") {
            if let Ok(mx) = ps.o_amount_max.trim().parse::<f64>() {
                price.push(kv("amount_max", ciborium::Value::Float(mx)));
            }
        }
        price.push(kv("currency", txt(ps.o_currency.trim())));
        if ps.o_unit_idx > 0 {
            price.push(kv("unit", txt(vocab::PRICE_UNITS[ps.o_unit_idx - 1])));
        }
    }
    m.push(kv("price", ciborium::Value::Map(price)));

    // Settlement: directory-only, always (the money firewall).
    let via = vocab::SETTLEMENT_CONTACTS[ps.o_contact_via_idx];
    let mut settlement: CborMap = vec![kv("mode", txt("directory")), kv("contact_via", txt(via))];
    if via == "provider_checkout" || !ps.o_checkout_uri.trim().is_empty() {
        if !ps.o_checkout_uri.trim().is_empty() {
            settlement.push(kv("checkout_uri", txt(ps.o_checkout_uri.trim())));
        }
    }
    if !ps.o_instructions.trim().is_empty() {
        settlement.push(kv("instructions", txt(ps.o_instructions.trim())));
    }
    m.push(kv("settlement", ciborium::Value::Map(settlement)));

    if ps.o_is_good {
        let avail = vocab::GOOD_AVAILABILITY[ps.g_avail_idx];
        let mut good: CborMap = vec![
            kv("condition", txt(vocab::GOOD_CONDITIONS[ps.g_condition_idx])),
            kv("availability_mode", txt(avail)),
        ];
        match avail {
            "in_stock" => {
                good.push(kv("quantity_available", int(ps.g_quantity.trim().parse().unwrap_or(0))));
            }
            "one_off" => good.push(kv("quantity_available", int(1))),
            "made_to_order" => {
                good.push(kv("lead_time_days", int(ps.g_lead_days.trim().parse().unwrap_or(0))));
            }
            _ => {}
        }
        m.push(kv("good", ciborium::Value::Map(good)));
    } else {
        let sk = vocab::SCHEDULE_KINDS[ps.s_schedule_idx];
        let mut avail: CborMap = vec![kv("schedule_kind", txt(sk))];
        if matches!(sk, "walk_in" | "recurring") {
            let rows: Vec<ciborium::Value> = vocab::WEEKDAYS
                .iter()
                .zip(&ps.s_days)
                .filter(|(_, on)| **on)
                .map(|(day, _)| {
                    ciborium::Value::Map(vec![
                        kv("day", txt(day)),
                        kv("start_time", txt(ps.s_start.trim())),
                        kv("end_time", txt(ps.s_end.trim())),
                    ])
                })
                .collect();
            avail.push(kv("recurring", ciborium::Value::Array(rows)));
        }
        let service: CborMap = vec![
            kv("action", txt(vocab::SERVICE_ACTIONS[ps.s_action_idx])),
            kv("availability", ciborium::Value::Map(avail)),
        ];
        m.push(kv("service", ciborium::Value::Map(service)));
    }
    ciborium::Value::Map(m)
}

// ── Sign + submit (worker thread) ───────────────────────────────────────────

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Step F's recovery-phrase guard on a shop or offering about to be signed and sent: true (and
/// the form says why, keeping what was typed) when its text holds the phrase. The payload's
/// debug text keeps each field's words together, which is all the guard's test needs.
fn phrase_stops(state: &GuiState, payload: &ciborium::Value) -> bool {
    let stops = crate::engine::warnings::holds_own_phrase(state, &[&format!("{payload:?}")]);
    if stops {
        with_pub(|ps| ps.error = crate::net::warnings::GUARD_LINE.to_string());
    }
    stops
}

/// Validate locally, sign, POST. Runs entirely off the UI thread: Dilithium
/// keygen + sign is tens of milliseconds, the POST is the network.
fn spawn_publish(
    base: String,
    seed: Vec<u8>,
    payload: ciborium::Value,
    object_type: &'static str,
    references: Vec<String>,
    was_provider: bool,
) -> Receiver<Result<PublishOk, String>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(sign_and_submit(
            &base,
            &seed,
            &payload,
            object_type,
            &references,
            was_provider,
        ));
    });
    rx
}

/// The whole publish pipeline: local validation, envelope signing,
/// submission. Pure of UI state so the end-to-end test drives the EXACT
/// path the button does.
pub(crate) fn sign_and_submit(
    base: &str,
    seed: &[u8],
    payload: &ciborium::Value,
    object_type: &str,
    references: &[String],
    was_provider: bool,
) -> Result<PublishOk, String> {
    // The payload is serialized DIRECTLY, not through the envelope's
    // canonical encoder (which prohibits floats): the envelope signs
    // and hashes the payload as OPAQUE BYTES, so these bytes only
    // need to be valid CBOR for the validators and readers. Prices
    // are floats, exactly as the importer publishes them.
    let mut canon = Vec::new();
    ciborium::into_writer(payload, &mut canon)
        .map_err(|e| format!("encode: {e}"))?;
    // The EXACT relay-side validators, run before anything leaves
    // this machine: same rules, same plain-language messages.
    if was_provider {
        crate::relay::core::market_payloads::validate_provider_v1(&canon)?;
    } else {
        crate::relay::core::market_payloads::validate_offering_v1(
            &canon,
            references,
            crate::relay::core::market_payloads::shared_categories(),
        )?;
    }
    let keypair = crate::net::identity::pq_object_keypair(seed);
    let mut builder = crate::relay::core::object::ObjectBuilder::new(object_type)
        .created_at(now_ms() as u64)
        .payload_raw(canon);
    for r in references {
        builder = builder.reference(r);
    }
    let object = builder.sign(&keypair).map_err(|e| format!("sign: {e}"))?;
    let object_id = object.object_id().map_err(|e| format!("id: {e}"))?.to_hex();

    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let submission = serde_json::json!({
        "protocol_version": object.protocol_version,
        "object_type": object.object_type,
        "author_public_key_b64": b64.encode(&object.author_public_key),
        "created_at": object.created_at,
        "references": object.references,
        "payload_schema_version": object.payload_schema_version,
        "payload_encoding": object.payload_encoding,
        "payload_b64": b64.encode(&object.payload),
        "signature_b64": b64.encode(&object.signature),
    });
    let resp = ureq::post(&format!("{base}/api/v2/objects"))
        .timeout(std::time::Duration::from_secs(15))
        // axum's Json extractor answers 415 without an explicit JSON content
        // type (ureq::send_string alone sends text/plain; the end-to-end
        // test caught this).
        .set("Content-Type", "application/json")
        .send_string(&submission.to_string());
    match resp {
        Ok(r) => {
            let body = r.into_string().unwrap_or_default();
            let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let id = v
                .get("object_id")
                .and_then(|x| x.as_str())
                .unwrap_or(&object_id)
                .to_string();
            Ok(PublishOk { object_id: id, was_provider })
        }
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| {
                    v.get("error").or_else(|| v.get("message")).and_then(|e| e.as_str()).map(String::from)
                })
                .unwrap_or(body);
            Err(tf("market.publish.server_said_no", &[("code", &code), ("message", &msg)]))
        }
        Err(e) => Err(tf("market.publish.network_error", &[("error", &e)])),
    }
}

// ── Draw ────────────────────────────────────────────────────────────────────

/// A labelled dropdown over one vocabulary list. `hint` is the key of the
/// plain-language hint, shown on the dropdown and on each of its choices.
fn combo(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: &str,
    label: &str,
    hint: &'static str,
    options: &[&str],
    idx: &mut usize,
) {
    widgets::form_row(ui, theme, label, |ui| {
        egui::ComboBox::from_id_salt(id)
            .selected_text(options.get(*idx).copied().unwrap_or("?").replace('_', " "))
            .show_ui(ui, |ui| {
                for (i, opt) in options.iter().enumerate() {
                    if ui.selectable_label(*idx == i, opt.replace('_', " ")).hint(hint).clicked() {
                        *idx = i;
                    }
                }
            })
            .response
            .hint(hint);
    });
}

/// The publish view body. `my_provider` is the caller's provider row from the
/// directory (None until one is published + fetched).
pub fn draw(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &GuiState,
    base: &str,
    my_provider: Option<&DirProvider>,
) {
    // Drain the worker.
    with_pub(|ps| {
        if let Some(rx) = &ps.rx {
            match rx.try_recv() {
                Ok(Ok(ok)) => {
                    ps.rx = None;
                    ps.published = true;
                    if ok.was_provider {
                        ps.created_provider_root = Some(ok.object_id.clone());
                        ps.status = t("market.publish.shop_published").to_string();
                    } else {
                        ps.status = tf("market.publish.offering_published", &[("key", &ps.o_key.trim())]);
                    }
                    ps.error.clear();
                }
                Ok(Err(e)) => {
                    ps.rx = None;
                    ps.error = e;
                    ps.status.clear();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => ps.rx = None,
            }
        }
    });

    // The root the offering form publishes into: the live directory row
    // beats the just-created id (refetch replaces the latter).
    let provider_root: Option<String> = my_provider
        .map(|p| p.root_id.clone())
        .or_else(|| with_pub(|ps| ps.created_provider_root.clone()));
    let have_identity = state.private_key_bytes.is_some();
    let busy = with_pub(|ps| ps.rx.is_some());
    let reality = if state.nav_top_category == "sim" { "sim" } else { "real" };

    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        if widgets::Button::secondary(t("market.directory.back"))
            .hint("market.publish.back_hint")
            .show(ui, theme)
        {
            with_pub(|ps| ps.open = false);
        }
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new(tf("market.publish.title", &[("server", &base)]))
                .size(theme.font_size_title)
                .color(theme.text_primary()),
        );
        ui.label(
            RichText::new(t("market.publish.intro"))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        if !have_identity {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new(t("market.publish.needs_identity"))
                    .color(theme.warning()),
            );
            return;
        }
        let (status, error) = with_pub(|ps| (ps.status.clone(), ps.error.clone()));
        if !status.is_empty() {
            ui.label(RichText::new(status).color(theme.success()));
        }
        if !error.is_empty() {
            ui.label(RichText::new(error).color(theme.warning()));
        }
        if busy {
            ui.label(RichText::new(t("market.publishing")).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_sm);

        // ── Shop ──
        let shop_title = if my_provider.is_some() {
            t("market.publish.your_shop")
        } else {
            t("market.publish.create_shop")
        };
        widgets::card_with_header(ui, theme, shop_title, |ui| {
            with_pub(|ps| {
                widgets::form_row(ui, theme, t("market.publish.name"), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut ps.p_display_name).desired_width(280.0))
                        .hint("market.publish.name_hint");
                });
                combo(
                    ui,
                    theme,
                    "pub_p_kind",
                    t("market.publish.kind"),
                    "market.publish.kind_hint",
                    vocab::PROVIDER_KINDS,
                    &mut ps.p_kind_idx,
                );
                widgets::form_row(ui, theme, t("market.description"), |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut ps.p_description)
                            .desired_width(280.0)
                            .desired_rows(2),
                    )
                    .hint("market.publish.shop_description_hint");
                });
                combo(
                    ui,
                    theme,
                    "pub_p_contact",
                    t("market.publish.contact"),
                    "market.publish.contact_hint",
                    vocab::CONTACT_CHANNELS,
                    &mut ps.p_contact_idx,
                );
                widgets::form_row(ui, theme, t("market.publish.website"), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut ps.p_website)
                            .hint_text(t("market.publish.website_placeholder"))
                            .desired_width(280.0),
                    )
                    .hint("market.publish.website_hint");
                });
                combo(
                    ui,
                    theme,
                    "pub_p_area",
                    t("market.publish.service_area"),
                    "market.publish.service_area_hint",
                    vocab::LOCATION_MODES,
                    &mut ps.p_area_mode_idx,
                );
                widgets::form_row(ui, theme, t("market.publish.area_label"), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut ps.p_area_label)
                            .hint_text(t("market.publish.area_label_placeholder"))
                            .desired_width(280.0),
                    )
                    .hint("market.publish.area_label_hint");
                });
                if vocab::LOCATION_MODES[ps.p_area_mode_idx] == "region" {
                    widgets::form_row(ui, theme, t("market.publish.region_code"), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut ps.p_region_code)
                                .hint_text("US-WA") // i18n-exempt: a region code sample, the same in every language
                                .desired_width(100.0),
                        )
                        .hint("market.publish.region_code_hint");
                    });
                }
            });
            ui.add_space(theme.spacing_sm);
            let (label, hint) = if my_provider.is_some() {
                (t("market.publish.update_shop"), "market.publish.update_shop_hint")
            } else {
                (t("market.publish.publish_shop"), "market.publish.publish_shop_hint")
            };
            if !busy && widgets::Button::primary(label).hint(hint).show(ui, theme) {
                let root = my_provider.map(|p| p.root_id.clone());
                let (payload, seed) = with_pub(|ps| {
                    (
                        provider_payload(ps, root.as_deref(), now_ms()),
                        state.private_key_bytes.clone().unwrap_or_default(),
                    )
                });
                if phrase_stops(state, &payload) {
                    return;
                }
                let rx = spawn_publish(
                    base.to_string(),
                    seed,
                    payload,
                    "provider_v1",
                    root.into_iter().collect(),
                    true,
                );
                with_pub(|ps| {
                    ps.rx = Some(rx);
                    ps.error.clear();
                    ps.status.clear();
                });
            }
        });
        ui.add_space(theme.spacing_sm);

        // ── Offering ──
        widgets::card_with_header(ui, theme, t("market.publish.offering_heading"), |ui| {
            let Some(root) = provider_root.clone() else {
                ui.label(
                    RichText::new(t("market.publish.shop_first"))
                        .color(theme.text_muted()),
                );
                return;
            };
            with_pub(|ps| {
                widgets::form_row(ui, theme, t("market.publish.sku"), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut ps.o_key)
                            .hint_text(t("market.publish.sku_placeholder"))
                            .desired_width(200.0),
                    )
                    .hint("market.publish.sku_hint");
                });
                widgets::form_row(ui, theme, t("market.publish.type"), |ui| {
                    if ui
                        .selectable_label(ps.o_is_good, t("market.publish.good"))
                        .hint("market.publish.good_hint")
                        .clicked()
                    {
                        ps.o_is_good = true;
                    }
                    if ui
                        .selectable_label(!ps.o_is_good, t("market.publish.service"))
                        .hint("market.publish.service_hint")
                        .clicked()
                    {
                        ps.o_is_good = false;
                    }
                });
                widgets::form_row(ui, theme, t("market.form_title"), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut ps.o_title).desired_width(280.0))
                        .hint("market.publish.offering_title_hint");
                });
                widgets::form_row(ui, theme, t("market.description"), |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut ps.o_description)
                            .desired_width(280.0)
                            .desired_rows(2),
                    )
                    .hint("market.publish.offering_description_hint");
                });
                widgets::form_row(ui, theme, t("market.form_category"), |ui| {
                    let selected = state
                        .market_categories
                        .iter()
                        .find(|c| c.id == ps.o_category_id)
                        .map(|c| c.label.as_str())
                        .unwrap_or(t("market.select"));
                    egui::ComboBox::from_id_salt("pub_o_cat")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for c in &state.market_categories {
                                if ui
                                    .selectable_label(ps.o_category_id == c.id, &c.label)
                                    .on_hover_text(&c.desc)
                                    .clicked()
                                {
                                    ps.o_category_id = c.id.clone();
                                }
                            }
                        })
                        .response
                        .hint("market.publish.offering_category_hint");
                });
                combo(
                    ui,
                    theme,
                    "pub_o_price",
                    t("market.form_price"),
                    "market.publish.price_mode_hint",
                    vocab::PRICE_MODES,
                    &mut ps.o_price_mode_idx,
                );
                let mode = vocab::PRICE_MODES[ps.o_price_mode_idx];
                if matches!(mode, "fixed" | "range" | "sliding_scale" | "pay_what_you_can") {
                    widgets::form_row(ui, theme, t("market.publish.amount"), |ui| {
                        ui.add(egui::TextEdit::singleline(&mut ps.o_amount).desired_width(70.0))
                            .hint("market.publish.amount_hint");
                        if matches!(mode, "range" | "sliding_scale") {
                            ui.label(RichText::new(t("market.publish.to")).color(theme.text_muted()));
                            ui.add(
                                egui::TextEdit::singleline(&mut ps.o_amount_max).desired_width(70.0),
                            )
                            .hint("market.publish.amount_max_hint");
                        }
                        // Sim prices in CR only; the validator enforces it, the
                        // form just spares the round-trip.
                        if reality == "sim" {
                            ps.o_currency = "CR".to_string();
                            ui.label(RichText::new("CR").color(theme.text_secondary())); // i18n-exempt: the sim currency's code, the same in every language
                        } else {
                            ui.add(
                                egui::TextEdit::singleline(&mut ps.o_currency).desired_width(60.0),
                            )
                            .hint("market.publish.currency_hint");
                        }
                        let mut unit_opts: Vec<&str> = vec![t("market.publish.no_unit")];
                        unit_opts.extend(vocab::PRICE_UNITS);
                        egui::ComboBox::from_id_salt("pub_o_unit")
                            .selected_text(unit_opts[ps.o_unit_idx].replace('_', " "))
                            .show_ui(ui, |ui| {
                                for (i, u) in unit_opts.iter().enumerate() {
                                    if ui
                                        .selectable_label(ps.o_unit_idx == i, u.replace('_', " "))
                                        .hint("market.publish.unit_hint")
                                        .clicked()
                                    {
                                        ps.o_unit_idx = i;
                                    }
                                }
                            })
                            .response
                            .hint("market.publish.unit_hint");
                    });
                }
                widgets::form_row(ui, theme, t("market.publish.fulfillment"), |ui| {
                    ui.vertical(|ui| {
                        for (i, f) in vocab::FULFILLMENT_MODES.iter().enumerate() {
                            ui.checkbox(&mut ps.o_fulfillment[i], f.replace('_', " "))
                                .hint("market.publish.fulfillment_hint");
                        }
                    });
                });
                combo(
                    ui,
                    theme,
                    "pub_o_via",
                    t("market.publish.arrange_via"),
                    "market.publish.arrange_via_hint",
                    vocab::SETTLEMENT_CONTACTS,
                    &mut ps.o_contact_via_idx,
                );
                if vocab::SETTLEMENT_CONTACTS[ps.o_contact_via_idx] == "provider_checkout" {
                    widgets::form_row(ui, theme, t("market.publish.checkout_url"), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut ps.o_checkout_uri)
                                .hint_text("https://") // i18n-exempt: the start of a web address, the same in every language
                                .desired_width(280.0),
                        )
                        .hint("market.publish.checkout_url_hint");
                    });
                }
                widgets::form_row(ui, theme, t("market.publish.instructions"), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut ps.o_instructions)
                            .hint_text(t("market.publish.instructions_placeholder"))
                            .desired_width(280.0),
                    )
                    .hint("market.publish.instructions_hint");
                });

                if ps.o_is_good {
                    combo(
                        ui,
                        theme,
                        "pub_g_cond",
                        t("market.publish.condition"),
                        "market.publish.condition_hint",
                        vocab::GOOD_CONDITIONS,
                        &mut ps.g_condition_idx,
                    );
                    combo(
                        ui,
                        theme,
                        "pub_g_avail",
                        t("market.publish.availability"),
                        "market.publish.availability_hint",
                        vocab::GOOD_AVAILABILITY,
                        &mut ps.g_avail_idx,
                    );
                    match vocab::GOOD_AVAILABILITY[ps.g_avail_idx] {
                        "in_stock" => {
                            widgets::form_row(ui, theme, t("market.publish.quantity"), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut ps.g_quantity)
                                        .desired_width(60.0),
                                )
                                .hint("market.publish.quantity_hint");
                            });
                        }
                        "made_to_order" => {
                            widgets::form_row(ui, theme, t("market.publish.lead_days"), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut ps.g_lead_days)
                                        .desired_width(60.0),
                                )
                                .hint("market.publish.lead_days_hint");
                            });
                        }
                        _ => {}
                    }
                } else {
                    combo(
                        ui,
                        theme,
                        "pub_s_action",
                        t("market.publish.service"),
                        "market.publish.service_action_hint",
                        vocab::SERVICE_ACTIONS,
                        &mut ps.s_action_idx,
                    );
                    combo(
                        ui,
                        theme,
                        "pub_s_sched",
                        t("market.publish.schedule"),
                        "market.publish.schedule_hint",
                        vocab::SCHEDULE_KINDS,
                        &mut ps.s_schedule_idx,
                    );
                    if matches!(vocab::SCHEDULE_KINDS[ps.s_schedule_idx], "walk_in" | "recurring") {
                        widgets::form_row(ui, theme, t("market.publish.days"), |ui| {
                            for (i, d) in vocab::WEEKDAYS.iter().enumerate() {
                                // Each day's short name by key (market.publish.day_monday ...),
                                // falling back to the vocabulary's first three letters.
                                let key = format!("market.publish.day_{d}");
                                let short = if crate::gui::i18n::has(&key) {
                                    crate::gui::i18n::t_dyn(&key)
                                } else {
                                    &d[..3]
                                };
                                ui.checkbox(&mut ps.s_days[i], short)
                                    .hint("market.publish.day_hint");
                            }
                        });
                        widgets::form_row(ui, theme, t("market.publish.hours"), |ui| {
                            ui.add(egui::TextEdit::singleline(&mut ps.s_start).desired_width(56.0))
                                .hint("market.publish.start_hint");
                            ui.label(RichText::new(t("market.publish.to")).color(theme.text_muted()));
                            ui.add(egui::TextEdit::singleline(&mut ps.s_end).desired_width(56.0))
                                .hint("market.publish.end_hint");
                            ui.label(
                                RichText::new(t("market.publish.hours_format")).size(theme.font_size_small).color(theme.text_muted()),
                            );
                        });
                    }
                }
            });
            ui.add_space(theme.spacing_sm);
            if !busy
                && widgets::Button::primary(t("market.publish.publish_offering"))
                    .hint("market.publish.publish_offering_hint")
                    .show(ui, theme)
            {
                let (payload, seed) = with_pub(|ps| {
                    (
                        offering_payload(ps, &root, reality, now_ms()),
                        state.private_key_bytes.clone().unwrap_or_default(),
                    )
                });
                if phrase_stops(state, &payload) {
                    return;
                }
                let rx = spawn_publish(
                    base.to_string(),
                    seed,
                    payload,
                    "offering_v1",
                    vec![root.clone()],
                    false,
                );
                with_pub(|ps| {
                    ps.rx = Some(rx);
                    ps.error.clear();
                    ps.status.clear();
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(v: &ciborium::Value) -> Vec<u8> {
        // Direct serialization, same as spawn_publish: payload bytes are
        // opaque to the envelope, and prices are floats.
        let mut out = Vec::new();
        ciborium::into_writer(v, &mut out).unwrap();
        out
    }

    fn filled_state() -> PublishState {
        let mut ps = PublishState::default();
        ps.p_display_name = "Riverside Tool Library".to_string();
        ps.p_description = "Neighborhood tool lending.".to_string();
        ps.p_area_label = "Riverside".to_string();
        ps.o_key = "drill-01".to_string();
        ps.o_title = "Cordless drill (borrow)".to_string();
        ps.o_description = "Weekly checkout, bring it back charged.".to_string();
        ps.o_category_id = "tools".to_string();
        ps.o_fulfillment[0] = true; // pickup_at_provider
        ps
    }

    /// The form's payloads must pass the EXACT relay validators: if this
    /// test compiles and passes, an in-app publish cannot be rejected for
    /// shape by the server it was built against.
    #[test]
    fn form_payloads_pass_the_relay_validators() {
        let ps = filled_state();
        let now = super::now_ms();
        let provider = provider_payload(&ps, None, now);
        crate::relay::core::market_payloads::validate_provider_v1(&canon(&provider))
            .expect("provider payload valid");

        let root = "a".repeat(64);
        // Free good, default fields.
        let offering = offering_payload(&ps, &root, "real", now);
        crate::relay::core::market_payloads::validate_offering_v1(
            &canon(&offering),
            &[root.clone()],
            None,
        )
        .expect("good offering valid");

        // Priced service with walk-in hours.
        let mut ps2 = filled_state();
        ps2.o_is_good = false;
        ps2.o_key = "repair-drop-in".to_string();
        ps2.o_title = "Saturday repair drop-in".to_string();
        ps2.o_category_id = "repair".to_string();
        ps2.o_price_mode_idx =
            vocab::PRICE_MODES.iter().position(|m| *m == "fixed").unwrap();
        ps2.o_amount = "12.50".to_string();
        ps2.s_schedule_idx =
            vocab::SCHEDULE_KINDS.iter().position(|s| *s == "walk_in").unwrap();
        ps2.s_days[5] = true; // saturday
        let offering2 = offering_payload(&ps2, &root, "real", now);
        crate::relay::core::market_payloads::validate_offering_v1(
            &canon(&offering2),
            &[root],
            None,
        )
        .expect("service offering valid");
    }

    /// END-TO-END against a real relay, driving the EXACT path the Publish
    /// buttons drive (sign_and_submit). Ignored in CI; run manually:
    ///   BIND_ADDRESS=127.0.0.1 PORT=3213 DATABASE_PATH=<scratch>/relay.db target/release/HumanityOS.exe --headless
    /// (BIND_ADDRESS=127.0.0.1: loopback only, so Windows raises no firewall
    /// prompt for the build; the test only talks to 127.0.0.1.)
    ///   cargo test --features native --lib market_publish -- --ignored
    /// Proves: provider accepted, offering accepted against the returned
    /// root, and a DIFFERENT identity's offering into the same shop rejected
    /// (the ownership rule), all through the full sign + POST pipeline.
    #[test]
    #[ignore = "needs a throwaway local relay; see doc comment"]
    fn end_to_end_against_local_relay() {
        let base = std::env::var("HUMANITY_TEST_RELAY")
            .unwrap_or_else(|_| "http://127.0.0.1:3213".to_string());
        let seed = crate::net::identity::generate_new_seed();
        let ps = filled_state();
        let now = super::now_ms();

        let provider = provider_payload(&ps, None, now);
        let ok = sign_and_submit(&base, &seed, &provider, "provider_v1", &[], true)
            .expect("provider accepted");
        let root = ok.object_id;
        assert_eq!(root.len(), 64, "object_id is 64-hex");

        let offering = offering_payload(&ps, &root, "real", now);
        let refs = vec![root.clone()];
        sign_and_submit(&base, &seed, &offering, "offering_v1", &refs, false)
            .expect("offering accepted");

        // Ownership: another identity cannot publish into this shop.
        let thief = crate::net::identity::generate_new_seed();
        let stolen = offering_payload(&ps, &root, "real", now);
        let err = sign_and_submit(&base, &thief, &stolen, "offering_v1", &refs, false)
            .expect_err("foreign-key offering must be rejected");
        assert!(err.contains("server said no"), "got: {err}");
    }

    /// Provider UPDATE carries provider_ref; sim offerings price in CR.
    #[test]
    fn update_and_sim_rules_hold() {
        let ps = filled_state();
        let now = super::now_ms();
        let root = "b".repeat(64);
        let update = provider_payload(&ps, Some(&root), now);
        crate::relay::core::market_payloads::validate_provider_v1(&canon(&update))
            .expect("provider update valid");

        let mut ps2 = filled_state();
        ps2.o_price_mode_idx =
            vocab::PRICE_MODES.iter().position(|m| *m == "fixed").unwrap();
        ps2.o_amount = "5".to_string();
        ps2.o_currency = "CR".to_string(); // what the sim path forces
        let offering = offering_payload(&ps2, &root, "sim", now);
        crate::relay::core::market_payloads::validate_offering_v1(
            &canon(&offering),
            &[root],
            None,
        )
        .expect("sim offering prices in CR");
    }
}
