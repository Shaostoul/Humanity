// Child of chat/inline_video.rs (#[path]): videos posted in chat, played inside their message
// (2026-11-06), drawn headless on the real chat page with the network replaced. The one address the
// app could try is a closed loopback port; each video's size is put in place before it is drawn, and
// a fetch copies a fixture from tests/fixtures/media instead (`ChatVideos::fetch_for_test`). The
// ffmpeg setting points at a file that is not ffmpeg, so no conversion can stand in for the player.
// Each test was seen failing once on purpose; the break and what it said are recorded at the test.

use super::*;
use crate::gui::pages::chat::public_files;
use crate::gui::theme::Theme;
use crate::gui::{ChatMessage, GuiState};

/// The server every test message came through: a loopback port nothing listens on.
const SERVER: &str = "http://127.0.0.1:9";

/// Every piece of text one headless frame drew, with where it was drawn.
pub(crate) fn texts_at(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn walk(shape: &egui::Shape, texts: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => texts.push((t.galley.job.text.clone(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        walk(&clipped.shape, &mut texts);
    }
    texts
}

pub(crate) fn texts_of(out: &egui::FullOutput) -> Vec<String> {
    texts_at(out).into_iter().map(|(t, _)| t).collect()
}

/// Every picture (a texture other than the font's) one frame painted, with where.
pub(crate) fn pictures_of(out: &egui::FullOutput) -> Vec<(egui::TextureId, egui::Rect)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, found: &mut Vec<(egui::TextureId, egui::Rect)>) {
        match shape {
            egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default() => found.push((m.texture_id, m.calc_bounds().intersect(clip))),
            egui::Shape::Rect(r) if r.fill_texture_id() != egui::TextureId::default() => found.push((r.fill_texture_id(), r.rect.intersect(clip))),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &out.shapes {
        walk(&clipped.shape, clipped.clip_rect, &mut found);
    }
    found
}

pub(crate) fn media_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/media").join(name)
}

/// A fetch that copies `fixture` into `dir` (a new copy each time, as each fetch makes its own).
pub(crate) fn fetch_copies(fixture: std::path::PathBuf, dir: std::path::PathBuf) -> FetchForTest {
    let n = std::sync::atomic::AtomicUsize::new(0);
    std::sync::Arc::new(move |_url: &str| {
        let copy = dir.join(format!("copy-{}.video", n.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::copy(&fixture, &copy).map_err(|e| e.to_string())?;
        Ok(copy)
    })
}

/// The real chat page in #global, its posts by a verified member (below Verified a line's files are
/// plain words since 2026-11-02), on a `w` x `h` screen.
pub(crate) struct Chat {
    pub(crate) ctx: egui::Context,
    pub(crate) theme: Theme,
    pub(crate) state: GuiState,
    pub(crate) screen: egui::Vec2,
    /// The test's folder (the copies, the stand-in ffmpeg), deleted with the guard.
    pub(crate) dir: humanity_core::test_temp::TempPath,
}

impl Chat {
    /// Each of `posts` is a message's words; each `/uploads/` address in them is given a size.
    pub(crate) fn new(test: &str, posts: &[&str], w: f32, h: f32) -> Chat {
        let dir = humanity_core::test_temp::dir(test);
        let fake = dir.join(crate::media::ffmpeg::FFMPEG_EXE);
        std::fs::write(&fake, b"not ffmpeg").unwrap(); // a conversion cannot rescue a test
        let mut state = GuiState::default();
        state.chat_active_channel = "global".into();
        state.active_page = crate::gui::GuiPage::Chat; // the dock (rung 3) reads which page is open
        state.settings.ffmpeg_path = fake.display().to_string();
        state.chat_users.push(crate::gui::ChatUser { name: "Ann".into(), public_key: "ann0".into(), role: "verified".into(), status: String::new() });
        // A video's poster picture (rung 2) is asked of no server: none is there, as for a video
        // posted before poster pictures, unless a test hands one over.
        state.chat_video.poster_fetch_for_test = Some(std::sync::Arc::new(|_url: &str| Err("not found (404)".to_string())));
        let mut chat = Chat { ctx: egui::Context::default(), theme: crate::gui::theme::load_theme(), state, screen: egui::vec2(w, h), dir };
        for p in posts {
            chat.post(p);
        }
        chat
    }

    pub(crate) fn post(&mut self, words: &str) {
        let i = self.state.chat_messages.len() as u64;
        for raw in public_files::files_in(words) {
            self.state.image_cache.set_size_for_test(&public_files::Card::of(&raw, SERVER).url, Some(1_121_676));
        }
        self.state.chat_messages.push(ChatMessage {
            sender_name: "Ann".into(),
            sender_key: "ann0".into(),
            content: words.into(),
            timestamp: "09:20".into(),
            timestamp_ms: 1_791_481_200_000 + i * 60_000,
            channel: "global".into(),
            server: SERVER.into(),
            ..Default::default()
        });
    }

    /// The address a video's card plays from.
    pub(crate) fn url(raw: &str) -> String {
        public_files::Card::of(raw, SERVER).url
    }

    /// One frame of the app as the engine draws it: the page, then the chat videos' dock (rung 3), the
    /// chat's video window and the end of the frame for the chat's videos (`draw_video_window`,
    /// called every frame).
    pub(crate) fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.screen)), events, ..Default::default() };
        let (theme, state) = (&self.theme, &mut self.state);
        self.ctx.run(input, |ctx| {
            crate::gui::pages::chat::draw(ctx, theme, state);
            crate::gui::pages::chat::draw_video_dock(ctx, theme, state);
            draw_video_window(ctx, theme, &mut state.chat_video);
        })
    }

    /// One frame on a page that is not the Chat page, as the engine draws it: a stand-in page (one
    /// button at its bottom right corner), then the dock (which follows the person there, rung 3),
    /// the video window and the end of the frame.
    pub(crate) fn frame_page(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.screen)), events, ..Default::default() };
        let (theme, state) = (&self.theme, &mut self.state);
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                    let _ = ui.button("A page's own button");
                });
            });
            crate::gui::pages::chat::draw_video_dock(ctx, theme, state);
            draw_video_window(ctx, theme, &mut state.chat_video);
        })
    }

    /// Press at `pos`: one frame to put the pointer there, one to press and let go.
    pub(crate) fn click(&mut self, pos: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        let m = egui::Modifiers::default();
        self.frame(vec![
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m },
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m },
        ])
    }

    /// Press the picture of the video at `url` (where it was last drawn).
    pub(crate) fn press_picture(&mut self, url: &str) -> egui::FullOutput {
        let at = self.state.chat_video.pictures.get(url).unwrap_or_else(|| panic!("the picture of {url} was drawn")).center();
        self.click(at)
    }

    /// Frames (10 ms apart) until `done` holds, at most 30 s; the last frame's output and whether it held.
    pub(crate) fn frames_until(&mut self, done: impl Fn(&GuiState, &[String]) -> bool) -> (egui::FullOutput, bool) {
        let started = std::time::Instant::now();
        loop {
            let out = self.frame(Vec::new());
            let held = done(&self.state, &texts_of(&out));
            if held || started.elapsed() > std::time::Duration::from_secs(30) {
                return (out, held);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

pub(crate) const A: &str = "/uploads/1760000000000_sliding_bar.mp4";
pub(crate) const B: &str = "/uploads/1760000000001_second_clip.mp4";

/// A CHAT FULL OF VIDEOS COSTS NOTHING UNTIL ONE IS PLAYED, AND EACH ROW DRAWS ITS PLAYER (the
/// operator, 2026-11-04: "Can we make the videos show in chat and playable in chat?"): twelve video
/// rows each draw a picture box with the Play mark's words and the controls (Play, the time, Mute,
/// Full window), and nothing is fetched and no player made. Seen red 2026-11-06: with the card's old
/// Play here button planted back in place of the player, "each row draws its player" (the page drew
/// "Play here", no Mute and no Full window); with `draw_player` starting a fetch for every row drawn,
/// "the box says how to watch it" (every row said "Fetching the video..." instead).
#[test]
fn a_chat_full_of_videos_fetches_and_decodes_nothing_until_play() {
    let posts: Vec<String> = (0..12).map(|i| format!("clip {i} /uploads/17600000000{i:02}_clip{i}.mp4")).collect();
    let posts: Vec<&str> = posts.iter().map(|s| s.as_str()).collect();
    let mut chat = Chat::new("chat_video_nothing_until_play", &posts, 1280.0, 1400.0);
    let mut texts = Vec::new();
    for _ in 0..4 {
        texts = texts_of(&chat.frame(Vec::new()));
    }
    let count = |w: &str| texts.iter().filter(|t| t.as_str() == w).count();
    assert!(count(t("chat_video.play")) >= 2 && count(t("chat_video.full_window")) >= 2 && count(t("chat_video.mute")) >= 2, "each row draws its player: {texts:?}");
    assert!(texts.iter().any(|t| t == t_press_play()), "the box says how to watch it: {texts:?}");
    assert!(!texts.iter().any(|t| t == crate::gui::widgets::video_view::PLAY_HERE), "no Play here button: {texts:?}");
    assert_eq!(chat.state.chat_video.fetches, 0, "nothing is fetched before Play");
    assert_eq!(chat.state.chat_video.live_count(), 0, "no player is made before Play");
}

fn t_press_play() -> &'static str {
    t("chat_video.press_play")
}

/// PLAY STARTS IT; A SECOND PLAY PAUSES THE FIRST. Pressing the first video's picture fetches it (once)
/// and plays it in its row, its picture drawn there; pressing the second's plays the second and
/// pauses the first, which keeps its player (it is still on screen). Seen red 2026-11-06 with
/// `end_frame`'s pause of every video but the current one taken out: "the first is paused once the
/// second plays" (both played at once).
#[test]
fn play_starts_it_and_a_second_play_pauses_the_first() {
    let mut chat = Chat::new("chat_video_one_at_a_time", &[&format!("first {A}"), &format!("second {B}")], 1280.0, 1500.0);
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(media_fixture("sliding-bar-av1-opus.mp4"), chat.dir.to_path_buf()));
    let (a, b) = (Chat::url(A), Chat::url(B));
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    chat.press_picture(&a);
    let (out, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(A)) && s.chat_video.pictures.contains_key(&Chat::url(A)));
    assert!(held, "Play starts it: {:?}", texts_of(&out));
    let (_, held) = chat.frames_until(|s, t| s.chat_video.shows_picture(&Chat::url(A)) && t.iter().any(|t| t == "Pause"));
    assert!(held, "its first picture comes");
    let out = chat.frame(Vec::new());
    let texts = texts_of(&out);
    assert!(texts.iter().any(|x| x == t("chat_video.pause")), "its Pause shows in its row: {texts:?}");
    let picture = chat.state.chat_video.pictures[&a];
    assert!(pictures_of(&out).iter().any(|(_, r)| r.intersects(picture) && r.width() > 10.0), "its picture is drawn in its row");
    assert_eq!(chat.state.chat_video.fetches, 1, "fetched once, when Play was pressed");
    assert!(!chat.state.chat_video.window_open(), "it plays in the message, not a window");

    chat.press_picture(&b);
    let (out, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(B)));
    assert!(held, "the second plays: {:?}", texts_of(&out));
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    assert!(chat.state.chat_video.has_player(&a), "the first keeps its player while it is on screen");
    assert!(!chat.state.chat_video.is_playing(&a), "the first is paused once the second plays");
    assert_eq!(chat.state.chat_video.playing(), Some(b.as_str()), "only the second plays");
    assert_eq!(chat.state.chat_video.fetches, 2);
}

