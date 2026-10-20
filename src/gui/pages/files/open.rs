//! Opening a Media item: the transfers behind it. A press on a row in the list (or a tile in the
//! Gallery) shows the item in the panes (2026-10-19: viewer.rs draws it to fit, item_view.rs its
//! details): the picture (the image viewer stays behind View full size), the video's Play here, a
//! sound's Play here and waveform, the words and every file with its own Download. This file is
//! what those buttons run: Play here fetches the video to a temporary file and plays it in the
//! app's own media player (`crate::media::VideoPlayer`, the one in-world screens use: WebM with
//! AV1 video, and on Windows an ordinary MP4 through the computer's own decoders,
//! `media/system.rs`; anything else says why it cannot and can still be downloaded), a sound the
//! same way through widgets/sound_view.rs ([`start_sound`]), each with its sound through the
//! app's mixer ([`sync_sound`], which the engine calls each frame); Download saves one file of the
//! item in the downloads folder. The item's transfers are read every frame whatever page is open
//! (media.rs `poll_upload`), so a song plays on while the person looks elsewhere.
//!
//! DOWNLOADS ARE COUNTED (round 5, docs/design/media-round-5-plan.md 3.5): every Download goes
//! through the relay's counting route, `/api/files/download/<id>?file=<n>` ([`download_origin`]),
//! never the file's `/uploads/` address, so the item's download count is the relay's own. A
//! members-only file's download asks for a ticket made for that route (`download: true`). Play
//! here is watching, not downloading: it fetches the item's own file the way it always did.
//!
//! FETCHES AND DOWNLOADS run on worker threads and stream to disk, so a large file never sits in
//! memory whole. A members-only item's file is fetched with a ticket (ticket.rs, the relay's R3-8,
//! round 3): the session's upload token goes once, in a header, to ask for it, and the transfer
//! itself carries no token at all. There is no limit on a whole transfer's time, only on a
//! connection that will not open or goes quiet ([`transfer_agent`]): a 2 GB video takes longer
//! than any fixed limit on a slow line.
//!
//! NOTHING HAS ITS REAL NAME UNTIL IT IS COMPLETE (round 3, the review's findings 6 and 7): a
//! transfer is written to a hidden part file (widgets/part_file.rs) and named only when the last
//! byte is in, by claiming a free name, never by checking one and then creating it. A failed or
//! stopped transfer leaves nothing; one the app's exit cuts off is deleted at the exit, or at the
//! next start.
//!
//! TEMPORARY FILES: Play here's copy lives in the system's temporary folder
//! (`HumanityOS-media`) under a name of its own for each fetch ([`play_name`]), and is deleted when
//! its page closes (`Drop for Opened`), when the app exits (engine/quit.rs calls
//! [`forget_temp_files`]), and, for whatever a run that did not close cleanly left, once at the
//! start of the next ([`sweep_temp_once`]). The app's player opens a file, so Play here fetches the
//! whole video before it plays.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use egui::RichText;

use super::api::{self, Item, ItemFile, ItemRef};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::part_file::Part;
use crate::gui::GuiState;

/// The most a fetch or a download takes (the Media budget's own ceiling is far below).
const MAX_FETCH: u64 = 64 * 1024 * 1024 * 1024;

/// The folder Play here's copies go in.
pub(crate) fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("HumanityOS-media")
}

/// The transfers of the item whose page is open: its video, playing here, and its downloads.
pub(crate) struct Opened {
    pub item: Item,
    /// Its video, fetched and played here (the shared widget, widgets/video_view.rs).
    pub video: widgets::video_view::VideoView,
    /// Its sound or song, fetched, played and drawn as a waveform here (the shared widget,
    /// widgets/sound_view.rs, 2026-10-19).
    pub sound: widgets::sound_view::SoundView,
    /// The downloads on their way, by the number of the file each is of (0, the item's own).
    saves: Vec<(i64, Receiver<Result<PathBuf, String>>)>,
    /// Where each finished Download put its file, or why it did not, by file number.
    pub saved: Vec<(i64, String)>,
    /// Where each finished Download put its file, by file number: its type and word of caution
    /// are read from its first bytes once it is here (net/file_risk.rs, 2026-10-17).
    pub saved_paths: Vec<(i64, PathBuf)>,
    /// A `.glb` or `.stl` file shown in 3D (View in 3D, docs/design/model-viewer.md).
    pub viewer: Option<crate::gui::model_view::Viewer>,
}

impl Opened {
    pub(crate) fn new(item: Item) -> Self {
        Opened { item, video: Default::default(), sound: Default::default(), saves: Vec::new(), saved: Vec::new(), saved_paths: Vec::new(), viewer: None }
    }

