//! Settings > Account > Recovery through trusted people (2026-10-20): what it
//! is, that it is not finished, and the two lookups that work today.
//!
//! It was a page of its own (`gui/pages/recovery.rs`, `GuiPage::Recovery`),
//! reached only from the Platform tab, until that tab was retired on
//! 2026-10-20. It sits here now, right after Security and beside the recovery
//! phrase it would one day rebuild, and its words were rewritten for people
//! rather than developers.
//!
//! What the server has is lookups only (`src/relay/api_v2_recovery.rs`): an
//! account's recovery setup (how many guardians, how many of them it takes,
//! who they are) and the pieces a guardian keeps for other people. There is no
//! screen to choose guardians or split the phrase into pieces, so nobody can
//! set this up yet, and the words say so plainly (docs/PRIORITIES.md,
//! "Recovery through trusted people is half built"). Finishing it needs no key
//! change: guardians hand back their pieces, the phrase is put back together,
//! and restoring from it gives back the same account.
//!
//! The two lookups used to set flags nothing read, so on the desktop app they
//! did nothing. They now ask the connected server, on a thread, and show the
//! answer. An account is named by the id the server files recovery under:
//! `did:hum:` and a short fingerprint of the account's key, the same id the
//! website shows ([`account_id`]). Left empty, a lookup asks about your own.
//!
//! The website's page is web/pages/recovery.html, linked from its Settings.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use egui::{Color32, RichText};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button};
use crate::gui::GuiState;

/// The subsection's title, as Settings draws it.
pub const TITLE: &str = "Recovery through trusted people";

/// An account's recovery setup as the server reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    /// How many guardians it takes together.
    pub needed: u32,
    /// How many guardians the account has.
    pub guardians_total: u32,
    /// The guardians' account ids.
    pub guardians: Vec<String>,
}

/// One piece of someone's recovery phrase that a guardian keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct HeldPiece {
    /// Whose phrase the piece is part of (an account id).
    pub owner: String,
    /// How many of that person's guardians it takes together.
    pub needed: u32,
    /// How many guardians that person has.
    pub guardians_total: u32,
}

/// One lookup: the account typed, the answer on its way, and the answer.
pub struct Lookup<T> {
    /// The account to ask about, as typed; empty means your own.
    pub account: String,
    pending: Option<Receiver<Result<T, String>>>,
    /// The last answer: the result, or why there is none.
    pub answer: Option<Result<T, String>>,
}

impl<T> Default for Lookup<T> {
    fn default() -> Self {
        Self { account: String::new(), pending: None, answer: None }
    }
}

impl<T: Send + 'static> Lookup<T> {
    fn start(&mut self, fetch: impl FnOnce() -> Result<T, String> + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.answer = None;
        std::thread::spawn(move || {
            let _ = tx.send(fetch());
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.pending else { return };
        match rx.try_recv() {
            Ok(answer) => {
                self.answer = Some(answer);
                self.pending = None;
            }
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(200)),
            Err(TryRecvError::Disconnected) => {
                self.answer = Some(Err("the lookup stopped before it had an answer".to_string()));
                self.pending = None;
            }
        }
    }

    /// Whether an answer is on its way.
    pub fn waiting(&self) -> bool {
        self.pending.is_some()
    }
}

/// The subsection's state (`GuiState::recovery`).
#[derive(Default)]
pub struct RecoveryLookups {
    /// Does an account have guardians? `Ok(None)` means it has none here.
    pub setup: Lookup<Option<Setup>>,
    /// The pieces an account keeps for other people.
    pub pieces: Lookup<Vec<HeldPiece>>,
}

/// The id the server files recovery under for an account's public key (the
/// Dilithium3 key, hex): `did:hum:` and the key's short fingerprint.
pub fn account_id(public_key_hex: &str) -> Option<String> {
    let bytes = hex::decode(public_key_hex.trim()).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(crate::relay::core::did::did_for_pubkey(&bytes))
}

/// Whether `s` has the shape of an account id. The fingerprint part is base58,
/// letters and digits only, so the id goes into a lookup's address as it is.
fn is_account_id(s: &str) -> bool {
    match s.strip_prefix("did:hum:") {
        Some(rest) => !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric()),
        None => false,
    }
}