/// A PAUSED ROW SCROLLED OUT OF VIEW FREES ITS DECODER, AND THEN DRAWS ITS STILL WITHOUT ONE. A
/// video played and then paused has its row pushed off the screen by eighty newer messages (the chat
/// stays at its bottom): within a few frames its player is gone (no decoder, no decode thread) and
/// its temporary copy deleted, a still of its picture is kept, and nothing goes into the dock (only
/// a PLAYING video does, rung 3: video_dock_tests.rs). Drawn again (its card on its own), the row
/// shows that still and its controls, with its place, and still no player and no new fetch. Seen red
/// 2026-11-06 with `end_frame` dropping nothing: "its player goes once its row is off the screen" (it
/// played on, off screen); with `forget` keeping no still: "a still of its picture is kept". Since
/// rung 3, seen red with `dock_moves` docking a paused video too: "a paused video is not docked".
#[test]
fn a_paused_row_scrolled_out_of_view_frees_its_decoder_then_draws_its_still() {
    let mut chat = Chat::new("chat_video_scrolled_away", &[&format!("watch {A}")], 1280.0, 900.0);
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(media_fixture("sliding-bar-av1-opus.mp4"), chat.dir.to_path_buf()));
    let a = Chat::url(A);
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    chat.press_picture(&a);
    // Its first picture shown, so its still is a picture it showed. (Waited for, not a few frames
    // counted: on 2026-11-06, with nine tests decoding at once, five frames 20 ms apart came and
    // went before the first picture, and "a still of its picture is kept" failed in the batch and
    // passed alone.)
    let (_, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(A)) && s.chat_video.shows_picture(&Chat::url(A)));
    assert!(held, "it plays, its first picture shown");
    let copy = chat.state.chat_video.temp_copy(&a).expect("it plays from a temporary copy");
    assert!(copy.exists());
    // Paused (its picture pressed again), then scrolled away.
    chat.press_picture(&a);
    assert!(!chat.state.chat_video.is_playing(&a), "paused");
    for i in 0..80 {
        chat.post(&format!("a newer line, number {i}, that pushes the video up and out of view"));
    }
    for _ in 0..4 {
        chat.frame(Vec::new());
    }
    assert!(chat.state.chat_video.dock.is_none(), "a paused video is not docked");
    assert!(!chat.state.chat_video.has_player(&a), "its player goes once its row is off the screen");
    assert_eq!(chat.state.chat_video.live_count(), 0, "nothing decodes for rows not on screen");
    assert!(!copy.exists(), "its temporary copy goes with its player");
    let still = chat.state.chat_video.still_texture(&a).expect("a still of its picture is kept");

    // The row drawn again: its still and its controls, no player, no fetch.
    let theme = crate::gui::theme::load_theme();
    let card = public_files::Card::of(A, SERVER);
    let mut cache = crate::gui::widgets::image_cache::ImageCache::new();
    cache.set_size_for_test(&card.url, Some(1_121_676));
    let mut viewer = None;
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))), ..Default::default() };
    let videos = &mut chat.state.chat_video;
    let out = chat.ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            public_files::draw(ui, &theme, &mut cache, &mut viewer, videos, "", &card, theme.bg_primary());
        });
        videos.end_frame(ctx);
    });
    assert!(pictures_of(&out).iter().any(|(id, _)| *id == still), "the row draws its still");
    let texts = texts_of(&out);
    assert!(texts.iter().any(|x| x == t("chat_video.play")) && texts.iter().any(|x| x == t("chat_video.full_window")), "and its controls: {texts:?}");
    assert!(texts.iter().any(|t| t.contains(" of 0:0")), "with its place and length: {texts:?}");
    assert!(!texts.iter().any(|t| t == t_press_play()), "no 'press Play' over a still: {texts:?}");
    assert_eq!(chat.state.chat_video.live_count(), 0, "drawing the still decodes nothing");
    assert_eq!(chat.state.chat_video.fetches, 1, "and fetches nothing");
}

