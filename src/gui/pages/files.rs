//! The Files page (called the Media page until 2026-10-19; increment 2 of
//! docs/design/files-showcase.md, 2026-10-11): the pictures,
//! videos, 3D models, sounds and files people on this server chose to show. The web mirrors it
//! (web/pages/files.html); the relay half is src/relay/files.rs.
//!
//! What it has, top to bottom: its heading, "Files on united-humanity.us" (the server it is for,
//! Media round 5), with the page's buttons and the view, Panes or Gallery; then three panes, whose
//! left one starts with the PLACES (2026-10-19, files/places.rs): on the server, "Shared on
//! <server>" (what follows), "Shared files" (the server's public file library,
//! files/shared_files.rs) and for admins "Admin" (the admins' section, files/admin.rs, which was in
//! Server Settings until round 5); on this computer, "HumanityOS folder" (the app's own folder, to
//! read and edit: files/app_folder.rs) and "Your folders" (folders on this computer to pick files
//! from, files/your_folders.rs, 2026-10-18, desktop only). The developer's Files page and the Media
//! page became this one page on 2026-10-19 (operator: "What's the real difference between these
//! two? Why would we not want to merge them?").
//!
//! SHARED ON <SERVER> IS THREE PANES (2026-10-19, files/panes.rs): on the left the list
//! (files/list.rs): the uploads in progress, each with its bar (files/uploading.rs), and the
//! offers to carry on ones a restart or a switch cut off; the person's own uploads waiting to be
//! published, and the server's unfinished ones this app lost track of (files/waiting.rs); the
//! featured shelf (`featured=1`: the main shelf, or the picked category's shelf, "Featured in Fire
//! flow arts"); the filters (category with its description under them, kind, person, Sort, My
//! picks, a search over titles: files/filters.rs); every item as a row, in the order picked
//! (Newest, Most picked, Most downloaded), whose next page is asked for once when the list's end
//! comes into view, with More under it as well for keyboards; and the foot. In the middle the
//! item pressed, to fit (files/viewer.rs: a picture, a video, a sound's waveform, a model in 3D,
//! a file's card); on the right everything about it and its buttons, or its edit form in place
//! (files/item_view.rs, files/edit.rs). Side by side on a wide window, stacked on a tall one. The
//! GALLERY is the other view: the page as it was before the panes, every item a tile with its
//! actions (Pick, Remove, Feature, Keep on the server, Report: files/tiles.rs); a press on a tile
//! shows that item in the panes. Add to Files (a file from this computer, sent in pieces:
//! files/resumable.rs) and Put on the Files page (a file already in Shared Files, from the Files page) open
//! the same short form. Suggest a category sends the admins a name and a note. At the list's foot,
//! for anyone: Report a copyright problem and the public takedown log (files/takedown.rs; an item's
//! details have the same Report a copyright problem).
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
//! restore from a recovery phrase), all of it is forgotten ([`sync_server`]), except the records of
//! uploads that did not finish, which say which server and person each is for.
//!
//! NOTHING RUNS ON THE FRAME BUT DRAWING: every request goes to a worker thread ([`spawn`]) and
//! its answer is read on a later frame ([`poll`]), the way the Files and Watch pages do theirs.
//! [`poll_upload`] runs every frame whatever page is open (src/lib.rs), so an upload and its
//! publish carry on while the person is elsewhere. The admins' half (this page's Admin tab,
//! files/admin.rs; Server Settings keeps one line pointing to it) uses the same jobs.

use std::sync::mpsc::Receiver;

use egui::RichText;
use serde_json::Value;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

/// The requests and the server's answers.
pub(crate) mod api;
/// Media round 5's requests and answers: an item's files, the licences, the order, the edit.
pub(crate) mod api_items;
/// The item shown: its details pane, its window over the chat, and where the answers for it land
/// (round 5's lane 1D; in the panes since 2026-10-19).
pub(crate) mod item_view;
/// An item's edit form (round 5, lane 1D): its words, licence, who can see it, files and cover.
pub(crate) mod edit;
/// The kinds of item, from data/files/kinds.json.
pub(crate) mod kinds;
/// Add to Files's upload, in pieces (and kept in the config when a restart cuts it off).
pub(crate) mod resumable;
/// How the upload waits: Pause, allowances, back-off, a server error that will not clear.
pub(crate) mod upload_waits;
/// Cancelling an upload on the server: asked again while busy, "Cancelled" only when confirmed.
pub(crate) mod upload_cancel;
/// The page's handle on one running upload: its phase, bar and words.
pub(crate) mod upload_run;
/// The grid's tiles: what each shows, and its actions.
mod tiles;
/// The Add to Files, Put on the Files page and Suggest a category forms.
mod add;
/// The uploads in progress, a list of their own, and the records of unfinished ones.
mod uploading;
/// Putting a file on Media, and undoing it on Cancel even after a switch.
mod publishing;
/// The person's waiting uploads and the server's unfinished ones.
mod waiting;
/// An opened item: a video playing, a model's or file's card with Download.
pub(crate) mod open;
/// The three panes (2026-10-19): the list, the item to fit, its details; and the view kept.
pub(crate) mod panes;
/// The left pane: every item as a row.
mod list;
/// The middle pane: the item shown to fit (a picture, a video, a sound's waveform, a model).
mod viewer;

