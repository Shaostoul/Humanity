//! Market directory: the signed provider/offering catalog (v0.1141).
//!
//! The read side of the Market: every view here is a query over the relay's
//! `GET /api/v2/objects` (object_type=provider_v1 / offering_v1). Payloads
//! are canonical CBOR signed by the merchant's chat identity; we decode them
//! with ciborium (the native build always carries the relay feature) and
//! resolve LATEST revisions client-side: an offering's identity is
//! (provider root, offering_key), a provider's identity is its root object
//! id, newest `updated_at` wins. Settlement is directory-only by design:
//! this page lists and introduces, it never moves money.

use egui::{RichText, ScrollArea};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiState, MarketCategory};

// ── Decoded view rows ───────────────────────────────────────────────────────

/// One provider (shop / org / neighbor), latest revision.
#[derive(Debug, Clone)]
pub struct DirProvider {
    /// Root object id: the id offerings reference (a provider UPDATE carries
    /// `provider_ref` back to this root; the first revision IS the root).
    pub root_id: String,
    /// Full Dilithium3 public key, hex: the merchant's chat identity.
    pub author_key_hex: String,
    pub display_name: String,
    pub kind: String,
    pub backed_by: String,
    pub description: String,
    pub status: String,
    pub contact_preferred: String,
    pub website: Option<String>,
    pub area_count: usize,
    pub languages: Vec<String>,
    pub updated_at: i64,
}

/// One offering (good or service), latest revision.
#[derive(Debug, Clone)]
pub struct DirOffering {
    pub provider_root: String,
    pub offering_key: String,
    pub kind: String,    // good | service
    pub reality: String, // real | sim
    pub title: String,
    pub summary: String,
    pub category: String, // vocabulary id (lowercase snake_case)
    pub tags: Vec<String>,
    pub status: String,
    pub updated_at: i64,
    pub expires_at: Option<i64>,
    pub ttl_days: i64,
    pub fulfillment: Vec<String>,
    // price
    pub price_mode: String,
    pub price_amount: Option<f64>,
    pub price_amount_max: Option<f64>,
    pub price_currency: String,
    pub price_unit: String,
    pub price_accepts: Vec<String>,
    pub price_notes: String,
    // settlement (directory-only today)
    pub contact_via: String,
    pub checkout_uri: Option<String>,
    pub instructions: String,
    // good branch
    pub condition: String,
    pub availability_mode: String,
    pub quantity_available: Option<i64>,
    pub lead_time_days: Option<i64>,
    // service branch
    pub action: String,
    pub schedule_kind: String,
    pub duration_minutes: Option<i64>,
    pub location_mode: String,
}

impl DirOffering {
    /// Stable UI selection key.
    fn sel_key(&self) -> String {
        format!("{}/{}", self.provider_root, self.offering_key)
    }

    /// Hidden once past its explicit expiry, or past `ttl_days` since the
    /// last touch (default 30, the schema's freshness contract: re-importing
    /// IS the keepalive, so the directory never fills with ghosts).
    fn expired(&self, now_ms: i64) -> bool {
        if let Some(exp) = self.expires_at {
            return exp <= now_ms;
        }
        self.updated_at + self.ttl_days.clamp(1, 365) * 86_400_000 <= now_ms
    }
}

// ── CBOR field extraction (mirrors the relay validator's shapes) ────────────

type CborMap = Vec<(ciborium::Value, ciborium::Value)>;

fn as_map(v: &ciborium::Value) -> Option<&CborMap> {
    match v {
        ciborium::Value::Map(m) => Some(m),
        _ => None,
    }
}

fn get<'a>(m: &'a CborMap, key: &str) -> Option<&'a ciborium::Value> {
    m.iter()
        .find(|(k, _)| matches!(k, ciborium::Value::Text(t) if t == key))
        .map(|(_, v)| v)
}