/// A VIDEO NOTHING HERE PLAYS SAYS SO IN ITS ROW, IN BUG-228'S PLAIN WORDS, WITH ITS DOWNLOAD. The
/// file fetched is not a video at all and the ffmpeg setting points at a file that is not ffmpeg, so
/// nothing can rescue it: the row says what the file is (by the name it was posted with) and what
/// will play it, beside its card's Download, and no window opens. Seen red 2026-11-06 with
/// `draw_player`'s words line taken out: "the row says what will play it" (the words were nowhere).
#[test]
fn a_video_nothing_here_plays_says_so_in_its_row() {
    let raw = "/uploads/1760000000003_not_a_film.mp4";
    let mut chat = Chat::new("chat_video_plain_words", &[&format!("look {raw}")], 1280.0, 900.0);
    let not_video = chat.dir.join("not-a-video.bin");
    std::fs::write(&not_video, b"these bytes are no film").unwrap();
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(not_video, chat.dir.to_path_buf()));
    let url = Chat::url(raw);
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    chat.press_picture(&url);
    let (out, held) = chat.frames_until(|_, t| t.iter().any(|t| t.contains("VLC")));
    let texts = texts_of(&out);
    assert!(held, "the row says what will play it: {texts:?}");
    let words = texts.iter().find(|t| t.contains("VLC")).unwrap();
    assert!(words.starts_with("not_a_film.mp4 is a kind of file this app does not read itself."), "what the file is, by its posted name: {words}");
    assert!(!words.contains("Ebml") && !words.contains("Expected"), "no reader's words: {words}");
    assert!(texts.iter().any(|t| t == "Download"), "its Download is there: {texts:?}");
    assert!(!chat.state.chat_video.window_open(), "in the row, not a window");
    // The words stay with the row once its player has gone (pushed off the screen and back).
    chat.state.chat_video.forget(&url, &chat.ctx.clone());
    let texts = texts_of(&chat.frame(Vec::new()));
    assert!(texts.iter().any(|t| t.contains("VLC")), "the words stay after its player goes: {texts:?}");
}

/// FULL WINDOW IS THE SAME PLAYER, BIGGER, AND CLOSING IT PUTS THE VIDEO BACK. Full window on a row
/// plays the video in a window of its own (fetched once), the row saying so; Close the window puts
/// it back in its row, still with its player. Seen red 2026-11-06 with the window's player not
/// counted as on screen (so never polled): "Full window plays it in a window" (the window stayed at
/// "Fetching the video...").
#[test]
fn full_window_is_the_same_player_bigger_and_closing_it_puts_the_video_back() {
    let mut chat = Chat::new("chat_video_full_window", &[&format!("big {A}")], 1600.0, 1000.0);
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(media_fixture("sliding-bar-av1-opus.mp4"), chat.dir.to_path_buf()));
    let a = Chat::url(A);
    let mut out = chat.frame(Vec::new());
    for _ in 0..2 {
        out = chat.frame(Vec::new());
    }
    let full = texts_at(&out).into_iter().find(|(x, _)| x == t("chat_video.full_window")).expect("Full window is drawn").1;
    chat.click(full.center());
    let (out, held) = chat.frames_until(|s, t| s.chat_video.is_playing(&Chat::url(A)) && t.iter().any(|t| t == "Close the window"));
    let texts = texts_of(&out);
    assert!(held && chat.state.chat_video.window_open(), "Full window plays it in a window: {texts:?}");
    assert!(texts.iter().any(|x| x == t("chat_video.in_window")), "the row says where it plays: {texts:?}");
    for _ in 0..4 {
        chat.frame(Vec::new());
    }
    assert!(chat.state.chat_video.has_player(&a), "the window keeps its video");
    assert_eq!(chat.state.chat_video.fetches, 1, "fetched once");
    let out = chat.frame(Vec::new());
    let close = texts_at(&out).into_iter().find(|(x, _)| x == t("chat_video.close_window")).expect("Close the window is drawn").1;
    chat.click(close.center());
    let out = chat.frame(Vec::new());
    assert!(!chat.state.chat_video.window_open(), "closed");
    assert!(chat.state.chat_video.has_player(&a), "the video is back in its row, its player kept");
    assert!(!texts_of(&out).iter().any(|x| x == t("chat_video.in_window")), "the row is its player again");
}