/// The open item's video or sound gets its sound from the app's mixer (`open::sync_sound`); the
/// engine calls this each frame it has a mixer.
pub(crate) fn sync_sound(state: &mut GuiState, audio: &mut crate::audio::AudioManager) {
    if let Some(opened) = state.files.open.as_mut() {
        open::sync_sound(opened, audio);
    }
}
/// The admins' section (moved from Server Settings, Media round 5): the settings, the categories,
/// the suggestions and the meter.
pub(crate) mod admin;
/// The filters row (moved from tiles.rs, Media round 5).
mod filters;
/// Tickets: a members-only file watched or downloaded without a token in any address (R3-8).
pub(crate) mod ticket;
/// "Report a copyright problem", the admins' decisions on those reports, and the public takedown
/// log (2026-10-13; the relay half is relay/files_takedown.rs, the website's files-takedown.js).
pub(crate) mod takedown;
/// A Media item posted in chat, shown as a card that opens its window (2026-10-17).
pub(crate) mod chat_card;
/// The places at the top of the left pane (2026-10-19): Shared on <server>, Shared files, Admin,
/// HumanityOS folder, Your folders.
pub(crate) mod places;
/// The "Shared files" place: the server's public file library, with Upload and Remove (the top
/// half of the developer's Files page until 2026-10-19).
pub(crate) mod shared_files;
/// A file picked for Shared files, cleaned on this device first (2026-10-11).
pub(crate) mod shared_upload_step;
/// The "HumanityOS folder" place: the app's own folder, to read and edit (the developer's Files
/// page until 2026-10-19).
pub(crate) mod app_folder;
// "How this page works" (2026-10-20): what the page and each place is, folded behind one toggle.
pub(crate) mod how;
/// Settings > Privacy, Files in messages, and Settings > Data, Kept files (2026-10-11).
pub(crate) mod in_messages;
/// The Your folders tab (2026-10-18, desktop only): folders on this computer to look through and
/// pick files from for Add to Files.
pub(crate) mod your_folders;
/// The private record of what this computer put on Files pages, which marks those files in Your
/// folders.
pub(crate) mod from_here;

pub(crate) use add::put_on_files_page;
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
    /// public on the server; `who` is the upload's or the form's number its answer goes to.
    Publish { of: add::Of, filename: String, who: u64 },
    Remove { id: Option<i64>, from_admin: bool },
    Pick { id: i64, on: bool },
    /// Featured on the main shelf, its category's only, or neither (round 5).
    Feature { id: i64, to: api::Featured },
    Keep { id: i64, on: bool, from_admin: bool },
    Suggest,
    /// The server's upload `id` cancelled (an upload in the list whose file is up, a record's, or
    /// one the server listed): the answer says whether the server confirmed (uploading.rs
    /// `cancel_answered`).
    Discard { id: String },
    /// The admins' section: the categories (a signed list read), saving them, the suggestions,
    /// a decision, and the usage.
    AdminCategories,
    SaveCategories,
    Suggestions,
    Decide,
    Usage,
    /// Round 5 (item_view.rs `on_answer`): one item read for its own page (opened by its number:
    /// item_view.rs `open_by_number`), an edit, a file taken off (each answers the item as it now
    /// is), and a view counted. A file ADDED to an item has no job of its own: it is an upload like
    /// any other, published onto the item as `Publish { of: Of::Attach, .. }` (publishing.rs
    /// `attach`), whose answer lands in publishing.rs `attach_answered`.
    Item { id: i64 },
    /// One item read for a card in chat (chat_card.rs).
    Card { id: i64 },
    Edit { id: i64 },
    FileRemove { id: i64, file: i64 },
    /// A file renamed, or a picture already on the item made its cover (2026-10-17): each answers
    /// the item as it now is.
    FileRename { id: i64, file: i64 },
    FileCover { id: i64, file: i64 },
    Viewed { id: i64 },
    /// "Report a copyright problem" (takedown.rs `on_answer`): the form sent (`who` is the form's
    /// number, so an answer for a form closed meanwhile is dropped), the public log read, the
    /// admins' read of the open reports, and an admin's decision on report `id`.
    TakedownSend { who: u64 },
    TakedownLog,
    TakedownOpen,
    TakedownDecide { id: i64, removed: bool },
}

impl Job {
    /// An action on one item, which the relay answers 409 when the tile was out of date (W5).
    fn acts_on_item(&self) -> bool {
        matches!(
            self,
            Job::Remove { id: Some(_), .. } | Job::Pick { .. } | Job::Feature { .. } | Job::Keep { .. } | Job::Edit { .. } | Job::FileRemove { .. }
        )
    }

    fn from_admin(&self) -> bool {
        matches!(self, Job::Remove { from_admin: true, .. } | Job::Keep { from_admin: true, .. })
    }
}

/// The admins' half (the Files page's Admin tab, files/admin.rs), kept with the page's state.
#[derive(Default)]
pub struct FilesAdmin {
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

/// The page's places (2026-10-19, files/places.rs; they were its tabs since round 5): what people
/// chose to show on the server; the server's shared files; for admins the section that was in
/// Server Settings (files/admin.rs); the app's own folder; and Your folders, folders on this
/// computer to pick from (2026-10-18, files/your_folders.rs, desktop only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    #[default]
    Shared,
    SharedFiles,
    Admin,
    AppFolder,
    YourFolders,
}

impl Place {
    /// A place on this computer, which no server's state belongs to.
    pub fn on_this_computer(self) -> bool {
        matches!(self, Place::AppFolder | Place::YourFolders)
    }
}

