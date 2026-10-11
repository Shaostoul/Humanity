//! The Media page (increment 2 of docs/design/media-showcase.md, 2026-10-11): the pictures,
//! videos, 3D models and files people on this server chose to show, in a grid. The web mirrors
//! it (web/pages/media.html); the relay half is src/relay/media.rs.
//!
//! What it has, top to bottom: an offer to carry on an upload a restart cut off; the person's own
//! uploads waiting to be published; a strip of every featured item (`featured=1`); the filters
//! (category, kind, person, My picks, a search over titles); the grid, newest first, with More for
//! the next page; each tile's actions (Pick, Remove, Feature, Keep on the server, Report). Add to
//! Media (a file from this computer, sent in pieces: media/resumable.rs) and Put on Media (a file
//! already in Shared Files, from the Files page) open the same short form. Suggest a category
//! sends the admins a name and a note.
//!
//! WHO SEES WHAT is the relay's to decide: a signed list from a member gets members-only items
//! too, anyone else the public ones, and on a members-only server nothing. Someone not signed in
//! to this server is told "Sign in to this server to see its members' media." A members-only
//! item's file is fetched through the members' file route with this session's upload token in a
//! header, never in an address.
//!
//! Pictures follow the chat's rules (gui/pages/chat/attach_view.rs): the protected setup's rule
//! first, then a click for someone who is not a friend; a labelled item (Spoiler, Flashing
//! lights, Graphic) shows its label instead of its picture until clicked, and the protected
//! setup hides labelled items altogether.
//!
//! THE STATE BELONGS TO ONE SERVER AND ONE PERSON: when either changes (another server, or a
//! restore from a recovery phrase), all of it is forgotten ([`sync_server`]), except an upload that
//! did not finish, which says which server and person it is for.
//!
//! NOTHING RUNS ON THE FRAME BUT DRAWING: every request goes to a worker thread ([`spawn`]) and
//! its answer is read on a later frame ([`poll`]), the way the Files and Watch pages do theirs.
//! [`poll_upload`] runs every frame whatever page is open (src/lib.rs), so an upload and its
//! publish carry on while the person is elsewhere. The admins' half (the Media section of Server
//! Settings, server_settings/media.rs) uses the same jobs.

use std::collections::HashSet;
use std::sync::mpsc::Receiver;

use egui::RichText;
use serde_json::Value;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

/// The requests and the server's answers.
pub(crate) mod api;
/// The kinds of item, from data/media/kinds.json.
pub(crate) mod kinds;
/// Add to Media's upload, in pieces (and kept in the config when a restart cuts it off).
pub(crate) mod resumable;
/// The grid's tiles: what each shows, and its actions.
mod tiles;
/// The Add to Media, Put on Media and Suggest a category forms.
mod add;
/// The upload under way: its progress, its controls, and the offer to carry one on.
mod uploading;
/// An opened item: a video playing, a model's or file's card with Download.
pub(crate) mod open;

pub(crate) use add::put_on_media;
pub(crate) use tiles::{KEEP as KEEP_LABEL, KEEP_TIP as KEEP_LABEL_TIP};
use api::{Filters, Item, ListAnswer, Refusal, Request};

/// What a request was for, so its answer lands in the right place when it comes back.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Job {
    /// A page of the list; `generation` is the filters' generation it was asked under, so an
    /// answer for filters since changed is dropped.
    List { append: bool, generation: u64 },
    /// A page of the featured items, for the strip.
    Featured { generation: u64, pages: u32 },
    /// The person's own use of the Media space and their waiting uploads.
    Mine,
    /// A publish of `filename`: uploaded here just now, a waiting Media upload, or a file already
    /// public on the server.
    Publish { of: add::Of, filename: String },
    Remove { id: Option<i64>, from_admin: bool },
    Pick { id: i64, on: bool },
    Feature { id: i64, on: bool },
    Keep { id: i64, on: bool, from_admin: bool },
    Suggest,
    /// An unfinished upload, cancelled on the server.
    Discard,
    /// The admins' section: the categories (a signed list read), saving them, the suggestions,
    /// a decision, and the usage.
    AdminCategories,
    SaveCategories,
    Suggestions,
    Decide,
    Usage,
}