fn text(m: &CborMap, key: &str) -> Option<String> {
    match get(m, key) {
        Some(ciborium::Value::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

fn int(m: &CborMap, key: &str) -> Option<i64> {
    match get(m, key) {
        Some(ciborium::Value::Integer(i)) => i64::try_from(*i).ok(),
        _ => None,
    }
}

fn float(m: &CborMap, key: &str) -> Option<f64> {
    match get(m, key) {
        Some(ciborium::Value::Float(f)) => Some(*f),
        Some(ciborium::Value::Integer(i)) => i64::try_from(*i).ok().map(|v| v as f64),
        _ => None,
    }
}

fn strs(m: &CborMap, key: &str) -> Vec<String> {
    match get(m, key) {
        Some(ciborium::Value::Array(a)) => a
            .iter()
            .filter_map(|v| match v {
                ciborium::Value::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn tbl<'a>(m: &'a CborMap, key: &str) -> Option<&'a CborMap> {
    get(m, key).and_then(as_map)
}

fn decode_provider(object_id: &str, author_key_hex: &str, payload: &[u8]) -> Option<DirProvider> {
    let value: ciborium::Value = ciborium::from_reader(payload).ok()?;
    let m = as_map(&value)?;
    let root_id = text(m, "provider_ref").unwrap_or_else(|| object_id.to_string());
    let contact = tbl(m, "contact");
    Some(DirProvider {
        root_id,
        author_key_hex: author_key_hex.to_string(),
        display_name: text(m, "display_name")?,
        kind: text(m, "kind").unwrap_or_default(),
        backed_by: text(m, "backed_by").unwrap_or_default(),
        description: text(m, "description").unwrap_or_default(),
        status: text(m, "status").unwrap_or_default(),
        contact_preferred: contact.and_then(|c| text(c, "preferred")).unwrap_or_default(),
        website: contact.and_then(|c| text(c, "website")),
        area_count: match get(m, "service_areas") {
            Some(ciborium::Value::Array(a)) => a.len(),
            _ => 0,
        },
        languages: strs(m, "languages"),
        updated_at: int(m, "updated_at").unwrap_or(0),
    })
}

fn decode_offering(payload: &[u8]) -> Option<DirOffering> {
    let value: ciborium::Value = ciborium::from_reader(payload).ok()?;
    let m = as_map(&value)?;
    let price = tbl(m, "price");
    let settlement = tbl(m, "settlement");
    let good = tbl(m, "good");
    let service = tbl(m, "service");
    let availability = service.and_then(|s| tbl(s, "availability"));
    Some(DirOffering {
        provider_root: text(m, "provider_ref")?,
        offering_key: text(m, "offering_key")?,
        kind: text(m, "kind")?,
        reality: text(m, "reality").unwrap_or_else(|| "real".to_string()),
        title: text(m, "title")?,
        summary: text(m, "summary").unwrap_or_default(),
        category: text(m, "category").unwrap_or_default(),
        tags: strs(m, "tags"),
        status: text(m, "status").unwrap_or_default(),
        updated_at: int(m, "updated_at").unwrap_or(0),
        expires_at: int(m, "expires_at"),
        ttl_days: int(m, "ttl_days").unwrap_or(30),
        fulfillment: strs(m, "fulfillment"),
        price_mode: price.and_then(|p| text(p, "mode")).unwrap_or_default(),
        price_amount: price.and_then(|p| float(p, "amount")),
        price_amount_max: price.and_then(|p| float(p, "amount_max")),
        price_currency: price.and_then(|p| text(p, "currency")).unwrap_or_default(),
        price_unit: price.and_then(|p| text(p, "unit")).unwrap_or_default(),
        price_accepts: price.map(|p| strs(p, "accepts")).unwrap_or_default(),
        price_notes: price.and_then(|p| text(p, "notes")).unwrap_or_default(),
        contact_via: settlement.and_then(|s| text(s, "contact_via")).unwrap_or_default(),
        checkout_uri: settlement.and_then(|s| text(s, "checkout_uri")),
        instructions: settlement.and_then(|s| text(s, "instructions")).unwrap_or_default(),
        condition: good.and_then(|g| text(g, "condition")).unwrap_or_default(),
        availability_mode: good.and_then(|g| text(g, "availability_mode")).unwrap_or_default(),
        quantity_available: good.and_then(|g| int(g, "quantity_available")),
        lead_time_days: good.and_then(|g| int(g, "lead_time_days")),
        action: service.and_then(|s| text(s, "action")).unwrap_or_default(),
        schedule_kind: availability.and_then(|a| text(a, "schedule_kind")).unwrap_or_default(),
        duration_minutes: service.and_then(|s| int(s, "duration_minutes")),
        location_mode: tbl(m, "location").and_then(|l| text(l, "mode")).unwrap_or_default(),
    })
}

// ── Fetch + latest-revision resolution ──────────────────────────────────────

type FetchResult = Result<(Vec<DirProvider>, Vec<DirOffering>), String>;

/// One REST round per object type; decode, then keep only the newest
/// revision per identity. Runs on a worker thread (same pattern as the
/// classifieds' reviews fetch).
fn fetch_directory(base: &str) -> FetchResult {
    let rows = |object_type: &str| -> Result<Vec<serde_json::Value>, String> {
        let url = format!("{base}/api/v2/objects?object_type={object_type}&limit=500");
        let body = ureq::get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .call()
            .map_err(|e| format!("{object_type}: {e}"))?
            .into_string()
            .map_err(|e| format!("{object_type} read: {e}"))?;
        serde_json::from_str::<Vec<serde_json::Value>>(&body)
            .map_err(|e| format!("{object_type} parse: {e}"))
    };
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;

    // Providers: newest revision per root id wins.
    let mut providers: HashMap<String, DirProvider> = HashMap::new();
    for row in rows("provider_v1")? {
        let object_id = row.get("object_id").and_then(|v| v.as_str()).unwrap_or_default();
        let key_hex = row
            .get("author_public_key_b64")
            .and_then(|v| v.as_str())
            .and_then(|s| b64.decode(s).ok())
            .map(hex::encode)
            .unwrap_or_default();
        let Some(payload) = row
            .get("payload_b64")
            .and_then(|v| v.as_str())
            .and_then(|s| b64.decode(s).ok())
        else {
            continue;
        };
        if let Some(p) = decode_provider(object_id, &key_hex, &payload) {
            match providers.get(&p.root_id) {
                Some(old) if old.updated_at >= p.updated_at => {}
                _ => {
                    providers.insert(p.root_id.clone(), p);
                }
            }
        }
    }

    // Offerings: newest revision per (provider root, offering_key) wins;
    // `received_at` breaks updated_at ties (a re-import touches freshness).
    let mut offerings: HashMap<(String, String), (i64, DirOffering)> = HashMap::new();
    for row in rows("offering_v1")? {
        let received = row.get("received_at").and_then(|v| v.as_i64()).unwrap_or(0);
        let Some(payload) = row
            .get("payload_b64")
            .and_then(|v| v.as_str())
            .and_then(|s| b64.decode(s).ok())
        else {
            continue;
        };
        if let Some(o) = decode_offering(&payload) {
            let key = (o.provider_root.clone(), o.offering_key.clone());
            match offerings.get(&key) {
                Some((old_rcv, old))
                    if (old.updated_at, *old_rcv) >= (o.updated_at, received) => {}
                _ => {
                    offerings.insert(key, (received, o));
                }
            }
        }
    }

    let mut provider_list: Vec<DirProvider> = providers.into_values().collect();
    provider_list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let mut offering_list: Vec<DirOffering> =
        offerings.into_values().map(|(_, o)| o).collect();
    offering_list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok((provider_list, offering_list))
}

// ── Page-local state ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubView {
    Offerings,
    Providers,
}

struct DirPageState {
    sub: SubView,
    search: String,
    /// Selected offering (detail view), keyed `provider_root/offering_key`.
    selected: Option<String>,
    /// Storefront filter: only this provider's offerings.
    provider_filter: Option<String>,
    rx: Option<Receiver<FetchResult>>,
    /// Base URL the current data was fetched from ("" = never fetched).
    fetched_for: String,
    providers: Vec<DirProvider>,
    offerings: Vec<DirOffering>,
    error: String,
    loaded: bool,
    /// Snapshot mode: seeded data, never fetch (headless determinism).
    frozen: bool,
}

impl Default for DirPageState {
    fn default() -> Self {
        Self {
            sub: SubView::Offerings,
            search: String::new(),
            selected: None,
            provider_filter: None,
            rx: None,
            fetched_for: String::new(),
            providers: Vec::new(),
            offerings: Vec::new(),
            error: String::new(),
            loaded: false,
            frozen: false,
        }
    }
}

fn with_dir<R>(f: impl FnOnce(&mut DirPageState) -> R) -> R {
    thread_local! {
        static STATE: RefCell<DirPageState> = RefCell::new(DirPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Snapshot seeding: fill the page with representative rows so the headless
/// renderer proves the card + detail layout without a live relay.
pub fn seed_for_snapshot() {
    let now = now_ms();
    with_dir(|ps| {
        ps.fetched_for = "snapshot".to_string();
        ps.loaded = true;
        ps.frozen = true;
        ps.providers = vec![DirProvider {
            root_id: "a".repeat(64),
            author_key_hex: "b".repeat(64),
            display_name: "Riverside Tool Library".to_string(),
            kind: "mutual_aid".to_string(),
            backed_by: "identity".to_string(),
            description: "Neighborhood tool lending and repair help.".to_string(),
            status: "active".to_string(),
            contact_preferred: "in_app_dm".to_string(),
            website: None,
            area_count: 1,
            languages: vec!["en".to_string()],
            updated_at: now,
        }];
        ps.offerings = vec![
            DirOffering {
                provider_root: "a".repeat(64),
                offering_key: "drill-01".to_string(),
                kind: "good".to_string(),
                reality: "real".to_string(),
                title: "Cordless drill (borrow)".to_string(),
                summary: "Weekly checkout, bring it back charged.".to_string(),
                category: "tools".to_string(),
                tags: vec!["lending".to_string()],
                status: "active".to_string(),
                updated_at: now,
                expires_at: None,
                ttl_days: 30,
                fulfillment: vec!["pickup_at_provider".to_string()],
                price_mode: "free".to_string(),
                price_amount: None,
                price_amount_max: None,
                price_currency: String::new(),
                price_unit: String::new(),
                price_accepts: Vec::new(),
                price_notes: String::new(),
                contact_via: "in_app_dm".to_string(),
                checkout_uri: None,
                instructions: String::new(),
                condition: "used_good".to_string(),
                availability_mode: "in_stock".to_string(),
                quantity_available: Some(3),
                lead_time_days: None,
                action: String::new(),
                schedule_kind: String::new(),
                duration_minutes: None,
                location_mode: "fixed_location".to_string(),
            },
            DirOffering {
                provider_root: "a".repeat(64),
                offering_key: "repair-drop-in".to_string(),
                kind: "service".to_string(),
                reality: "real".to_string(),
                title: "Saturday repair drop-in".to_string(),
                summary: "Bring the broken thing; we fix it together.".to_string(),
                category: "repair".to_string(),
                tags: Vec::new(),
                status: "active".to_string(),
                updated_at: now,
                expires_at: None,
                ttl_days: 30,
                fulfillment: vec!["at_provider_location".to_string()],
                price_mode: "donation".to_string(),
                price_amount: None,
                price_amount_max: None,
                price_currency: String::new(),
                price_unit: String::new(),
                price_accepts: Vec::new(),
                price_notes: String::new(),
                contact_via: "in_person".to_string(),
                checkout_uri: None,
                instructions: String::new(),
                condition: String::new(),
                availability_mode: String::new(),
                quantity_available: None,
                lead_time_days: None,
                action: "repair".to_string(),
                schedule_kind: "walk_in".to_string(),
                duration_minutes: None,
                location_mode: "fixed_location".to_string(),
            },
        ];
    });
}

// ── Display helpers ─────────────────────────────────────────────────────────

/// Human price line: "Free", "12.50 USD each", "10 to 20 USD", "Donation".
fn price_line(o: &DirOffering) -> String {
    let unit = if o.price_unit.is_empty() {
        String::new()
    } else if o.price_unit == "each" {
        " each".to_string()
    } else {
        format!(" per {}", o.price_unit.replace('_', " "))
    };
    match o.price_mode.as_str() {
        "free" => "Free".to_string(),
        "trade" => "Trade".to_string(),
        "donation" => "Donation".to_string(),
        "inquire" => "Ask".to_string(),
        "pay_what_you_can" => "Pay what you can".to_string(),
        "sliding_scale" => match (o.price_amount, o.price_amount_max) {
            (Some(a), Some(b)) => {
                format!("Sliding {a:.2} to {b:.2} {}{unit}", o.price_currency)
            }
            _ => "Sliding scale".to_string(),
        },
        "range" => match (o.price_amount, o.price_amount_max) {
            (Some(a), Some(b)) => format!("{a:.2} to {b:.2} {}{unit}", o.price_currency),
            _ => "Range".to_string(),
        },
        "fixed" => match o.price_amount {
            Some(a) => format!("{a:.2} {}{unit}", o.price_currency),
            None => "Priced".to_string(),
        },
        other => other.replace('_', " "),
    }
}

/// "today", "3d ago", "2mo ago" from a millisecond timestamp.
fn age_line(updated_ms: i64, now_ms: i64) -> String {
    let days = (now_ms.saturating_sub(updated_ms)) / 86_400_000;
    match days {
        0 => "today".to_string(),
        1..=59 => format!("{days}d ago"),
        _ => format!("{}mo ago", days / 30),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Vocabulary label for a category id (falls back to the id itself).
fn cat_label<'a>(cats: &'a [MarketCategory], id: &'a str) -> &'a str {
    cats.iter().find(|c| c.id == id).map(|c| c.label.as_str()).unwrap_or(id)
}

fn enum_label(s: &str) -> String {
    s.replace('_', " ")
}

// ── Draw ────────────────────────────────────────────────────────────────────

/// Central-panel body of the Market page's Directory tab. The category
/// SIDEBAR is shared with the classifieds tab; `filter_label` is the selected
/// label there ("" = All), which we map back to a vocabulary id.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let reality = if state.nav_top_category == "sim" { "sim" } else { "real" };
    let filter_cat_id: Option<String> = if state.listing_filter_category.is_empty() {
        None
    } else {
        state
            .market_categories
            .iter()
            .find(|c| c.label == state.listing_filter_category)
            .map(|c| c.id.clone())
    };

    // Fetch lifecycle: first view per server fetches; Refresh refetches.
    let mut want_fetch = false;
    with_dir(|ps| {
        if let Some(rx) = &ps.rx {
            match rx.try_recv() {
                Ok(Ok((providers, offerings))) => {
                    ps.providers = providers;
                    ps.offerings = offerings;
                    ps.error.clear();
                    ps.loaded = true;
                    ps.rx = None;
                }
                Ok(Err(e)) => {
                    ps.error = e;
                    ps.loaded = true;
                    ps.rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => ps.rx = None,
            }
        }
        if !base.is_empty() && ps.fetched_for != base && ps.rx.is_none() && !ps.frozen {
            want_fetch = true;
        }
    });
    let spawn_fetch = |ps: &mut DirPageState| {
        let (tx, rx) = channel();
        ps.rx = Some(rx);
        ps.fetched_for = base.clone();
        let b = base.clone();
        std::thread::spawn(move || {
            let _ = tx.send(fetch_directory(&b));
        });
    };
    if want_fetch {
        with_dir(|ps| spawn_fetch(ps));
    }

    // Detail view swallows the whole panel.
    let selected = with_dir(|ps| ps.selected.clone());
    if let Some(sel) = selected {
        let offering = with_dir(|ps| {
            ps.offerings.iter().find(|o| o.sel_key() == sel).cloned()
        });
        if let Some(o) = offering {
            let provider = with_dir(|ps| {
                ps.providers.iter().find(|p| p.root_id == o.provider_root).cloned()
            });
            draw_detail(ui, theme, state, &o, provider.as_ref());
        } else {
            with_dir(|ps| ps.selected = None);
        }
        return;
    }

    // Header row.
    let (sub, n_off, n_prov, loading) = with_dir(|ps| {
        (ps.sub, ps.offerings.len(), ps.providers.len(), ps.rx.is_some())
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Market Directory")
                .size(theme.font_size_title)
                .color(theme.text_primary()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !base.is_empty() && widgets::secondary_button(ui, theme, "Refresh") {
                with_dir(|ps| {
                    if ps.rx.is_none() {
                        spawn_fetch(ps);
                    }
                });
            }
            if loading {
                ui.label(RichText::new("Loading...").color(theme.text_muted()));
            }
        });
    });

    // Sub-view toggle + storefront filter breadcrumb.
    ui.horizontal(|ui| {
        for (v, label) in [(SubView::Offerings, "Offerings"), (SubView::Providers, "Providers")] {
            if ui.selectable_label(sub == v, label).clicked() {
                with_dir(|ps| ps.sub = v);
            }
        }
        let filter_name = with_dir(|ps| {
            ps.provider_filter.as_ref().and_then(|root| {
                ps.providers.iter().find(|p| &p.root_id == root).map(|p| p.display_name.clone())
            })
        });
        if let Some(name) = filter_name {
            ui.separator();
            ui.label(
                RichText::new(format!("Storefront: {name}")).color(theme.accent()),
            );
            if widgets::secondary_button(ui, theme, "All providers") {
                with_dir(|ps| ps.provider_filter = None);
            }
        }
    });
    let plural = |n: usize, word: &str| {
        if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
    };
    ui.label(
        RichText::new(format!(
            "{} from {}. Listings and introductions only: money never moves through the platform.",
            plural(n_off, "offering"),
            plural(n_prov, "provider"),
        ))
        .size(theme.font_size_small)
        .color(theme.text_muted()),
    );
    let err = with_dir(|ps| ps.error.clone());
    if !err.is_empty() {
        ui.label(
            RichText::new(format!("Fetch failed: {err}"))
                .size(theme.font_size_small)
                .color(theme.warning()),
        );
    }
    if base.is_empty() {
        ui.add_space(theme.spacing_md);
        ui.label(
            RichText::new("Connect to a server (Chat page) to browse its directory.")
                .color(theme.warning()),
        );
        return;
    }
    ui.add_space(theme.spacing_sm);

    match sub {
        SubView::Offerings => draw_offerings(ui, theme, state, reality, filter_cat_id.as_deref()),
        SubView::Providers => draw_providers(ui, theme),
    }
}

fn draw_offerings(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &GuiState,
    reality: &str,
    filter_cat_id: Option<&str>,
) {
    let mut search = with_dir(|ps| ps.search.clone());
    if widgets::search_bar(ui, theme, &mut search, "Search offerings...") {
        // edited; stored below either way
    }
    with_dir(|ps| ps.search = search.clone());
    let needle = search.trim().to_ascii_lowercase();
    ui.add_space(theme.spacing_sm);

    let now = now_ms();
    let rows: Vec<DirOffering> = with_dir(|ps| {
        ps.offerings
            .iter()
            .filter(|o| o.status == "active" && !o.expired(now))
            .filter(|o| o.reality == reality)
            .filter(|o| filter_cat_id.map_or(true, |c| o.category == c))
            .filter(|o| {
                ps.provider_filter.as_ref().map_or(true, |root| &o.provider_root == root)
            })
            .filter(|o| {
                needle.is_empty()
                    || o.title.to_ascii_lowercase().contains(&needle)
                    || o.summary.to_ascii_lowercase().contains(&needle)
                    || o.tags.iter().any(|t| t.contains(&needle))
            })
            .cloned()
            .collect()
    });

    if rows.is_empty() {
        let loaded = with_dir(|ps| ps.loaded);
        if loaded {
            ui.label(
                RichText::new(if reality == "sim" {
                    "No sim offerings here yet."
                } else {
                    "No offerings match. Publish yours with scripts/import-offerings.mjs (see docs/admin/market-importer.md)."
                })
                .color(theme.text_muted()),
            );
        }
        return;
    }

    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for o in &rows {
            let provider_name = with_dir(|ps| {
                ps.providers
                    .iter()
                    .find(|p| p.root_id == o.provider_root)
                    .map(|p| p.display_name.clone())
                    .unwrap_or_else(|| "Unknown provider".to_string())
            });
            let resp = widgets::card_clickable(ui, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&o.title)
                            .size(theme.font_size_body)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(price_line(o))
                                .size(theme.font_size_body)
                                .color(theme.accent()),
                        );
                    });
                });
                ui.horizontal(|ui| {
                    widgets::badge_sm(
                        ui,
                        theme,
                        cat_label(&state.market_categories, &o.category),
                        super::market::category_color(&o.category),
                    );
                    let kind_note = if o.kind == "service" {
                        format!("service: {}", enum_label(&o.action))
                    } else {
                        enum_label(&o.condition)
                    };
                    ui.label(
                        RichText::new(kind_note)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.label(
                        RichText::new(format!("by {provider_name}"))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(age_line(o.updated_at, now))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                });
                if !o.summary.is_empty() {
                    ui.label(
                        RichText::new(&o.summary)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                }
            });
            if resp.clicked() {
                with_dir(|ps| ps.selected = Some(o.sel_key()));
            }
            ui.add_space(theme.spacing_xs);
        }
    });
}

fn draw_providers(ui: &mut egui::Ui, theme: &Theme) {
    let now = now_ms();
    let rows: Vec<(DirProvider, usize)> = with_dir(|ps| {
        ps.providers
            .iter()
            .filter(|p| p.status == "active")
            .map(|p| {
                let n = ps
                    .offerings
                    .iter()
                    .filter(|o| {
                        o.provider_root == p.root_id && o.status == "active" && !o.expired(now)
                    })
                    .count();
                (p.clone(), n)
            })
            .collect()
    });
    if rows.is_empty() {
        let loaded = with_dir(|ps| ps.loaded);
        if loaded {
            ui.label(
                RichText::new("No providers yet. Publish yours with scripts/import-offerings.mjs.")
                    .color(theme.text_muted()),
            );
        }
        return;
    }
    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for (p, n_offerings) in &rows {
            let resp = widgets::card_clickable(ui, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&p.display_name)
                            .size(theme.font_size_body)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    ui.label(
                        RichText::new(enum_label(&p.kind))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{n_offerings} offerings"))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                });
                if !p.description.is_empty() {
                    ui.label(
                        RichText::new(&p.description)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                }
            });
            if resp.clicked() {
                with_dir(|ps| {
                    ps.provider_filter = Some(p.root_id.clone());
                    ps.sub = SubView::Offerings;
                });
            }
            ui.add_space(theme.spacing_xs);
        }
    });
}

