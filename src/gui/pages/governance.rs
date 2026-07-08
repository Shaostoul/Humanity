//! Governance page: LIVE proposals + weighted tallies + Dilithium-signed voting
//! and proposal creation (v0.660). Mirrors web/pages/governance.html's read path
//! and goes beyond it: the native write path builds `vote_v1`/`proposal_v1`
//! signed objects with the SAME in-crate `ObjectBuilder`/`DilithiumKeypair` the
//! relay verifies with, so canonical-CBOR signing can never drift between what
//! this client produces and what the server checks.
//!
//! Data flow (all network on background threads, drained here each frame):
//! - read:  GET /api/v2/proposals -> per proposal GET /api/v2/objects/{id}
//!          (title/body live in the object's CBOR payload, not the index row)
//!          + GET /api/v2/proposals/{id}/tally
//! - write: POST /api/v2/objects with a signed SignedObjectSubmission
//!
//! Votes are FINAL: the relay's vote index is INSERT OR IGNORE keyed on
//! (proposal, voter DID), so a second vote by the same voter is dropped
//! server-side. The UI says so before the user commits.

use egui::{RichText, ScrollArea};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// One proposal joined for display: index row + payload title/body + tally.
#[derive(Debug, Clone)]
pub struct ProposalView {
    pub id: String,
    pub proposer_did: String,
    pub proposal_type: String,
    pub scope: String,
    pub opens_at: i64,
    pub closes_at: i64,
    pub title: String,
    pub body: String,
    pub tally: Option<TallyView>,
}

/// Weighted tally (vote weight = trust score at vote time, capped 0.95).
/// The rule fields (v0.759) come from the server's data-driven proposal-type
/// rules; None against older servers that predate them.
#[derive(Debug, Clone, Default)]
pub struct TallyView {
    pub yes_weight: f64,
    pub no_weight: f64,
    pub abstain_weight: f64,
    pub total_weight: f64,
    pub vote_count: u64,
    pub quorum_fraction: Option<f64>,
    pub electorate: Option<i64>,
    pub quorum_met: Option<bool>,
    pub passing: Option<bool>,
}

/// Proposal types the form offers, (wire id, display label).
const PROPOSAL_TYPES: [(&str, &str); 3] = [
    ("local_rule", "Local rule"),
    ("parameter_change", "Parameter change"),
    ("accord_amendment", "Accord amendment"),
];
const SCOPES: [(&str, &str); 2] = [("local", "Local (this server)"), ("civilization", "Civilization-wide")];

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// (is_open_for_voting, human label) for a proposal's voting window.
fn fmt_window(opens_at: i64, closes_at: i64, now: i64) -> (bool, String) {
    fn span(ms: i64) -> String {
        let mins = ms / 60_000;
        if mins >= 2880 {
            format!("{}d", mins / 1440)
        } else if mins >= 120 {
            format!("{}h", mins / 60)
        } else {
            format!("{}m", mins.max(1))
        }
    }
    if now < opens_at {
        (false, format!("opens in {}", span(opens_at - now)))
    } else if now < closes_at {
        (true, format!("closes in {}", span(closes_at - now)))
    } else {
        (false, "closed".to_string())
    }
}

/// Decode a signed object's base64 CBOR payload and pull the `title` + `body`
/// text fields (how proposal content travels; the index row doesn't carry them).
fn payload_texts(payload_b64: &str) -> Option<(String, String)> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(payload_b64).ok()?;
    let value = crate::relay::core::encoding::from_canonical_bytes(&bytes).ok()?;
    let ciborium::Value::Map(entries) = value else { return None };
    let mut title = String::new();
    let mut body = String::new();
    for (k, v) in entries {
        if let (ciborium::Value::Text(k), ciborium::Value::Text(v)) = (k, v) {
            match k.as_str() {
                "title" => title = v,
                "body" => body = v,
                _ => {}
            }
        }
    }
    Some((title, body))
}