impl Job {
    /// An action on one item, which the relay answers 409 when the tile was out of date (W5).
    fn acts_on_item(&self) -> bool {
        matches!(self, Job::Remove { id: Some(_), .. } | Job::Pick { .. } | Job::Feature { .. } | Job::Keep { .. })
    }

    fn from_admin(&self) -> bool {
        matches!(self, Job::Remove { from_admin: true, .. } | Job::Keep { from_admin: true, .. })
    }
}

/// The admins' half (Server Settings > ADMIN > Media), kept with the page's state.
#[derive(Default)]
pub struct MediaAdmin {
    /// Asked for this server's categories, suggestions and usage.
    pub requested: bool,
    /// The admin's working copy of the category list, and whether it differs from the server's.
    /// While it does, an answer from the server does not replace it (a Refresh never throws an
    /// edit away); Discard does.
    pub cats: Vec<api::CategoryEdit>,
    pub cats_dirty: bool,
    pub new_cat: String,
    pub suggestions: Vec<api::Suggestion>,
    pub usage: Option<api::Usage>,
    /// Which prune list is shown: 0 largest, 1 oldest, 2 least viewed, 3 waiting.
    pub prune_tab: usize,
    /// A Remove pressed in a prune list, waiting for its confirmation.
    pub confirm: Option<api::RemoveTarget>,
    pub status: String,
}

/// The Media page's state, kept in `GuiState::media`. It belongs to one server and one person:
/// when either changes, all of it is forgotten ([`sync_server`]) but `unfinished`.
#[derive(Default)]
pub struct MediaState {
    /// The server (normalised address) this state is for.
    pub server: String,
    /// The identity (public key) this state is for.
    pub key: String,
    /// The first page has been asked for.
    pub requested: bool,
    /// The list as answered so far (pages appended), with its categories and labels.
    pub list: ListAnswer,
    /// An answer has arrived (so "nothing here" can be said).
    pub answered: bool,
    pub filters: Filters,
    /// Bumped whenever the filters change.
    pub generation: u64,
    /// The search box (applied to `filters.q` on Enter or Search).
    pub search: String,
    /// The people the list has shown so far: (key, name), for the person filter.
    pub people: Vec<(String, String)>,
    /// One line about the last thing done or refused.
    pub status: String,
    pub jobs: Vec<(Job, Receiver<Result<Value, Refusal>>)>,
    /// Every featured item, for the strip (asked for page by page with `featured=1`).
    pub featured: Vec<Item>,
    pub featured_generation: u64,
    /// This person's own use of the Media space, as the server counts it, with their uploads
    /// waiting to be published (`/api/media/mine`).
    pub mine: Option<api::Mine>,
    /// Asking failed: the server checks the share itself then.
    pub mine_failed: bool,
    /// A Remove waiting for its confirmation.
    pub confirm_remove: Option<Item>,
    /// A waiting upload's Remove, waiting for its confirmation.
    pub confirm_waiting: Option<api::Waiting>,
    /// The Add to Media or Put on Media form, while open.
    pub form: Option<add::Form>,
    /// The file picker of Add to Media, while open.
    pub picker: Option<widgets::file_browser::FilePickerState>,
    /// The Suggest a category form: (name, note), while open.
    pub suggest: Option<(String, String)>,
    /// An opened video, model or file.
    pub open: Option<open::Opened>,
    pub admin: MediaAdmin,
    /// The Add to Media upload under way, on its worker thread.
    pub upload: Option<resumable::Run>,
    /// An upload that did not finish, kept in the app's config (it says which server and person
    /// it is for, so it survives a change of either).
    pub unfinished: Option<resumable::Unfinished>,
    /// Uploads whose publish was cancelled while on its way: taken off again when it lands.
    pub cancelled_publishes: HashSet<String>,
    /// A fingerprint of the upload token members-only pictures were last asked for with: when it
    /// changes (a reconnect), a refusal under the old one is forgotten.
    pub token_mark: String,
}

/// The server's web address for requests (`https://host`, from whatever form the app holds).
pub(crate) fn base_of(state: &GuiState) -> String {
    crate::gui::widgets::image_cache::http_base(&state.server_url)
}