/// The account a lookup asks about: what was typed, or your own when nothing
/// was. `Err` says why there is none, in words for the person.
fn chosen(typed: &str, mine: &Option<String>) -> Result<String, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return mine.clone().ok_or_else(|| {
            "There is no identity on this device yet, so there is nothing of yours to look up.".to_string()
        });
    }
    if is_account_id(typed) {
        Ok(typed.to_string())
    } else {
        Err("That is not an account ID. One starts with did:hum: followed by letters and numbers.".to_string())
    }
}

/// GET a recovery route. `Ok(None)` is the server's "not found".
fn get(url: &str) -> Result<Option<serde_json::Value>, String> {
    match ureq::get(url).timeout(Duration::from_secs(8)).call() {
        Ok(resp) => {
            let body = resp.into_string().map_err(|e| format!("its answer could not be read ({e})"))?;
            serde_json::from_str(&body).map(Some).map_err(|e| format!("its answer could not be read ({e})"))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(code, _)) => Err(format!("the server answered with an error ({code})")),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    }
}

/// An account's setup, from the server's JSON (`RecoverySetup`).
pub(crate) fn parse_setup(v: &serde_json::Value) -> Option<Setup> {
    Some(Setup {
        needed: v.get("threshold")?.as_u64()? as u32,
        guardians_total: v.get("total_shares")?.as_u64()? as u32,
        guardians: v
            .get("guardians")?
            .as_array()?
            .iter()
            .filter_map(|g| g.as_str().map(String::from))
            .collect(),
    })
}