/// The wall screen of the operator's screens den, in the great room (data/machines/home.ron, `desk_monitor_2`,
/// `page:chat`), at 1280 x 720 pixels, drawing the chat page under its own context with its own set
/// of videos, as the engine runs it (gui/screen_surface.rs `ScreenCore::run`).
pub(crate) struct Wall {
    pub(crate) core: crate::gui::screen_surface::ScreenCore,
    theme: Theme,
}

impl Wall {
    const SIZE: (f32, f32) = (1280.0, 720.0);

    pub(crate) fn new(fetch: FetchForTest) -> Wall {
        let theme = crate::gui::theme::load_theme();
        let mut core = crate::gui::screen_surface::ScreenCore::new("desk_monitor_2", "page:chat", Self::SIZE.0 as u32, Self::SIZE.1 as u32, &theme);
        core.chat_videos().fetch_for_test = Some(fetch);
        Wall { core, theme }
    }

    pub(crate) fn frame(&mut self, state: &mut GuiState) -> egui::FullOutput {
        self.core.run(&mut self.theme, state)
    }

    /// Press at `pos` on the wall, the way the look ray does: the pointer there, then a press and a
    /// let-go.
    pub(crate) fn click(&mut self, state: &mut GuiState, pos: egui::Pos2) {
        let uv = (pos.x / Self::SIZE.0, pos.y / Self::SIZE.1);
        self.core.pointer_moved(uv);
        self.frame(state);
        self.core.button(uv, true);
        self.core.button(uv, false);
        self.frame(state);
    }