fn draw_detail(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &GuiState,
    o: &DirOffering,
    provider: Option<&DirProvider>,
) {
    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        if widgets::secondary_button(ui, theme, "< Back to Directory") {
            with_dir(|ps| ps.selected = None);
        }
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new(&o.title).size(theme.font_size_title).color(theme.accent()),
        );
        ui.horizontal(|ui| {
            widgets::badge(
                ui,
                theme,
                cat_label(&state.market_categories, &o.category),
                super::market::category_color(&o.category),
            );
            if o.reality == "sim" {
                widgets::badge_sm(ui, theme, "sim", theme.text_muted());
            }
        });
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new(price_line(o)).size(theme.font_size_title).color(theme.accent()),
        );
        if !o.price_accepts.is_empty() {
            ui.label(
                RichText::new(format!("Accepts: {}", o.price_accepts.join(", ")))
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        }
        if !o.price_notes.is_empty() {
            ui.label(
                RichText::new(&o.price_notes)
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        }
        ui.add_space(theme.spacing_sm);
        if !o.summary.is_empty() {
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new(&o.summary).color(theme.text_primary()));
            });
            ui.add_space(theme.spacing_sm);
        }

        widgets::card_with_header(ui, theme, "Details", |ui| {
            let mut fact = |label: &str, value: String| {
                if !value.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).color(theme.text_muted()));
                        ui.label(RichText::new(value).color(theme.text_primary()));
                    });
                }
            };
            if o.kind == "good" {
                fact("Condition:", enum_label(&o.condition));
                let avail = match (o.availability_mode.as_str(), o.quantity_available) {
                    ("in_stock", Some(q)) => format!("{q} in stock"),
                    ("one_off", _) => "one of a kind".to_string(),
                    ("made_to_order", _) => match o.lead_time_days {
                        Some(d) => format!("made to order, {d} day lead"),
                        None => "made to order".to_string(),
                    },
                    (a, _) => enum_label(a),
                };
                fact("Availability:", avail);
            } else {
                fact("Service:", enum_label(&o.action));
                fact("Schedule:", enum_label(&o.schedule_kind));
                if let Some(d) = o.duration_minutes {
                    fact("Duration:", format!("{d} min"));
                }
            }
            fact(
                "Fulfillment:",
                o.fulfillment.iter().map(|f| enum_label(f)).collect::<Vec<_>>().join(", "),
            );
            fact("Location:", enum_label(&o.location_mode));
            if !o.tags.is_empty() {
                fact("Tags:", o.tags.join(", "));
            }
            fact("Updated:", age_line(o.updated_at, now_ms()));
        });
        ui.add_space(theme.spacing_sm);

        widgets::card_with_header(ui, theme, "Provider", |ui| {
            match provider {
                Some(p) => {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&p.display_name)
                                .color(theme.text_primary())
                                .strong(),
                        );
                        ui.label(
                            RichText::new(enum_label(&p.kind))
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    });
                    if !p.description.is_empty() {
                        ui.label(
                            RichText::new(&p.description)
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    }
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Contact: {}",
                                enum_label(&o.contact_via)
                            ))
                            .color(theme.text_primary()),
                        );
                        if !p.author_key_hex.is_empty()
                            && widgets::secondary_button(ui, theme, "Copy provider key")
                        {
                            ui.ctx().copy_text(p.author_key_hex.clone());
                        }
                    });
                    ui.label(
                        RichText::new(
                            "Paste the key into Chat to DM the provider. Arrange payment and handoff directly; the platform only lists.",
                        )
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                    if let Some(w) = &p.website {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Website:").color(theme.text_muted()));
                            ui.label(RichText::new(w).color(theme.text_primary()));
                            if widgets::secondary_button(ui, theme, "Copy") {
                                ui.ctx().copy_text(w.clone());
                            }
                        });
                    }
                }
                None => {
                    ui.label(
                        RichText::new("Provider entry not found on this server.")
                            .color(theme.warning()),
                    );
                }
            }
            if let Some(uri) = &o.checkout_uri {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Checkout:").color(theme.text_muted()));
                    ui.label(RichText::new(uri).color(theme.text_primary()));
                    if widgets::secondary_button(ui, theme, "Copy") {
                        ui.ctx().copy_text(uri.clone());
                    }
                });
            }
            if !o.instructions.is_empty() {
                ui.label(
                    RichText::new(&o.instructions)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cbor_bytes(v: ciborium::Value) -> Vec<u8> {
        let mut out = Vec::new();
        ciborium::into_writer(&v, &mut out).unwrap();
        out
    }

    fn t(s: &str) -> ciborium::Value {
        ciborium::Value::Text(s.to_string())
    }
    fn kv(k: &str, v: ciborium::Value) -> (ciborium::Value, ciborium::Value) {
        (t(k), v)
    }

    #[test]
    fn decodes_a_minimal_offering_payload() {
        let payload = cbor_bytes(ciborium::Value::Map(vec![
            kv("provider_ref", t(&"c".repeat(64))),
            kv("offering_key", t("eggs-dozen")),
            kv("kind", t("good")),
            kv("reality", t("real")),
            kv("title", t("Fresh eggs")),
            kv("category", t("food")),
            kv("status", t("active")),
            kv("updated_at", ciborium::Value::Integer(1_700_000_000_000i64.into())),
            kv("fulfillment", ciborium::Value::Array(vec![t("pickup_at_provider")])),
            kv(
                "price",
                ciborium::Value::Map(vec![
                    kv("mode", t("fixed")),
                    kv("amount", ciborium::Value::Float(4.5)),
                    kv("currency", t("USD")),
                    kv("unit", t("dozen")),
                ]),
            ),
            kv(
                "settlement",
                ciborium::Value::Map(vec![
                    kv("mode", t("directory")),
                    kv("contact_via", t("in_app_dm")),
                ]),
            ),
            kv(
                "good",
                ciborium::Value::Map(vec![
                    kv("condition", t("new")),
                    kv("availability_mode", t("in_stock")),
                    kv("quantity_available", ciborium::Value::Integer(12i64.into())),
                ]),
            ),
        ]));
        let o = decode_offering(&payload).expect("decodes");
        assert_eq!(o.title, "Fresh eggs");
        assert_eq!(o.category, "food");
        assert_eq!(o.price_mode, "fixed");
        assert_eq!(o.quantity_available, Some(12));
        assert_eq!(price_line(&o), "4.50 USD per dozen");
        assert!(!o.expired(1_700_000_000_001));
        // TTL default 30 days: expired 31 days later.
        assert!(o.expired(1_700_000_000_000 + 31 * 86_400_000));
    }

    #[test]
    fn provider_root_follows_provider_ref_on_updates() {
        let root = "d".repeat(64);
        let update = cbor_bytes(ciborium::Value::Map(vec![
            kv("provider_ref", t(&root)),
            kv("display_name", t("Riverside Tool Library")),
            kv("kind", t("mutual_aid")),
            kv("backed_by", t("identity")),
            kv("description", t("tools")),
            kv("status", t("active")),
            kv("updated_at", ciborium::Value::Integer(2i64.into())),
        ]));
        let p = decode_provider("ff00", "aabb", &update).expect("decodes");
        assert_eq!(p.root_id, root, "updates keep the root identity");
        let first = cbor_bytes(ciborium::Value::Map(vec![
            kv("display_name", t("Riverside Tool Library")),
            kv("kind", t("mutual_aid")),
            kv("backed_by", t("identity")),
            kv("description", t("tools")),
            kv("status", t("active")),
            kv("updated_at", ciborium::Value::Integer(1i64.into())),
        ]));
        let p0 = decode_provider("ff00", "aabb", &first).expect("decodes");
        assert_eq!(p0.root_id, "ff00", "first revision is its own root");
    }
}
