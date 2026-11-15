//! Organisations page (rung 1, 2026-11-14, docs/design/organisations.md): your organisations, start
//! one, open an invitation, acceptance or update someone sent you, and for each organisation its
//! members (the directory with privacy levels), invitations, roles, the record of changes (the
//! audit log), its settings and how you are listed. Every change is an `org_entry_v1` signed with
//! your key (`humanity_protocol::org`); the logs are kept by `crate::gui::orgs_store`. Every action
//! is listed in the admin action registry as `orgs.*` (`data/admin/actions.json`, run "device").
//! The website's mirror is web/pages/orgs-app.js.
//!
//! Rung 1 carries changes as text the person copies (an invitation, an acceptance, "the latest");
//! rung 2 sends them as sealed messages to each member.

use std::collections::{BTreeMap, BTreeSet};

use egui::{Frame, RichText, ScrollArea};
use humanity_core::i18n::{t, t_dyn, tf, tn};
use humanity_protocol::object::Object;
use humanity_protocol::org::{self, Act, Directory, Listing, OrgState, ReadRefusal, Role, Shown};

use crate::gui::org_mail::{self, MailState, OutItem};
use crate::gui::orgs_store::{OrgStore, StoreProblem};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::GuiState;

/// Which part of the page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    List,
    Found,
    Open,
    Org,
}

/// The tabs of one organisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Members,
    Invite,
    Roles,
    Log,
    Settings,
    Me,
}

/// Everything the page keeps between frames.
pub struct OrgsPage {
    store: Option<OrgStore>,
    store_for: String,
    /// Each organisation's fold and the store version it was made at (rebuilt when the store
    /// changes, and every frame with the reuse switch off: CLAUDE.md, the reuse rule).
    folded: BTreeMap<String, (u64, Result<OrgState, org::FoldError>)>,
    pub view: View,
    pub tab: Tab,
    pub selected: Option<String>,
    found_name: String,
    found_about: String,
    found_directory: Directory,
    paste: String,
    /// An invitation for me found in the last opened text: (org id, invite id).
    pending_invite: Option<(String, String)>,
    invite_key: String,
    invite_role: String,
    invite_days: u32,
    /// A text to copy and what it is.
    out_text: String,
    out_note: &'static str,
    my_name: String,
    my_level: Listing,
    role_name: String,
    role_rank: u64,
    role_perms: BTreeSet<String>,
    set_name: String,
    set_about: String,
    set_directory: Directory,
    pub status: String,
    roles_book: Option<org::RoleBook>,
    /// Delivery's memory (rung 2, engine/org_mail.rs): sends awaiting answers, members the server
    /// could not reach, notes parked for the log that proves their sender.
    pub(crate) mail: MailState,
    /// The member a handover is being offered to (the Settings tab), as key hex.
    handover_to: String,
}

impl Default for OrgsPage {
    fn default() -> Self {
        Self {
            store: None,
            store_for: String::new(),
            folded: BTreeMap::new(),
            view: View::List,
            tab: Tab::Members,
            selected: None,
            found_name: String::new(),
            found_about: String::new(),
            found_directory: Directory::Members,
            paste: String::new(),
            pending_invite: None,
            invite_key: String::new(),
            invite_role: org::MEMBER.into(),
            invite_days: 7,
            out_text: String::new(),
            out_note: "",
            my_name: String::new(),
            my_level: Listing::Members,
            role_name: String::new(),
            role_rank: 20,
            role_perms: BTreeSet::new(),
            set_name: String::new(),
            set_about: String::new(),
            set_directory: Directory::Members,
            status: String::new(),
            roles_book: None,
            mail: MailState::default(),
            handover_to: String::new(),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// `YYYY-MM-DD HH:MM` (UTC) of a time in ms.
pub fn when(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let (h, m) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}

/// The Dilithium3 keypair of the identity whose seed is `seed`.
fn keypair(seed: &[u8]) -> humanity_protocol::pq_crypto::DilithiumKeypair {
    let dil_seed = humanity_protocol::pq_crypto::derive_dilithium_seed(seed);
    humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&dil_seed)
}

/// The words for role `id` in `st`: the app's own for owner, admin and member, the organisation's
/// for a custom role.
pub fn role_word(st: &OrgState, id: &str) -> String {
    if org::is_built_in(id) {
        return t_dyn(&format!("orgs.role.{id}")).to_string();
    }
    st.role(id).map(|r| if r.name.is_empty() { r.id.clone() } else { r.name.clone() }).unwrap_or_else(|| id.to_string())
}

fn perm_word(p: &str) -> &'static str {
    t_dyn(&format!("orgs.perm.{}", p.replace('.', "_")))
}

fn shown_word(s: &Shown) -> String {
    match s {
        Shown::Private => t("orgs.private_person").to_string(),
        Shown::Named { did, name, .. } => match name {
            Some(n) => n.clone(),
            None => short_did(did),
        },
    }
    .replace('\n', " ")
    .chars()
    .take(80)
    .collect::<String>()
}

fn short_did(did: &str) -> String {
    let tail = did.strip_prefix("did:hum:").unwrap_or(did);
    tail.chars().take(10).collect()
}

/// One line of the record of changes, in words.
pub fn log_words(st: &OrgState, line: &org::LogLine, actor: &Shown, subject: Option<&Shown>) -> String {
    let who = shown_word(actor);
    let subj = subject.map(shown_word).unwrap_or_default();
    let what = match (&line.act, line.entry_id == st.org_id) {
        (_, true) => t("orgs.act.founding").to_string(),
        (None, _) => t("orgs.act.unknown").to_string(),
        (Some(a), _) => {
            let role = match a {
                Act::Invite { role, .. } | Act::Assign { role, .. } | Act::RoleDrop { role } => role_word(st, role),
                Act::RoleSet { role } => if role.name.is_empty() { role.id.clone() } else { role.name.clone() },
                _ => String::new(),
            };
            humanity_core::i18n::fill(t_dyn(&format!("orgs.act.{}", a.word())), &[("subject", &subj), ("role", &role)])
        }
    };
    let mut s = format!("{}  {who}: {what}", when(line.at));
    if let Some(r) = line.refused {
        s.push_str("  ");
        s.push_str(&tf("orgs.not_applied", &[("why", &t_dyn(&format!("orgs.refused.{}", r.word())))]));
    }
    s
}

impl OrgsPage {
    /// Load this identity's store when the identity changes.
    fn ensure_store(&mut self, state: &GuiState) {
        let me = state.profile_public_key.clone();
        self.ensure_store_with(&me, state.private_key_bytes.as_deref());
    }