/// Whether this server has signed this person in on this connection (it sent an upload token):
/// only then are reads signed and writes offered.
pub(crate) fn signed_in_here(state: &GuiState) -> bool {
    crate::gui::pages::chat::upload_token_here(state).is_some()
}

/// The signer, when this person is signed in to this server and their identity is unlocked.
pub(crate) fn signer_here(state: &GuiState) -> Option<api::Signer> {
    if !signed_in_here(state) {
        return None;
    }
    api::signer_of(state)
}

/// What is said when something needs a signature and there is none to make.
pub(crate) const UNLOCK_LINE: &str = "Unlock your identity and sign in to this server first.";

/// The signer for a write, or None with [`UNLOCK_LINE`] said on the page and in its Server
/// Settings section, so a press is never silently dropped.
pub(crate) fn signer_or_say(state: &mut GuiState) -> Option<api::Signer> {
    let signer = signer_here(state);
    if signer.is_none() {
        state.media.status = UNLOCK_LINE.to_string();
        state.media.admin.status = UNLOCK_LINE.to_string();
    }
    signer
}

/// This person's role on this server, as its member list says ('' when it does not).
fn my_role(state: &GuiState) -> &str {
    let me = &state.profile_public_key;
    if me.is_empty() {
        return "";
    }
    state.chat_users.iter().find(|u| &u.public_key == me).map(|u| u.role.as_str()).unwrap_or("")
}

/// Whether this person is an admin of this server (its member list says).
pub(crate) fn is_admin(state: &GuiState) -> bool {
    matches!(my_role(state), "admin" | "owner")
}

/// Whether this person may keep items on the server: an admin or a moderator (the relay's A7).
pub(crate) fn may_keep(state: &GuiState) -> bool {
    matches!(my_role(state), "admin" | "owner" | "mod" | "moderator")
}

/// Forget everything when the server or the person changed; returns true when either did. An
/// upload under way is stopped where it is (the server keeps it a day), and the record of it is
/// kept, so it is offered again on the same server to the same person.
pub(crate) fn sync_server(state: &mut GuiState) -> bool {
    let here = crate::gui::pages::chat::norm_server_url(&state.server_url);
    if state.media.server == here && state.media.key == state.profile_public_key {
        return false;
    }
    let old = std::mem::take(&mut state.media);
    if let Some(run) = old.upload.as_ref() {
        run.ctl.abandon();
    }
    state.media = MediaState { server: here, key: state.profile_public_key.clone(), unfinished: old.unfinished, ..Default::default() };
    true
}

/// Run `req` on a worker thread; its answer is read by [`poll`].
pub(crate) fn spawn(state: &mut GuiState, job: Job, req: Request) {
    let base = base_of(state);
    log::info!("Media: {} {}", if req.body.is_some() { "POST" } else { "GET" }, req.path.split('?').next().unwrap_or(""));
    spawn_with(state, job, move || api::send(&base, &req));
}

/// Run `work` on a worker thread as `job`.
pub(crate) fn spawn_with(state: &mut GuiState, job: Job, work: impl FnOnce() -> Result<Value, Refusal> + Send + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    state.media.jobs.push((job, rx));
}

/// Every frame, whatever page is open (src/lib.rs): forget another server's or person's state,
/// read the answers that came back, keep an upload under way going, and hand it the current
/// upload token. Once per run it also clears the folder "Play here" fetches into, of anything a
/// run that did not close cleanly left there.
pub(crate) fn poll_upload(ctx: &egui::Context, state: &mut GuiState) {
    open::sweep_temp_once();
    sync_server(state);
    poll(ctx, state);
    note_token(state);
    uploading::poll(ctx, state);
}

/// Read every answer that has come back. Called every frame by [`poll_upload`], and by the Media
/// page and the Media section of Server Settings.
pub(crate) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let mut done = Vec::new();
    state.media.jobs.retain(|(job, rx)| match rx.try_recv() {
        Ok(result) => {
            done.push((job.clone(), result));
            false
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => true,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            done.push((job.clone(), Err(Refusal::new(0, "The request stopped unexpectedly."))));
            false
        }
    });
    if !state.media.jobs.is_empty() {
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    for (job, result) in done {
        on_done(state, job, result);
    }
}

