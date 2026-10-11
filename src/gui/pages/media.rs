//! The Media page (increment 2 of docs/design/media-showcase.md, 2026-10-11): the pictures,
//! videos, 3D models and files people on this server chose to show, in a grid. The web mirrors
//! it (web/pages/media.html); the relay half is src/relay/media.rs.
//!
//! What it has, top to bottom: a strip of featured items when there are any; the filters
//! (category, kind, person, My picks, a search over titles); the grid, newest first, with More
//! for the next page; each tile's actions (Pick, Remove, Feature, Report). Add to Media (a file
//! from this computer) and Put on Media (a file already in Shared Files, from the Files page)
//! open the same short form. Suggest a category sends the admins a name and a note.
//!
//! WHO SEES WHAT is the relay's to decide: a signed list from a member gets members-only items
//! too, anyone else the public ones, and on a members-only server nothing. Someone not signed in
//! to this server is told "Sign in to this server to see its members' media." A members-only
//! item's file is fetched through the members' file route with this session's upload token.
//!
//! Pictures follow the chat's rules (gui/pages/chat/attach_view.rs): the protected setup's rule
//! first, then a click for someone who is not a friend; a labelled item (Spoiler, Flashing
//! lights, Graphic) shows its label instead of its picture until clicked, and the protected
//! setup hides labelled items altogether.
//!
//! NOTHING RUNS ON THE FRAME BUT DRAWING: every request goes to a worker thread
//! ([`spawn`]) and its answer is read on a later frame ([`poll`]), the way the Files and Watch
//! pages do theirs. The admins' half (the Media section of Server Settings,
//! server_settings/media.rs) uses the same jobs, so its answers land whichever page is open.

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
/// The grid's tiles: what each shows, and its actions.
mod tiles;
/// The Add to Media, Put on Media and Suggest a category forms.
mod add;
/// An opened item: a video playing, a model's or file's card with Download.
mod open;

pub(crate) use add::put_on_media;
use api::{Filters, Item, ListAnswer, Request};

/// What a request was for, so its answer lands in the right place when it comes back.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Job {
    /// A page of the list; `generation` is the filters' generation it was asked under, so an
    /// answer for filters since changed is dropped.
    List { append: bool, generation: u64 },
    /// The person's own picks, to know which tiles say Unpick.
    MyPicks,
    /// Adding up the person's own items, page by page, for their share.
    MyUse { used: i64, pages: u32 },
    /// A publish (after an upload when `uploaded`).
    Publish { uploaded: bool },
    Remove { id: Option<i64>, from_admin: bool },
    Pick { id: i64, on: bool },
    Feature { id: i64, on: bool },
    Suggest,
    /// The admins' section: the categories (a signed list read), saving them, the suggestions,
    /// a decision, and the usage.
    AdminCategories,
    SaveCategories,
    Suggestions,
    Decide,
    Usage,
}