    /// Whether file `file` is being downloaded now.
    pub(crate) fn downloading(&self, file: i64) -> bool {
        self.saves.iter().any(|(f, _)| *f == file)
    }

    /// What the last Download of file `file` said ("Saved to ..." or "Not saved: ..."), if any.
    pub(crate) fn saved_words(&self, file: i64) -> Option<&str> {
        self.saved.iter().find(|(f, _)| *f == file).map(|(_, w)| w.as_str())
    }

    /// Where a Download saved file `file`, if one did.
    pub(crate) fn saved_path(&self, file: i64) -> Option<&Path> {
        self.saved_paths.iter().find(|(f, _)| *f == file).map(|(_, p)| p.as_path())
    }

    /// Whether something is still on its way (the page repaints until it is not).
    pub(crate) fn busy(&self) -> bool {
        self.video.fetching() || self.sound.fetching() || !self.saves.is_empty()
    }
}

/// Delete everything in the temporary folder, once per run (the first frame): what a run that did
/// not close cleanly left there.
pub(crate) fn sweep_temp_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if !DONE.swap(true, Ordering::SeqCst) {
        sweep(&temp_dir());
    }
}

/// Delete every file in `dir` (best effort: a file another program still holds stays).
pub(crate) fn sweep(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The app is exiting (engine/quit.rs): close the item's page, whose copy goes with it, clear the
/// folder, and delete every part file still being written (a download or a Save cut off by the
/// exit).
pub(crate) fn forget_temp_files(state: &mut GuiState) {
    state.files.open = None;
    sweep(&temp_dir());
    crate::gui::widgets::part_file::sweep_own();
}

/// The name of one Play here copy: the server and item it is from, then this process and a number
/// no other fetch in this run has had (the review's finding 7: one name per item let two fetches
/// of it write into the same file).
pub(crate) fn play_name(server: &str, id: i64) -> String {
    let h = blake3::hash(format!("{server}\n{id}").as_bytes());
    format!("{}-{}-{}.video", &h.to_hex()[..16], std::process::id(), super::next_id())
}

/// Where a transfer of one of an item's files comes from: a public address, or, for a
/// members-only item, a ticket asked for on the worker (ticket.rs) for file `file`, made for the
/// counted download route when `download`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Origin {
    Address(String),
    Ticket { base: String, token: String, item: ItemRef, file: i64, download: bool },
}

impl Origin {
    /// The address to fetch, asking for a ticket first when it needs one. Blocking.
    pub(crate) fn address(&self) -> Result<String, String> {
        match self {
            Origin::Address(url) => Ok(url.clone()),
            Origin::Ticket { base, token, item, file, download } => super::ticket::ask_for(base, token, item, *file, *download),
        }
    }
}

/// Where `item`'s own file is watched from (Play here) on the server at `base`, with this session's
/// upload `token`: its address, or a ticket to the members' file route. Not counted as a download.
pub(crate) fn origin_of(base: &str, item: &Item, token: Option<&str>) -> Result<Origin, String> {
    if !item.members_only() {
        return Ok(Origin::Address(api::file_url(base, item)));
    }
    let token = token.ok_or_else(|| super::SIGN_IN_LINE.to_string())?;
    Ok(Origin::Ticket { base: base.to_string(), token: token.to_string(), item: item.reference()?, file: 0, download: false })
}

/// Where Download takes file `file` of `item` from (round 5): always the counted download route,
/// `/api/files/download/<id>?file=<n>`, never `/uploads/`, so every download is counted; for a
/// members-only item through a ticket made for that route, the token never in an address.
pub(crate) fn download_origin(base: &str, item: &Item, file: i64, token: Option<&str>) -> Result<Origin, String> {
    if !item.members_only() {
        return Ok(Origin::Address(super::api_items::download_url(base, item, file)));
    }
    let token = token.ok_or_else(|| super::SIGN_IN_LINE.to_string())?;
    Ok(Origin::Ticket { base: base.to_string(), token: token.to_string(), item: item.reference()?, file, download: true })
}

/// A tile in the Gallery was clicked: the panes show it (2026-10-19; item_view.rs). A public
/// picture's full size is one press away there (View full size), in the app's image viewer.
pub(crate) fn open_item(state: &mut GuiState, item: &Item) {
    super::item_view::open(state, item);
    super::panes::set_gallery(state, false);
}

/// A client for whole files: no limit on a whole transfer's time (a big file on a slow line
/// takes as long as it takes), only on a connection that will not open or goes quiet for `idle`.
pub(crate) fn transfer_agent(idle: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(30)).timeout_read(idle).timeout_write(idle).build()
}