    /// [`Self::ensure_store`] from the key and seed (the engine's delivery, engine/org_mail.rs, calls
    /// it with the page not drawn). True when a store for `me` is open. Test builds never read or
    /// write the person's real file.
    pub(crate) fn ensure_store_with(&mut self, me: &str, seed: Option<&[u8]>) -> bool {
        if self.store.is_some() && self.store_for == me {
            return true;
        }
        self.folded.clear();
        self.selected = None;
        self.mail = Default::default();
        #[cfg(not(test))]
        let open = |seed: &[u8]| OrgStore::load(seed, me);
        #[cfg(test)]
        let open = |seed: &[u8]| OrgStore::load_at(seed, None);
        self.store = seed.filter(|_| !me.is_empty()).map(open);
        self.store_for = me.to_string();
        self.store.is_some()
    }

    /// Use an in-memory store (tests, snapshots).
    pub fn with_store(store: OrgStore, me_hex: &str) -> Self {
        Self { store: Some(store), store_for: me_hex.to_string(), ..Default::default() }
    }

    // ── For the delivery (engine/org_mail.rs) ──

    pub(crate) fn objects(&self, org_id: &str) -> &[Object] {
        self.store.as_ref().map(|s| s.objects(org_id)).unwrap_or(&[])
    }
    pub(crate) fn add(&mut self, org_id: &str, objects: Vec<Object>) -> usize {
        self.store.as_mut().map(|s| s.add(org_id, objects)).unwrap_or(0)
    }
    pub(crate) fn pick(&self, org_id: &str, ids: &[String]) -> Vec<Object> {
        self.store.as_ref().map(|s| s.pick(org_id, ids)).unwrap_or_default()
    }
    pub(crate) fn queue(&mut self, items: Vec<OutItem>) {
        if let Some(s) = self.store.as_mut() {
            s.queue(items);
        }
    }
    pub(crate) fn outbox_len(&self) -> usize {
        self.store.as_ref().map_or(0, |s| s.outbox().len())
    }
    pub(crate) fn outbox_item(&self, i: usize) -> Option<OutItem> {
        self.store.as_ref().and_then(|s| s.outbox().get(i).cloned())
    }
    pub(crate) fn take_out(&mut self, i: usize) -> Option<OutItem> {
        self.store.as_mut().and_then(|s| s.take_out(i))
    }
    pub(crate) fn flush_store(&self) {
        if let Some(s) = self.store.as_ref() {
            s.flush();
        }
    }
    /// How many deliveries of `org_id` wait to go.
    fn waiting_for(&self, org_id: &str) -> usize {
        self.store.as_ref().map_or(0, |s| s.outbox().iter().filter(|i| i.org == org_id).count())
    }

    /// A delivered text brought an organisation this device did not hold: when it holds an open
    /// invitation for me, the page shows it (as opening the text would).
    pub(crate) fn note_invitation(&mut self, org_id: &str, st: &OrgState, me: &[u8], now: u64) {
        if st.member(me).is_some() {
            return;
        }
        if let Some(inv) = st.open_invites(now).into_iter().find(|i| i.who == me) {
            self.pending_invite = Some((org_id.to_string(), inv.id.clone()));
            self.status = tf("orgs.mail.invited", &[("org", &st.name)]);
        }
    }

    fn book(&mut self) -> Option<&org::RoleBook> {
        if self.roles_book.is_none() {
            let path = humanity_core::storage::data_dir().join("orgs").join("roles.json");
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| org::role_book_from_json(&t)) {
                Ok(b) => self.roles_book = Some(b),
                Err(e) => log::warn!("[orgs] {}: {e}", path.display()),
            }
        }
        self.roles_book.as_ref()
    }

    /// The fold of `org_id`, refolded when the store changed (or always, with reuse off).
    pub fn state_of(&mut self, org_id: &str) -> Option<&OrgState> {
        let store = self.store.as_ref()?;
        let v = store.version();
        let keep = humanity_core::reuse::enabled() && self.folded.get(org_id).is_some_and(|(at, _)| *at == v);
        if !keep {
            let st = org::fold(org_id, store.objects(org_id), now_ms());
            self.folded.insert(org_id.to_string(), (v, st));
        }
        self.folded.get(org_id).and_then(|(_, r)| r.as_ref().ok())
    }

    /// Start an organisation: sign its founding with `seed`'s key. Its id.
    pub fn found(&mut self, seed: &[u8], name: &str, about: &str, directory: Directory) -> Result<String, String> {
        let roles = self.book().map(|b| b.roles.clone()).ok_or("data/orgs/roles.json")?;
        let f = org::Founding { name: name.trim().to_string(), about: about.trim().to_string(), directory, roles };
        let o = org::build_founding(&keypair(seed), &f, now_ms())?;
        let id = o.object_id().map_err(|e| e.to_string())?.to_hex();
        self.store.as_mut().ok_or("no store")?.add(&id, vec![o]);
        // My other devices hold it too (rung 2).
        let me = hex::encode(keypair(seed).public_key());
        self.queue(org_mail::chunked(&id, &me, std::slice::from_ref(&id)));
        Ok(id)
    }