/// The Files page's state, kept in `GuiState::media`. It belongs to one server and one person:
/// when either changes, all of it is forgotten ([`sync_server`]) but `unfinished`.
#[derive(Default)]
pub struct FilesState {
    /// Which place is shown (2026-10-19; the tab, since round 5).
    pub place: Place,
    /// The item shown in the panes, or in its window over the chat (round 5, item_view.rs).
    pub item: Option<item_view::ItemPage>,
    /// Answered questions about an edit's changes: the item to show next once they were let go,
    /// or the item's window to close (media.rs `after_questions`).
    pub open_next: Option<Item>,
    pub close_window: bool,
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
    /// Every featured item on the shelf shown (asked for page by page with `featured=1`, and the
    /// picked category with it when one is picked: round 5).
    pub featured: Vec<Item>,
    pub featured_generation: u64,
    /// The category the shelf was last asked for (None: the main shelf), and the one `featured`
    /// holds now (set when its first page arrives, so the heading never names a shelf before its
    /// items are in).
    pub shelf_asked: Option<i64>,
    pub shelf_of: Option<i64>,
    /// A next page could not be had (the server refused it, or could not be reached): the end of
    /// the list coming into view asks again only after More is pressed or the list is loaded
    /// afresh, so a failing server is never asked once a frame.
    pub more_paused: bool,
    /// This person's own use of the Media space, as the server counts it, with their uploads
    /// waiting to be published (`/api/files/mine`).
    pub mine: Option<api::Mine>,
    /// Asking failed: the server checks the share itself then.
    pub mine_failed: bool,
    /// A Remove waiting for its confirmation.
    pub confirm_remove: Option<Item>,
    /// A waiting upload's Remove, waiting for its confirmation.
    pub confirm_waiting: Option<api::Waiting>,
    /// The Add to Files or Put on the Files page form, while open.
    pub form: Option<add::Form>,
    /// The file picker of Add to Files, while open.
    pub picker: Option<widgets::file_browser::FilePickerState>,
    /// The Suggest a category form: (name, note), while open.
    pub suggest: Option<(String, String)>,
    /// An opened video, model or file.
    pub open: Option<open::Opened>,
    pub admin: FilesAdmin,
    /// The uploads in progress, each on its worker thread, each published by what its own form
    /// said (uploading.rs).
    pub uploads: Vec<uploading::Entry>,
    /// The uploads the server has named that did not finish, kept in the app's config (each says
    /// which server and person it is for, so it survives a change of either).
    pub unfinished: Vec<resumable::Unfinished>,
    /// Items posted in chat, as their cards know them (chat_card.rs), the cards opened out by their
    /// key, and what the cards asked for, done at the start of the chat's next frame.
    pub cards: std::collections::HashMap<i64, chat_card::CardState>,
    pub cards_open: std::collections::HashSet<String>,
    pub card_acts: Vec<chat_card::Act>,
    /// The publishes on their way, by the number of the upload or form that asked: Cancel reaches
    /// them through this (publishing.rs).
    pub publishes: Vec<(u64, publishing::Fate)>,
    /// The server's uploads (by id) whose cancel is on its way: each is shown "Cancelling..." and
    /// stays listed until the server confirmed (round 4, D4-2).
    pub cancelling: Vec<String>,
    /// A fingerprint of the upload token members-only pictures were last asked for with: when it
    /// changes (a reconnect), a refusal under the old one is forgotten.
    pub token_mark: String,
    /// What the picked file of the open Add to Files form says (the form's number and its look,
    /// worked out on a worker; the files section of docs/design/files-showcase.md, 2026-10-11).
    pub look: Option<(u64, crate::gui::pages::chat::attach_step::Looking)>,
    /// When the cleaned copies of uploads were last swept (uploading.rs `sweep_cleaned`).
    pub last_sweep: Option<std::time::Instant>,
    /// "Report a copyright problem": its form, the public log's window, and the admins' open
    /// reports (takedown.rs).
    pub takedown: takedown::Takedowns,
    /// The Your folders place: the folder open in it and the files ticked (your_folders.rs). Kept
    /// when the server or the person changes, since nothing in it belongs to a server.
    pub folders: your_folders::Browse,
    /// The Shared files place: the server's public file library (shared_files.rs).
    pub shared: shared_files::SharedFiles,
    /// The HumanityOS folder place: the app's own folder (app_folder.rs). Kept when the server or
    /// the person changes, as Your folders is.
    pub app_folder: app_folder::AppFolder,
}

/// The server this page is for: the one the connection was dialled for and signs in to, never an
/// address being typed into a Server field (round 4 of docs/design/files-showcase.md, D4-3: the
/// Chat page's connect form and the main menu edit `server_url` letter by letter, and one letter
/// typed while uploads waited for sign-in took the page to "another server" and abandoned them
/// all). Normalised; empty when there is none.
pub(crate) fn server_here(state: &GuiState) -> String {
    state.active_server()
}

/// The server's web address for requests (`https://host`, from whatever form the app holds).
pub(crate) fn base_of(state: &GuiState) -> String {
    crate::gui::widgets::image_cache::http_base(&server_here(state))
}

/// The server's host as a person reads it ("united-humanity.us", "127.0.0.1:8099"): the server
/// strip's own `host_of` over the same connected server, so the strip under the top menu and
/// this page's heading always name the same host. Empty with no server.
pub(crate) fn host_here(state: &GuiState) -> String {
    crate::gui::pages::server_strip::host_of(&server_here(state))
}

/// The page's heading (round 5, 3.9 of docs/design/media-round-5-plan.md): "Files on
/// united-humanity.us", so nobody wonders whose Files page this is; plain "Files" with no server.
pub(crate) fn heading(state: &GuiState) -> String {
    let host = host_here(state);
    if host.is_empty() {
        "Files".to_string()
    } else {
        format!("Files on {host}")
    }
}