/// Blocking: list proposals then join each with its payload title/body + tally.
/// Individual object/tally failures degrade gracefully (untitled / no tally)
/// rather than failing the whole list.
fn fetch_proposals_blocking(base: &str) -> Result<Vec<ProposalView>, String> {
    #[derive(serde::Deserialize)]
    struct IndexRow {
        proposal_object_id: String,
        proposer_did: String,
        proposal_type: String,
        scope: String,
        opens_at: i64,
        closes_at: i64,
    }
    #[derive(serde::Deserialize)]
    struct ObjectResp {
        payload_b64: String,
    }
    #[derive(serde::Deserialize)]
    struct TallyResp {
        yes_weight: f64,
        no_weight: f64,
        abstain_weight: f64,
        total_weight: f64,
        vote_count: u64,
        #[serde(default)]
        quorum_fraction: Option<f64>,
        #[serde(default)]
        electorate: Option<i64>,
        #[serde(default)]
        quorum_met: Option<bool>,
        #[serde(default)]
        passing: Option<bool>,
    }
    let get = |url: &str| -> Result<String, String> {
        ureq::get(url)
            .timeout(std::time::Duration::from_secs(6))
            .call()
            .map_err(|e| e.to_string())?
            .into_string()
            .map_err(|e| e.to_string())
    };
    let rows: Vec<IndexRow> = serde_json::from_str(&get(&format!("{base}/api/v2/proposals?limit=50"))?)
        .map_err(|e| format!("proposal list: {e}"))?;
    // Circuit breaker (adversarial review 2026-07-01): the join is up to 2 GETs
    // per proposal on ONE sequential thread. Against a server that answers the
    // list but black-holes the follow-ups, 100 x timeout would pin this worker
    // (and the page's "Loading...") for many minutes -- so after a few
    // consecutive failures the remaining rows fall back to untitled/no-tally
    // instead of burning a timeout each.
    let mut consecutive_failures = 0usize;
    const GIVE_UP_AFTER: usize = 3;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let joined = if consecutive_failures < GIVE_UP_AFTER {
            get(&format!("{base}/api/v2/objects/{}", r.proposal_object_id)).ok()
        } else {
            None
        };
        if consecutive_failures < GIVE_UP_AFTER {
            if joined.is_some() {
                consecutive_failures = 0;
            } else {
                consecutive_failures += 1;
            }
        }
        let (title, body) = joined
            .and_then(|s| serde_json::from_str::<ObjectResp>(&s).ok())
            .and_then(|o| payload_texts(&o.payload_b64))
            .unwrap_or_else(|| ("(untitled proposal)".to_string(), String::new()));
        let tally = if consecutive_failures < GIVE_UP_AFTER {
            get(&format!("{base}/api/v2/proposals/{}/tally", r.proposal_object_id)).ok()
        } else {
            None
        }
        .and_then(|s| serde_json::from_str::<TallyResp>(&s).ok())
            .map(|t| TallyView {
                yes_weight: t.yes_weight,
                no_weight: t.no_weight,
                abstain_weight: t.abstain_weight,
                total_weight: t.total_weight,
                vote_count: t.vote_count,
                quorum_fraction: t.quorum_fraction,
                electorate: t.electorate,
                quorum_met: t.quorum_met,
                passing: t.passing,
            });
        out.push(ProposalView {
            id: r.proposal_object_id,
            proposer_did: r.proposer_did,
            proposal_type: r.proposal_type,
            scope: r.scope,
            opens_at: r.opens_at,
            closes_at: r.closes_at,
            title,
            body,
            tally,
        });
    }
    Ok(out)
}

/// Build + Dilithium-sign a `vote_v1` object referencing `proposal_id`.
/// Uses the exact same ObjectBuilder/keypair code the relay verifies with.
fn build_vote(seed32: &[u8], proposal_id: &str, choice: &str) -> Result<crate::relay::core::object::Object, String> {
    use crate::relay::core::encoding::{cbor_map, cbor_text};
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    let kp = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    let payload = cbor_map(vec![("choice", cbor_text(choice))]);
    crate::relay::core::object::ObjectBuilder::new("vote_v1")
        .reference(proposal_id)
        .created_at(now_ms() as u64)
        .payload_cbor(&payload)
        .map_err(|e| format!("encode vote: {e:?}"))?
        .sign(&kp)
        .map_err(|e| format!("sign vote: {e:?}"))
}

/// Build + Dilithium-sign a `proposal_v1` object.
fn build_proposal(
    seed32: &[u8],
    proposal_type: &str,
    scope: &str,
    title: &str,
    body: &str,
    window_days: f32,
) -> Result<crate::relay::core::object::Object, String> {
    use crate::relay::core::encoding::{cbor_int, cbor_map, cbor_text};
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    let kp = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    let now = now_ms() as u64;
    // Clamp FIRST, then convert whole+fraction together in f64 -- the previous
    // split (`max(0.04) as u64` + unclamped `.fract()`) truncated a sub-day
    // clamp to 0 and produced an instantly-closed proposal for inputs <= 0.
    let days = f64::from(window_days).max(0.04);
    let closes = now + (days * 86_400_000.0) as u64;
    let payload = cbor_map(vec![
        ("proposal_type", cbor_text(proposal_type)),
        ("scope", cbor_text(scope)),
        ("title", cbor_text(title)),
        ("body", cbor_text(body)),
        ("opens_at", cbor_int(now)),
        ("closes_at", cbor_int(closes)),
    ]);
    crate::relay::core::object::ObjectBuilder::new("proposal_v1")
        .created_at(now)
        .payload_cbor(&payload)
        .map_err(|e| format!("encode proposal: {e:?}"))?
        .sign(&kp)
        .map_err(|e| format!("sign proposal: {e:?}"))
}