/// The admins' half (Server Settings > ADMIN > Media), kept with the page's state.
#[derive(Default)]
pub struct MediaAdmin {
    /// Asked for this server's categories, suggestions and usage.
    pub requested: bool,
    /// The admin's working copy of the category list, and whether it differs from the server's.
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

/// The Media page's state, kept in `GuiState::media`. It belongs to one server: when the
/// server changes, all of it is forgotten ([`sync_server`]).
#[derive(Default)]
pub struct MediaState {
    /// The server (normalised address) this state is for.
    pub server: String,
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
    pub jobs: Vec<(Job, Receiver<Result<Value, String>>)>,
    /// Items this person has picked.
    pub picked: HashSet<i64>,
    /// How much of the Media space this person's items take (their share's use), once added up.
    pub my_use: Option<i64>,
    /// Adding it up failed: the server checks the share itself then.
    pub my_use_failed: bool,
    /// Items this session put on Media from this computer, and from a file already shared, so
    /// Remove knows whether the file goes with it.
    pub uploaded_here: HashSet<i64>,
    pub put_here: HashSet<i64>,
    /// A Remove waiting for its confirmation.
    pub confirm_remove: Option<Item>,
    /// The Add to Media or Put on Media form, while open.
    pub form: Option<add::Form>,
    /// The file picker of Add to Media, while open.
    pub picker: Option<widgets::file_browser::FilePickerState>,
    /// The Suggest a category form: (name, note), while open.
    pub suggest: Option<(String, String)>,
    /// An opened video, model or file.
    pub open: Option<open::Opened>,
    pub admin: MediaAdmin,
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

/// Whether this person is an admin of this server (its member list says).
pub(crate) fn is_admin(state: &GuiState) -> bool {
    let me = &state.profile_public_key;
    !me.is_empty() && state.chat_users.iter().any(|u| &u.public_key == me && matches!(u.role.as_str(), "admin" | "owner"))
}

/// Forget everything when the server changed; returns true when it did.
pub(crate) fn sync_server(state: &mut GuiState) -> bool {
    let here = crate::gui::pages::chat::norm_server_url(&state.server_url);
    if state.media.server == here {
        return false;
    }
    state.media = MediaState { server: here, ..Default::default() };
    true
}

/// Run `req` on a worker thread; its answer is read by [`poll`].
pub(crate) fn spawn(state: &mut GuiState, job: Job, req: Request) {
    let base = base_of(state);
    log::info!("Media: {} {}", if req.body.is_some() { "POST" } else { "GET" }, req.path.split('?').next().unwrap_or(""));
    spawn_with(state, job, move || api::send(&base, &req));
}

/// Run `work` on a worker thread as `job`.
pub(crate) fn spawn_with(state: &mut GuiState, job: Job, work: impl FnOnce() -> Result<Value, String> + Send + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    state.media.jobs.push((job, rx));
}

/// Read every answer that has come back. Called by the Media page and by the Media section of
/// Server Settings each frame.
pub(crate) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let mut done = Vec::new();
    state.media.jobs.retain(|(job, rx)| match rx.try_recv() {
        Ok(result) => {
            done.push((job.clone(), result));
            false
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => true,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            done.push((job.clone(), Err("The request stopped unexpectedly.".to_string())));
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

/// Put an answer where it belongs.
pub(crate) fn on_done(state: &mut GuiState, job: Job, result: Result<Value, String>) {
    match (job, result) {
        (Job::List { append, generation }, Ok(v)) => {
            if generation == state.media.generation {
                apply_list(&mut state.media, api::parse_list(&v), append);
            }
        }
        (Job::List { generation, .. }, Err(e)) => {
            if generation == state.media.generation {
                state.media.answered = true;
                state.media.status = e;
            }
        }
        (Job::MyPicks, Ok(v)) => {
            state.media.picked.extend(api::parse_list(&v).items.iter().map(|i| i.id));
        }
        (Job::MyUse { used, pages }, Ok(v)) => {
            let answer = api::parse_list(&v);
            let used = used + answer.items.iter().map(|i| i.size_bytes.max(0)).sum::<i64>();
            match answer.next_before {
                // A person with more than 2,000 items is counted from the first 2,000.
                Some(before) if pages < 20 => add_up_my_use_from(state, used, pages + 1, Some(before)),
                _ => state.media.my_use = Some(used),
            }
        }
        (Job::MyUse { .. }, Err(e)) => {
            log::warn!("Media: the share's use could not be added up: {e}");
            state.media.my_use_failed = true;
        }
        (Job::Publish { .. }, Ok(v)) => {
            let item = v.get("item").and_then(Item::from_json);
            let source_picked = state.media.form.as_ref().map(|f| f.is_picked());
            state.media.form = None;
            if let Some(item) = item {
                match source_picked {
                    Some(true) => state.media.uploaded_here.insert(item.id),
                    _ => state.media.put_here.insert(item.id),
                };
                state.media.my_use = state.media.my_use.map(|u| u + item.size_bytes.max(0));
                state.media.status = format!("\"{}\" is on the Media page now.", item.display_title());
            }
            reload(state);
        }
        (Job::Publish { uploaded }, Err(e)) => {
            if let Some(f) = state.media.form.as_mut() {
                f.sending = false;
                f.problem = if uploaded { format!("Not added: {e}") } else { e };
            } else {
                state.media.status = e;
            }
        }
        (Job::Remove { id, from_admin }, Ok(_)) => {
            if let Some(id) = id {
                let size = state.media.list.items.iter().find(|i| i.id == id).map(|i| i.size_bytes.max(0)).unwrap_or(0);
                let mine = state.media.list.items.iter().any(|i| i.id == id && i.uploader_key == state.profile_public_key);
                state.media.list.items.retain(|i| i.id != id);
                if mine {
                    state.media.my_use = state.media.my_use.map(|u| (u - size).max(0));
                }
            }
            if from_admin {
                state.media.admin.status = "Removed.".to_string();
                refresh_usage(state);
            } else {
                state.media.status = "Removed from Media.".to_string();
            }
        }
        (Job::Remove { from_admin, .. }, Err(e)) => {
            if from_admin {
                state.media.admin.status = e;
            } else {
                state.media.status = e;
            }
        }
        (Job::Pick { id, on }, Ok(v)) => {
            if on {
                state.media.picked.insert(id);
            } else {
                state.media.picked.remove(&id);
            }
            if let (Some(item), Some(n)) = (state.media.list.items.iter_mut().find(|i| i.id == id), v.get("picks").and_then(Value::as_i64)) {
                item.picks = n;
            }
        }
        (Job::Feature { id, on }, Ok(_)) => {
            if let Some(item) = state.media.list.items.iter_mut().find(|i| i.id == id) {
                item.featured = on;
            }
        }
        (Job::Pick { .. } | Job::Feature { .. }, Err(e)) => state.media.status = e,
        (Job::Suggest, Ok(_)) => {
            state.media.suggest = None;
            state.media.status = SUGGESTION_SENT.to_string();
        }
        (Job::Suggest, Err(e)) => state.media.status = e,
        (Job::AdminCategories, Ok(v)) => {
            let answer = api::parse_list(&v);
            state.media.admin.cats = answer
                .categories
                .iter()
                .map(|c| api::CategoryEdit { id: Some(c.id), name: c.name.clone(), description: c.description.clone() })
                .collect();
            state.media.admin.cats_dirty = false;
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
            state.media.admin.status = e;
        }
        (Job::MyPicks, Err(e)) => log::warn!("Media: the picks could not be read: {e}"),
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

/// Ask for the first page again (the filters changed, or something was added or removed).
pub(crate) fn reload(state: &mut GuiState) {
    if state.server_url.trim().is_empty() {
        return;
    }
    state.media.generation += 1;
    state.media.requested = true;
    let req = api::list_request(&state.media.filters, None, api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.media.generation;
    spawn(state, Job::List { append: false, generation }, req);
    if let Some(me) = Some(state.profile_public_key.clone()).filter(|k| !k.is_empty() && signed_in_here(state)) {
        let picks = Filters { my_picks: true, ..Default::default() };
        spawn(state, Job::MyPicks, api::list_request(&picks, None, 100, &me, None));
    }
}

/// Ask for the next page.
fn more(state: &mut GuiState) {
    let Some(before) = state.media.list.next_before else { return };
    let req = api::list_request(&state.media.filters, Some(before), api::PAGE, &state.profile_public_key, signer_here(state).as_ref());
    let generation = state.media.generation;
    spawn(state, Job::List { append: true, generation }, req);
}

/// Start adding up this person's own items for their share (Add to Media's line).
pub(crate) fn add_up_my_use(state: &mut GuiState) {
    if state.media.my_use.is_some() || state.media.jobs.iter().any(|(j, _)| matches!(j, Job::MyUse { .. })) {
        return;
    }
    state.media.my_use_failed = false;
    add_up_my_use_from(state, 0, 1, None);
}

fn add_up_my_use_from(state: &mut GuiState, used: i64, pages: u32, before: Option<i64>) {
    let Some(signer) = signer_here(state) else {
        state.media.my_use_failed = true;
        return;
    };
    let mine = Filters { by: Some(signer.key.clone()), ..Default::default() };
    let req = api::list_request(&mine, before, 100, &signer.key, Some(&signer));
    spawn(state, Job::MyUse { used, pages }, req);
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
    sync_server(state);
    poll(ctx, state);
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
        if signed_in && widgets::Button::primary("Add to Media").tooltip("Upload a picture, video, model or file and show it here.").show(ui, theme) {
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
    ui.add_space(theme.spacing_sm);

    let featured: Vec<Item> = state.media.list.items.iter().filter(|i| i.featured).cloned().collect();
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
    ui.add_space(theme.spacing_sm);

    let items = state.media.list.items.clone();
    if items.is_empty() {
        let line = if !state.media.answered {
            "Loading..."
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
