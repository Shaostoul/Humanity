// Child of chat/public_files.rs (#[path]): files posted into a public channel, drawn headless with
// the network replaced (2026-10-17, the "video attach" bug). Nothing here reaches a server: the
// one address the app could try is a closed loopback port, and each file's size is put in place
// before the card is drawn, so it is never asked. Each test was seen failing once on purpose,
// recorded at the test.

use super::*;

/// The server every test message came through: a loopback port nothing listens on.
const SERVER: &str = "http://127.0.0.1:9";

/// Every piece of text one headless frame drew.
fn texts_of(out: &egui::FullOutput) -> Vec<String> {
    fn walk(shape: &egui::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => texts.push(t.galley.job.text.clone()),
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

/// One frame of the card for `card`, and the text it drew.
fn card_frame(ctx: &egui::Context, cache: &ImageCache, card: &Card) -> Vec<String> {
    let theme = crate::gui::theme::load_theme();
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            draw_card(ui, &theme, cache, card, theme.bg_primary());
        });
    });
    texts_of(&out)
}

/// A FILE IS SHOWN BY THE NAME IT WAS SENT WITH, AS ON THE WEBSITE (the shared cases in
/// scripts/tests/fixtures/file-card-names.json, which the website's test runs too): the server's time
/// prefix comes off, a query and a fragment are not part of it, and `%20` reads as a space again.
/// Seen red 2026-10-17 with the prefix left on: "1760000000000_WH40K_Marriah_Carey.mp4".
#[test]
fn a_file_is_shown_by_the_name_it_was_sent_with() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/file-card-names.json");
    let cases: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let cases = cases["cases"].as_array().unwrap();
    assert!(cases.len() >= 6);
    for c in cases {
        let (url, name) = (c["address"].as_str().unwrap(), c["name"].as_str().unwrap());
        assert_eq!(shown_name(url), name, "{url}");
    }
}

/// A CARD KNOWS WHERE ITS FILE SITS: one on the message's own server loads (its size is asked, View
/// in 3D is offered for a model), one on another website names that website and offers no 3D view,
/// and a video either plays here (Play here: WebM always, MP4 where the computer's own decoders
/// are, 2026-10-18) or carries the line about the computer's own player (with the patent reason
/// for MP4 where they are not). Seen red 2026-10-17 with `other_site` always None: "a file on
/// another website names it"; and 2026-10-18 with MP4 never offered: "an MP4 plays here".
#[test]
fn a_card_knows_where_its_file_sits_and_what_it_is() {
    let own = Card::of("/uploads/1760000000000_part.glb", SERVER);
    assert_eq!(own.url, format!("{SERVER}/uploads/1760000000000_part.glb"));
    assert_eq!((own.name.as_str(), own.other_site.as_deref()), ("part.glb", None));
    assert!(own.viewable_in_3d(), "a model on the message's own server gets View in 3D");
    let theirs = Card::of("https://files.example.com/part.glb", SERVER);
    assert_eq!(theirs.other_site.as_deref(), Some("files.example.com"), "a file on another website names it");
    assert!(!theirs.viewable_in_3d(), "and is not fetched for a 3D view");
    let mp4 = Card::of("/uploads/1_clip.mp4", SERVER);
    if plays_mp4_here() {
        assert!(mp4.is_video() && mp4.plays_here() && mp4.video_line().is_none(), "an MP4 plays here");
    } else {
        assert!(mp4.is_video() && mp4.video_line().is_some_and(|l| l.contains(VIDEO_ELSEWHERE) && l.contains("patents")));
    }
    let webm = Card::of("/uploads/1_clip.webm", SERVER);
    assert!(webm.plays_here() && webm.video_line().is_none(), "a WebM plays here");
    let theirs_mp4 = Card::of("https://files.example.com/clip.mp4", SERVER);
    assert!(!theirs_mp4.plays_here(), "a video on another website is never fetched before Download");
    assert_eq!(Card::of("/uploads/1_a.pdf", SERVER).video_line(), None);
}