    /// What a new entry of `act` follows (rung 2): the heads of this log as a member's fold sees
    /// it; for an acceptance, as the invitee's fold sees the invitation (a member's fold holds the
    /// invitation's entries, whose own `prev` name entries it leaves out).
    fn prev_for(&mut self, org_id: &str, act: &Act) -> Vec<String> {
        if matches!(act, Act::Accept { .. }) {
            let objects = self.store.as_ref().map(|s| s.objects(org_id).to_vec()).unwrap_or_default();
            return org::fold_invitation(org_id, &objects, now_ms()).map(|st| st.heads).unwrap_or_default();
        }
        self.state_of(org_id).map(|st| st.heads.clone()).unwrap_or_default()
    }

    /// Sign one change to `org_id` with `seed`'s key, keep it, and say whether the fold applied it.
    ///
    /// Rung 2: an applied change is queued at once for delivery (saved with the log, sent by
    /// engine/org_mail.rs): to my own other devices, to every member it concerns, and an invitation
    /// to the person invited (gui/org_mail.rs `deliveries`).
    pub fn sign(&mut self, seed: &[u8], org_id: &str, act: Act) -> Result<(Object, Option<org::Refusal>), String> {
        let prev = self.prev_for(org_id, &act);
        let kp = keypair(seed);
        let o = org::build_entry(&kp, org_id, &act, &prev, now_ms())?;
        let id = o.object_id().map_err(|e| e.to_string())?.to_hex();
        self.store.as_mut().ok_or("no store")?.add(org_id, vec![o.clone()]);
        // An acceptance is judged as the invitee sees the invitation; anything else as a member does.
        let after = if matches!(act, Act::Accept { .. }) {
            org::fold_invitation(org_id, self.objects(org_id), now_ms()).ok()
        } else {
            self.state_of(org_id).cloned()
        };
        let refused = after.as_ref().and_then(|st| st.log.iter().find(|l| l.entry_id == id)).and_then(|l| l.refused);
        if let (None, Some(st)) = (refused, after.as_ref()) {
            let invitation = match &act {
                Act::Invite { .. } => st.invites.get(&id).map(|i| std::iter::once(org_id.to_string()).chain(i.support.iter().cloned()).chain(std::iter::once(id.clone())).collect::<Vec<_>>()),
                _ => None,
            };
            let items = org_mail::deliveries(st, &kp.public_key(), &id, &act, invitation.as_deref());
            self.queue(items);
        }
        Ok((o, refused))
    }

    /// Open a carried text: merge it in and look for an invitation for `me`. How many were new.
    pub fn open_text(&mut self, text: &str, me: &[u8]) -> Result<(String, usize), String> {
        let (org_id, objects) = org::parse_bundle(text)?;
        let added = self.store.as_mut().ok_or("no store")?.add(&org_id, objects);
        // As the invitee sees it: a member's fold holds the invitation's entries (rung 2).
        let st = org::fold_invitation(&org_id, self.objects(&org_id), now_ms()).map_err(|_| "no_founding")?;
        let pending = st.open_invites(now_ms()).into_iter().find(|i| i.who == me && st.member(me).is_none()).map(|i| i.id.clone());
        self.pending_invite = pending.map(|i| (org_id.clone(), i));
        Ok((org_id, added))
    }

    /// The text a member shares so others catch up: every change.
    pub fn latest_text(&self, org_id: &str) -> Option<String> {
        let store = self.store.as_ref()?;
        Some(org::bundle_text(org_id, store.objects(org_id)))
    }

    /// The text an invitation carries.
    pub fn invite_text(&mut self, org_id: &str, invite_id: &str) -> Option<String> {
        let objects = self.store.as_ref()?.objects(org_id).to_vec();
        self.state_of(org_id)?.invite_bundle(&objects, invite_id)
    }
}

/// The viewer's public key bytes.
fn my_key(state: &GuiState) -> Vec<u8> {
    hex::decode(&state.profile_public_key).unwrap_or_default()
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut page = std::mem::take(&mut state.orgs_page);
    page.ensure_store(state);
    egui::CentralPanel::default().frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(16.0)).show(ctx, |ui| {
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| draw_body(ui, theme, state, &mut page));
    });
    state.orgs_page = page;
}

fn draw_body(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, page: &mut OrgsPage) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("orgs.title")).size(theme.font_size_title).color(theme.text_primary()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Always reachable; signed out, they say why nothing can be signed yet.
            if widgets::Button::secondary(t("orgs.open_button")).hint("orgs.open_button_hint").show(ui, theme) {
                page.view = View::Open;
                page.out_text.clear();
                page.status.clear();
            }
            if widgets::Button::primary(t("orgs.found_button")).hint("orgs.found_button_hint").show(ui, theme) {
                page.view = View::Found;
                page.status.clear();
            }
        });
    });
    widgets::body_hint(ui, theme, t("orgs.intro"));
    if page.store.is_none() {
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new(t("orgs.sign_in_first")).color(theme.text_muted()));
        return;
    }
    match page.store.as_ref().and_then(|s| s.problem().cloned()) {
        Some(StoreProblem::KeptAside(p)) => {
            ui.label(RichText::new(tf("orgs.store.kept_aside", &[("path", &p.display())])).color(theme.warning()));
        }
        Some(StoreProblem::Unreadable(e)) => {
            ui.label(RichText::new(tf("orgs.store.unreadable", &[("why", &e)])).color(theme.danger()));
        }
        None => {}
    }
    if !page.status.is_empty() {
        ui.label(RichText::new(&page.status).color(theme.text_secondary()).size(theme.font_size_small));
    }
    ui.add_space(theme.spacing_md);
    match page.view {
        View::List => draw_list(ui, theme, state, page),
        View::Found => draw_found(ui, theme, state, page),
        View::Open => draw_open(ui, theme, state, page),
        View::Org => draw_org(ui, theme, state, page),
    }
}