/// The SignedObjectSubmission JSON the relay's POST /api/v2/objects expects.
/// Field names are locked by the `submission_json_matches_the_relay_wire_struct`
/// test, which deserializes this into the relay's own struct.
fn submission_json(obj: &crate::relay::core::object::Object) -> serde_json::Value {
    use base64::Engine as _;
    let b64 = &base64::engine::general_purpose::STANDARD;
    serde_json::json!({
        "protocol_version": obj.protocol_version,
        "object_type": obj.object_type,
        "space_id": obj.space_id,
        "channel_id": obj.channel_id,
        "author_public_key_b64": b64.encode(&obj.author_public_key),
        "created_at": obj.created_at,
        "references": obj.references,
        "payload_schema_version": obj.payload_schema_version,
        "payload_encoding": obj.payload_encoding,
        "payload_b64": b64.encode(&obj.payload),
        "signature_b64": b64.encode(&obj.signature),
    })
}

/// Blocking: POST a signed object to the relay as a SignedObjectSubmission.
fn post_signed_object(base: &str, obj: &crate::relay::core::object::Object) -> Result<(), String> {
    let submission = submission_json(obj);
    match ureq::post(&format!("{base}/api/v2/objects"))
        .timeout(std::time::Duration::from_secs(10))
        .set("Content-Type", "application/json")
        .send_string(&submission.to_string())
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, resp)) => {
            let body: String = resp.into_string().unwrap_or_default().chars().take(200).collect();
            Err(format!("server rejected ({code}): {body}"))
        }
        Err(e) => Err(format!("network: {e}")),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let base = state.server_url.trim_end_matches('/').to_string();

    // ── Staleness guard (adversarial review 2026-07-01): proposals belonging
    // to a DIFFERENT server are cleared before anything renders. Without this,
    // server A's proposals stayed on screen after switching to server B, their
    // vote buttons stayed live, and a click posted a validly-signed vote
    // referencing A's proposal to B -- which stores it as an orphan and 200s,
    // so the user believed a vote A never received was "recorded". Same rule
    // the Donate money-routing fix follows.
    if !state.governance_fetched_for.is_empty() && state.governance_fetched_for != base {
        state.governance_proposals.clear();
        state.governance_fetched_for.clear();
        state.governance_error.clear();
    }

    // ── Land finished background work (fetch / vote / propose) ──
    if let Some((from_url, rx)) = &state.governance_rx {
        match rx.try_recv() {
            Ok(res) => {
                let from_url = from_url.clone();
                state.governance_rx = None;
                // A result from a server we've since switched away from is dropped.
                if from_url == base {
                    // Tag the data's origin even on error (stops a refetch loop
                    // against a failing server; Refresh retries explicitly).
                    state.governance_fetched_for = from_url;
                    match res {
                        Ok(props) => {
                            state.governance_proposals = props;
                            state.governance_error.clear();
                        }
                        Err(e) => state.governance_error = format!("Could not load proposals: {e}"),
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.governance_rx = None;
                state.governance_error = "Proposal fetch worker exited unexpectedly.".to_string();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    if let Some(rx) = &state.governance_vote_rx {
        match rx.try_recv() {
            Ok(Ok((pid, choice))) => {
                state.governance_vote_rx = None;
                state.governance_my_votes.insert(pid, choice);
                // Honest wording: the server keeps the FIRST vote per voter
                // (INSERT OR IGNORE) but still 200s a duplicate submission, so
                // a re-vote from an earlier session cannot be detected here.
                state.governance_vote_status =
                    "Vote submitted. If this identity already voted on this proposal in an earlier session, the original vote stands."
                        .to_string();
                // Refetch so the tally reflects the new vote. governance_refresh
                // is cleared only when a fetch SPAWNS, so this survives a fetch
                // that is already in flight (no clobbered invalidation).
                state.governance_refresh = true;
            }
            Ok(Err(e)) => {
                state.governance_vote_rx = None;
                state.governance_vote_status = format!("Vote failed: {e}");
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.governance_vote_rx = None;
                state.governance_vote_status = "Vote worker exited unexpectedly.".to_string();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    if let Some(rx) = &state.governance_propose_rx {
        match rx.try_recv() {
            Ok(Ok(())) => {
                state.governance_propose_rx = None;
                state.governance_vote_status = "Proposal submitted.".to_string();
                state.governance_new_title.clear();
                state.governance_new_body.clear();
                state.governance_show_propose = false;
                state.governance_refresh = true;
            }
            Ok(Err(e)) => {
                state.governance_propose_rx = None;
                state.governance_vote_status = format!("Proposal failed: {e}");
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.governance_propose_rx = None;
                state.governance_vote_status = "Proposal worker exited unexpectedly.".to_string();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    // ── Auto-fetch on first view / server change / requested refresh. An
    // in-flight fetch aimed at a DIFFERENT server is replaced outright (its
    // receiver drops; the late send fails harmlessly) so a slow or hung old
    // server can never block loading the new one.
    let fetch_wanted = state.governance_fetched_for != base || state.governance_refresh;
    let fetching_this_server = state.governance_rx.as_ref().map_or(false, |(u, _)| *u == base);
    if state.server_connected && !base.is_empty() && fetch_wanted && !fetching_this_server {
        let (tx, rx) = std::sync::mpsc::channel();
        let url = base.clone();
        std::thread::spawn(move || {
            let _ = tx.send(fetch_proposals_blocking(&url));
        });
        state.governance_rx = Some((base.clone(), rx));
        state.governance_refresh = false;
    }
    if state.governance_rx.is_some()
        || state.governance_vote_rx.is_some()
        || state.governance_propose_rx.is_some()
    {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    let can_sign = state.private_key_bytes.is_some();
    let now = now_ms();
    let mut vote_click: Option<(String, String)> = None;

    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "Civic Participation");
                ui.label(
                    RichText::new(
                        "Local-scope proposals run on this server. Civilization-scope proposals \
                         (Accord amendments, federation floor policy) federate to all servers and \
                         need federation-wide quorum to pass. Vote weight equals your trust score \
                         at vote time, capped at 0.95, so no single high-trust voter can dominate. \
                         AI agents are excluded from tallies per the Accord: votes require \
                         sentient consent.",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_md);

                // Scope + open/all filters, refresh, and the new-proposal toggle.
                ui.horizontal(|ui| {
                    let scopes = ["All scopes", "Local", "Civilization"];
                    widgets::tab_bar(ui, theme, &scopes, &mut state.governance_scope_tab);
                    ui.add_space(theme.spacing_sm);
                    let filters = ["Open", "All"];
                    widgets::tab_bar(ui, theme, &filters, &mut state.governance_filter_tab);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::secondary_button(ui, theme, "Refresh") {
                            // A flag, not a state clear: survives an in-flight
                            // fetch (the spawn logic runs it as soon as the
                            // current one lands) instead of being a no-op.
                            state.governance_refresh = true;
                        }
                        let label = if state.governance_show_propose { "Close form" } else { "New proposal" };
                        if widgets::primary_button(ui, theme, label) {
                            state.governance_show_propose = !state.governance_show_propose;
                        }
                    });
                });
                ui.add_space(theme.spacing_sm);

                // Status lines.
                if !state.governance_vote_status.is_empty() {
                    let col = if state.governance_vote_status.contains("failed")
                        || state.governance_vote_status.contains("exited")
                    {
                        theme.danger()
                    } else {
                        theme.success()
                    };
                    ui.label(RichText::new(&state.governance_vote_status).color(col).size(theme.font_size_small));
                }
                if !state.governance_error.is_empty() {
                    ui.label(RichText::new(&state.governance_error).color(theme.danger()).size(theme.font_size_small));
                }
                if state.governance_rx.is_some() {
                    ui.label(RichText::new("Loading proposals...").color(theme.text_muted()).size(theme.font_size_small));
                }

                // ── New-proposal form ──
                if state.governance_show_propose {
                    ui.add_space(theme.spacing_sm);
                    widgets::card_with_header(ui, theme, "New proposal", |ui| {
                        if !can_sign {
                            ui.label(
                                RichText::new("Unlock your identity to submit a proposal (it is signed with your Dilithium key).")
                                    .color(theme.warning())
                                    .size(theme.font_size_small),
                            );
                        }
                        ui.label(RichText::new("Title").color(theme.text_secondary()).size(theme.font_size_small));
                        ui.add(egui::TextEdit::singleline(&mut state.governance_new_title).desired_width(420.0));
                        ui.add_space(theme.spacing_xs);
                        ui.label(RichText::new("Body").color(theme.text_secondary()).size(theme.font_size_small));
                        ui.add(
                            egui::TextEdit::multiline(&mut state.governance_new_body)
                                .desired_width(420.0)
                                .desired_rows(4),
                        );
                        ui.add_space(theme.spacing_xs);
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("gov_new_type")
                                .selected_text(PROPOSAL_TYPES[state.governance_new_type_idx.min(2)].1)
                                .show_ui(ui, |ui| {
                                    for (i, (_, label)) in PROPOSAL_TYPES.iter().enumerate() {
                                        ui.selectable_value(&mut state.governance_new_type_idx, i, *label);
                                    }
                                });
                            egui::ComboBox::from_id_salt("gov_new_scope")
                                .selected_text(SCOPES[state.governance_new_scope_idx.min(1)].1)
                                .show_ui(ui, |ui| {
                                    for (i, (_, label)) in SCOPES.iter().enumerate() {
                                        ui.selectable_value(&mut state.governance_new_scope_idx, i, *label);
                                    }
                                });
                            ui.add(
                                egui::Slider::new(&mut state.governance_new_days, 1.0..=30.0)
                                    .step_by(1.0)
                                    .fixed_decimals(0)
                                    .suffix(" days")
                                    .text("voting window"),
                            );
                        });
                        ui.add_space(theme.spacing_sm);
                        let ready = can_sign
                            && state.server_connected
                            && state.governance_propose_rx.is_none()
                            && !state.governance_new_title.trim().is_empty()
                            && !state.governance_new_body.trim().is_empty();
                        if state.governance_propose_rx.is_some() {
                            ui.label(RichText::new("Submitting proposal...").color(theme.text_muted()).size(theme.font_size_small));
                        }
                        ui.add_enabled_ui(ready, |ui| {
                            if widgets::primary_button(ui, theme, "Submit proposal") {
                                if let Some(seed) = state.private_key_bytes.clone() {
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    let url = base.clone();
                                    let ptype = PROPOSAL_TYPES[state.governance_new_type_idx.min(2)].0.to_string();
                                    let scope = SCOPES[state.governance_new_scope_idx.min(1)].0.to_string();
                                    let title = state.governance_new_title.trim().to_string();
                                    let body = state.governance_new_body.trim().to_string();
                                    let days = state.governance_new_days;
                                    std::thread::spawn(move || {
                                        let res = build_proposal(&seed, &ptype, &scope, &title, &body, days)
                                            .and_then(|obj| post_signed_object(&url, &obj));
                                        let _ = tx.send(res);
                                    });
                                    state.governance_propose_rx = Some(rx);
                                    state.governance_vote_status.clear();
                                }
                            }
                        });
                    });
                }

                ui.add_space(theme.spacing_md);

                // ── Proposal feed ──
                if !state.server_connected {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("Connect to a server to see its proposals and vote.")
                                .color(theme.text_muted()),
                        );
                    });
                } else if state.governance_proposals.is_empty() && state.governance_rx.is_none() {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("No proposals yet. Use \"New proposal\" to start the first one.")
                                .color(theme.text_muted()),
                        );
                    });
                }

                // The feed renders ONLY while connected: proposals held from
                // before a disconnect must not present live vote buttons (the
                // vote would go nowhere or to the wrong server).
                let proposals = if state.server_connected {
                    state.governance_proposals.clone()
                } else {
                    Vec::new()
                };
                let mut shown = 0usize;
                for p in &proposals {
                    // Scope tab: 0 = all, 1 = local, 2 = civilization.
                    if state.governance_scope_tab == 1 && p.scope != "local" { continue; }
                    if state.governance_scope_tab == 2 && p.scope != "civilization" { continue; }
                    let (is_open, window_label) = fmt_window(p.opens_at, p.closes_at, now);
                    // Filter tab: 0 = open only, 1 = all.
                    if state.governance_filter_tab == 0 && !is_open { continue; }
                    shown += 1;

                    let my_vote = state.governance_my_votes.get(&p.id).cloned();
                    widgets::expandable_row(
                        ui,
                        ("proposal", p.id.as_str()),
                        false,
                        None,
                        |ui| {
                            let (chip, chip_col) = if is_open {
                                ("OPEN", theme.success())
                            } else {
                                ("CLOSED", theme.text_muted())
                            };
                            ui.label(RichText::new(chip).strong().color(chip_col).size(theme.font_size_small));
                            ui.add_space(theme.spacing_sm);
                            ui.label(RichText::new(&p.scope).color(theme.text_muted()).size(theme.font_size_small));
                            ui.add_space(theme.spacing_sm);
                            ui.add(
                                egui::Label::new(RichText::new(&p.title).strong().color(theme.text_primary()))
                                    .wrap_mode(egui::TextWrapMode::Extend),
                            );
                            ui.add_space(theme.spacing_sm);
                            let mut meta = window_label.clone();
                            if let Some(t) = &p.tally {
                                meta = format!("{meta} \u{00b7} {} votes", t.vote_count);
                                // Data-driven verdicts (v0.759): the server's
                                // proposal-type rules say what this needs.
                                if let (Some(qm), Some(pass)) = (t.quorum_met, t.passing) {
                                    let verdict = match (qm, pass) {
                                        (true, true) => "quorum met \u{00b7} passing",
                                        (true, false) => "quorum met \u{00b7} not passing",
                                        (false, _) => "below quorum",
                                    };
                                    meta = format!("{meta} \u{00b7} {verdict}");
                                }
                            }
                            ui.label(RichText::new(meta).color(theme.text_muted()).size(theme.font_size_small));
                        },
                        |ui| {
                            ui.add_space(theme.spacing_xs);
                            if !p.body.is_empty() {
                                ui.label(RichText::new(&p.body).color(theme.text_secondary()));
                                ui.add_space(theme.spacing_xs);
                            }
                            ui.label(
                                RichText::new(format!("Type: {}   Proposer: {}", p.proposal_type, p.proposer_did))
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                            );
                            if let Some(t) = &p.tally {
                                ui.add_space(theme.spacing_xs);
                                let total = t.total_weight.max(f64::EPSILON);
                                for (label, weight, col) in [
                                    ("Yes", t.yes_weight, theme.success()),
                                    ("No", t.no_weight, theme.danger()),
                                    ("Abstain", t.abstain_weight, theme.text_muted()),
                                ] {
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [56.0, 14.0],
                                            egui::Label::new(
                                                RichText::new(label).color(theme.text_secondary()).size(theme.font_size_small),
                                            ),
                                        );
                                        let frac = if t.total_weight > 0.0 { (weight / total) as f32 } else { 0.0 };
                                        ui.add(
                                            egui::ProgressBar::new(frac)
                                                .desired_width(180.0)
                                                .fill(col)
                                                .text(format!("{weight:.2}")),
                                        );
                                    });
                                }
                                ui.label(
                                    RichText::new(format!(
                                        "{} votes, {:.2} total weight (weight = trust score, capped 0.95)",
                                        t.vote_count, t.total_weight
                                    ))
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                                );
                                // Data-driven quorum requirement (v0.759).
                                if let (Some(qf), Some(el)) = (t.quorum_fraction, t.electorate) {
                                    let needed = ((qf * el as f64).ceil() as i64).max(1);
                                    ui.label(
                                        RichText::new(format!(
                                            "Quorum: {needed} of {el} members must vote ({} have)",
                                            t.vote_count
                                        ))
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                    );
                                }
                            }
                            ui.add_space(theme.spacing_xs);
                            if let Some(choice) = &my_vote {
                                ui.label(
                                    RichText::new(format!("Your vote: {choice} (votes are final)"))
                                        .color(theme.accent())
                                        .size(theme.font_size_small),
                                );
                            } else if is_open {
                                if !can_sign {
                                    ui.label(
                                        RichText::new("Unlock your identity to vote.")
                                            .color(theme.text_muted())
                                            .size(theme.font_size_small),
                                    );
                                } else if state.governance_vote_rx.is_some() {
                                    ui.label(
                                        RichText::new("Submitting vote...")
                                            .color(theme.text_muted())
                                            .size(theme.font_size_small),
                                    );
                                } else {
                                    ui.horizontal(|ui| {
                                        for choice in ["yes", "no", "abstain"] {
                                            let label = match choice {
                                                "yes" => "Vote Yes",
                                                "no" => "Vote No",
                                                _ => "Abstain",
                                            };
                                            if widgets::secondary_button(ui, theme, label) {
                                                vote_click = Some((p.id.clone(), choice.to_string()));
                                            }
                                        }
                                        ui.label(
                                            RichText::new("Votes are final and cannot be changed.")
                                                .color(theme.text_muted())
                                                .size(theme.font_size_small),
                                        );
                                    });
                                }
                            }
                        },
                    );
                }
                if shown == 0 && !state.governance_proposals.is_empty() {
                    ui.label(
                        RichText::new("No proposals match the current filters.")
                            .color(theme.text_muted())
                            .size(theme.font_size_small),
                    );
                }

                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("Same view on the web: united-humanity.us/governance")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            });
        });

    // Spawn the vote submission decided during rendering (kept outside the
    // closures so the borrow of the cloned proposal list has ended). Gated on
    // server_connected: a vote must only ever go to the server whose proposal
    // list is on screen.
    if let Some((pid, choice)) = vote_click {
        if !state.server_connected {
            state.governance_vote_status = "Not connected -- vote not sent.".to_string();
        } else if let Some(seed) = state.private_key_bytes.clone() {
            let (tx, rx) = std::sync::mpsc::channel();
            let url = base.clone();
            let pid_for_result = pid.clone();
            let choice_for_result = choice.clone();
            std::thread::spawn(move || {
                let res = build_vote(&seed, &pid, &choice)
                    .and_then(|obj| post_signed_object(&url, &obj))
                    .map(|_| (pid_for_result, choice_for_result));
                let _ = tx.send(res);
            });
            state.governance_vote_rx = Some(rx);
            state.governance_vote_status = "Submitting vote...".to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vote object the native client builds must pass the EXACT verification
    /// the relay runs (verify_signature = Dilithium over the canonical CBOR with
    /// a zeroed signature placeholder). This is the whole point of building it
    /// with the in-crate ObjectBuilder: signing can never drift from verifying.
    #[test]
    fn built_vote_passes_relay_verification_and_carries_the_choice() {
        let seed = [7u8; 32];
        let obj = build_vote(&seed, "abc123", "yes").expect("vote builds");
        assert_eq!(obj.object_type, "vote_v1");
        assert_eq!(obj.references, vec!["abc123".to_string()]);
        obj.verify_signature().expect("relay-side verification accepts the native-built vote");
        let value = crate::relay::core::encoding::from_canonical_bytes(&obj.payload).expect("payload decodes");
        let ciborium::Value::Map(entries) = value else { panic!("payload is a map") };
        let choice = entries.iter().find_map(|(k, v)| match (k, v) {
            (ciborium::Value::Text(k), ciborium::Value::Text(v)) if k == "choice" => Some(v.clone()),
            _ => None,
        });
        assert_eq!(choice.as_deref(), Some("yes"));
    }

    /// Same guarantee for proposals, plus the payload carries every field the
    /// relay's index_proposal requires (proposal_type/scope/title/body/opens/closes).
    #[test]
    fn built_proposal_passes_relay_verification_with_required_fields() {
        let seed = [9u8; 32];
        let obj = build_proposal(&seed, "local_rule", "local", "Quiet hours", "No horns after 22:00", 7.0)
            .expect("proposal builds");
        assert_eq!(obj.object_type, "proposal_v1");
        obj.verify_signature().expect("relay-side verification accepts the native-built proposal");
        let value = crate::relay::core::encoding::from_canonical_bytes(&obj.payload).expect("payload decodes");
        let ciborium::Value::Map(entries) = value else { panic!("payload is a map") };
        let keys: Vec<String> = entries.iter().filter_map(|(k, _)| match k {
            ciborium::Value::Text(t) => Some(t.clone()),
            _ => None,
        }).collect();
        for required in ["proposal_type", "scope", "title", "body", "opens_at", "closes_at"] {
            assert!(keys.iter().any(|k| k == required), "payload missing {required}");
        }
    }

    #[test]
    fn payload_texts_round_trips_title_and_body() {
        use crate::relay::core::encoding::{cbor_map, cbor_text, to_canonical_bytes};
        use base64::Engine as _;
        let payload = cbor_map(vec![
            ("title", cbor_text("Test title")),
            ("body", cbor_text("Test body")),
            ("scope", cbor_text("local")),
        ]);
        let bytes = to_canonical_bytes(&payload).expect("encodes");
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let (title, body) = payload_texts(&b64).expect("decodes");
        assert_eq!(title, "Test title");
        assert_eq!(body, "Test body");
    }

    /// The hand-written wire JSON must deserialize into the relay's OWN
    /// `SignedObjectSubmission` struct -- locks the field names/types so the
    /// client and the POST /api/v2/objects handler can't silently drift.
    #[test]
    fn submission_json_matches_the_relay_wire_struct() {
        use base64::Engine as _;
        let seed = [3u8; 32];
        let obj = build_vote(&seed, "deadbeef", "no").expect("vote builds");
        let json = submission_json(&obj);
        let parsed: crate::relay::api_v2_objects::SignedObjectSubmission =
            serde_json::from_value(json).expect("relay wire struct accepts the client JSON");
        assert_eq!(parsed.object_type, "vote_v1");
        assert_eq!(parsed.references, vec!["deadbeef".to_string()]);
        let b64 = &base64::engine::general_purpose::STANDARD;
        assert_eq!(b64.decode(&parsed.payload_b64).unwrap(), obj.payload);
        assert_eq!(b64.decode(&parsed.signature_b64).unwrap(), obj.signature);
        assert_eq!(b64.decode(&parsed.author_public_key_b64).unwrap(), obj.author_public_key);
    }

    /// FULL LOOP against the real relay storage: a natively-built proposal
    /// stores + indexes, a natively-built vote referencing it stores + tallies.
    /// This is the end-to-end guarantee (minus HTTP transport, which the wire
    /// test above covers) that the native Governance page's write path works
    /// against the actual server logic.
    #[test]
    fn native_built_proposal_and_vote_round_trip_through_relay_storage() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_gov_page_test_{pid}_{nanos}.db"));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");

        let proposer_seed = [11u8; 32];
        let proposal = build_proposal(&proposer_seed, "local_rule", "local", "Test", "Body", 7.0)
            .expect("proposal builds");
        db.put_signed_object(&proposal, None).expect("proposal stores");
        let pid_hex = proposal.object_id().expect("id").to_hex();
        let indexed = db.get_proposal(&pid_hex).expect("query").expect("indexed");
        assert_eq!(indexed.proposal_type, "local_rule");
        assert_eq!(indexed.scope, "local");

        let voter_seed = [12u8; 32];
        let vote = build_vote(&voter_seed, &pid_hex, "yes").expect("vote builds");
        db.put_signed_object(&vote, None).expect("vote stores");
        let tally = db.tally_proposal(&pid_hex).expect("tally");
        assert_eq!(tally.vote_count, 1, "the native-built vote landed in the tally");
    }

    /// The minimum-window clamp must actually hold: a zero/negative window
    /// still yields a proposal that is OPEN for a while (review nit: the old
    /// clamp truncated to closes_at == opens_at, an instantly-closed proposal).
    #[test]
    fn proposal_window_clamp_prevents_instantly_closed_proposals() {
        let seed = [4u8; 32];
        for bad_days in [0.0f32, -3.0] {
            let obj = build_proposal(&seed, "local_rule", "local", "t", "b", bad_days).expect("builds");
            let value = crate::relay::core::encoding::from_canonical_bytes(&obj.payload).expect("decodes");
            let ciborium::Value::Map(entries) = value else { panic!("map") };
            let get_int = |name: &str| -> i128 {
                entries.iter().find_map(|(k, v)| match (k, v) {
                    (ciborium::Value::Text(k), ciborium::Value::Integer(n)) if k == name => Some(i128::from(*n)),
                    _ => None,
                }).expect(name)
            };
            let opens = get_int("opens_at");
            let closes = get_int("closes_at");
            assert!(closes > opens + 3_000_000, "window_days={bad_days} must clamp to a real voting window (got {}ms)", closes - opens);
        }
        // And a fractional window converts exactly: 7.5 days.
        let obj = build_proposal(&seed, "local_rule", "local", "t", "b", 7.5).expect("builds");
        let value = crate::relay::core::encoding::from_canonical_bytes(&obj.payload).expect("decodes");
        let ciborium::Value::Map(entries) = value else { panic!("map") };
        let get_int = |name: &str| -> i128 {
            entries.iter().find_map(|(k, v)| match (k, v) {
                (ciborium::Value::Text(k), ciborium::Value::Integer(n)) if k == name => Some(i128::from(*n)),
                _ => None,
            }).expect(name)
        };
        assert_eq!(get_int("closes_at") - get_int("opens_at"), 7 * 86_400_000 + 43_200_000);
    }

    #[test]
    fn window_formatting_tracks_open_state() {
        let now = 1_000_000_000_000i64;
        let hour = 3_600_000i64;
        let (open, label) = fmt_window(now - hour, now + 25 * hour, now);
        assert!(open);
        assert_eq!(label, "closes in 25h");
        let (open, label) = fmt_window(now - 2 * hour, now - hour, now);
        assert!(!open);
        assert_eq!(label, "closed");
        let (open, label) = fmt_window(now + 3 * 24 * hour, now + 10 * 24 * hour, now);
        assert!(!open, "not open before opens_at");
        assert_eq!(label, "opens in 3d");
    }
}