/// When this session's upload token changes (a reconnect), members-only pictures refused under
/// the old one are asked for again: a refusal is never kept past the token it was given to.
fn note_token(state: &mut GuiState) {
    let token = crate::gui::pages::chat::upload_token_here(state);
    let mark = token.as_deref().map(|t| blake3::hash(t.as_bytes()).to_hex()[..16].to_string()).unwrap_or_default();
    if mark != state.media.token_mark {
        state.media.token_mark = mark;
        let keys: Vec<String> =
            state.media.list.items.iter().chain(state.media.featured.iter()).filter(|i| i.members_only()).map(|i| tiles::cache_key(state, i)).collect();
        for key in keys {
            state.image_cache.retry(&key);
        }
    }
    if let Some(run) = state.media.upload.as_ref() {
        run.ctl.set_token(token);
    }
}

/// What is said when an action on an item was refused because the page was out of date (409).
pub(crate) const STALE_LINE: &str = "That item changed since this page was loaded, so nothing was done. The page has been loaded again.";

/// Put an answer where it belongs.
pub(crate) fn on_done(state: &mut GuiState, job: Job, result: Result<Value, Refusal>) {
    // A 409 to an action on an item: the tile was out of date (W5). Load it all again, and say so.
    if let Err(r) = &result {
        if r.stale() && job.acts_on_item() {
            if job.from_admin() {
                state.media.admin.status = STALE_LINE.to_string();
                refresh_usage(state);
            } else {
                state.media.status = STALE_LINE.to_string();
            }
            reload(state);
            return;
        }
    }
    match (job, result) {
        (Job::List { append, generation }, Ok(v)) => {
            if generation == state.media.generation {
                apply_list(&mut state.media, api::parse_list(&v), append);
            }
        }
        (Job::List { generation, .. }, Err(e)) => {
            if generation == state.media.generation {
                state.media.answered = true;
                state.media.status = e.words;
            }
        }
        (Job::Featured { generation, pages }, Ok(v)) => {
            if generation == state.media.featured_generation {
                let answer = api::parse_list(&v);
                if pages == 1 {
                    state.media.featured.clear();
                }
                state.media.featured.extend(answer.items);
                // Every featured item, page by page (a strip of more than 480 shows the first 480).
                if let (Some(before), true) = (answer.next_before, pages < 10) {
                    featured_page(state, Some(before), pages + 1);
                }
            }
        }
        (Job::Featured { .. }, Err(e)) => log::warn!("Media: the featured items could not be read: {e}"),
        (Job::Mine, Ok(v)) => {
            state.media.mine = Some(api::parse_mine(&v));
            state.media.mine_failed = false;
        }
        (Job::Mine, Err(e)) => {
            log::warn!("Media: this person's own use could not be read: {e}");
            state.media.mine_failed = true;
        }
        (Job::Publish { of, filename }, result) => uploading::published(state, of, &filename, result),
        (Job::Remove { id, from_admin }, Ok(_)) => {
            if let Some(id) = id {
                state.media.list.items.retain(|i| i.id != id);
                state.media.featured.retain(|i| i.id != id);
            }
            if from_admin {
                state.media.admin.status = "Removed.".to_string();
                refresh_usage(state);
            } else {
                state.media.status = "Removed from Media.".to_string();
            }
            refresh_mine(state);
        }
        (Job::Remove { from_admin, .. }, Err(e)) => {
            if from_admin {
                state.media.admin.status = e.words;
            } else {
                state.media.status = e.words;
            }
        }
        (Job::Pick { id, on }, Ok(v)) => {
            let picks = v.get("picks").and_then(Value::as_i64);
            for item in state.media.list.items.iter_mut().chain(state.media.featured.iter_mut()).filter(|i| i.id == id) {
                item.picked_by_me = on;
                if let Some(n) = picks {
                    item.picks = n;
                }
            }
        }
        (Job::Feature { id, on }, Ok(_)) => {
            for item in state.media.list.items.iter_mut().filter(|i| i.id == id) {
                item.featured = on;
            }
            reload_featured(state);
        }
        (Job::Keep { id, on, from_admin }, Ok(_)) => {
            for item in state.media.list.items.iter_mut().chain(state.media.featured.iter_mut()).filter(|i| i.id == id) {
                item.kept = on;
            }
            let words = if on { "Kept on the server: it no longer counts against its uploader's share." } else { "No longer kept: it counts against its uploader's share again." };
            if from_admin {
                state.media.admin.status = words.to_string();
            } else {
                state.media.status = words.to_string();
            }
            if state.media.admin.requested {
                refresh_usage(state);
            }
        }
        (Job::Keep { from_admin: true, .. }, Err(e)) => state.media.admin.status = e.words,
        (Job::Pick { .. } | Job::Feature { .. } | Job::Keep { .. }, Err(e)) => state.media.status = e.words,
        (Job::Suggest, Ok(_)) => {
            state.media.suggest = None;
            state.media.status = SUGGESTION_SENT.to_string();
        }
        (Job::Suggest, Err(e)) => state.media.status = e.words,
        (Job::Discard, Ok(_)) => state.media.status = "The unfinished upload was cancelled.".to_string(),
        (Job::Discard, Err(e)) => log::warn!("Media: the unfinished upload could not be cancelled on the server ({e}); it goes after a day."),
        (Job::AdminCategories, Ok(v)) => {
            let answer = api::parse_list(&v);
            // An admin's unsaved edits are never thrown away by an answer: Save or Discard decides.
            if !state.media.admin.cats_dirty {
                state.media.admin.cats = answer
                    .categories
                    .iter()
                    .map(|c| api::CategoryEdit { id: Some(c.id), name: c.name.clone(), description: c.description.clone() })
                    .collect();
            }
            state.media.list.categories = answer.categories;
        }
        (Job::SaveCategories, Ok(v)) => {
            let cats = api::parse_categories(v.get("categories"));
            state.media.admin.cats = cats
                .iter()
                .map(|c| api::CategoryEdit { id: Some(c.id), name: c.name.clone(), description: c.description.clone() })
                .collect();
            state.media.admin.cats_dirty = false;
            state.media.list.categories = cats;
            state.media.admin.status = "Categories saved.".to_string();
        }
        (Job::Suggestions, Ok(v)) => state.media.admin.suggestions = api::parse_suggestions(&v),
        (Job::Decide, Ok(_)) => {
            state.media.admin.status = "Decided; the person who suggested it is told.".to_string();
            refresh_admin(state);
        }
        (Job::Usage, Ok(v)) => state.media.admin.usage = Some(api::parse_usage(&v)),
        (Job::AdminCategories | Job::SaveCategories | Job::Suggestions | Job::Decide | Job::Usage, Err(e)) => {
            state.media.admin.status = e.words;
        }
    }
}