fn back_button(ui: &mut egui::Ui, theme: &Theme, page: &mut OrgsPage) {
    if widgets::Button::secondary(t("orgs.back")).hint("orgs.back_hint").show(ui, theme) {
        page.view = View::List;
        page.out_text.clear();
        page.pending_invite = None;
    }
}

fn draw_out_text(ui: &mut egui::Ui, theme: &Theme, page: &mut OrgsPage) {
    if page.out_text.is_empty() {
        return;
    }
    widgets::card(ui, theme, |ui| {
        if !page.out_note.is_empty() {
            ui.label(RichText::new(t_dyn(page.out_note)).color(theme.text_primary()));
        }
        ui.horizontal(|ui| {
            if widgets::Button::primary(t("orgs.copy")).hint("orgs.copy_hint").show(ui, theme) {
                ui.ctx().copy_text(page.out_text.clone());
                page.status = t("orgs.copied").to_string();
            }
        });
    });
}

fn draw_list(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, page: &mut OrgsPage) {
    draw_invitation(ui, theme, state, page);
    draw_out_text(ui, theme, page);
    let ids = page.store.as_ref().map(OrgStore::ids).unwrap_or_default();
    if ids.is_empty() {
        ui.label(RichText::new(t("orgs.none_yet")).color(theme.text_muted()));
        return;
    }
    let me = page.store_for.clone();
    let me = hex::decode(me).unwrap_or_default();
    for id in ids {
        let Some(st) = page.state_of(&id) else { continue };
        let role = st.member(&me).map(|m| tf("orgs.my_role", &[("role", &role_word(st, &m.role))])).unwrap_or_else(|| t("orgs.not_member").to_string());
        let label = format!("{}  ({}, {})", st.name, tn("orgs.members_count", st.members.len() as i64, &[]), role);
        if widgets::Button::secondary(&label).hint("orgs.list_hint").show(ui, theme) {
            page.selected = Some(id.clone());
            page.view = View::Org;
            page.tab = Tab::Members;
            page.out_text.clear();
        }
        ui.add_space(theme.spacing_sm);
    }
}

fn draw_found(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, page: &mut OrgsPage) {
    back_button(ui, theme, page);
    ui.label(RichText::new(t("orgs.found_title")).size(theme.font_size_heading).color(theme.accent()));
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, t("orgs.name"), |ui| {
            ui.add(egui::TextEdit::singleline(&mut page.found_name).desired_width(320.0).char_limit(org::NAME_MAX).hint_text(t("orgs.name_placeholder"))).hint("orgs.name_hint");
        });
        widgets::form_row(ui, theme, t("orgs.about"), |ui| {
            ui.add(egui::TextEdit::multiline(&mut page.found_about).desired_width(320.0).desired_rows(3).char_limit(org::ABOUT_MAX)).hint("orgs.about_hint");
        });
        directory_choice(ui, theme, &mut page.found_directory);
    });
    ui.add_space(theme.spacing_md);
    let can = !page.found_name.trim().is_empty();
    ui.add_enabled_ui(can, |ui| {
        if widgets::Button::primary(t("orgs.found_submit")).hint("orgs.found_submit_hint").show(ui, theme) {
            if let Some(seed) = state.private_key_bytes.clone() {
                let (n, a, d) = (page.found_name.clone(), page.found_about.clone(), page.found_directory);
                match page.found(&seed, &n, &a, d) {
                    Ok(id) => {
                        page.selected = Some(id);
                        page.view = View::Org;
                        page.tab = Tab::Members;
                        page.found_name.clear();
                        page.found_about.clear();
                        page.status = t("orgs.signed").to_string();
                    }
                    Err(e) => page.status = tf("orgs.sign_failed", &[("why", &e)]),
                }
            }
        }
    });
}

fn directory_choice(ui: &mut egui::Ui, theme: &Theme, d: &mut Directory) {
    widgets::form_row(ui, theme, t("orgs.directory"), |ui| {
        ui.vertical(|ui| {
            ui.radio_value(d, Directory::Members, t("orgs.dir.members")).hint("orgs.dir.members_hint");
            ui.radio_value(d, Directory::Organisers, t("orgs.dir.organisers")).hint("orgs.dir.organisers_hint");
            ui.radio_value(d, Directory::Public, t("orgs.dir.public")).hint("orgs.dir.public_hint");
        });
    });
}

fn draw_open(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, page: &mut OrgsPage) {
    back_button(ui, theme, page);
    ui.label(RichText::new(t("orgs.open_title")).size(theme.font_size_heading).color(theme.accent()));
    widgets::form_row(ui, theme, t("orgs.paste"), |ui| {
        ui.add(egui::TextEdit::multiline(&mut page.paste).desired_width(480.0).desired_rows(5)).hint("orgs.paste_hint");
    });
    if widgets::Button::primary(t("orgs.open_submit")).hint("orgs.open_submit_hint").show(ui, theme) {
        let text = std::mem::take(&mut page.paste);
        match page.open_text(&text, &my_key(state)) {
            Ok((_, 0)) => page.status = t("orgs.opened_nothing").to_string(),
            Ok((_, n)) => page.status = tn("orgs.opened", n as i64, &[]),
            Err(e) => page.status = t_dyn(&format!("orgs.bad.{e}")).to_string(),
        }
    }
    draw_invitation(ui, theme, state, page);
    draw_out_text(ui, theme, page);
}