    /// Frames (10 ms apart) until `done` holds, at most 30 s; the last frame's texts and whether it held.
    pub(crate) fn frames_until(&mut self, state: &mut GuiState, done: impl Fn(&mut crate::gui::screen_surface::ScreenCore, &[String]) -> bool) -> (Vec<String>, bool) {
        let started = std::time::Instant::now();
        loop {
            let texts = texts_of(&self.frame(state));
            let held = done(&mut self.core, &texts);
            if held || started.elapsed() > std::time::Duration::from_secs(30) {
                return (texts, held);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

/// A WALL SCREEN'S CHAT PLAYS ITS VIDEOS IN THEIR ROWS, ITS FULL WINDOW OPENS ON THE WALL, AND IT
/// LEAVES NO COPY BEHIND (BUG-258, then 2026-11-06). The chat on the screens den's wall screen
/// draws each video row's player; pressing the picture fetches the video and plays it in its row on
/// the wall, polled there (so its fetch is taken and its temporary copy known). Full window, pressed
/// on the wall, opens the window ON THE WALL: the wall's own run draws it (the operator: "the whole
/// display would be dedicated to the whole page"), the app screen's own set stays untouched, and
/// Close the window, pressed on the wall, puts the video back in its row. When the screen goes (a
/// home reload drops its surfaces), its copy goes with it.
///
/// Seen red 2026-11-06 (BUG-258) with the wall's set built as the app screen's and its window drawn
/// only for the app screen: the row said "Playing in its own window." and was never polled again.
/// Seen red 2026-11-06 (the wall's own Full window) with `draw_screen_full_views` not drawing the
/// video window (only the frame's end): "the window opens on the wall" failed, no "Close the window"
/// among the wall's texts.
#[test]
fn a_wall_screens_chat_plays_in_its_rows_opens_full_window_on_the_wall_and_deletes_its_copy() {
    let mut chat = Chat::new("chat_video_wall_screen", &[&format!("on the wall {A}")], 1280.0, 720.0);
    let mut wall = Wall::new(fetch_copies(media_fixture("sliding-bar-av1-opus.mp4"), chat.dir.to_path_buf()));
    let a = Chat::url(A);
    let mut texts = Vec::new();
    for _ in 0..3 {
        texts = texts_of(&wall.frame(&mut chat.state));
    }
    assert!(texts.iter().any(|x| x == t("chat_video.play")) && texts.iter().any(|x| x == t("chat_video.mute")), "the row draws its player on the wall: {texts:?}");
    assert!(texts.iter().any(|x| x == t("chat_video.full_window")), "a wall screen offers Full window: {texts:?}");

    let at = wall.core.chat_videos().pictures.get(&a).expect("the picture is drawn on the wall").center();
    wall.click(&mut chat.state, at);
    let (texts, held) = wall.frames_until(&mut chat.state, |core, t| {
        let v = core.chat_videos();
        v.is_playing(&Chat::url(A)) && v.shows_picture(&Chat::url(A)) && t.iter().any(|x| x == "Pause")
    });
    assert!(held, "it plays in its row on the wall, its picture there and its Pause: {texts:?}");
    assert!(!wall.core.chat_videos().window_open(), "in its row until Full window is pressed");
    let copy = wall.core.chat_videos().temp_copy(&a).expect("its fetch was taken: the copy it plays from is known");
    assert!(copy.exists());

    // Full window, pressed on the wall: the window opens on the wall, drawn by the wall's own run.
    let full = texts_at(&wall.frame(&mut chat.state)).into_iter().find(|(x, _)| x == t("chat_video.full_window")).expect("Full window is drawn on the wall").1;
    wall.click(&mut chat.state, full.center());
    let shown = texts_at(&wall.frame(&mut chat.state));
    assert!(wall.core.chat_videos().window_open(), "Full window opened the wall's own window");
    let close = shown.iter().find(|(x, _)| x == t("chat_video.close_window")).map(|(_, r)| *r);
    assert!(close.is_some(), "the window opens on the wall, its Close there: {:?}", shown.iter().map(|(x, _)| x).collect::<Vec<_>>());
    assert!(!chat.state.chat_video.window_open() && chat.state.chat_video.live_count() == 0, "the app screen's own set is untouched");

    // Close the window, pressed on the wall: the video goes back to its row.
    wall.click(&mut chat.state, close.unwrap().center());
    wall.frame(&mut chat.state);
    assert!(!wall.core.chat_videos().window_open(), "Close the window, pressed on the wall, closes it");
    assert!(wall.core.chat_videos().has_player(&a), "the video is back in its row, its player kept");
    drop(wall);
    assert!(!copy.exists(), "the screen gone, its temporary copy goes");
}

/// A PICTURE OPENED ON A WALL SCREEN FILLS THAT SCREEN, NEVER THE APP'S (2026-11-06). The picture
/// viewer's address was one `GuiState` field shared with the app screen, so a picture pressed on a
/// wall screen opened its full-screen viewer over the app screen. Now it is the screen's own
/// (`ScreenTextures::image_viewer_url`, swapped in for its run): a page that opens it on the wall
/// leaves the app screen's viewer shut, the wall's run draws the viewer (its Close) over its own
/// page, it stays open on the wall's next run, and its Close, pressed on the wall, closes it there.
///
/// Seen red 2026-11-06 with the viewer's address left out of the swap: "the app screen's viewer
/// stays shut" failed, the address the wall's page set left on the app's `GuiState`.
#[test]
fn a_picture_opened_on_a_wall_screen_fills_that_screen_not_the_apps() {
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    let mut core = crate::gui::screen_surface::ScreenCore::new("desk_monitor_2", "page:chat", 1280, 720, &theme);
    let views = |ctx: &egui::Context, st: &mut GuiState| crate::gui::screen_surface::draw_screen_full_views(ctx, &theme, st);
    // A page on the wall opens a picture, the way a chat attachment's click does.
    core.run_with(&mut state, |ctx, st| {
        st.image_viewer_url = Some("https://example.invalid/wall.png".into());
        views(ctx, st);
    });
    assert!(state.image_viewer_url.is_none(), "the app screen's viewer stays shut");
    // Its next run: still open on the wall (the address is the wall's), its Close drawn there.
    let mut kept = false;
    let out = core.run_with(&mut state, |ctx, st| {
        kept = st.image_viewer_url.as_deref() == Some("https://example.invalid/wall.png");
        views(ctx, st);
    });
    assert!(kept, "the wall keeps its picture open");
    let close = texts_at(&out).into_iter().find(|(x, _)| x == "Close").map(|(_, r)| r.center());
    let close = close.unwrap_or_else(|| panic!("the viewer is drawn on the wall: {:?}", texts_of(&out)));
    // Its Close, pressed on the wall, closes it there.
    let uv = (close.x / 1280.0, close.y / 720.0);
    core.pointer_moved(uv);
    core.run_with(&mut state, views);
    core.button(uv, true);
    core.run_with(&mut state, views);
    core.button(uv, false);
    core.run_with(&mut state, views);
    let mut still_open = true;
    core.run_with(&mut state, |_, st| still_open = st.image_viewer_url.is_some());
    assert!(!still_open, "Close, pressed on the wall, closed the wall's picture");
    assert!(state.image_viewer_url.is_none());
}

/// A VIEW THAT GOES BEFORE ITS FINISHED FETCH WAS TAKEN STILL DELETES THE COPY (BUG-258): a wall
/// screen's chat not drawn again after Play (the person walked away) never polls its video, so the
/// fetch's worker hands over a copy nobody reads; dropping the view takes it and deletes it.
///
/// Seen red 2026-11-06 with `Drop for VideoView` as before (only `temp`, set by a poll, was
/// deleted): "the unread copy goes with the view".
#[test]
fn a_view_dropped_before_its_fetch_was_taken_deletes_the_copy() {
    let dir = humanity_core::test_temp::dir("chat_video_unread_fetch");
    let fake = dir.join(crate::media::ffmpeg::FFMPEG_EXE);
    std::fs::write(&fake, b"not ffmpeg").unwrap(); // no conversion: the copy is all there is
    let copy = dir.join("copy.video");
    let mut view = VideoView::default();
    let at = copy.clone();
    view.start(&fake.display().to_string(), "clip.mp4", move || {
        std::fs::write(&at, b"bytes of a fetched video").map_err(|e| e.to_string())?;
        Ok(at)
    });
    // Wait (never polling the view) until its worker has handed the copy over.
    let started = std::time::Instant::now();
    while !view.handed.load(std::sync::atomic::Ordering::SeqCst) && started.elapsed() < std::time::Duration::from_secs(30) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(view.handed.load(std::sync::atomic::Ordering::SeqCst), "the fetch handed its copy over");
    assert!(copy.exists(), "the fetch wrote its copy");
    assert!(view.temp.is_none(), "never polled, so the view never took it");
    drop(view);
    assert!(!copy.exists(), "the unread copy goes with the view");
}

/// FULL WINDOW KEEPS CLOSE THE WINDOW INSIDE IT, AND ITSELF ON THE SCREEN, AT EVERY WINDOW SIZE AND
/// FONT SIZE (BUG-258; BUG-251's rule for the 3D window). The window as the engine draws it
/// (`draw_video_window`), for a video with a long name that cannot play (BUG-228's long words under
/// its picture, the tallest the controls get), on screens of 1024x640 to 2560x1440 at Font Size
/// 100% to 200% (the screen in points is the pixels over the Font Size): the window stays on the
/// screen, Close the window lies inside it and inside what of it can be seen, and the picture's box
/// keeps a height.
///
/// Seen red 2026-11-06 with the window as it was (the picture sized to 0.72 of the screen's height,
/// the controls and Close under it, no `body_room`): "Close the window lies inside its window, on
/// the screen" at 10 of the 20: 1024x640 at 125% to 200%, 1280x720 at 125% to 200%, 1920x1080 at
/// 175% and 200%; e.g. "1280x720 at 200%: screen [[0.0 0.0] - [640.0 360.0]], window [[4.1 -52.6] -
/// [635.9 412.6]], button None" (the window past both edges, Close not drawn on the screen at all),
/// and "1920x1080 at 200%: ... window [[127.4 -19.4] - [832.6 559.4]], button [[134.4 523.9] -
/// [287.9 552.4]]" (Close below the screen's 540). With the height laid out from the bottom up, it
/// was red again at 1024x640 at 175% and 200%, sideways: "screen [[0.0 0.0] - [585.1 365.7]],
/// window [[-26.0 3.0] - [611.2 362.8]]" (the long name as the window's title widened it past both
/// edges), until the title was cut to the window's width.
#[test]
fn full_window_keeps_close_inside_it_at_every_size_and_font_size() {
    use crate::gui::model_view::tests::{inside, painted_button, screens_and_font_sizes};
    const URL: &str = "http://127.0.0.1:9/uploads/1760000000005_family_reunion_2026.mp4";
    const NAME: &str = "Family reunion at the lake, summer 2026, the whole afternoon.mp4";
    let words = "Family reunion at the lake, summer 2026, the whole afternoon.mp4 is an MP4 file with H.265 video and AAC sound, which this app cannot play as it is. This video is H.265. Windows plays it only with the HEVC Video Extensions from the Microsoft Store. It will play in another player, such as VLC.";
    let mut bad = Vec::new();
    for (px, percent) in screens_and_font_sizes() {
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        let mut videos = ChatVideos::default();
        let said = words.to_string();
        videos.fetch_for_test = Some(std::sync::Arc::new(move |_: &str| Err(said.clone())));
        videos.press_play(URL, NAME, "");
        videos.window = Some(URL.to_string());
        assert!(videos.wait_for_picture_or_words(URL, &ctx), "its words come");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, px / (percent / 100.0));
        let mut size = percent;
        let mut out = None;
        for _ in 0..4 {
            let mut input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            crate::gui::font_size::before_frame(&ctx, &mut input, &mut size);
            out = Some(ctx.run(input, |ctx| draw_video_window(ctx, &theme, &mut videos)));
        }
        let out = out.expect("four passes ran");
        let window = ctx.memory(|m| m.area_rect(egui::Id::new(WINDOW_ID))).expect("the window is drawn");
        let close = painted_button(&out, t("chat_video.close_window"));
        let picture = videos.pictures.get(URL).copied().unwrap_or(egui::Rect::NOTHING);
        let words_shown = texts_of(&out).iter().any(|x| x.contains("HEVC"));
        match close {
            Some((button, seen)) if inside(screen, window) && inside(window, button) && inside(seen, button) && picture.height() >= 1.0 && words_shown => {}
            _ => bad.push(format!("{}x{} at {percent}%: screen {screen:?}, window {window:?}, button {close:?}, picture {picture:?}, words shown {words_shown}", px.x, px.y)),
        }
        assert!(videos.window_open(), "the window stays open while it is drawn");
    }
    assert!(bad.is_empty(), "Close the window lies inside its window, on the screen:\n{}", bad.join("\n"));
}

/// WHAT ONE PLAYING VIDEO COSTS THE FRAME (the brief's measurement, docs/design/media-player.md):
/// the chat page with twenty video rows drawn, egui's own work and its tessellation, timed over
/// frames with nothing playing and then with one playing. The video is the AV1 fixture (320x180) or
/// `HUMANITY_CHAT_VIDEO_BENCH_FILE`. Run it in release:
/// `cargo test --release --features native --lib chat::inline_video::tests::bench -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement; run in release by hand"]
fn bench_one_playing_video_frame_cost() {
    let posts: Vec<String> = (0..20).map(|i| format!("clip {i} /uploads/17600000000{i:02}_clip{i}.mp4")).collect();
    let posts: Vec<&str> = posts.iter().map(|s| s.as_str()).collect();
    let mut chat = Chat::new("chat_video_bench", &posts, 2560.0, 1440.0);
    let file = std::env::var_os("HUMANITY_CHAT_VIDEO_BENCH_FILE").map(std::path::PathBuf::from).unwrap_or_else(|| media_fixture("sliding-bar-av1-opus.mp4"));
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(file.clone(), chat.dir.to_path_buf()));
    let time = |chat: &mut Chat, frames: usize| -> (f64, f64, usize) {
        let mut ms = Vec::with_capacity(frames);
        let mut uploads = 0;
        for _ in 0..frames {
            let started = std::time::Instant::now();
            let out = chat.frame(Vec::new());
            let _ = chat.ctx.tessellate(out.shapes, out.pixels_per_point);
            uploads += out.textures_delta.set.iter().filter(|(id, _)| matches!(id, egui::TextureId::Managed(n) if *n > 0)).count();
            ms.push(started.elapsed().as_secs_f64() * 1000.0);
            // A 120 Hz frame, so the video's own clock moves as it does on the operator's screen.
            std::thread::sleep(std::time::Duration::from_micros(8333u64.saturating_sub(started.elapsed().as_micros() as u64)));
        }
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (ms.iter().sum::<f64>() / ms.len() as f64, ms[ms.len() * 95 / 100], uploads)
    };
    for _ in 0..30 {
        chat.frame(Vec::new());
    }
    let (idle, idle95, _) = time(&mut chat, 240);
    let last = chat.state.chat_messages.last().map(|m| public_files::files_in(&m.content)[0].clone()).unwrap();
    let url = Chat::url(&last);
    chat.press_picture(&url);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&url));
    assert!(held, "the video plays");
    let (busy, busy95, uploads) = time(&mut chat, 240);
    let size = chat.state.chat_video.live.get(&url).and_then(|l| l.view.texture()).map(|t| t.size());
    println!("BENCH chat videos: {} ({size:?} px)", file.display());
    println!("BENCH nothing playing: mean {idle:.3} ms, p95 {idle95:.3} ms per frame");
    println!("BENCH one playing:     mean {busy:.3} ms, p95 {busy95:.3} ms per frame; {uploads} pictures uploaded in 240 frames");
    println!("BENCH one video costs the frame loop {:.3} ms (mean)", busy - idle);
    // The picture's own step on the frame loop, alone: a decoded frame made into an egui image, the
    // way VideoView::poll did it (unmultiplied) and does it (premultiplied: the same bytes for an
    // opaque picture), at the playing video's size.
    if let Some([w, h]) = size {
        let rgba = vec![200u8; w * h * 4];
        let each = |f: &dyn Fn() -> egui::ColorImage| {
            let started = std::time::Instant::now();
            for _ in 0..30 {
                std::hint::black_box(f());
            }
            started.elapsed().as_secs_f64() * 1000.0 / 30.0
        };
        let un = each(&|| egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba));
        let pre = each(&|| egui::ColorImage::from_rgba_premultiplied([w, h], &rgba));
        println!("BENCH one picture made into an image ({w}x{h}): unmultiplied {un:.3} ms, premultiplied {pre:.3} ms");
    }
}