/// What is said when a suggestion went.
pub(crate) const SUGGESTION_SENT: &str = "Your suggestion went to this server's admins. You will be told when they decide.";

/// Put a list answer in place: the first page replaces, More appends.
pub(crate) fn apply_list(m: &mut MediaState, answer: ListAnswer, append: bool) {
    m.answered = true;
    let ListAnswer { items, next_before, members_view, media_visibility, members_only_for_you, categories, labels } = answer;
    for i in &items {
        if !i.uploader_key.is_empty() && !m.people.iter().any(|(k, _)| k == &i.uploader_key) {
            m.people.push((i.uploader_key.clone(), i.by_name().to_string()));
        }
    }
    if append {
        m.list.items.extend(items);
    } else {
        m.list.items = items;
    }
    m.list.next_before = next_before;
    m.list.members_view = members_view;
    m.list.media_visibility = media_visibility;
    m.list.members_only_for_you = members_only_for_you;
    // An answer with none (a members-only server to the public) leaves what is known as it is.
    if !categories.is_empty() {
        m.list.categories = categories;
    }
    if !labels.is_empty() {
        m.list.labels = labels;
    }
}

/// Ask for everything again: the first page, the featured strip and the person's own use (the
/// page opened, Refresh, or something was added or removed).
pub(crate) fn reload(state: &mut GuiState) {
    reload_list(state);
    reload_featured(state);
    refresh_mine(state);
}