/// An invitation for me, from an opened text or delivered as a sealed message (rung 2): who
/// invited me, to what, until when, and Accept.
fn draw_invitation(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, page: &mut OrgsPage) {
    if let Some((org_id, inv_id)) = page.pending_invite.clone() {
        // As the invitee sees it: a member's fold holds the invitation's entries (rung 2).
        let Ok(st) = org::fold_invitation(&org_id, page.objects(&org_id), now_ms()) else { return };
        let st = &st;
        let Some(inv) = st.invites.get(&inv_id) else { return };
        let inviter = shown_word(&Shown::Named { did: humanity_protocol::did::did_for_pubkey(&inv.by), name: st.member(&inv.by).and_then(|m| m.name.clone()), role: None });
        let words = tf("orgs.invited", &[("inviter", &inviter), ("org", &st.name), ("role", &role_word(st, &inv.role))]);
        let until = tf("orgs.invited_until", &[("when", &when(inv.until))]);
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new(words).color(theme.text_primary()));
            ui.label(RichText::new(until).size(theme.font_size_small).color(theme.text_muted()));
            ui.horizontal(|ui| {
                if widgets::Button::primary(t("orgs.accept")).hint("orgs.accept_hint").show(ui, theme) {
                    if let Some(seed) = state.private_key_bytes.clone() {
                        match page.sign(&seed, &org_id, Act::Accept { invite: inv_id.clone() }) {
                            Ok((o, None)) => {
                                // Joining a group is civics practice (data/skills/practice.json).
                                humanity_systems::skills::queue_practice("join_org");
                                page.out_text = org::bundle_text(&org_id, &[o]);
                                page.out_note = "orgs.send_back";
                                page.pending_invite = None;
                                page.status = t("orgs.signed").to_string();
                            }
                            Ok((_, Some(r))) => page.status = tf("orgs.not_applied", &[("why", &t_dyn(&format!("orgs.refused.{}", r.word())))]),
                            Err(e) => page.status = tf("orgs.sign_failed", &[("why", &e)]),
                        }
                    }
                }
                if widgets::Button::secondary(t("orgs.not_now")).hint("orgs.not_now_hint").show(ui, theme) {
                    page.pending_invite = None;
                }
            });
        });
    }
}

fn draw_org(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, page: &mut OrgsPage) {
    back_button(ui, theme, page);
    let Some(org_id) = page.selected.clone() else { return };
    let me = my_key(state);
    let Some(st) = page.state_of(&org_id).cloned() else { return };
    ui.label(RichText::new(&st.name).size(theme.font_size_heading).color(theme.accent()));
    if !st.about.is_empty() {
        widgets::body_hint(ui, theme, &st.about);
    }
    let my_role = st.member(&me).map(|m| m.role.clone());
    ui.horizontal(|ui| {
        let role = my_role.as_deref().map(|r| tf("orgs.my_role", &[("role", &role_word(&st, r))])).unwrap_or_else(|| t("orgs.not_member").to_string());
        ui.label(RichText::new(role).color(theme.text_secondary()));
        if my_role.is_some() && widgets::Button::secondary(t("orgs.share_latest")).hint("orgs.share_latest_hint").show(ui, theme) {
            page.out_text = page.latest_text(&org_id).unwrap_or_default();
            page.out_note = "orgs.share_latest_note";
        }
    });
    if st.waiting > 0 {
        ui.label(RichText::new(tn("orgs.waiting", st.waiting as i64, &[])).size(theme.font_size_small).color(theme.text_muted()));
    }
    draw_delivery(ui, theme, &st, page);
    draw_out_text(ui, theme, page);
    ui.add_space(theme.spacing_sm);
    ui.horizontal_wrapped(|ui| {
        for (tab, label, hint) in [
            (Tab::Members, "orgs.tab.members", "orgs.tab.members_hint"),
            (Tab::Invite, "orgs.tab.invite", "orgs.tab.invite_hint"),
            (Tab::Roles, "orgs.tab.roles", "orgs.tab.roles_hint"),
            (Tab::Log, "orgs.tab.log", "orgs.tab.log_hint"),
            (Tab::Settings, "orgs.tab.settings", "orgs.tab.settings_hint"),
            (Tab::Me, "orgs.tab.me", "orgs.tab.me_hint"),
        ] {
            if widgets::Button::secondary(t_dyn(label)).active(page.tab == tab).hint(hint).show(ui, theme) {
                page.tab = tab;
                if tab == Tab::Settings {
                    page.set_name = st.name.clone();
                    page.set_about = st.about.clone();
                    page.set_directory = st.directory;
                }
                if tab == Tab::Me {
                    if let Some(m) = st.member(&me) {
                        page.my_name = m.name.clone().unwrap_or_default();
                        page.my_level = m.listing;
                    }
                }
            }
        }
    });
    ui.add_space(theme.spacing_sm);
    let seed = state.private_key_bytes.clone();
    let mut todo: Option<Act> = None;
    match page.tab {
        Tab::Members => draw_members(ui, theme, &st, &me, &mut todo),
        Tab::Invite => draw_invite(ui, theme, &st, &me, page, &mut todo),
        Tab::Roles => draw_roles(ui, theme, &st, &me, page, &mut todo),
        Tab::Log => draw_log(ui, theme, &st, &me),
        Tab::Settings => draw_settings(ui, theme, &st, &me, page, &mut todo),
        Tab::Me => draw_me(ui, theme, &st, &me, page, &mut todo),
    }
    if let (Some(act), Some(seed)) = (todo, seed) {
        let is_invite = matches!(act, Act::Invite { .. });
        match page.sign(&seed, &org_id, act) {
            Ok((o, None)) => {
                page.status = t("orgs.signed").to_string();
                if is_invite {
                    let id = o.object_id().map(|h| h.to_hex()).unwrap_or_default();
                    page.out_text = page.invite_text(&org_id, &id).unwrap_or_default();
                    page.out_note = "orgs.invite_send";
                    page.invite_key.clear();
                }
            }
            Ok((_, Some(r))) => page.status = tf("orgs.not_applied", &[("why", &t_dyn(&format!("orgs.refused.{}", r.word())))]),
            Err(e) => page.status = tf("orgs.sign_failed", &[("why", &e)]),
        }
    }
}