/// Write what `url` serves into a file named `name` in `dir`, streamed, refusing more than `max`
/// bytes, with `header` on the request when one is given. It is written to a hidden part file and
/// gets its name (`name`, or "name (2)" and so on when that is taken, claimed, never checked and
/// then made) only when the last byte is in: a failure leaves nothing, and nothing ever has the
/// real name while it is incomplete. Returns where it is. The words never hold a token or ticket.
pub(crate) fn fetch_to(agent: &ureq::Agent, url: &str, header: Option<(&str, &str)>, dir: &Path, name: &str, max: u64) -> Result<PathBuf, String> {
    let mut req = agent.get(url);
    if let Some((h, value)) = header {
        req = req.set(h, value);
    }
    let resp = req.call().map_err(|e| {
        let words = match e {
            ureq::Error::Status(code, r) => {
                let text = r.into_string().unwrap_or_default();
                if text.trim().is_empty() || text.trim_start().starts_with('<') { format!("The server refused it ({code}).") } else { text.trim().to_string() }
            }
            other => format!("The server could not be reached ({other})."),
        };
        crate::gui::widgets::image_cache::hide_token(&words)
    })?;
    let mut part = Part::create(dir)?;
    let mut reader = resp.into_reader().take(max + 1);
    let n = std::io::copy(&mut reader, part.file()).map_err(|e| crate::gui::widgets::image_cache::hide_token(&format!("The download stopped ({e}).")))?;
    part.file().flush().map_err(|e| format!("The file could not be written ({e})."))?;
    if n > max {
        return Err("The file is larger than this app fetches.".to_string());
    }
    part.finish(dir, name)
}

/// A file name safe to write: letters, digits, spaces, dots, dashes and underscores, at most 80
/// characters, never empty.
pub(crate) fn safe_name(name: &str) -> String {
    let cleaned: String = crate::net::file_risk::plain_name(name).chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_') { c } else { '_' }).collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    // Shortened before its ending, never through it (a review, 2026-10-17: a name past 80 letters
    // lost its ending, so its badge said "File" and the saved file had no type).
    let cleaned = if cleaned.chars().count() <= SAFE_NAME_MAX {
        cleaned
    } else {
        let lower = cleaned.to_ascii_lowercase();
        let ending = if lower.ends_with(".tar.gz") { Some("tar.gz".len()) } else { cleaned.rsplit_once('.').map(|(_, e)| e.chars().count()) };
        match ending.filter(|n| (1..=16).contains(n)) {
            Some(n) => {
                let ending: String = cleaned.chars().skip(cleaned.chars().count() - n).collect();
                let stem: String = cleaned.chars().take(SAFE_NAME_MAX - n - 1).collect();
                format!("{}.{ending}", stem.trim_end().trim_end_matches('.'))
            }
            None => cleaned.chars().take(SAFE_NAME_MAX).collect(),
        }
    };
    if cleaned.is_empty() {
        "media".to_string()
    } else {
        cleaned
    }
}

/// How many letters a saved file's name keeps at most.
const SAFE_NAME_MAX: usize = 80;

/// The name file `f` of `item` is SHOWN by (2026-10-17): the uploader's own name as given, else its
/// stored name, made plain (`file_risk::plain_name`: no direction marks, no control characters),
/// so people read the real name and its badge judges its real ending. Download saves it under
/// [`file_download_name`].
pub(crate) fn shown_file_name(item: &Item, f: &ItemFile) -> String {
    let (own, stored) = if f.file == 0 { (item.original_name.as_str(), item.stored.as_str()) } else { (f.original_name.as_str(), f.filename.as_str()) };
    let name = crate::net::file_risk::plain_name(if own.trim().is_empty() { stored } else { own });
    if name.is_empty() {
        file_download_name(item, f)
    } else {
        name
    }
}

/// The name Download saves `item` under: the uploader's own name for the file (the relay's A1,
/// so a members-only file keeps its extension), else its stored name, else its title and number
/// with the extension of its stored name when one is known.
pub(crate) fn download_name(item: &Item) -> String {
    if !item.original_name.trim().is_empty() {
        return safe_name(&item.original_name);
    }
    if !item.stored.trim().is_empty() {
        return safe_name(&item.stored);
    }
    format!("{}-{}", safe_name(&item.display_title()), item.id)
}