/// Ask for the first page again (the filters changed).
pub(crate) fn reload_list(state: &mut GuiState) {
    if state.server_url.trim().is_empty() {
        return;
    }
    state.media.generation += 1;
    state.media.requested = true;
    let req = api::list_request(&state.media.filters, None, api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.media.generation;
    spawn(state, Job::List { append: false, generation }, req);
}

/// Ask for every featured item again, for the strip.
pub(crate) fn reload_featured(state: &mut GuiState) {
    if state.server_url.trim().is_empty() {
        return;
    }
    state.media.featured_generation += 1;
    featured_page(state, None, 1);
}

fn featured_page(state: &mut GuiState, before: Option<i64>, pages: u32) {
    let f = Filters { featured: true, ..Default::default() };
    let req = api::list_request(&f, before, api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.media.featured_generation;
    spawn(state, Job::Featured { generation, pages }, req);
}

/// Ask the server for this person's own use of the Media space, with their waiting uploads
/// (`/api/media/mine`): after every upload, publish and remove, and when the form opens.
pub(crate) fn refresh_mine(state: &mut GuiState) {
    let Some(signer) = signer_here(state) else { return };
    if state.media.jobs.iter().any(|(j, _)| *j == Job::Mine) {
        return;
    }
    spawn(state, Job::Mine, api::mine_request(&signer));
}

/// Ask for the next page.
fn more(state: &mut GuiState) {
    let Some(before) = state.media.list.next_before else { return };
    let req = api::list_request(&state.media.filters, Some(before), api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.media.generation;
    spawn(state, Job::List { append: true, generation }, req);
}

/// The admins' section asks again for the categories, the suggestions and the usage.
pub(crate) fn refresh_admin(state: &mut GuiState) {
    let Some(signer) = signer_here(state) else {
        state.media.admin.status = "Sign in to this server to manage its Media page.".to_string();
        return;
    };
    state.media.admin.requested = true;
    let cats = api::list_request(&Filters::default(), None, 1, &signer.key, Some(&signer));
    spawn(state, Job::AdminCategories, cats);
    spawn(state, Job::Suggestions, api::suggestions_request(&signer));
    refresh_usage(state);
}

/// The admins' meter and prune lists, asked for again.
pub(crate) fn refresh_usage(state: &mut GuiState) {
    if let Some(signer) = signer_here(state) {
        spawn(state, Job::Usage, api::usage_request(&signer));
    }
}

/// Open the Media page on one item, by its number (the Reports page's Open: the relay's A5). The
/// list then holds that item alone, if this person may see it, with a line to show everything.
pub(crate) fn open_item_by_number(state: &mut GuiState, id: i64) {
    sync_server(state);
    state.media.filters = Filters { id: Some(id), ..Default::default() };
    state.media.search.clear();
    reload_list(state);
    state.push_nav_to(GuiPage::Media);
}

/// Whether the whole Media page is members only on this server: its answer says, else its
/// settings, else not known (public).
pub(crate) fn members_only_server(state: &GuiState) -> bool {
    if !state.media.list.media_visibility.is_empty() {
        return state.media.list.media_visibility != "public";
    }
    state.server_settings.as_ref().is_some_and(|s| s.media_visibility != "public")
}

/// The line for someone who gets only the public view.
pub(crate) const SIGN_IN_LINE: &str = "Sign in to this server to see its members' media.";
/// The line when the list is empty.
pub(crate) const EMPTY_LINE: &str = "Nothing is on this server's Media page yet.";

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    poll_upload(ctx, state);
    if !state.media.requested && !state.server_url.trim().is_empty() {
        reload(state);
    }
    if crate::engine::protected::is_on(state) {
        crate::engine::protected::ensure_preset(state);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.panel_margin))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("media_page").auto_shrink([false, false]).show(ui, |ui| {
                draw_body(ui, theme, state);
            });
        });

    add::draw_dialogs(ctx, theme, state);
    tiles::draw_confirm_remove(ctx, theme, state);
    uploading::draw_confirm_waiting(ctx, theme, state);
    open::draw(ctx, theme, state);
    // A Report pressed on a tile opens the existing report dialog, drawn here while this page is
    // on screen (the chat page draws it on its own).
    crate::gui::pages::chat::draw_report_dialog(ctx, theme, state);
}