/// This connection's upload token, when it is for the server this page is for ([`server_here`]).
pub(crate) fn token_here(state: &GuiState) -> Option<String> {
    let here = server_here(state);
    state.upload_token.as_ref().filter(|(server, _)| !here.is_empty() && *server == here).map(|(_, t)| t.clone())
}

/// Whether this server has signed this person in on this connection (it sent an upload token):
/// only then are reads signed and writes offered.
pub(crate) fn signed_in_here(state: &GuiState) -> bool {
    token_here(state).is_some()
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

/// The signer for a write, or None with [`UNLOCK_LINE`] said on the page and on its Admin tab,
/// so a press is never silently dropped.
pub(crate) fn signer_or_say(state: &mut GuiState) -> Option<api::Signer> {
    let signer = signer_here(state);
    if signer.is_none() {
        state.files.status = UNLOCK_LINE.to_string();
        state.files.admin.status = UNLOCK_LINE.to_string();
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
/// kept, so it is offered again on the same server to the same person. The server is the
/// connected one ([`server_here`]), never an address being typed.
pub(crate) fn sync_server(state: &mut GuiState) -> bool {
    let here = server_here(state);
    if state.files.server == here && state.files.key == state.profile_public_key {
        return false;
    }
    let old = std::mem::take(&mut state.files);
    uploading::abandon_all(&old.uploads);
    // Your folders and the HumanityOS folder belong to this computer, not to a server: they stay as
    // they were, shown if one was.
    let place = if old.place.on_this_computer() { old.place } else { Place::default() };
    state.files = FilesState { server: here, key: state.profile_public_key.clone(), unfinished: old.unfinished, folders: old.folders, app_folder: old.app_folder, place, ..Default::default() };
    true
}

/// A number no other upload or form on this page has had in this run: what a publish's answer
/// finds its upload or form by.
pub(crate) fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::SeqCst)
}

/// Run `req` on a worker thread; its answer is read by [`poll`].
pub(crate) fn spawn(state: &mut GuiState, job: Job, req: Request) {
    let base = base_of(state);
    log::info!("Files: {} {}", if req.body.is_some() { "POST" } else { "GET" }, req.path.split('?').next().unwrap_or(""));
    spawn_with(state, job, move || api::send(&base, &req));
}

/// Run `work` on a worker thread as `job`.
pub(crate) fn spawn_with(state: &mut GuiState, job: Job, work: impl FnOnce() -> Result<Value, Refusal> + Send + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    state.files.jobs.push((job, rx));
}

/// Every frame, whatever page is open (src/lib.rs): forget another server's or person's state,
/// read the answers that came back, keep an upload under way going, and hand it the current
/// upload token. Once per run it also clears the folder "Play here" fetches into, and the
/// downloads folder's unfinished part files, of anything a run that did not close cleanly left.
pub(crate) fn poll_upload(ctx: &egui::Context, state: &mut GuiState) {
    open::sweep_temp_once();
    crate::gui::widgets::image_cache::sweep_downloads_once();
    sync_server(state);
    poll(ctx, state);
    note_token(state);
    uploading::poll(ctx, state);
    // The item shown: its downloads finish, and its video or sound plays on, whatever is on screen.
    if let Some(opened) = state.files.open.as_mut() {
        open::poll(ctx, opened);
    }
}

/// Read every answer that has come back. Called every frame by [`poll_upload`], and by the Media
/// page and the Media section of Server Settings.
pub(crate) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let mut done = Vec::new();
    state.files.jobs.retain(|(job, rx)| match rx.try_recv() {
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
    if !state.files.jobs.is_empty() {
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    for (job, result) in done {
        on_done(state, job, result);
    }
}

/// When this session's upload token changes (a reconnect), members-only pictures refused under
/// the old one are asked for again: a refusal is never kept past the token it was given to.
fn note_token(state: &mut GuiState) {
    let token = token_here(state);
    let mark = token.as_deref().map(|t| blake3::hash(t.as_bytes()).to_hex()[..16].to_string()).unwrap_or_default();
    if mark != state.files.token_mark {
        state.files.token_mark = mark;
        let keys: Vec<String> =
            state.files.list.items.iter().chain(state.files.featured.iter()).filter(|i| i.members_only()).map(|i| tiles::cache_key(state, i)).collect();
        for key in keys {
            state.image_cache.retry(&key);
        }
    }
    for e in &state.files.uploads {
        e.run.ctl.set_token(token.clone());
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
                state.files.admin.status = STALE_LINE.to_string();
                refresh_usage(state);
            } else {
                state.files.status = STALE_LINE.to_string();
            }
            reload(state);
            return;
        }
    }
    match (job, result) {
        (Job::List { append, generation }, Ok(v)) => {
            if generation == state.files.generation {
                apply_list(&mut state.files, api::parse_list(&v), append);
                state.files.more_paused = false;
            }
        }
        (Job::List { append, generation }, Err(e)) => {
            if generation == state.files.generation {
                state.files.answered = true;
                state.files.status = e.words;
                // A next page that failed is asked again only when More is pressed: the end of
                // the list stays in view, and would otherwise ask the failing server every frame.
                if append {
                    state.files.more_paused = true;
                }
            }
        }
        (Job::Featured { generation, pages }, Ok(v)) => {
            if generation == state.files.featured_generation {
                let answer = api::parse_list(&v);
                if pages == 1 {
                    state.files.featured.clear();
                    // The shelf now holds the category it was asked for (its heading follows).
                    state.files.shelf_of = state.files.shelf_asked;
                }
                state.files.featured.extend(answer.items);
                // Every featured item, page by page (a strip of more than 480 shows the first 480).
                if let (Some(cursor), true) = (answer.next_cursor, pages < 10) {
                    featured_page(state, Some(&cursor), pages + 1);
                }
            }
        }
        (Job::Featured { .. }, Err(e)) => log::warn!("Files: the featured items could not be read: {e}"),
        (Job::Mine, Ok(v)) => {
            state.files.mine = Some(api::parse_mine(&v));
            state.files.mine_failed = false;
        }
        (Job::Mine, Err(e)) => {
            log::warn!("Files: this person's own use could not be read: {e}");
            state.files.mine_failed = true;
        }
        (Job::Publish { of, filename, who }, result) => publishing::published(state, of, who, &filename, result),
        (Job::Remove { id, from_admin }, Ok(_)) => {
            if let Some(id) = id {
                state.files.list.items.retain(|i| i.id != id);
                state.files.featured.retain(|i| i.id != id);
            }
            if from_admin {
                state.files.admin.status = "Removed.".to_string();
                refresh_usage(state);
            } else {
                state.files.status = "Removed from Files.".to_string();
            }
            refresh_mine(state);
        }
        (Job::Remove { from_admin, .. }, Err(e)) => {
            if from_admin {
                state.files.admin.status = e.words;
            } else {
                state.files.status = e.words;
            }
        }
        (Job::Pick { id, on }, Ok(v)) => {
            let picks = v.get("picks").and_then(Value::as_i64);
            for item in state.files.list.items.iter_mut().chain(state.files.featured.iter_mut()).filter(|i| i.id == id) {
                item.picked_by_me = on;
                if let Some(n) = picks {
                    item.picks = n;
                }
            }
        }
        (Job::Feature { id, to }, Ok(_)) => {
            for item in state.files.list.items.iter_mut().filter(|i| i.id == id) {
                item.featured = to;
            }
            reload_featured(state);
        }
        (Job::Keep { id, on, from_admin }, Ok(_)) => {
            for item in state.files.list.items.iter_mut().chain(state.files.featured.iter_mut()).filter(|i| i.id == id) {
                item.kept = on;
            }
            let words = if on { "Kept on the server: it no longer counts against its uploader's share." } else { "No longer kept: it counts against its uploader's share again." };
            if from_admin {
                state.files.admin.status = words.to_string();
            } else {
                state.files.status = words.to_string();
            }
            if state.files.admin.requested {
                refresh_usage(state);
            }
        }
        (Job::Keep { from_admin: true, .. }, Err(e)) => state.files.admin.status = e.words,
        (Job::Pick { .. } | Job::Feature { .. } | Job::Keep { .. }, Err(e)) => state.files.status = e.words,
        (Job::Suggest, Ok(_)) => {
            state.files.suggest = None;
            state.files.status = SUGGESTION_SENT.to_string();
        }
        (Job::Suggest, Err(e)) => state.files.status = e.words,
        (Job::Discard { id }, result) => uploading::cancel_answered(state, &id, result),
        (Job::AdminCategories, Ok(v)) => {
            let answer = api::parse_list(&v);
            // An admin's unsaved edits are never thrown away by an answer: Save or Discard decides.
            if !state.files.admin.cats_dirty {
                state.files.admin.cats = answer
                    .categories
                    .iter()
                    .map(|c| api::CategoryEdit { id: Some(c.id), name: c.name.clone(), description: c.description.clone() })
                    .collect();
            }
            state.files.list.categories = answer.categories;
        }
        (Job::SaveCategories, Ok(v)) => {
            let cats = api::parse_categories(v.get("categories"));
            state.files.admin.cats = cats
                .iter()
                .map(|c| api::CategoryEdit { id: Some(c.id), name: c.name.clone(), description: c.description.clone() })
                .collect();
            state.files.admin.cats_dirty = false;
            state.files.list.categories = cats;
            state.files.admin.status = "Categories saved.".to_string();
        }
        (Job::Suggestions, Ok(v)) => state.files.admin.suggestions = api::parse_suggestions(&v),
        (Job::Decide, Ok(_)) => {
            state.files.admin.status = "Decided; the person who suggested it is told.".to_string();
            refresh_admin(state);
        }
        (Job::Usage, Ok(v)) => state.files.admin.usage = Some(api::parse_usage(&v)),
        (Job::AdminCategories | Job::SaveCategories | Job::Suggestions | Job::Decide | Job::Usage, Err(e)) => {
            state.files.admin.status = e.words;
        }
        (Job::Card { id }, result) => chat_card::on_answer(state, id, result),
        (job @ (Job::Item { .. } | Job::Edit { .. } | Job::FileRemove { .. } | Job::FileRename { .. } | Job::FileCover { .. } | Job::Viewed { .. }), result) => {
            item_view::on_answer(state, job, result)
        }
        (job @ (Job::TakedownSend { .. } | Job::TakedownLog | Job::TakedownOpen | Job::TakedownDecide { .. }), result) => {
            takedown::on_answer(state, job, result)
        }
    }
}

/// What is said when a suggestion went.
pub(crate) const SUGGESTION_SENT: &str = "Your suggestion went to this server's admins. You will be told when they decide.";

/// Put a list answer in place: the first page replaces, More appends.
pub(crate) fn apply_list(m: &mut FilesState, answer: ListAnswer, append: bool) {
    m.answered = true;
    let ListAnswer { items, next_cursor, members_view, media_visibility, members_only_for_you, categories, labels, licences } = answer;
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
    m.list.next_cursor = next_cursor;
    m.list.licences = licences;
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
    if server_here(state).is_empty() {
        return;
    }
    state.files.generation += 1;
    state.files.requested = true;
    // A fresh list starts with its next page askable again.
    state.files.more_paused = false;
    let req = api::list_request(&state.files.filters, None, api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.files.generation;
    spawn(state, Job::List { append: false, generation }, req);
}

/// Ask for every featured item again, for the shelf: the main shelf, or with a category picked
/// in the filters, that category's shelf (round 5: `featured=1` with `category=c`, which the relay
/// answers with the items featured on the main shelf or on that category's alone, in it).
pub(crate) fn reload_featured(state: &mut GuiState) {
    if server_here(state).is_empty() {
        return;
    }
    state.files.featured_generation += 1;
    state.files.shelf_asked = state.files.filters.category;
    featured_page(state, None, 1);
}

/// One page of the shelf asked for: of the category it was last asked for, so its later pages
/// are the same shelf's.
fn featured_page(state: &mut GuiState, cursor: Option<&str>, pages: u32) {
    let f = Filters { featured: true, category: state.files.shelf_asked, ..Default::default() };
    let req = api::list_request(&f, cursor, api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.files.featured_generation;
    spawn(state, Job::Featured { generation, pages }, req);
}

/// Ask the server for this person's own use of the Media space, with their waiting uploads
/// (`/api/files/mine`): after every upload, publish and remove, and when the form opens.
pub(crate) fn refresh_mine(state: &mut GuiState) {
    let Some(signer) = signer_here(state) else { return };
    if state.files.jobs.iter().any(|(j, _)| *j == Job::Mine) {
        return;
    }
    spawn(state, Job::Mine, api::mine_request(&signer));
}

/// Whether a page of the list is on its way: a next page, or a first page after the filters
/// changed (whose answer the cursor held now does not belong to).
pub(crate) fn page_on_its_way(state: &GuiState) -> bool {
    state.files.jobs.iter().any(|(j, _)| matches!(j, Job::List { .. }))
}

/// Ask for the next page, once: nothing more is asked while a page of the list is on its way, so
/// the end of the list staying in view for many frames asks for one page, not one a frame. More
/// and the end coming into view both come here.
fn more(state: &mut GuiState) {
    if page_on_its_way(state) {
        return;
    }
    let Some(cursor) = state.files.list.next_cursor.clone() else { return };
    let req = api::list_request(&state.files.filters, Some(&cursor), api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.files.generation;
    spawn(state, Job::List { append: true, generation }, req);
}

/// The end of the list came into view (3.9): the next page, unless one failed since the list was
/// loaded, which waits for More.
pub(crate) fn more_when_seen(state: &mut GuiState) {
    if !state.files.more_paused {
        more(state);
    }
}

/// Server Settings' "Open them" (round 5): the Files page, on its Admin tab. The page's state is
/// brought to this server and person first, so the tab set here is never undone by the page
/// forgetting another server's state on its next frame.
pub(crate) fn open_admin_tab(state: &mut GuiState) {
    sync_server(state);
    state.files.place = Place::Admin;
    state.push_nav_to(GuiPage::Files);
}

/// The admins' section asks again for the categories, the suggestions and the usage.
pub(crate) fn refresh_admin(state: &mut GuiState) {
    let Some(signer) = signer_here(state) else {
        state.files.admin.status = "Sign in to this server to manage its Files page.".to_string();
        return;
    };
    state.files.admin.requested = true;
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

/// Open the Files page on one item, by its number (the Reports page's Open: the relay's A5), shown
/// in the panes as a press on its row shows it (item_view.rs), never the list filtered down to it: an item
/// the list or the shelf holds opens as it is, any other is asked for by its number, the way the
/// website's `route` does (files-app.js `openById`). Until it comes the page says "Loading...", and
/// if this person may not see it, that it is not there.
///
/// The list itself is left as the person had it (it was filtered to the one item until 2026-10-12,
/// which showed a tile to click instead of the item, and lost the person's own filters).
pub(crate) fn open_item_by_number(state: &mut GuiState, id: i64) {
    // The page's state brought to this server and person first, so the item page opened here is
    // never forgotten by the page catching up with another server's state on its next frame.
    sync_server(state);
    // What is shared, not another place the person may have left open: the item is what was
    // asked for.
    state.files.place = Place::Shared;
    item_view::open_by_number(state, id);
    // Shown in the panes, wherever the page was left.
    panes::set_gallery(state, false);
    state.push_nav_to(GuiPage::Files);
}

/// Whether the whole Files page is members only on this server: its answer says, else its
/// settings, else not known (public).
pub(crate) fn members_only_server(state: &GuiState) -> bool {
    if !state.files.list.media_visibility.is_empty() {
        return state.files.list.media_visibility != "public";
    }
    state.server_settings.as_ref().is_some_and(|s| s.media_visibility != "public")
}

/// The line for someone who gets only the public view.
pub(crate) const SIGN_IN_LINE: &str = "Sign in to this server to see its members' media.";
/// The line when the list is empty.
pub(crate) const EMPTY_LINE: &str = "Nothing is on this server's Files page yet.";

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    poll_upload(ctx, state);
    if !state.files.requested && !server_here(state).is_empty() {
        reload(state);
    }
    if crate::engine::protected::is_on(state) {
        crate::engine::protected::ensure_preset(state);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.panel_margin))
        .show(ctx, |ui| {
            draw_page(ui, theme, state);
        });

    // Shared files takes dropped files itself (to Shared Files); everywhere else on the page a
    // drop opens Add to Files.
    if places::shown_place(state.files.place, places::admin_here(state)) != Place::SharedFiles {
        offer_drop(ctx, state);
    }
    add::draw_dialogs(ctx, theme, state);
    your_folders::draw_dialogs(ctx, theme, state);
    tiles::draw_confirm_remove(ctx, theme, state);
    waiting::draw_confirm_waiting(ctx, theme, state);
    // The item's questions: Remove a file, Remove item, and changes not saved.
    item_view::draw_page_dialogs(ctx, theme, state);
    after_questions(state);
    // A Report pressed on a tile opens the existing report dialog, drawn here while this page is
    // on screen (the chat page draws it on its own).
    crate::gui::pages::chat::draw_report_dialog(ctx, theme, state);
    // "Report a copyright problem" and the public takedown log, over the page.
    takedown::draw_windows(ctx, theme, state);
}

/// What an answered question asked for: another item shown once the changes were let go, or the
/// item's window closed.
pub(crate) fn after_questions(state: &mut GuiState) {
    if let Some(next) = state.files.open_next.take() {
        let window = state.files.item.as_ref().is_some_and(|p| p.window);
        if window {
            item_view::open_in_window(state, &next);
        } else {
            item_view::open(state, &next);
        }
    }
    if std::mem::take(&mut state.files.close_window) {
        state.files.item = None;
        state.files.open = None;
    }
}

pub(crate) use add::{drop_and_post_words, post_key, upload_dropped, DROP_TO_UPLOAD};

/// Files dropped on the window open the upload window (Add to Files, every control: the first is
/// the item, the rest added once it is up), as choosing them with its button does; with no server
/// or signed out, why not is said. At `level`: the Files page's own, or `Level::App` for anywhere
/// in the app (gui/drop_files.rs).
fn offer_drop_at(ctx: &egui::Context, state: &GuiState, level: crate::gui::drop_files::Level) {
    use crate::gui::drop_files;
    if server_here(state).is_empty() {
        drop_files::refuse(ctx, level, "Files cannot be dropped here: connect to a server first, on the Chat page.");
    } else if !signed_in_here(state) {
        drop_files::refuse(ctx, level, "Files cannot be dropped here: sign in to this server first, on the Chat page.");
    } else {
        drop_files::register(ctx, level, DROP_TO_UPLOAD, |state: &mut GuiState, paths| upload_dropped(state, &paths, None));
    }
}

fn offer_drop(ctx: &egui::Context, state: &GuiState) {
    offer_drop_at(ctx, state, crate::gui::drop_files::Level::Page);
}

/// The app-wide default for dropped files: the upload window, from any page (src/lib.rs, each
/// frame, before the pages draw).
pub fn offer_drop_anywhere(ctx: &egui::Context, state: &GuiState) {
    offer_drop_at(ctx, state, crate::gui::drop_files::Level::App);
}

/// While the Files page is not on screen (src/lib.rs, each frame): an item brought up from another
/// page (a card in chat, chat_card.rs) in its window over whatever is; or, when the page was left
/// with an item's edit unsaved, the question about it.
pub fn draw_item_window(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if state.active_page == GuiPage::Files || state.files.item.is_none() {
        return;
    }
    if state.files.item.as_ref().is_some_and(|p| p.window) {
        item_view::draw_window(ctx, theme, state);
        after_questions(state);
    } else {
        item_view::draw_leave_question(ctx, theme, state);
    }
}

/// The upload window opened by a drop on another page (it is the Files page's own form, drawn here
/// while that page is not on screen).
pub fn draw_upload_window(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if state.active_page != GuiPage::Files && state.files.form.is_some() {
        add::draw_form(ctx, theme, state);
    }
}

/// The page: its heading row, its line, then the place shown (files/places.rs) in its panes: what
/// is shared on the server (the three panes, panes.rs, or the Gallery, every item as a tile,
/// [`draw_gallery`]), the server's shared files, the admins' section, the app's own folder or Your
/// folders. The places are at the top of the left pane whichever is shown.
pub(crate) fn draw_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let no_server = server_here(state).is_empty();
    // Only an admin has the Admin place; anyone else on it sees what is shared, whatever the state
    // says (an admin who lost the role, or a place left set by Server Settings' "Open them").
    let place = places::shown_place(state.files.place, places::admin_here(state));
    let on_list = place == Place::Shared;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(heading(state)).size(theme.font_size_title).color(theme.text_primary()));
        // The list's actions belong to what is shared; the other places have their own.
        if on_list {
            ui.add_space(theme.spacing_md);
            let signed_in = signed_in_here(state);
            if signed_in && widgets::Button::primary("Add to Files").tooltip("Upload a picture, video, model or file and show it here.").show(ui, theme) {
                add::begin_add(state);
            }
            if signed_in && widgets::Button::secondary("Suggest a category").tooltip("Ask this server's admins for a new category.").show(ui, theme) {
                state.files.suggest = Some((String::new(), String::new()));
            }
            if widgets::Button::ghost("Refresh").show(ui, theme) {
                reload(state);
            }
            if !no_server {
                ui.add_space(theme.spacing_md);
                panes::draw_switch(ui, theme, state);
            }
        }
        ui.add_space(theme.spacing_md);
        how::toggle(ui, theme, state);
    });
    // The page's line and each place's, folded until the person opens them (files/how.rs).
    how::panel(ui, theme, state);
    ui.add_space(theme.spacing_sm);

    // The places that need a server say so without one; Your folders and the HumanityOS folder need
    // none (Add to Files says why it cannot go without one).
    let needs_server = matches!(place, Place::Shared | Place::Admin);
    match place {
        _ if no_server && needs_server => panes::draw_two(ui, theme, state, |ui, theme, _state| {
            ui.label(RichText::new("Connect to a server first (Chat page) to see its Files page.").size(theme.font_size_small).color(theme.warning()));
        }, |ui, theme, _state| {
            viewer::nothing_chosen(ui, theme, "Connect to a server first (Chat page) to see what is shared on it.");
        }),
        Place::Admin => panes::draw_two(ui, theme, state, |_ui, _theme, _state| {}, |ui, theme, state| {
            egui::ScrollArea::vertical().id_salt("media_page").auto_shrink([false, false]).show(ui, |ui| admin::draw_admin(ui, theme, state));
        }),
        Place::Shared if state.settings.files_layout.gallery => panes::draw_two(ui, theme, state, |_ui, _theme, _state| {}, |ui, theme, state| {
            egui::ScrollArea::vertical().id_salt("media_page").auto_shrink([false, false]).show(ui, |ui| draw_gallery(ui, theme, state));
        }),
        Place::Shared => panes::draw(ui, theme, state),
        Place::SharedFiles => panes::draw_three(ui, theme, state, shared_files::draw_list, shared_files::draw_file, shared_files::draw_details),
        Place::AppFolder => panes::draw_three(ui, theme, state, app_folder::draw_tree, app_folder::draw_editor, app_folder::draw_details),
        Place::YourFolders => {
            widgets::thumbnails::next_frame();
            panes::draw_two(ui, theme, state, |ui, theme, state| your_folders::draw_side(ui, theme, state), |ui, theme, state| {
                egui::ScrollArea::vertical().id_salt("media_page").auto_shrink([false, false]).show(ui, |ui| your_folders::draw_open(ui, theme, state));
            })
        }
    }
}