/// THE CARD SAYS WHAT A RISKY FILE IS: an archive's card carries the archive's word of caution by
/// its name; a video's carries none; once a "video" is saved and its bytes are a program, the card
/// calls it out as disguised. Seen red 2026-10-17 with the caution line left out of the card:
/// "an archive's card says so".
#[test]
fn the_card_says_what_a_risky_file_is() {
    let ctx = egui::Context::default();
    let mut cache = ImageCache::new();
    let zip = Card::of("/uploads/1_photos.zip", SERVER);
    cache.set_size_for_test(&zip.url, Some(2 * 1024 * 1024));
    let texts = card_frame(&ctx, &cache, &zip).join("\n");
    assert!(texts.contains("photos.zip") && texts.contains("2.0 MB") && texts.contains("Download"), "name, size and Download: {texts}");
    assert!(texts.contains("This is an archive (.zip)"), "an archive's card says so: {texts}");

    let clip = Card::of("/uploads/1_clip.mp4", SERVER);
    let texts = card_frame(&ctx, &cache, &clip).join("\n");
    assert!(!texts.contains('\u{26a0}'), "a video by its name: no word of caution: {texts}");

    let dir = crate::test_temp::dir("public_file_disguised");
    let saved = dir.join("clip.mp4");
    std::fs::write(&saved, [0x4d, 0x5a, 0x90, 0x00, 0x03, 0x00]).unwrap();
    cache.set_saved_for_test(&clip.key(), saved.clone());
    let texts = card_frame(&ctx, &cache, &clip).join("\n");
    assert!(texts.contains("named like a video (.mp4), but what is inside is a program"), "a disguised file is called out once saved: {texts}");
    assert!(texts.contains(&format!("Saved to {}", saved.display())) && texts.contains("Show in folder"), "{texts}");
}

/// A FILE ON ANOTHER WEBSITE IS NOT CONTACTED BEFORE A CLICK: its size is never asked (`draw` asks
/// only for the message's own server), and the card says that saving it shows that website this
/// device's address. The own server's file is asked (a HEAD to the closed loopback port, which
/// fails at once). Seen red 2026-10-17 with the size asked for every card: "nothing is asked of
/// another website".
#[test]
fn a_file_on_another_website_is_not_contacted_before_a_click() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut cache = ImageCache::new();
    let mut viewer = None;
    let mut video = None;
    let theirs = Card::of("https://files.example.com/notes.pdf", SERVER);
    let own = Card::of("/uploads/1_notes.pdf", SERVER);
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            draw(ui, &theme, &mut cache, &mut viewer, &mut video, &theirs, theme.bg_primary());
            draw(ui, &theme, &mut cache, &mut viewer, &mut video, &own, theme.bg_primary());
        });
    });
    let texts = texts_of(&out).join("
");
    assert!(texts.contains("on files.example.com") && texts.contains("Saving this shows files.example.com your network address."), "{texts}");
    assert!(!cache.size_asked(&theirs.url), "nothing is asked of another website");
    assert!(cache.size_asked(&own.url), "the message's own server is asked for its file's size");
}

/// DOWNLOAD SAVES UNDER THE NAME IT WAS SENT WITH, AND A FAILED ONE LEAVES NOTHING: the file is
/// written whole under its shown name, a second one beside it as "(2)", and a fetch that fails
/// leaves no file and no part file. Seen red 2026-10-17 with the part file finished under the
/// address's last part: "WH40K_Marriah_Carey.mp4" was saved as "1760000000000_WH40K_Marriah_Carey.mp4".
#[test]
fn download_saves_under_the_name_it_was_sent_with_and_a_failed_one_leaves_nothing() {
    let dir = crate::test_temp::dir("public_file_save");
    let card = Card::of("/uploads/1760000000000_WH40K_Marriah_Carey.mp4", SERVER);
    let bytes = b"\x1a\x45\xdf\xa3 a video's bytes".to_vec();
    let fetch = |want: &[u8]| {
        let want = want.to_vec();
        move |url: &str, max: u64, file: &mut std::fs::File| {
            assert!(url.ends_with("WH40K_Marriah_Carey.mp4") && max == MAX_PUBLIC_FILE);
            std::io::Write::write_all(file, &want).map_err(|e| e.to_string())?;
            Ok(want.len() as u64)
        }
    };
    let first = save_public(&card.url, &card.name, &dir, fetch(&bytes)).unwrap();
    assert_eq!(first.file_name().unwrap().to_string_lossy(), "WH40K_Marriah_Carey.mp4");
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    let second = save_public(&card.url, &card.name, &dir, fetch(&bytes)).unwrap();
    assert_eq!(second.file_name().unwrap().to_string_lossy(), "WH40K_Marriah_Carey (2).mp4");
    let failed = save_public(&card.url, &card.name, &dir, |_: &str, _: u64, _: &mut std::fs::File| Err("the line went quiet".to_string()));
    assert!(failed.is_err());
    let left: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left.len(), 2, "a failed Download leaves nothing: {left:?}");
}