fn draw_members(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, me: &[u8], todo: &mut Option<Act>) {
    let view = match st.directory_for(me) {
        Ok(v) => v,
        Err(r) => {
            let key = match r {
                ReadRefusal::NotAMember => "orgs.refused_read.not_a_member",
                ReadRefusal::OrganisersOnly => "orgs.refused_read.organisers_only",
            };
            ui.label(RichText::new(t_dyn(key)).color(theme.text_muted()));
            return;
        }
    };
    let my_rank = st.rank_of(me).unwrap_or(0);
    let lower_roles: Vec<Role> = st.roles.iter().filter(|r| r.rank < my_rank).cloned().collect();
    for s in &view.named {
        let Shown::Named { did, name, role } = s else { continue };
        let Some(m) = st.members.get(did) else { continue };
        widgets::card(ui, theme, |ui| {
            ui.horizontal_wrapped(|ui| {
                let shown = name.clone().unwrap_or_else(|| t("orgs.no_name").to_string());
                let you = if m.key == me { format!("  ({})", t("orgs.you")) } else { String::new() };
                ui.label(RichText::new(format!("{shown}{you}")).color(theme.text_primary()).strong());
                ui.label(RichText::new(role.as_deref().map(|r| role_word(st, r)).unwrap_or_default()).color(theme.text_secondary()));
                ui.label(RichText::new(short_did(did)).size(theme.font_size_small).color(theme.text_muted()));
            });
            let their_rank = st.role(&m.role).map(|r| r.rank).unwrap_or(0);
            if m.key != me && their_rank < my_rank && m.role != org::OWNER {
                ui.horizontal_wrapped(|ui| {
                    if st.can(me, org::perm::ASSIGN) {
                        for r in &lower_roles {
                            if r.id != m.role && widgets::Button::secondary(&role_word(st, &r.id)).hint("orgs.change_role_hint").show(ui, theme) {
                                *todo = Some(Act::Assign { who: m.key.clone(), role: r.id.clone() });
                            }
                        }
                    }
                    if st.can(me, org::perm::REMOVE) && widgets::Button::danger(t("orgs.remove")).hint("orgs.remove_hint").show(ui, theme) {
                        *todo = Some(Act::Remove { who: m.key.clone() });
                    }
                });
            }
        });
    }
    if view.private > 0 {
        ui.label(RichText::new(tn("orgs.private_count", view.private as i64, &[])).color(theme.text_muted()));
    }
    ui.add_space(theme.spacing_sm);
    widgets::body_hint(ui, theme, t("orgs.privacy_note"));
}

fn draw_invite(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, me: &[u8], page: &mut OrgsPage, todo: &mut Option<Act>) {
    if !st.can(me, org::perm::INVITE) {
        ui.label(RichText::new(t("orgs.invite_needs")).color(theme.text_muted()));
        return;
    }
    let my_rank = st.rank_of(me).unwrap_or(0);
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, t("orgs.invite_key"), |ui| {
            ui.add(egui::TextEdit::singleline(&mut page.invite_key).desired_width(320.0)).hint("orgs.invite_key_hint");
        });
        widgets::form_row(ui, theme, t("orgs.invite_role"), |ui| {
            ui.horizontal_wrapped(|ui| {
                for r in st.roles.iter().filter(|r| r.rank < my_rank) {
                    if widgets::Button::secondary(&role_word(st, &r.id)).active(page.invite_role == r.id).hint("orgs.invite_role_hint").show(ui, theme) {
                        page.invite_role = r.id.clone();
                    }
                }
            });
        });
        widgets::form_row(ui, theme, t("orgs.invite_days"), |ui| {
            ui.add(egui::DragValue::new(&mut page.invite_days).range(1..=60)).hint("orgs.invite_days_hint");
        });
    });
    let key = hex::decode(page.invite_key.trim()).ok().filter(|k| k.len() == humanity_protocol::pq_crypto::DILITHIUM_PK_LEN);
    if !page.invite_key.trim().is_empty() && key.is_none() {
        ui.label(RichText::new(t("orgs.invite_key_bad")).color(theme.warning()));
    }
    ui.add_enabled_ui(key.is_some(), |ui| {
        if widgets::Button::primary(t("orgs.invite_submit")).hint("orgs.invite_submit_hint").show(ui, theme) {
            if let Some(who) = key.clone() {
                *todo = Some(Act::Invite { who, role: page.invite_role.clone(), until: now_ms() + u64::from(page.invite_days) * 86_400_000 });
            }
        }
    });
    ui.add_space(theme.spacing_md);
    widgets::subsection_label(ui, theme, t("orgs.open_invites"));
    let open = st.open_invites(now_ms());
    if open.is_empty() {
        ui.label(RichText::new(t("orgs.no_open_invites")).color(theme.text_muted()));
    }
    for inv in open {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}  {}  {}", short_did(&inv.who_did), role_word(st, &inv.role), when(inv.until))).color(theme.text_secondary()));
            if widgets::Button::secondary(t("orgs.copy")).hint("orgs.copy_hint").show(ui, theme) {
                let objects = page.store.as_ref().map(|s| s.objects(&st.org_id).to_vec()).unwrap_or_default();
                if let Some(text) = st.invite_bundle(&objects, &inv.id) {
                    ui.ctx().copy_text(text);
                    page.status = t("orgs.copied").to_string();
                }
            }
            if widgets::Button::danger(t("orgs.withdraw")).hint("orgs.withdraw_hint").show(ui, theme) {
                *todo = Some(Act::Withdraw { invite: inv.id.clone() });
            }
        });
    }
}

