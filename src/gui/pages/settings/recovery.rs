//! Settings > Account > Recovery through trusted people (2026-10-21: finished; the section that
//! said "not finished yet" since 2026-10-20).
//!
//! Three parts, the same on the website (web/pages/recovery.html, web/pages/recovery-app.js):
//! - **Your trusted people.** Choose friends (people you follow who follow you back) and how many
//!   of them must help; the app splits the 32 bytes your recovery phrase writes down into pieces,
//!   locks each to one friend, and sends them. It shows who keeps a piece (read from notes only
//!   you can open), sends new pieces when you change it, and takes them all back.
//! - **Help someone get back in.** Who is asking for your help, the plain guidance to make sure it
//!   is really them outside the app, the code they read you, and your piece sent to their new
//!   device, locked so only that device can open it.
//! - **Lost your device? Ask your trusted people for help.** On a new device: find your account,
//!   open a request, read its code to your trusted people (or let them scan it), and when enough
//!   pieces arrive restore the account through the same path as "Restore from recovery phrase".
//!
//! Every call to the server runs on a thread (`recovery_net.rs`); this file draws and holds the
//! state. The protocol is relay/core/recovery.rs, the design
//! docs/design/recovery-through-trusted-people.md.

use std::collections::{BTreeSet, HashMap};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, RichText};

use super::recovery_net::{self as net, Asking, CodeLookup, Found, Friend, LiveAsk, MySetup, Poll};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button};
use crate::gui::GuiState;
use crate::relay::core::recovery as proto;

/// The subsection's title, as Settings draws it.
pub const TITLE: &str = "Recovery through trusted people";

/// How often a waiting request is looked at.
const POLL_EVERY: Duration = Duration::from_secs(5);

/// Work on a thread: the answer on its way, and the answer.
pub struct Job<T> {
    pending: Option<Receiver<Result<T, String>>>,
    /// The last answer: the result, or why there is none.
    pub answer: Option<Result<T, String>>,
}

impl<T> Default for Job<T> {
    fn default() -> Self {
        Self { pending: None, answer: None }
    }
}

impl<T: Send + 'static> Job<T> {
    fn start(&mut self, work: impl FnOnce() -> Result<T, String> + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.answer = None;
        std::thread::spawn(move || {
            let _ = tx.send(work());
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
                self.answer = Some(Err("the work stopped before it had an answer".to_string()));
                self.pending = None;
            }
        }
    }

}

impl<T> Job<T> {
    /// Whether an answer is on its way.
    pub fn waiting(&self) -> bool {
        self.pending.is_some()
    }

    /// The answer, when it arrived and was a success.
    pub fn ok(&self) -> Option<&T> {
        self.answer.as_ref().and_then(|a| a.as_ref().ok())
    }
}

/// The section's state (`GuiState::recovery`). Nothing here is saved: a request for help lives
/// in memory only, on purpose (closing the app means asking again with a new code).
#[derive(Default)]
pub struct RecoveryState {
    // Your trusted people.
    pub mine: Job<MySetup>,
    /// The account `mine` was read for (read again when it changes).
    mine_for: String,
    pub editor_open: bool,
    /// Friends picked in the editor, by key.
    pub picked: BTreeSet<String>,
    /// How many must help, as chosen in the editor (0 = the suggestion).
    pub needed: usize,
    pub send: Job<String>,
    pub withdraw_armed: bool,
    pub withdraw: Job<String>,
    pub not_me: Job<String>,
    // Help someone get back in.
    pub asking: Job<Vec<Asking>>,
    asking_for: String,
    pub help_code: String,
    pub lookup: Job<CodeLookup>,
    pub help_confirmed: bool,
    pub help_send: Job<String>,
    pub kept: Job<Vec<String>>,
    // Lost your device?
    pub ask_who: String,
    pub find: Job<Vec<Found>>,
    pub open: Job<LiveAsk>,
    pub live: Option<Arc<LiveAsk>>,
    pub poll: Job<Poll>,
    last_poll: Option<Instant>,
    /// What the last look at the request found.
    pub progress: Option<Poll>,
    qr: Option<(String, egui::TextureHandle)>,
    pub restore_note: Option<Result<String, String>>,
}