/// THE START VOLUME APPLIES TO A NEW CHAT VIDEO (2026-11-06; Settings > Audio, Videos): with "a set
/// volume" of 5%, a video Play starts begins at 5%, and so does the next one whatever the last was
/// turned to; with "my last volume" a fresh start begins where the last volume bar was left (what the
/// row's bar does on a move: the chat volume and `start_volume::note_volume`). The fetch fails at
/// once (nothing to fetch here): the volume is set as Play starts the video. On this thread's own
/// choice, so no other test sees it. RED first: with neither `ChatVideos::start` nor the
/// `VideoView::start` under it asking the start volume, "a set 5%: the video starts at 5%" failed
/// (left: (1.0, 1.0), right: (0.05, 0.05)).
#[test]
fn a_new_chat_video_starts_at_the_chosen_start_volume() {
    use humanity_core::audio::start_volume::{self as sv, StartAt};
    sv::use_on_this_thread(StartAt::Set, 0.05, 0.8);
    let mut videos = ChatVideos::default();
    videos.fetch_for_test = Some(std::sync::Arc::new(|_: &str| Err("not fetched in this test".to_string())));
    let (a, b) = ("http://127.0.0.1:9/uploads/a.mp4", "http://127.0.0.1:9/uploads/b.mp4");
    videos.press_play(a, "a.mp4", "");
    assert_eq!((videos.volume, videos.live[a].view.volume), (0.05, 0.05), "a set 5%: the video starts at 5%");
    videos.volume = 0.6;
    sv::note_volume(0.6);
    videos.press_play(b, "b.mp4", "");
    assert_eq!((videos.volume, videos.live[b].view.volume), (0.05, 0.05), "the next video starts at 5% too, whatever the last was turned to");
    sv::set_choice(StartAt::Last, 0.05);
    videos.live.remove(a);
    videos.press_play(a, "a.mp4", "");
    assert_eq!((videos.volume, videos.live[a].view.volume), (0.6, 0.6), "my last volume: where the last bar was left");
}