fn draw_roles(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, me: &[u8], page: &mut OrgsPage, todo: &mut Option<Act>) {
    widgets::body_hint(ui, theme, t("orgs.roles_intro"));
    let my_rank = st.rank_of(me).unwrap_or(0);
    let mut roles = st.roles.clone();
    roles.sort_by(|a, b| b.rank.cmp(&a.rank));
    for r in &roles {
        widgets::card(ui, theme, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(role_word(st, &r.id)).color(theme.text_primary()).strong());
                ui.label(RichText::new(tf("orgs.rank", &[("n", &r.rank)])).size(theme.font_size_small).color(theme.text_muted()));
                if !r.is_built_in() && r.rank < my_rank && st.can(me, org::perm::DEFINE) && widgets::Button::danger(t("orgs.role_drop")).hint("orgs.role_drop_hint").show(ui, theme) {
                    *todo = Some(Act::RoleDrop { role: r.id.clone() });
                }
            });
            if r.perms.is_empty() {
                ui.label(RichText::new(t("orgs.no_perms")).size(theme.font_size_small).color(theme.text_muted()));
            }
            for p in &r.perms {
                ui.label(RichText::new(perm_word(p)).size(theme.font_size_small).color(theme.text_secondary()));
            }
        });
    }
    ui.add_space(theme.spacing_md);
    widgets::subsection_label(ui, theme, t("orgs.new_role"));
    if !st.can(me, org::perm::DEFINE) {
        ui.label(RichText::new(t("orgs.role_needs")).color(theme.text_muted()));
        return;
    }
    let my_perms: Vec<String> = st.member(me).and_then(|m| st.role(&m.role)).map(|r| r.perms.clone()).unwrap_or_default();
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, t("orgs.role_name"), |ui| {
            ui.add(egui::TextEdit::singleline(&mut page.role_name).desired_width(240.0).char_limit(org::ROLE_NAME_MAX)).hint("orgs.role_name_hint");
        });
        widgets::form_row(ui, theme, t("orgs.role_rank"), |ui| {
            ui.add(egui::DragValue::new(&mut page.role_rank).range(0..=my_rank.saturating_sub(1))).hint("orgs.role_rank_hint");
        });
        for p in &my_perms {
            let mut on = page.role_perms.contains(p);
            if ui.checkbox(&mut on, perm_word(p)).hint("orgs.role_perm_hint").changed() {
                if on {
                    page.role_perms.insert(p.clone());
                } else {
                    page.role_perms.remove(p);
                }
            }
        }
    });
    let id = role_id_from(&page.role_name);
    ui.add_enabled_ui(!id.is_empty() && !org::is_built_in(&id), |ui| {
        if widgets::Button::primary(t("orgs.role_submit")).hint("orgs.role_submit_hint").show(ui, theme) {
            let perms: Vec<String> = my_perms.iter().filter(|p| page.role_perms.contains(*p)).cloned().collect();
            *todo = Some(Act::RoleSet { role: Role { id, name: page.role_name.trim().to_string(), rank: page.role_rank.min(my_rank.saturating_sub(1)), perms } });
            page.role_name.clear();
            page.role_perms.clear();
        }
    });
}

/// A role id from its name: lowercase letters and digits, the rest as `_`, at most 32.
pub fn role_id_from(name: &str) -> String {
    let mut id: String = name.trim().to_lowercase().chars().map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() { c } else { '_' }).collect();
    id.truncate(org::ROLE_ID_MAX);
    if id.trim_matches('_').is_empty() {
        String::new()
    } else {
        id
    }
}

fn draw_log(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, me: &[u8]) {
    let Ok(lines) = st.audit_log_for(me) else {
        ui.label(RichText::new(t("orgs.refused_read.log_not_a_member")).color(theme.text_muted()));
        return;
    };
    widgets::body_hint(ui, theme, t("orgs.log_intro"));
    for (line, actor, subject) in lines.iter().take(500) {
        let colour = if line.refused.is_some() { theme.text_muted() } else { theme.text_secondary() };
        ui.label(RichText::new(log_words(st, line, actor, subject.as_ref())).size(theme.font_size_small).color(colour));
    }
}

fn draw_settings(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, me: &[u8], page: &mut OrgsPage, todo: &mut Option<Act>) {
    if !st.can(me, org::perm::SETTINGS) {
        ui.label(RichText::new(t("orgs.settings_needs")).color(theme.text_muted()));
        return;
    }
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, t("orgs.name"), |ui| {
            ui.add(egui::TextEdit::singleline(&mut page.set_name).desired_width(320.0).char_limit(org::NAME_MAX)).hint("orgs.name_hint");
        });
        widgets::form_row(ui, theme, t("orgs.about"), |ui| {
            ui.add(egui::TextEdit::multiline(&mut page.set_about).desired_width(320.0).desired_rows(3).char_limit(org::ABOUT_MAX)).hint("orgs.about_hint");
        });
        directory_choice(ui, theme, &mut page.set_directory);
    });
    let changed = page.set_name.trim() != st.name || page.set_about != st.about || page.set_directory != st.directory;
    ui.add_enabled_ui(changed && !page.set_name.trim().is_empty(), |ui| {
        if widgets::Button::primary(t("orgs.settings_submit")).hint("orgs.settings_submit_hint").show(ui, theme) {
            *todo = Some(Act::Settings {
                name: (page.set_name.trim() != st.name).then(|| page.set_name.trim().to_string()),
                about: (page.set_about != st.about).then(|| page.set_about.clone()),
                directory: (page.set_directory != st.directory).then_some(page.set_directory),
            });
        }
    });
}