/// This device's account: the 32 bytes the recovery phrase writes down, and its account id.
fn my_account(state: &GuiState) -> Option<(Vec<u8>, String)> {
    let secret = state.private_key_bytes.clone().filter(|s| s.len() == proto::SECRET_LEN)?;
    // The account id from the key already derived (drawn every frame: no key derivation here).
    let key = hex::decode(state.profile_public_key.trim()).ok().filter(|k| !k.is_empty())?;
    Some((secret, crate::relay::core::did::did_for_pubkey(&key)))
}

/// My friends: people I follow who follow me back, from this device's message store, with the
/// name the app knows them by and their Kyber768 key when one is pinned from their checked key
/// card (engine/key_cards.rs).
pub fn friends(state: &GuiState) -> Vec<Friend> {
    let Some(store) = state.dm_store.as_ref() else { return Vec::new() };
    let mut out: Vec<Friend> = store
        .following()
        .iter()
        .filter(|k| store.is_follower(k))
        .filter_map(|key| {
            let bytes = hex::decode(key).ok().filter(|b| !b.is_empty())?;
            let pinned = crate::engine::key_cards::pinned_for_recovery(state, key);
            Some(Friend {
                key: key.clone(),
                did: crate::relay::core::did::did_for_pubkey(&bytes),
                name: crate::engine::dm::dm_display_name(state, key),
                kyber_b64: pinned.clone().ok().flatten(),
                key_changed: pinned.is_err(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.key.cmp(&b.key)));
    out
}

/// The suggested number who must help: more than half of them, never fewer than two.
pub fn suggested_needed(total: usize) -> usize {
    (total / 2 + 1).clamp(proto::MIN_NEEDED, total.max(proto::MIN_NEEDED))
}

fn small(ui: &mut egui::Ui, theme: &Theme, text: &str, color: Color32) {
    ui.label(RichText::new(text).color(color).size(theme.font_size_small));
}

fn heading(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(text).color(theme.text_primary()).strong());
    ui.add_space(theme.spacing_xs);
}

fn result_line<T>(ui: &mut egui::Ui, theme: &Theme, job: &Job<T>, ok: impl Fn(&T) -> String) {
    if job.waiting() {
        small(ui, theme, "Working on it...", theme.text_muted());
    } else if let Some(answer) = &job.answer {
        match answer {
            Ok(v) => small(ui, theme, &ok(v), theme.success()),
            Err(why) => small(ui, theme, why, theme.warning()),
        }
    }
}

fn day(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

/// The subsection, drawn by Settings > Account right after Security.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let ctx = ui.ctx().clone();
    {
        let r = &mut state.recovery;
        // Sending, taking back or stopping a request changes what the server holds: read the
        // setup again the frame each one finishes.
        let changing = r.send.waiting() || r.withdraw.waiting() || r.not_me.waiting();
        r.mine.poll(&ctx);
        r.send.poll(&ctx);
        r.withdraw.poll(&ctx);
        r.not_me.poll(&ctx);
        if changing && !(r.send.waiting() || r.withdraw.waiting() || r.not_me.waiting()) {
            r.mine_for.clear();
        }
        r.asking.poll(&ctx);
        r.lookup.poll(&ctx);
        r.help_send.poll(&ctx);
        r.kept.poll(&ctx);
        r.find.poll(&ctx);
        r.open.poll(&ctx);
        r.poll.poll(&ctx);
    }
    let base = crate::gui::widgets::image_cache::http_base(&state.active_server());
    let account = my_account(state);

    widgets::subsection_header(ui, theme, accent, TITLE, "");
    draw_intro(ui, theme);
    draw_mine(ui, theme, state, &base, account.as_ref());
    draw_help(ui, theme, state, &base, account.as_ref());
    draw_ask(ui, theme, state, &base, account.as_ref());
    small(ui, theme, "The same on the website: united-humanity.us/recovery", theme.text_muted());
}

fn draw_intro(ui: &mut egui::Ui, theme: &Theme) {
    widgets::card(ui, theme, |ui| {
        small(
            ui,
            theme,
            "If you ever lose your device and your recovery phrase too, people you trust can help you back into \
             your account. You choose a few of your friends. Each keeps one piece of your recovery phrase, locked \
             so that only they can open it. The server keeps the locked pieces without being able to read them, \
             and it is never told who your trusted people are. One piece alone is no use to anyone: it takes as \
             many of them as you chose, together, to put your phrase back together, and with it you are back in \
             the same account.",
            theme.text_secondary(),
        );
        ui.add_space(theme.spacing_xs);
        small(
            ui,
            theme,
            "Choose people who know your voice and your face. Anyone who wants into your account would have to \
             pretend to be you to them, so they should always check by calling you or seeing you before they help.",
            theme.text_muted(),
        );
        small(
            ui,
            theme,
            "Your recovery phrase is still the main way back in: keep it written down somewhere safe (Identity, \
             above). Trusted people are for when that is lost too.",
            theme.text_muted(),
        );
    });
}

fn draw_mine(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, base: &str, account: Option<&(Vec<u8>, String)>) {
    let friends = friends(state);
    widgets::card(ui, theme, |ui| {
        heading(ui, theme, "Your trusted people");
        let Some((secret, did)) = account else {
            small(
                ui,
                theme,
                "This device is not signed in to an account yet, so there is nothing to protect here. (Lost your device? See the last part of this section.)",
                theme.text_muted(),
            );
            return;
        };
        let r = &mut state.recovery;
        if r.mine_for != *did && !r.mine.waiting() {
            r.mine_for = did.clone();
            let (b, s) = (base.to_string(), secret.clone());
            r.mine.start(move || net::load_my_setup(&b, &s));
        }
        let mut standing = false;
        match (&r.mine.answer, r.mine.waiting()) {
            (_, true) => small(ui, theme, "Looking up who keeps your pieces...", theme.text_muted()),
            (Some(Ok(m)), _) => {
                if m.keepers.is_empty() {
                    small(ui, theme, "No one keeps a piece of your recovery phrase yet.", theme.text_secondary());
                } else {
                    standing = true;
                    let short: Vec<String> = m.keepers.iter().map(|k| k.split(" (did:hum:").next().unwrap_or(k).to_string()).collect();
                    small(
                        ui,
                        theme,
                        &format!("{} each keep a piece. It takes {} of them to put your recovery phrase back together.", net::list(&short), m.needed),
                        theme.text_secondary(),
                    );
                    for (i, k) in m.keepers.iter().enumerate() {
                        small(ui, theme, &format!("Piece {}: {k}", i + 1), theme.text_muted());
                    }
                }
                if let Some((have, total)) = m.sending {
                    small(
                        ui,
                        theme,
                        &format!("Sending new pieces: {have} of {total} have arrived. Until all have, the pieces before them stay."),
                        theme.text_muted(),
                    );
                }
                if let Some(since) = m.asked_since {
                    ui.add_space(theme.spacing_xs);
                    small(
                        ui,
                        theme,
                        &format!(
                            "Someone asked your trusted people for help to get into your account (on {}). If that was not you, tell your trusted people not to send anything, and press the button below.",
                            day(since)
                        ),
                        theme.warning(),
                    );
                    if Button::danger("That was not me: stop it").disabled(r.not_me.waiting()).show(ui, theme) {
                        let (b, s) = (base.to_string(), secret.clone());
                        r.not_me.start(move || net::not_me(&b, &s));
                        r.mine_for.clear();
                    }
                    result_line(ui, theme, &r.not_me, |m| m.clone());
                }
            }
            (Some(Err(why)), _) => small(ui, theme, &format!("Could not look it up: {why}"), theme.warning()),
            (None, _) => {}
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            let label = if standing { "Change who keeps the pieces" } else { "Choose trusted people" };
            if Button::primary(label).show(ui, theme) {
                r.editor_open = !r.editor_open;
            }
            if standing && Button::secondary("Take all pieces back").show(ui, theme) {
                r.withdraw_armed = true;
            }
            if Button::secondary("Look again").disabled(r.mine.waiting()).show(ui, theme) {
                r.mine_for.clear();
            }
        });
        if r.withdraw_armed {
            small(
                ui,
                theme,
                "This takes every piece off the server, and nobody can help you back in until you choose trusted people again. \
                 A piece someone already opened could still be with them; pieces from different setups never fit together.",
                theme.warning(),
            );
            ui.horizontal(|ui| {
                if Button::danger("Take them all back").disabled(r.withdraw.waiting()).show(ui, theme) {
                    let (b, s) = (base.to_string(), secret.clone());
                    r.withdraw.start(move || net::withdraw(&b, &s));
                    r.withdraw_armed = false;
                    r.mine_for.clear();
                }
                if Button::secondary("Keep them").show(ui, theme) {
                    r.withdraw_armed = false;
                }
            });
        }
        result_line(ui, theme, &r.withdraw, |m| m.clone());
        if r.editor_open {
            ui.add_space(theme.spacing_sm);
            draw_editor(ui, theme, r, &friends, base, secret);
        }
        result_line(ui, theme, &r.send, |m| m.clone());
    });
}

fn draw_editor(ui: &mut egui::Ui, theme: &Theme, r: &mut RecoveryState, friends: &[Friend], base: &str, secret: &[u8]) {
    small(ui, theme, "Choose from your friends (people you follow who follow you back), between 2 and 10 of them.", theme.text_secondary());
    if friends.is_empty() {
        small(ui, theme, "You have no friends on this device yet. Trusted people are friends: people you follow who follow you back.", theme.text_muted());
        return;
    }
    for f in friends {
        let mut on = r.picked.contains(&f.key);
        let label = if f.key_changed {
            format!("{} (their encryption key changed: accept it in your conversation with them first)", f.name)
        } else if f.kyber_b64.is_some() {
            f.name.clone()
        } else {
            format!("{} (their key card is fetched from the server and checked when you send)", f.name)
        };
        if ui.checkbox(&mut on, RichText::new(label).size(theme.font_size_small)).changed() {
            if on {
                r.picked.insert(f.key.clone());
            } else {
                r.picked.remove(&f.key);
            }
        }
    }
    r.picked.retain(|k| friends.iter().any(|f| &f.key == k));
    let total = r.picked.len();
    if r.needed < proto::MIN_NEEDED || r.needed > total {
        r.needed = suggested_needed(total);
    }
    ui.horizontal(|ui| {
        small(ui, theme, "How many of them must help:", theme.text_secondary());
        egui::ComboBox::from_id_salt("recovery_needed")
            .selected_text(format!("{} of {}", r.needed, total.max(proto::MIN_NEEDED)))
            .show_ui(ui, |ui| {
                for k in proto::MIN_NEEDED..=total.max(proto::MIN_NEEDED) {
                    ui.selectable_value(&mut r.needed, k, format!("{k} of {}", total.max(proto::MIN_NEEDED)));
                }
            });
    });
    small(
        ui,
        theme,
        "Fewer is easier if someone is hard to reach. More is safer if one of them is ever fooled. Changing it later sends new pieces, and the old ones stop counting on the server.",
        theme.text_muted(),
    );
    let ok = (proto::MIN_PEOPLE..=proto::MAX_PEOPLE).contains(&total);
    if !ok {
        small(ui, theme, &format!("Choose between {} and {} of them.", proto::MIN_PEOPLE, proto::MAX_PEOPLE), theme.text_muted());
    }
    ui.horizontal(|ui| {
        if Button::primary("Send the pieces").disabled(!ok || r.send.waiting()).show(ui, theme) {
            let people: Vec<Friend> = friends.iter().filter(|f| r.picked.contains(&f.key)).cloned().collect();
            let (b, s, n) = (base.to_string(), secret.to_vec(), r.needed);
            r.send.start(move || net::send_setup(&b, &s, people, n));
            r.editor_open = false;
            r.mine_for.clear();
        }
        if Button::secondary("Cancel").show(ui, theme) {
            r.editor_open = false;
        }
    });
}

fn draw_help(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, base: &str, account: Option<&(Vec<u8>, String)>) {
    let friends = friends(state);
    let names: HashMap<String, String> = friends.iter().map(|f| (f.did.clone(), f.name.clone())).collect();
    widgets::card(ui, theme, |ui| {
        heading(ui, theme, "Help someone get back in");
        let Some((secret, did)) = account else {
            small(ui, theme, "Open this on the device where you use your account to help someone.", theme.text_muted());
            return;
        };
        let r = &mut state.recovery;
        if r.asking_for != *did && !r.asking.waiting() {
            r.asking_for = did.clone();
            let (b, s, n) = (base.to_string(), secret.clone(), names.clone());
            r.asking.start(move || net::who_is_asking(&b, &s, &n));
        }
        match (&r.asking.answer, r.asking.waiting()) {
            (_, true) => small(ui, theme, "Checking who is asking for help...", theme.text_muted()),
            (Some(Ok(list)), _) if list.is_empty() => {
                small(ui, theme, "Nobody whose piece you keep is asking for help right now.", theme.text_muted())
            }
            (Some(Ok(list)), _) => {
                for a in list {
                    small(
                        ui,
                        theme,
                        &format!("{} is asking for your help to recover their account (since {}).", a.name, day(a.since)),
                        theme.text_primary(),
                    );
                }
            }
            (Some(Err(why)), _) => small(ui, theme, &format!("Could not check: {why}"), theme.warning()),
            (None, _) => {}
        }
        ui.add_space(theme.spacing_xs);
        small(
            ui,
            theme,
            "Before you send anything, make sure it is really them. Call them on a number you already had, or see them \
             in person. Do not trust a text, an email or a message from a new account on its own: someone who wants \
             into their account would send exactly that. Ask them to read you the code their new device shows.",
            theme.warning(),
        );
        let looked = ui
            .horizontal(|ui| {
                ui.add_sized(
                    [(ui.available_width() - 130.0).max(160.0), theme.input_height],
                    egui::TextEdit::singleline(&mut r.help_code).hint_text("The code they read you, like ABCD-EFGH-JKMN-PQRS"),
                );
                Button::secondary("Look it up").disabled(r.lookup.waiting()).show(ui, theme)
            })
            .inner;
        if looked {
            let (b, s, n, code) = (base.to_string(), secret.clone(), names.clone(), r.help_code.clone());
            r.help_confirmed = false;
            r.help_send = Job::default();
            r.lookup.start(move || net::look_up_code(&b, &s, &code, &n));
        }
        if r.lookup.waiting() {
            small(ui, theme, "Looking up the request...", theme.text_muted());
        }
        match &r.lookup.answer {
            Some(Ok(l)) => {
                small(ui, theme, &format!("{} is asking for your help to recover their account.", l.name), theme.text_primary());
                ui.checkbox(
                    &mut r.help_confirmed,
                    RichText::new(format!(
                        "I spoke to {} myself, by a call on a number I already had or face to face, and they read me this code.",
                        l.name
                    ))
                    .size(theme.font_size_small),
                );
                if Button::primary("Send my piece").disabled(!r.help_confirmed || r.help_send.waiting()).show(ui, theme) {
                    let (b, l) = (base.to_string(), l.clone());
                    r.help_send.start(move || net::send_piece(&b, &l));
                }
                small(ui, theme, "Your piece goes only to their new device, locked so that nobody else can open it, the server included.", theme.text_muted());
            }
            Some(Err(why)) => small(ui, theme, why, theme.warning()),
            None => {}
        }
        result_line(ui, theme, &r.help_send, |m| m.clone());
        ui.add_space(theme.spacing_xs);
        ui.horizontal_wrapped(|ui| {
            if Button::secondary("Check who is asking").disabled(r.asking.waiting()).show(ui, theme) {
                r.asking_for.clear();
            }
            if Button::secondary("Whose pieces do I keep?").disabled(r.kept.waiting()).show(ui, theme) {
                let list: Vec<(String, String)> = friends.iter().map(|f| (f.did.clone(), f.name.clone())).collect();
                let (b, s) = (base.to_string(), secret.clone());
                r.kept.start(move || net::pieces_i_keep(&b, &s, &list));
            }
        });
        result_line(ui, theme, &r.kept, |names| {
            if names.is_empty() {
                "You keep no piece for any of your friends on this server.".to_string()
            } else {
                format!("You keep a piece for {}.", net::list(names))
            }
        });
    });
}

fn draw_ask(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, base: &str, account: Option<&(Vec<u8>, String)>) {
    let ctx = ui.ctx().clone();
    // A rebuilt account is restored through the very path "Restore from recovery phrase" takes.
    let mut restore: Option<[u8; proto::SECRET_LEN]> = None;
    widgets::card(ui, theme, |ui| {
        heading(ui, theme, "Lost your device? Ask your trusted people for help");
        let r = &mut state.recovery;
        if let Some(live) = r.live.clone() {
            // Look at the request every few seconds while it waits.
            let due = r.last_poll.is_none_or(|t| t.elapsed() >= POLL_EVERY);
            let done = matches!(r.progress, Some(Poll::Rebuilt(_)) | Some(Poll::Ended));
            if due && !done && !r.poll.waiting() {
                r.last_poll = Some(Instant::now());
                let (b, l) = (base.to_string(), live.clone());
                r.poll.start(move || net::poll_ask(&b, &l));
            }
            if let Some(Ok(p)) = &r.poll.answer {
                r.progress = Some(p.clone());
            }
            if !done {
                ctx.request_repaint_after(POLL_EVERY);
            }
            match &r.progress {
                Some(Poll::Rebuilt(secret)) => {
                    small(ui, theme, "Your account is back together.", theme.success());
                    if account.is_some_and(|(_, did)| *did != live.found.did) {
                        small(
                            ui,
                            theme,
                            "This device has another account open. Restoring puts yours here in its place (the other one is not erased: its own recovery phrase still opens it).",
                            theme.warning(),
                        );
                    }
                    if Button::primary("Restore my account on this device").show(ui, theme) {
                        restore = Some(*secret);
                    }
                    small(ui, theme, "Then write down your recovery phrase again (Identity, above) and keep it somewhere safe.", theme.text_muted());
                }
                other => {
                    small(
                        ui,
                        theme,
                        "Call each of your trusted people, or see them in person, and read them this code. They check it is really you, type it in, and send their piece to this device. Nobody but this device can open what they send.",
                        theme.text_secondary(),
                    );
                    ui.label(RichText::new(proto::show_code(&live.code)).monospace().size(theme.font_size_heading).color(theme.accent()).strong());
                    let link = format!("{base}/recovery#help={}", live.code);
                    if r.qr.as_ref().is_none_or(|(c, _)| *c != live.code) {
                        r.qr = crate::gui::pages::settings::build_link_qr_texture(&ctx, &link).map(|t| (live.code.clone(), t));
                    }
                    if let Some((_, tex)) = &r.qr {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(180.0, 180.0)));
                        small(ui, theme, "Someone with you can scan this instead: it opens the website's Recovery page on their phone with the code filled in.", theme.text_muted());
                    }
                    let words = match other {
                        Some(Poll::Waiting { arrived, needed }) => format!("Pieces arrived: {arrived} of the {needed} needed."),
                        Some(Poll::DoNotFit { arrived }) => format!(
                            "{arrived} pieces arrived, but they do not fit together. Ask your trusted people to check it was your account they helped."
                        ),
                        Some(Poll::Ended) => format!("This request has ended (it lasts {} days). Ask again for a new code.", proto::ASK_DAYS),
                        _ => "Waiting for the first piece...".to_string(),
                    };
                    small(ui, theme, &words, theme.text_muted());
                    if let Some(Err(why)) = &r.poll.answer {
                        small(ui, theme, &format!("Could not look at the request just now: {why}"), theme.warning());
                    }
                    small(ui, theme, "Keep this open until enough have answered. Closing the app means asking again with a new code.", theme.text_muted());
                }
            }
            if Button::secondary("Stop asking").show(ui, theme) {
                let b = base.to_string();
                std::thread::spawn(move || net::close_ask(&b, &live));
                r.live = None;
                r.progress = None;
                r.poll = Job::default();
                r.last_poll = None;
            }
            return;
        }
        small(
            ui,
            theme,
            "Use this on your new device. Keep it open until enough of your trusted people have answered (a request lasts 7 days).",
            theme.text_secondary(),
        );
        let find = ui
            .horizontal(|ui| {
                ui.add_sized(
                    [(ui.available_width() - 150.0).max(160.0), theme.input_height],
                    egui::TextEdit::singleline(&mut r.ask_who).hint_text("The name you used, or your account ID"),
                );
                Button::secondary("Find my account").disabled(r.find.waiting()).show(ui, theme)
            })
            .inner;
        if find {
            let (b, who) = (base.to_string(), r.ask_who.clone());
            r.find.start(move || net::find_account(&b, &who));
        }
        if r.find.waiting() {
            small(ui, theme, "Looking for your account...", theme.text_muted());
        }
        let mut chosen: Option<Found> = None;
        match &r.find.answer {
            Some(Ok(found)) => {
                for f in found {
                    small(ui, theme, &format!("{} ({}): it takes {} of your {} trusted people.", f.name, f.did, f.needed, f.total), theme.text_secondary());
                    if Button::primary("Ask my trusted people for help").disabled(r.open.waiting()).show(ui, theme) {
                        chosen = Some(f.clone());
                    }
                }
            }
            Some(Err(why)) => small(ui, theme, why, theme.warning()),
            None => {}
        }
        if let Some(f) = chosen {
            let b = base.to_string();
            r.open.start(move || net::open_ask(&b, f));
        }
        if r.open.waiting() {
            small(ui, theme, "Opening the request...", theme.text_muted());
        }
        if let Some(answer) = r.open.answer.take() {
            match answer {
                Ok(live) => {
                    r.live = Some(Arc::new(live));
                    r.progress = None;
                    r.last_poll = None;
                }
                Err(why) => r.open.answer = Some(Err(why)),
            }
        }
        if let Some(Err(why)) = &r.open.answer {
            small(ui, theme, why, theme.warning());
        }
        if let Some(note) = &r.restore_note {
            match note {
                Ok(n) => small(ui, theme, n, theme.success()),
                Err(why) => small(ui, theme, why, theme.warning()),
            }
        }
    });
    if let Some(secret) = restore {
        let note = match crate::net::identity::mnemonic_from_seed(&secret) {
            Some(phrase) => crate::gui::pages::settings::restore_from_phrase(state, &phrase)
                .map(|()| "Restored: this device is your account again. Write down your recovery phrase again (Identity, above).".to_string()),
            None => Err("The rebuilt account could not be written as a recovery phrase.".to_string()),
        };
        if note.is_ok() {
            if let Some(live) = state.recovery.live.take() {
                let b = base.to_string();
                std::thread::spawn(move || net::close_ask(&b, &live));
            }
            state.recovery.progress = None;
            state.recovery.find = Job::default();
            state.recovery.mine_for.clear();
            state.recovery.asking_for.clear();
        }
        state.recovery.restore_note = Some(note);
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