/// The pieces a guardian keeps, from the server's JSON (a list of
/// `RecoveryShareIndex`).
pub(crate) fn parse_pieces(v: &serde_json::Value) -> Vec<HeldPiece> {
    v.as_array()
        .map(|list| {
            list.iter()
                .filter_map(|p| {
                    Some(HeldPiece {
                        owner: p.get("holder_did")?.as_str()?.to_string(),
                        needed: p.get("threshold")?.as_u64()? as u32,
                        guardians_total: p.get("total_shares")?.as_u64()? as u32,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

fn small(ui: &mut egui::Ui, theme: &Theme, text: &str, color: Color32) {
    ui.label(RichText::new(text).color(color).size(theme.font_size_small));
}

fn id_line(ui: &mut egui::Ui, theme: &Theme, id: &str) {
    ui.label(RichText::new(id).monospace().color(theme.text_secondary()).size(theme.font_size_small));
}

/// The subsection, drawn by Settings > Account right after Security.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let ctx = ui.ctx().clone();
    state.recovery.setup.poll(&ctx);
    state.recovery.pieces.poll(&ctx);
    let mine = account_id(&state.profile_public_key);
    let base = crate::gui::widgets::image_cache::http_base(&state.active_server());

    widgets::subsection_header(ui, theme, accent, TITLE, "");
    widgets::card(ui, theme, |ui| {
        small(
            ui,
            theme,
            "If you ever lose this device and your recovery phrase too, people you trust could help \
             you back into your account. You would pick a few of them as your guardians. Each would \
             keep one piece of your recovery phrase, locked so that only that guardian can open it, \
             and the server would hold the pieces without being able to read them. One piece alone \
             is not enough: it takes as many of your guardians as you chose, together, to put the \
             phrase back together, and with it you are back in the same account.",
            theme.text_secondary(),
        );
        ui.add_space(theme.spacing_sm);
        small(
            ui,
            theme,
            "Not finished yet. Choosing your guardians and splitting your phrase into pieces has not \
             been built, so nobody can set this up today. Until it is, your recovery phrase is the \
             only way back into your account: keep it written down somewhere safe (Identity, above).",
            theme.warning(),
        );
        ui.add_space(theme.spacing_sm);
        small(
            ui,
            theme,
            "What works today is looking things up on the server you are connected to: whether an \
             account has guardians, and the pieces a guardian keeps for other people.",
            theme.text_muted(),
        );
        if let Some(id) = &mine {
            ui.add_space(theme.spacing_xs);
            ui.horizontal_wrapped(|ui| {
                small(ui, theme, "Your account ID:", theme.text_muted());
                id_line(ui, theme, id);
            });
        }
    });

    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Does an account have guardians?").color(theme.text_primary()).strong());
        ui.add_space(theme.spacing_xs);
        let asked = ui
            .horizontal(|ui| {
                ui.add_sized(
                    [(ui.available_width() - 120.0).max(120.0), theme.input_height],
                    egui::TextEdit::singleline(&mut state.recovery.setup.account)
                        .hint_text("Account ID (yours, if left empty)"),
                );
                Button::primary("Look up").show(ui, theme)
            })
            .inner;
        if asked {
            match chosen(&state.recovery.setup.account, &mine) {
                Ok(id) => {
                    let url = format!("{base}/api/v2/recovery/setup/{id}");
                    state.recovery.setup.start(move || match get(&url)? {
                        None => Ok(None),
                        Some(v) => parse_setup(&v).map(Some).ok_or_else(|| "its answer could not be read".to_string()),
                    });
                }
                Err(why) => state.recovery.setup.answer = Some(Err(why)),
            }
        }
        let lookup = &state.recovery.setup;
        if lookup.waiting() {
            small(ui, theme, "Looking it up...", theme.text_muted());
        } else if let Some(answer) = &lookup.answer {
            match answer {
                Ok(Some(s)) => {
                    small(
                        ui,
                        theme,
                        &format!(
                            "This account has {}, and it takes {} of them to put its phrase back together:",
                            plural(s.guardians_total, "guardian", "guardians"),
                            s.needed
                        ),
                        theme.text_secondary(),
                    );
                    for g in &s.guardians {
                        id_line(ui, theme, g);
                    }
                }
                Ok(None) => small(ui, theme, "This account has no guardians on this server.", theme.text_secondary()),
                Err(why) => small(ui, theme, &format!("Could not look it up: {why}"), theme.warning()),
            }
        }
    });

    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Are you a guardian?").color(theme.text_primary()).strong());
        small(ui, theme, "See the pieces you keep for other people.", theme.text_muted());
        ui.add_space(theme.spacing_xs);
        let asked = ui
            .horizontal(|ui| {
                ui.add_sized(
                    [(ui.available_width() - 190.0).max(120.0), theme.input_height],
                    egui::TextEdit::singleline(&mut state.recovery.pieces.account)
                        .hint_text("Account ID (yours, if left empty)"),
                );
                Button::primary("Show the pieces I keep").show(ui, theme)
            })
            .inner;
        if asked {
            match chosen(&state.recovery.pieces.account, &mine) {
                Ok(id) => {
                    let url = format!("{base}/api/v2/recovery/shares-held-by/{id}");
                    state
                        .recovery
                        .pieces
                        .start(move || Ok(get(&url)?.map(|v| parse_pieces(&v)).unwrap_or_default()));
                }
                Err(why) => state.recovery.pieces.answer = Some(Err(why)),
            }
        }
        let lookup = &state.recovery.pieces;
        if lookup.waiting() {
            small(ui, theme, "Looking it up...", theme.text_muted());
        } else if let Some(answer) = &lookup.answer {
            match answer {
                Ok(pieces) if pieces.is_empty() => {
                    small(ui, theme, "This account keeps no pieces for anyone on this server.", theme.text_secondary())
                }
                Ok(pieces) => {
                    small(
                        ui,
                        theme,
                        &format!("This account keeps {}:", plural(pieces.len() as u32, "piece", "pieces")),
                        theme.text_secondary(),
                    );
                    for p in pieces {
                        id_line(ui, theme, &p.owner);
                        small(
                            ui,
                            theme,
                            &format!(
                                "It needs {} of its {} together.",
                                p.needed,
                                plural(p.guardians_total, "guardian", "guardians")
                            ),
                            theme.text_muted(),
                        );
                    }
                }
                Err(why) => small(ui, theme, &format!("Could not look it up: {why}"), theme.warning()),
            }
        }
    });
    small(ui, theme, "The same lookups on the website: united-humanity.us/recovery", theme.text_muted());
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