// ── Rung 2: the poster picture (2026-11-06) ──

/// A poster picture as the posting app makes one (chat/video_poster.rs `encode_poster`): a 320 x 180
/// JPEG of one colour, handed over by every fetch, and the count of fetches.
fn poster_fetch(chat: &mut Chat) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let rgba: Vec<u8> = std::iter::repeat([200u8, 60, 160, 255]).take(320 * 180).flatten().collect();
    let jpeg = super::super::video_poster::encode_poster(&rgba, 320, 180).unwrap();
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = asked.clone();
    chat.state.chat_video.poster_fetch_for_test = Some(std::sync::Arc::new(move |_url: &str| {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(jpeg.clone())
    }));
    asked
}

/// THE DESKTOP ROW DRAWS THE POSTER PICTURE BEFORE PLAY AND FETCHES NOTHING ELSE (rung 2; the
/// operator, 2026-11-04: "Can we make the videos show in chat and playable in chat?"). A video row
/// on screen asks for its poster picture at the video's address with `.poster.jpg` after it, once
/// over many frames, and draws it in its picture box with the Play mark over it (no "press Play"
/// words over a picture); the video itself is not fetched and no player is made. Pressing Play then
/// fetches the video, and the poster picture stays in the box until the first frame comes.
///
/// Seen red 2026-11-06 with `draw_player`'s `want_poster` call taken out: "its poster picture
/// comes" (nothing was asked); with `paint_picture` leaving the poster picture out of its chain: "the
/// box draws the poster picture" (only the black box and its words).
#[test]
fn the_desktop_row_draws_the_poster_picture_before_play_and_fetches_nothing_else() {
    let mut chat = Chat::new("chat_video_poster_before_play", &[&format!("from the meetup {A}")], 1280.0, 900.0);
    let asked = poster_fetch(&mut chat);
    let a = Chat::url(A);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.poster_texture(&Chat::url(A)).is_some());
    assert!(held, "its poster picture comes");
    for _ in 0..10 {
        chat.frame(Vec::new());
    }
    let out = chat.frame(Vec::new());
    let poster = chat.state.chat_video.poster_texture(&a).unwrap();
    let picture = chat.state.chat_video.pictures[&a];
    assert!(pictures_of(&out).iter().any(|(id, r)| *id == poster && r.intersects(picture) && r.width() > 100.0), "the box draws the poster picture");
    let texts = texts_of(&out);
    assert!(!texts.iter().any(|t| t == t_press_play()), "no 'press Play' words over a picture: {texts:?}");
    assert!(texts.iter().any(|x| x == t("chat_video.play")), "its Play is there: {texts:?}");
    assert_eq!(chat.state.chat_video.poster_fetches, vec![format!("{a}.poster.jpg")], "asked once, at the video's address with .poster.jpg after it");
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(chat.state.chat_video.fetches, 0, "the video itself is not fetched");
    assert_eq!(chat.state.chat_video.live_count(), 0, "and no player is made");

    // Play: the video is fetched, and the poster picture stays until its first picture comes.
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(media_fixture("sliding-bar-av1-opus.mp4"), chat.dir.to_path_buf()));
    chat.press_picture(&a);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.shows_picture(&Chat::url(A)));
    assert!(held, "Play plays it");
    assert_eq!(chat.state.chat_video.fetches, 1);
    assert_eq!(chat.state.chat_video.poster_fetches.len(), 1, "the poster picture is not asked again");
}