fn draw_me(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, me: &[u8], page: &mut OrgsPage, todo: &mut Option<Act>) {
    let Some(m) = st.member(me) else {
        ui.label(RichText::new(t("orgs.not_member")).color(theme.text_muted()));
        forget_button(ui, theme, st, page);
        return;
    };
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, t("orgs.me_name"), |ui| {
            ui.add(egui::TextEdit::singleline(&mut page.my_name).desired_width(240.0).char_limit(org::LISTING_NAME_MAX)).hint("orgs.me_name_hint");
        });
        widgets::form_row(ui, theme, t("orgs.me_level"), |ui| {
            ui.vertical(|ui| {
                ui.radio_value(&mut page.my_level, Listing::Members, t("orgs.listing.members")).hint("orgs.listing.members_hint");
                ui.radio_value(&mut page.my_level, Listing::Organisers, t("orgs.listing.organisers")).hint("orgs.listing.organisers_hint");
            });
        });
    });
    if widgets::Button::primary(t("orgs.me_submit")).hint("orgs.me_submit_hint").show(ui, theme) {
        *todo = Some(Act::Listing { name: page.my_name.trim().to_string(), level: page.my_level });
    }
    ui.add_space(theme.spacing_md);
    if m.role == org::OWNER {
        ui.label(RichText::new(t("orgs.leave_owner")).size(theme.font_size_small).color(theme.text_muted()));
    } else if widgets::Button::danger(t("orgs.leave")).hint("orgs.leave_hint").show(ui, theme) {
        *todo = Some(Act::Leave);
    }
    ui.add_space(theme.spacing_md);
    draw_handover(ui, theme, st, me, page, todo);
}

/// How delivery stands for this organisation (rung 2): how changes travel, what waits to go, which
/// members the server could not reach (their "who can reach me"), and what waits for an earlier
/// change.
fn draw_delivery(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, page: &OrgsPage) {
    ui.label(RichText::new(t("orgs.mail.how")).size(theme.font_size_small).color(theme.text_muted()));
    let waiting = page.waiting_for(&st.org_id);
    if waiting > 0 {
        ui.label(RichText::new(tn("orgs.mail.waiting", waiting as i64, &[])).size(theme.font_size_small).color(theme.text_secondary()));
    }
    let names: Vec<String> = page
        .mail
        .unreachable
        .iter()
        .filter(|(o, _)| *o == st.org_id)
        .filter_map(|(_, k)| hex::decode(k).ok())
        .map(|k| shown_word(&Shown::Named { did: humanity_protocol::did::did_for_pubkey(&k), name: st.member(&k).and_then(|m| m.name.clone()), role: None }))
        .collect();
    if !names.is_empty() {
        ui.label(RichText::new(tf("orgs.mail.unreachable", &[("names", &names.join(", "))])).size(theme.font_size_small).color(theme.warning()));
    }
    if st.held > 0 {
        ui.label(RichText::new(tn("orgs.held", st.held as i64, &[])).size(theme.font_size_small).color(theme.text_muted()));
    }
}

/// Handing the organisation on (rung 2): the owner offers it to a member, the member it names
/// takes it here, and the owner's role moves only with both signatures.
fn draw_handover(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, me: &[u8], page: &mut OrgsPage, todo: &mut Option<Act>) {
    let now = now_ms();
    let open = st.open_transfer(now).cloned();
    let name_of = |key: &[u8]| shown_word(&Shown::Named { did: humanity_protocol::did::did_for_pubkey(key), name: st.member(key).and_then(|m| m.name.clone()), role: None });
    if let Some(tr) = open.as_ref().filter(|tr| tr.who == me) {
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new(tf("orgs.handover.offered_you", &[("who", &name_of(&tr.by)), ("when", &when(tr.until))])).color(theme.text_primary()));
            if widgets::Button::primary(t("orgs.handover.take")).hint("orgs.handover.take_hint").show(ui, theme) {
                *todo = Some(Act::AcceptTransfer { transfer: tr.id.clone() });
            }
        });
        return;
    }
    if st.member(me).map(|m| m.role.as_str()) != Some(org::OWNER) {
        return;
    }
    ui.label(RichText::new(t("orgs.handover.title")).size(theme.font_size_heading).color(theme.accent()));
    widgets::body_hint(ui, theme, t("orgs.handover.intro"));
    if let Some(tr) = open {
        ui.label(RichText::new(tf("orgs.handover.offered", &[("who", &name_of(&tr.who)), ("when", &when(tr.until))])).color(theme.text_secondary()));
        if widgets::Button::secondary(t("orgs.handover.withdraw")).hint("orgs.handover.withdraw_hint").show(ui, theme) {
            *todo = Some(Act::Withdraw { invite: tr.id });
        }
        return;
    }
    let others: Vec<(String, String)> = st.members.values().filter(|m| m.key != me).map(|m| (hex::encode(&m.key), name_of(&m.key))).collect();
    widgets::form_row(ui, theme, t("orgs.handover.member"), |ui| {
        let chosen = others.iter().find(|(k, _)| *k == page.handover_to).map(|(_, n)| n.clone()).unwrap_or_else(|| t("orgs.handover.pick").to_string());
        egui::ComboBox::from_id_salt("orgs_handover_to")
            .selected_text(chosen)
            .show_ui(ui, |ui| {
                for (k, n) in &others {
                    ui.selectable_value(&mut page.handover_to, k.clone(), n).hint("orgs.handover.member_hint");
                }
            })
            .response
            .hint("orgs.handover.member_hint");
    });
    let picked = hex::decode(&page.handover_to).ok().filter(|k| st.member(k).is_some() && k.as_slice() != me);
    ui.add_enabled_ui(picked.is_some(), |ui| {
        if widgets::Button::primary(t("orgs.handover.offer")).hint("orgs.handover.offer_hint").show(ui, theme) {
            if let Some(who) = picked.clone() {
                *todo = Some(Act::Transfer { who, until: now + 7 * 86_400_000 });
                page.handover_to.clear();
            }
        }
    });
}

fn forget_button(ui: &mut egui::Ui, theme: &Theme, st: &OrgState, page: &mut OrgsPage) {
    if widgets::Button::danger(t("orgs.forget")).hint("orgs.forget_hint").show(ui, theme) {
        if let Some(s) = page.store.as_mut() {
            s.forget(&st.org_id);
        }
        page.folded.remove(&st.org_id);
        page.selected = None;
        page.view = View::List;
    }
}

#[cfg(test)]
#[path = "orgs_tests.rs"]
mod tests;