/// THE CHAT PAGE SHOWS A POSTED VIDEO AS A CARD, NOT A BARE PATH (the bug as a member met it): a
/// message holding an `/uploads/` video shows the video's name, Download and the player line, its
/// words keep the rest of the message, and the address itself is not drawn. Seen red 2026-10-17
/// with the cards left out of the page: "the video's name is shown" (only the bare path was).
#[test]
fn the_chat_page_shows_a_posted_video_as_a_card_not_a_bare_path() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    state.chat_messages.push(ChatMessage {
        sender_name: "Ann".into(),
        sender_key: "ann0".into(),
        content: "Found this one /uploads/1760000000000_WH40K_Marriah_Carey.mp4".into(),
        timestamp: "09:20".into(),
        timestamp_ms: 1_791_481_200_000,
        channel: "general".into(),
        server: SERVER.into(),
        ..Default::default()
    });
    let url = format!("{SERVER}/uploads/1760000000000_WH40K_Marriah_Carey.mp4");
    state.image_cache.set_size_for_test(&url, Some(1_121_676));
    let input = || egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))), ..Default::default() };
    let mut texts = String::new();
    for _ in 0..3 {
        let out = ctx.run(input(), |ctx| crate::gui::pages::chat::draw(ctx, &theme, &mut state));
        texts = texts_of(&out).join("\n");
    }
    assert!(texts.contains("WH40K_Marriah_Carey.mp4"), "the video's name is shown: {texts}");
    assert!(texts.contains("1.1 MB") && texts.contains("Download"), "{texts}");
    if plays_mp4_here() {
        assert!(texts.contains(crate::gui::widgets::video_view::PLAY_HERE) && !texts.contains(MP4_WHY), "Play here, not the patent line: {texts}");
    } else {
        assert!(texts.contains(VIDEO_ELSEWHERE), "{texts}");
    }
    assert!(texts.contains("Found this one"), "the message's own words stay: {texts}");
    assert!(!texts.contains("/uploads/"), "the bare address is not drawn: {texts}");
}

/// PLAY HERE PLAYS A VIDEO POSTED IN CHAT IN ITS OWN WINDOW (2026-10-18; it had said to watch it in
/// another player): the window fetches the video (here a copy of the H.264 fixture handed over in
/// place of the network), plays it with Pause and Start again, and Close takes it away with its
/// temporary copy. Seen red 2026-10-18 with the window not polling its view: "the video plays in its
/// window" (it stayed at "Fetching the video..."; that run also showed its Close reading "Close the
/// 3D view", the model window's words, now its own).
#[cfg(windows)]
#[test]
fn play_here_plays_a_video_posted_in_chat_in_its_own_window() {
    if !plays_mp4_here() {
        println!("skipped: this copy of Windows has no Media Foundation");
        return;
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/media/moving-box-h264-aac.mp4");
    let dir = crate::test_temp::dir("chat_play_here");
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join("clip.video");
    std::fs::copy(&fixture, &copy).unwrap();
    let mut view = crate::gui::widgets::video_view::VideoView::default();
    let handed = copy.clone();
    view.start(move || Ok(handed));
    let mut video = Some(ChatVideo { name: "clip.mp4".into(), view });
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let input = || egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))), ..Default::default() };
    let started = std::time::Instant::now();
    let mut texts = Vec::new();
    while started.elapsed() < std::time::Duration::from_secs(30) {
        let out = ctx.run(input(), |ctx| draw_video_window(ctx, &theme, &mut video));
        texts = texts_of(&out);
        if texts.iter().any(|t| t == "Pause") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(texts.iter().any(|t| t == "Pause") && texts.iter().any(|t| t == "Start again"), "the video plays in its window: {texts:?}");
    assert!(texts.iter().any(|t| t == "clip.mp4"), "the window is named for it: {texts:?}");
    assert!(texts.iter().any(|t| t == crate::gui::widgets::video_view::CLOSE), "its own Close: {texts:?}");
    video = None;
    assert!(video.is_none() && !copy.exists(), "Close takes the temporary copy with it");
}