/// A VIDEO POSTED BEFORE POSTER PICTURES KEEPS TODAY'S BLACK BOX WITH ITS PLAY MARK. Its poster
/// picture's address answers 404: the row draws no picture, says how to watch it, and does not ask
/// again frame after frame.
///
/// Seen red 2026-11-06 with `take_posters` dropping a refusal (the entry removed, not marked
/// missing, so the next frame asks again): "the 404 is taken as no poster picture".
#[test]
fn a_video_posted_before_poster_pictures_keeps_the_old_box() {
    let mut chat = Chat::new("chat_video_poster_old_box", &[&format!("an old one {A}")], 1280.0, 900.0);
    let a = Chat::url(A);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.poster_missing(&Chat::url(A)));
    assert!(held, "the 404 is taken as no poster picture");
    for _ in 0..12 {
        chat.frame(Vec::new());
    }
    let out = chat.frame(Vec::new());
    let picture = chat.state.chat_video.pictures[&a];
    assert!(!pictures_of(&out).iter().any(|(_, r)| r.intersects(picture)), "no picture in the box");
    assert!(texts_of(&out).iter().any(|t| t == t_press_play()), "the box says how to watch it");
    assert_eq!(chat.state.chat_video.poster_fetches.len(), 1, "asked once, not frame after frame / left: {} / right: 1", chat.state.chat_video.poster_fetches.len());
    assert_eq!(chat.state.chat_video.fetches, 0);
}

/// POSTER PICTURES ARE MEMORY ONLY, UNDER THE REUSE SWITCH. With reuse on, a row scrolled out of
/// view keeps its poster picture (drawn again with no new fetch), and no more than `MAX_POSTERS`
/// are kept however many rows have passed, the one seen longest ago going first. With the Dev
/// page's "turn off all reuse" (set for this one set of videos), the picture goes the moment its
/// row is off the screen and is asked for again when it comes back.
///
/// Seen red 2026-11-06 with `trim_posters` keeping everything: "with reuse off, a row off the screen
/// lets its poster picture go / left: 1 / right: 0".
#[test]
fn poster_pictures_are_memory_only_under_the_reuse_switch() {
    for reuse in [true, false] {
        let mut chat = Chat::new(&format!("chat_video_poster_reuse_{reuse}"), &[&format!("watch {A}")], 1280.0, 900.0);
        chat.state.chat_video.reuse_for_test = Some(reuse);
        poster_fetch(&mut chat);
        let a = Chat::url(A);
        let (_, held) = chat.frames_until(|s, _| s.chat_video.poster_texture(&Chat::url(A)).is_some());
        assert!(held, "its poster picture comes");
        for i in 0..80 {
            chat.post(&format!("a newer line, number {i}, that pushes the video up and out of view"));
        }
        for _ in 0..4 {
            chat.frame(Vec::new());
        }
        if reuse {
            assert_eq!(chat.state.chat_video.posters_kept(), 1, "with reuse on, a row off the screen keeps its poster picture");
        } else {
            assert_eq!(chat.state.chat_video.posters_kept(), 0, "with reuse off, a row off the screen lets its poster picture go / left: {} / right: 0", chat.state.chat_video.posters_kept());
        }
        // The row drawn again, on its own.
        let theme = crate::gui::theme::load_theme();
        let card = public_files::Card::of(A, SERVER);
        let mut cache = crate::gui::widgets::image_cache::ImageCache::new();
        cache.set_size_for_test(&card.url, Some(1_121_676));
        let mut viewer = None;
        for _ in 0..200 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))), ..Default::default() };
            let videos = &mut chat.state.chat_video;
            let _ = chat.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    public_files::draw(ui, &theme, &mut cache, &mut viewer, videos, "", &card, theme.bg_primary());
                });
                videos.end_frame(ctx);
            });
            if chat.state.chat_video.poster_texture(&a).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(chat.state.chat_video.poster_texture(&a).is_some(), "drawn again, it shows its poster picture");
        let asked = chat.state.chat_video.poster_fetches.len();
        assert_eq!(asked, if reuse { 1 } else { 2 }, "reuse {reuse}: asked again only with reuse off");
    }

    // However many rows pass, no more than MAX_POSTERS are kept; the newest stay.
    let mut videos = ChatVideos { reuse_for_test: Some(true), poster_fetch_for_test: Some(std::sync::Arc::new(|_url: &str| Err("not found (404)".to_string()))), ..Default::default() };
    let ctx = egui::Context::default();
    let urls: Vec<String> = (0..MAX_POSTERS + 12).map(|i| format!("{SERVER}/uploads/17600000{i:05}_clip.mp4")).collect();
    for url in &urls {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            videos.want_poster(url);
            videos.end_frame(ctx);
        });
    }
    assert_eq!(videos.posters_kept(), MAX_POSTERS, "no more than MAX_POSTERS are kept");
    assert!(videos.posters.contains_key(urls.last().unwrap()) && !videos.posters.contains_key(&urls[0]), "the one seen longest ago went first");
}

/// BELOW VERIFIED A VIDEO STAYS PLAIN WORDS, AND NO POSTER PICTURE IS ASKED FOR (chat access rung 1:
/// a member not yet Verified posts no attachments). The same row from a member: its address is words,
/// no player is drawn, and nothing is fetched, the poster picture included.
///
/// Seen red 2026-11-06 with chat/access.rs `shows_links` answering true for everyone (the rung-1 rule
/// bypassed): "no picture box drawn" (the player was drawn, and its poster picture asked for).
#[test]
fn below_verified_a_video_stays_words_and_no_poster_picture_is_asked_for() {
    let mut chat = Chat::new("chat_video_poster_unverified", &[&format!("look {A}")], 1280.0, 900.0);
    chat.state.chat_users[0].role = "member".into();
    poster_fetch(&mut chat);
    let mut texts = Vec::new();
    for _ in 0..6 {
        texts = texts_of(&chat.frame(Vec::new()));
    }
    assert!(chat.state.chat_video.pictures.is_empty(), "no picture box drawn");
    assert!(!texts.iter().any(|x| x == t("chat_video.full_window") || x == t("chat_video.mute")), "no player's controls: {texts:?}");
    assert_eq!(chat.state.chat_video.poster_fetches.len(), 0, "no poster picture is asked for / left: {} / right: 0", chat.state.chat_video.poster_fetches.len());
    assert_eq!(chat.state.chat_video.fetches, 0);
}