/// The Gallery (the page as it was before the panes, kept as the other view): the status, the
/// uploads, the featured shelf, the filters and every item as a tile with its buttons, then More and
/// the page's foot. A press on a tile shows that item in the panes.
pub(crate) fn draw_gallery(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !state.files.status.is_empty() {
        ui.label(RichText::new(&state.files.status).size(theme.font_size_small).color(theme.text_secondary()));
    }
    if state.files.answered && !state.files.list.members_view {
        ui.label(RichText::new(SIGN_IN_LINE).size(theme.font_size_small).color(theme.warning()));
    }
    uploading::draw_list(ui, theme, state);
    waiting::draw(ui, theme, state);
    ui.add_space(theme.spacing_sm);

    let featured = state.files.featured.clone();
    if !featured.is_empty() {
        // "Featured", or "Featured in Fire flow arts" when it is a category's shelf (3.9).
        widgets::subsection_label(ui, theme, &filters::shelf_heading(state));
        egui::ScrollArea::horizontal().id_salt("media_featured").show(ui, |ui| {
            ui.horizontal(|ui| {
                for item in &featured {
                    tiles::draw_tile(ui, theme, state, item);
                }
            });
        });
        ui.add_space(theme.spacing_md);
    }
    filters::draw_filters(ui, theme, state);
    ui.add_space(theme.spacing_sm);

    let items = state.files.list.items.clone();
    if items.is_empty() {
        let line = if !state.files.answered {
            "Loading..."
        } else if state.files.filters != Filters::default() {
            "Nothing matches these filters."
        } else {
            EMPTY_LINE
        };
        ui.label(RichText::new(line).size(theme.font_size_body).color(theme.text_muted()));
    } else {
        tiles::draw_grid(ui, theme, state, &items);
    }
    draw_more(ui, theme, state);
    // The page's foot (2026-10-13, takedown.rs): "Report a copyright problem" for anyone, signed in
    // or not, and the public takedown log, as the website's Files page has them.
    takedown::draw_foot(ui, theme, state);
}