/// The name Download saves file `f` of `item` under: the item's own file as [`download_name`]
/// does; an added file or the cover by the uploader's own name for it, else its stored name, else
/// the item's title, number and the file's number.
pub(crate) fn file_download_name(item: &Item, f: &ItemFile) -> String {
    if f.file == 0 {
        return download_name(item);
    }
    if !f.original_name.trim().is_empty() {
        return safe_name(&f.original_name);
    }
    if !f.filename.trim().is_empty() {
        return safe_name(&f.filename);
    }
    format!("{}-{}-{}", safe_name(&item.display_title()), item.id, f.file)
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(all(test, windows))]
#[path = "open_sound_tests.rs"]
mod sound_tests;

/// The page's video gets its sound from the app's mixer (the widget's `sync_sound`); the engine
/// calls this each frame it has a mixer.
pub(crate) fn sync_sound(opened: &mut Opened, audio: &mut crate::audio::AudioManager) {
    opened.video.sync_sound(audio);
    opened.sound.sync_sound(audio);
}

/// Read what the transfers of `opened` finished, and the video's next frame. Every frame its page
/// is drawn.
pub(crate) fn poll(ctx: &egui::Context, opened: &mut Opened) {
    opened.video.poll(ctx);
    opened.sound.poll(ctx);
    let mut finished = Vec::new();
    opened.saves.retain(|(file, rx)| match rx.try_recv() {
        Ok(result) => {
            finished.push((*file, result));
            false
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => true,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            finished.push((*file, Err("The download stopped unexpectedly.".to_string())));
            false
        }
    });
    for (file, result) in finished {
        opened.saved_paths.retain(|(f, _)| *f != file);
        let words = match result {
            Ok(path) => {
                // What is inside, now that it is here: a file whose name says a video while its
                // bytes say a program is called out (net/file_risk.rs, item_view.rs draw_files).
                let words = format!("Saved to {}", path.display());
                opened.saved_paths.push((file, path));
                words
            }
            Err(e) => format!("Not saved: {e}"),
        };
        opened.saved.retain(|(f, _)| *f != file);
        opened.saved.push((file, words));
    }
    if opened.busy() {
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

/// The name of one Play here copy of a sound: as [`play_name`], ending as the file does (the
/// decoders read the ending as a hint).
pub(crate) fn sound_name(server: &str, item: &Item) -> String {
    let own = super::item_view::files_of(item)[0].clone();
    let ending = super::edit::stored_ending(&shown_file_name(item, &own));
    let base = play_name(server, item.id);
    let stem = base.trim_end_matches(".video");
    if ending.is_empty() || !ending.chars().all(|c| c.is_ascii_alphanumeric()) {
        format!("{stem}.sound")
    } else {
        format!("{stem}.{ending}")
    }
}

/// Play here on a sound: fetch the item's own file to a temporary copy on a worker, then play it
/// and read its waveform (widgets/sound_view.rs). Watching is not downloading: not counted.
pub(crate) fn start_sound(state: &GuiState, opened: &mut Opened) {
    let token = super::token_here(state);
    match origin_of(&super::base_of(state), &opened.item, token.as_deref()) {
        Ok(origin) => {
            let name = sound_name(&state.files.server, &opened.item);
            opened.sound.start(move || origin.address().and_then(|url| fetch_to(&transfer_agent(Duration::from_secs(60)), &url, None, &temp_dir(), &name, MAX_FETCH)));
        }
        Err(words) => opened.sound.problem = words,
    }
}

/// Play here was pressed: fetch the item's own file to a temporary copy on a worker, then play it.
pub(crate) fn start_play(state: &GuiState, opened: &mut Opened) {
    let token = super::token_here(state);
    match origin_of(&super::base_of(state), &opened.item, token.as_deref()) {
        Ok(origin) => {
            let name = play_name(&state.files.server, opened.item.id);
            opened.video.start(move || origin.address().and_then(|url| fetch_to(&transfer_agent(Duration::from_secs(60)), &url, None, &temp_dir(), &name, MAX_FETCH)));
        }
        Err(words) => opened.video.problem = words,
    }
}

/// Download file `f` of the open item into the downloads folder, on a worker, through the counted
/// download route ([`download_origin`]).
pub(crate) fn start_download(state: &GuiState, opened: &mut Opened, f: &ItemFile) {
    let token = super::token_here(state);
    opened.saved.retain(|(file, _)| *file != f.file);
    match download_origin(&super::base_of(state), &opened.item, f.file, token.as_deref()) {
        Ok(origin) => {
            let (dir, name) = (crate::gui::widgets::image_cache::default_downloads_dir(), file_download_name(&opened.item, f));
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(origin.address().and_then(|url| fetch_to(&transfer_agent(Duration::from_secs(60)), &url, None, &dir, &name, MAX_FETCH)));
            });
            opened.saves.push((f.file, rx));
        }
        Err(words) => opened.saved.push((f.file, format!("Not saved: {words}"))),
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
