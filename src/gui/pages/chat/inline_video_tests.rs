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
fn texts_at(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
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

fn texts_of(out: &egui::FullOutput) -> Vec<String> {
    texts_at(out).into_iter().map(|(t, _)| t).collect()
}

/// Every picture (a texture other than the font's) one frame painted, with where.
fn pictures_of(out: &egui::FullOutput) -> Vec<(egui::TextureId, egui::Rect)> {
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

fn media_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/media").join(name)
}

/// A fetch that copies `fixture` into `dir` (a new copy each time, as each fetch makes its own).
fn fetch_copies(fixture: std::path::PathBuf, dir: std::path::PathBuf) -> FetchForTest {
    let n = std::sync::atomic::AtomicUsize::new(0);
    std::sync::Arc::new(move |_url: &str| {
        let copy = dir.join(format!("copy-{}.video", n.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::copy(&fixture, &copy).map_err(|e| e.to_string())?;
        Ok(copy)
    })
}

/// The real chat page in #global, its posts by a verified member (below Verified a line's files are
/// plain words since 2026-11-02), on a `w` x `h` screen.
struct Chat {
    ctx: egui::Context,
    theme: Theme,
    state: GuiState,
    screen: egui::Vec2,
    /// The test's folder (the copies, the stand-in ffmpeg), deleted with the guard.
    dir: humanity_core::test_temp::TempPath,
}

impl Chat {
    /// Each of `posts` is a message's words; each `/uploads/` address in them is given a size.
    fn new(test: &str, posts: &[&str], w: f32, h: f32) -> Chat {
        let dir = humanity_core::test_temp::dir(test);
        let fake = dir.join(crate::media::transcode::FFMPEG_EXE);
        std::fs::write(&fake, b"not ffmpeg").unwrap(); // a conversion cannot rescue a test
        let mut state = GuiState::default();
        state.chat_active_channel = "global".into();
        state.settings.ffmpeg_path = fake.display().to_string();
        state.chat_users.push(crate::gui::ChatUser { name: "Ann".into(), public_key: "ann0".into(), role: "verified".into(), status: String::new() });
        let mut chat = Chat { ctx: egui::Context::default(), theme: crate::gui::theme::load_theme(), state, screen: egui::vec2(w, h), dir };
        for p in posts {
            chat.post(p);
        }
        chat
    }

    fn post(&mut self, words: &str) {
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
    fn url(raw: &str) -> String {
        public_files::Card::of(raw, SERVER).url
    }

    /// One frame of the app as the engine draws it: the page, then the chat's video window and the
    /// end of the frame for the chat's videos (`draw_video_window`, called every frame).
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.screen)), events, ..Default::default() };
        let (theme, state) = (&self.theme, &mut self.state);
        self.ctx.run(input, |ctx| {
            crate::gui::pages::chat::draw(ctx, theme, state);
            draw_video_window(ctx, theme, &mut state.chat_video);
        })
    }

    /// Press at `pos`: one frame to put the pointer there, one to press and let go.
    fn click(&mut self, pos: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        let m = egui::Modifiers::default();
        self.frame(vec![
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m },
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m },
        ])
    }

    /// Press the picture of the video at `url` (where it was last drawn).
    fn press_picture(&mut self, url: &str) -> egui::FullOutput {
        let at = self.state.chat_video.pictures.get(url).unwrap_or_else(|| panic!("the picture of {url} was drawn")).center();
        self.click(at)
    }

    /// Frames (10 ms apart) until `done` holds, at most 30 s; the last frame's output and whether it held.
    fn frames_until(&mut self, done: impl Fn(&GuiState, &[String]) -> bool) -> (egui::FullOutput, bool) {
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

const A: &str = "/uploads/1760000000000_sliding_bar.mp4";
const B: &str = "/uploads/1760000000001_second_clip.mp4";

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

/// A ROW SCROLLED OUT OF VIEW PAUSES AND FREES ITS DECODER, AND THEN DRAWS ITS STILL WITHOUT ONE. A
/// playing video's row is pushed off the screen by eighty newer messages (the chat stays at its
/// bottom): within a few frames its player is gone (no decoder, no decode thread) and its temporary
/// copy deleted, and a still of its picture is kept. Drawn again (its card on its own), the row shows
/// that still and its controls, with its place, and still no player and no new fetch. Seen red
/// 2026-11-06 with `end_frame` dropping nothing: "its player goes once its row is off the screen" (it
/// played on, off screen); with `forget` keeping no still: "a still of its picture is kept".
#[test]
fn a_row_scrolled_out_of_view_pauses_and_frees_its_decoder_then_draws_its_still() {
    let mut chat = Chat::new("chat_video_scrolled_away", &[&format!("watch {A}")], 1280.0, 900.0);
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(media_fixture("sliding-bar-av1-opus.mp4"), chat.dir.to_path_buf()));
    let a = Chat::url(A);
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    chat.press_picture(&a);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(A)));
    assert!(held, "it plays");
    // Let a few frames of it be shown, so its still is a frame it showed.
    for _ in 0..5 {
        chat.frame(Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let copy = chat.state.chat_video.temp_copy(&a).expect("it plays from a temporary copy");
    assert!(copy.exists());
    for i in 0..80 {
        chat.post(&format!("a newer line, number {i}, that pushes the video up and out of view"));
    }
    for _ in 0..4 {
        chat.frame(Vec::new());
    }
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