/// Under the list (the panes' and the Gallery's): More while there is a next page, and the strip
/// whose coming into view asks for it.
pub(crate) fn draw_more(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if state.files.list.next_cursor.is_none() {
        return;
    }
    ui.add_space(theme.spacing_sm);
    let next_on_its_way = state.files.jobs.iter().any(|(j, _)| matches!(j, Job::List { append: true, .. }));
    // While a fresh first page is on its way (the filters changed), More is not offered: that
    // page brings its own cursor.
    if next_on_its_way {
        ui.label(RichText::new("Loading more...").size(theme.font_size_small).color(theme.text_muted()));
    } else if !page_on_its_way(state)
        && widgets::Button::secondary("More").tooltip("The next page. It also comes by itself as the end of the list scrolls into view.").show(ui, theme) {
        // Pressed: asked whatever happened to the last one (a failure waits for this press).
        state.files.more_paused = false;
        more(state);
    }
    // The end of the list (3.9): a strip one point high under the list and More. When the
    // scroll area shows it, the next page is asked for, once (`more` asks nothing while a
    // page is on its way); a short page that shows its end at once fills itself the same way.
    let (sentinel, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(1.0), 1.0), egui::Sense::hover());
    if ui.is_rect_visible(sentinel) {
        more_when_seen(state);
    }
}

#[cfg(test)]
#[path = "files/page_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "files/uploads_tests.rs"]
mod uploads_tests;

#[cfg(test)]
#[path = "files/relay_round3_tests.rs"]
mod relay_round3_tests;

#[cfg(test)]
#[path = "files/round4_tests.rs"]
mod round4_tests;

#[cfg(test)]
#[path = "files/round5_list_tests.rs"]
mod round5_list_tests;

#[cfg(test)]
#[path = "files/panes_tests.rs"]
mod panes_tests;