/// Everything inside the page's scroll area.
pub(crate) fn draw_body(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Media").size(theme.font_size_title).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
        let signed_in = signed_in_here(state);
        let busy = state.media.upload.is_some();
        if signed_in
            && widgets::Button::primary("Add to Media")
                .disabled(busy)
                .tooltip(if busy { "One upload at a time: wait for the one under way." } else { "Upload a picture, video, model or file and show it here." })
                .show(ui, theme)
        {
            add::begin_add(state);
        }
        if signed_in && widgets::Button::secondary("Suggest a category").tooltip("Ask this server's admins for a new category.").show(ui, theme) {
            state.media.suggest = Some((String::new(), String::new()));
        }
        if widgets::Button::secondary("Shared Files").tooltip("The server's public file library, on the Files page.").show(ui, theme) {
            state.push_nav_to(GuiPage::Files);
        }
        if widgets::Button::ghost("Refresh").show(ui, theme) {
            reload(state);
        }
    });
    widgets::body_hint(
        ui,
        theme,
        "Pictures, videos, 3D models and files the people on this server chose to show. Nothing is here unless the person who uploaded it put it here, and nothing from a private conversation can be.",
    );
    if is_admin(state) {
        ui.horizontal(|ui| {
            widgets::body_hint(ui, theme, "Admins: the Media space, its categories and suggestions are in Server Settings.");
            if widgets::Button::ghost("Open Server Settings").show(ui, theme) {
                state.push_nav_to(GuiPage::ServerSettings);
            }
        });
    }
    ui.add_space(theme.spacing_sm);

    if state.server_url.trim().is_empty() {
        ui.label(RichText::new("Connect to a server first (Chat page) to see its Media page.").size(theme.font_size_small).color(theme.warning()));
        return;
    }
    if !state.media.status.is_empty() {
        ui.label(RichText::new(&state.media.status).size(theme.font_size_small).color(theme.text_secondary()));
    }
    if state.media.answered && !state.media.list.members_view {
        ui.label(RichText::new(SIGN_IN_LINE).size(theme.font_size_small).color(theme.warning()));
    }
    uploading::draw_on_page(ui, theme, state);
    ui.add_space(theme.spacing_sm);

    if let Some(id) = state.media.filters.id {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("Showing item {id} only, from a report.")).size(theme.font_size_small).color(theme.info()));
            if widgets::Button::secondary("Show everything").show(ui, theme) {
                state.media.filters = Filters::default();
                reload_list(state);
            }
        });
    } else {
        let featured = state.media.featured.clone();
        if !featured.is_empty() {
            widgets::subsection_label(ui, theme, "Featured");
            egui::ScrollArea::horizontal().id_salt("media_featured").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for item in &featured {
                        tiles::draw_tile(ui, theme, state, item);
                    }
                });
            });
            ui.add_space(theme.spacing_md);
        }
        tiles::draw_filters(ui, theme, state);
    }
    ui.add_space(theme.spacing_sm);

    let items = state.media.list.items.clone();
    if items.is_empty() {
        let line = if !state.media.answered {
            "Loading..."
        } else if state.media.filters.id.is_some() {
            "That item is not on the Media page, or you may not see it."
        } else if state.media.filters != Filters::default() {
            "Nothing matches these filters."
        } else {
            EMPTY_LINE
        };
        ui.label(RichText::new(line).size(theme.font_size_body).color(theme.text_muted()));
    } else {
        tiles::draw_grid(ui, theme, state, &items);
    }
    if state.media.list.next_before.is_some() {
        ui.add_space(theme.spacing_sm);
        let loading = state.media.jobs.iter().any(|(j, _)| matches!(j, Job::List { append: true, .. }));
        if loading {
            ui.label(RichText::new("Loading more...").size(theme.font_size_small).color(theme.text_muted()));
        } else if widgets::Button::secondary("More").show(ui, theme) {
            more(state);
        }
    }
}

#[cfg(test)]
#[path = "media/page_tests.rs"]
pub(crate) mod tests;
