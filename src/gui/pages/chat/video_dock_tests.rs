// Child of chat/video_dock.rs (#[path]): the chat videos' dock (rung 3, 2026-11-06), drawn headless on
// the real chat page with the network replaced, through inline_video.rs's test harness (`Chat`: the
// page, then the dock, then the Full window and the end of the frame, as the engine draws them). The
// operator: "When we scroll them off page can we add like an anchored picture-in-picture mode that's
// stuck to the side of the chat it's at? It could be like a collapsible tap that has minimize (keeps
// playing unless paused) and close (closes anchored section.)" Each test was seen failing once on
// purpose; the break and what it said are recorded at the test.

use super::*;
use crate::gui::pages::chat::inline_video::tests::{fetch_copies, media_fixture, pictures_of, texts_at, texts_of, Chat, Wall, A, B};

/// The fixture every video here plays (the AV1 one: 320 x 180, two seconds, with sound).
fn fixture() -> std::path::PathBuf {
    media_fixture("sliding-bar-av1-opus.mp4")
}

/// Where the words `w` were drawn last frame, if they were.
fn words_at(out: &egui::FullOutput, w: &str) -> Option<egui::Rect> {
    texts_at(out).into_iter().find(|(x, _)| x == w).map(|(_, r)| r)
}

/// The dock's Area as egui last placed it.
fn dock_rect(chat: &Chat) -> egui::Rect {
    chat.ctx.memory(|m| m.area_rect(egui::Id::new(AREA_ID))).expect("the dock was drawn")
}

/// A chat whose one video (A, under a second one, B) plays and whose row is then pushed off the
/// screen by eighty newer lines: frames until it is docked. Returns the chat and A's address.
fn docked(test: &str, w: f32, h: f32) -> (Chat, String) {
    let mut chat = Chat::new(test, &[&format!("watch {A}")], w, h);
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(fixture(), chat.dir.to_path_buf()));
    let a = Chat::url(A);
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    // Play, as its picture or its Play button does (a press is the inline tests' business; here the
    // screen may be too short at Font Size 200% for the picture's middle to be on it).
    let ffmpeg = chat.state.settings.ffmpeg_path.clone();
    chat.state.chat_video.press_play(&a, "sliding_bar.mp4", &ffmpeg);
    let (out, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(A)) && s.chat_video.shows_picture(&Chat::url(A)));
    assert!(held, "it plays in its row: {:?}", texts_of(&out));
    for i in 0..80 {
        chat.post(&format!("a newer line, number {i}, that pushes the video up and out of view"));
    }
    chat.state.chat_video.rewind_for_test(&a);
    let (out, held) = chat.frames_until(|s, _| s.chat_video.dock.is_some());
    assert!(held, "its row pushed off the screen, it goes into the dock: {:?} (where: {:?})", texts_of(&out), chat.state.chat_video.where_is(&a));
    // Drawn: open, or as the tab where the room is too small for a picture (the clip is two seconds
    // long, so it starts again before the dock's first frames are looked at).
    chat.state.chat_video.rewind_for_test(&a);
    let (out, held) = chat.frames_until(|_, texts| texts.iter().any(|x| x == t_go_to() || x == t("chat_video_dock.tab_playing")));
    assert!(held, "the dock is drawn, open or as its tab, its video playing: {:?} (where: {:?})", texts_of(&out), chat.state.chat_video.where_is(&a));
    (chat, a)
}

fn t_go_to() -> &'static str {
    t("chat_video_dock.go_to")
}

/// A PLAYING VIDEO SCROLLED OUT OF VIEW GOES INTO THE DOCK AND PLAYS ON: THE SAME PLAYER, NOTHING
/// FETCHED AGAIN. Its row pushed off the screen while it plays, the video is in the dock at the top
/// right of the message list: still playing, from the same temporary copy (one fetch, the first
/// copy), its clock carrying on (not started again), its picture drawn in the dock, with where it
/// came from, Minimize, Close, Play or Pause, the time, Mute and Go to message.
///
/// Seen red 2026-11-06 with `dock_moves` docking nothing (the code before rung 3): "its row pushed
/// off the screen, it goes into the dock" (the player was dropped and the video stopped).
#[test]
fn a_playing_video_scrolled_out_goes_into_the_dock_and_plays_on() {
    let (mut chat, a) = docked("chat_video_dock_takes", 1600.0, 1000.0);
    let copy = chat.state.chat_video.temp_copy(&a).expect("it plays from its temporary copy");
    assert!(copy.ends_with("copy-0.video") && copy.exists(), "the first fetch's copy: {copy:?}");
    let before = chat.state.chat_video.player(&a).map(|p| p.position_s()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(150));
    let out = chat.frame(Vec::new());
    assert!(chat.state.chat_video.is_playing(&a), "it plays on in the dock");
    assert_eq!(chat.state.chat_video.fetches, 1, "nothing fetched again");
    assert_eq!(chat.state.chat_video.temp_copy(&a), Some(copy), "the same player, from the same copy");
    let after = chat.state.chat_video.player(&a).map(|p| p.position_s()).unwrap();
    assert!(after > before, "its clock carried on, not started again: {before} then {after}");
    let texts = texts_of(&out);
    for w in [t("chat_video_dock.from_here").replace("{{channel}}", "#global").as_str(), t("chat_video_dock.minimize"), t("common.close"), t("chat_video.pause"), t("chat_video.mute"), t_go_to()] {
        assert!(texts.iter().any(|x| x == w), "the dock shows {w:?}: {texts:?}");
    }
    let dock = dock_rect(&chat);
    assert!(pictures_of(&out).iter().any(|(_, r)| dock.contains_rect(*r) && r.width() > 100.0), "its picture is drawn in the dock {dock:?}");
    let list = chat.state.chat_video.last_list_rect().expect("the chat page noted its list");
    assert!(list.contains_rect(dock), "inside the message list {list:?}: {dock:?}");
    assert!((dock.top() - list.top()) < 20.0 && (list.right() - dock.right()) < 20.0, "at its top right corner: list {list:?}, dock {dock:?}");
}

/// MINIMIZE KEEPS IT PLAYING AND SHOWS THE TAB; THE TAB OPENS IT AGAIN. Minimize folds the dock to
/// the slim tab ("Video playing"): no picture and no controls drawn, the video playing on (its clock
/// moving, its player polled without its picture made an image); pressing the tab opens the dock.
///
/// Seen red 2026-11-06 with `draw_video_dock` ignoring `minimized`: "minimized, the tab shows" (the
/// open dock was drawn on); with the tab drawn but its video not polled (`on_dock` left out of the
/// tab's branch): "it plays on, minimized" (its player was dropped at the end of the frame).
#[test]
fn minimize_keeps_it_playing_and_shows_the_tab() {
    let (mut chat, a) = docked("chat_video_dock_minimize", 1600.0, 1000.0);
    let out = chat.frame(Vec::new());
    let minimize = words_at(&out, t("chat_video_dock.minimize")).expect("Minimize is drawn");
    chat.click(minimize.center());
    chat.frame(Vec::new()); // the tab's first frame is egui's sizing pass, drawn unseen
    chat.state.chat_video.rewind_for_test(&a);
    let before = chat.state.chat_video.player(&a).map(|p| p.position_s()).unwrap_or(0.0);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let out = chat.frame(Vec::new());
    let texts = texts_of(&out);
    assert!(texts.iter().any(|x| x == t("chat_video_dock.tab_playing")), "minimized, the tab shows: {texts:?}");
    assert!(!texts.iter().any(|x| x == t_go_to()), "and not the dock: {texts:?}");
    assert!(chat.state.chat_video.is_playing(&a), "it plays on, minimized");
    let after = chat.state.chat_video.player(&a).map(|p| p.position_s()).unwrap_or(0.0);
    assert!(after > before, "its clock moves: {before} then {after}");
    let tab = chat.ctx.memory(|m| m.area_rect(egui::Id::new(TAB_ID))).expect("the tab was drawn");
    let list = chat.state.chat_video.last_list_rect().unwrap();
    assert!(list.contains_rect(tab) && (list.right() - tab.right()) < 20.0, "the tab is on the list's right edge: {tab:?} in {list:?}");
    chat.click(tab.center());
    let out = chat.frame(Vec::new());
    assert!(texts_of(&out).iter().any(|x| x == t_go_to()), "the tab pressed, the dock opens again");
}

/// CLOSE STOPS IT, CLOSES THE DOCK, AND THE ROW KEEPS ITS STILL. Close in the dock: the video is
/// paused and its player goes (its temporary copy with it), the dock is gone, and a still of its
/// picture and its place are kept for its row.
///
/// Seen red 2026-11-06 with `close_dock` only closing the dock: "its player goes" (it played on, out
/// of sight, in no row and no dock, until the end of the next frame dropped it).
#[test]
fn close_stops_it_and_the_row_keeps_its_still() {
    let (mut chat, a) = docked("chat_video_dock_close", 1600.0, 1000.0);
    let copy = chat.state.chat_video.temp_copy(&a).unwrap();
    let out = chat.frame(Vec::new());
    let close = words_at(&out, t("common.close")).expect("Close is drawn");
    chat.click(close.center());
    assert!(chat.state.chat_video.dock.is_none(), "the dock is closed");
    assert!(!chat.state.chat_video.has_player(&a), "its player goes");
    assert!(!copy.exists(), "and its temporary copy");
    assert!(chat.state.chat_video.still_texture(&a).is_some(), "its row keeps a still of its picture");
    let texts = texts_of(&chat.frame(Vec::new()));
    assert!(!texts.iter().any(|x| x == t_go_to() || x == t("chat_video_dock.tab_playing")), "nothing of the dock is drawn: {texts:?}");
}

/// SCROLLING THE ROW BACK INTO VIEW RETURNS THE VIDEO INLINE, THE SAME PLAYER; AND GO TO MESSAGE,
/// FROM ANOTHER PAGE, OPENS THE CHAT THERE AND BRINGS IT BACK. The newer lines gone (the row on
/// screen again), the dock closes and the video plays in its row from the same copy. Docked again and
/// the person on another page (the dock follows), Go to message opens the Chat page and scrolls the
/// row into view, where the video plays on in its row, nothing fetched again.
///
/// Seen red 2026-11-06 with `dock_moves` never giving a video back (the `row_seen` check taken out):
/// "its row back on screen, the dock closes"; with `go_to_message` opening the page but asking no
/// scroll (`go_to` not set): "Go to message scrolls its row into view and the video comes back".
/// The same words on the first run, before the message list stopped sticking to its bottom while a
/// Go to message waits (egui put the offset back at the end every frame, undoing the jump).
#[test]
fn scrolling_back_returns_it_inline_and_go_to_message_brings_it_back() {
    let (mut chat, a) = docked("chat_video_dock_back", 1600.0, 1000.0);
    let copy = chat.state.chat_video.temp_copy(&a).unwrap();
    // The row on screen again.
    chat.state.chat_messages.truncate(1);
    chat.state.chat_video.rewind_for_test(&a);
    let (out, held) = chat.frames_until(|s, _| s.chat_video.dock.is_none());
    assert!(held, "its row back on screen, the dock closes: {:?}", texts_of(&out));
    let out = chat.frame(Vec::new());
    assert!(chat.state.chat_video.is_playing(&a), "it plays on in its row");
    assert_eq!(chat.state.chat_video.temp_copy(&a), Some(copy.clone()), "the same player");
    let picture = chat.state.chat_video.pictures[&a];
    assert!(pictures_of(&out).iter().any(|(_, r)| r.intersects(picture) && r.width() > 100.0), "its picture is in its row again");
    assert!(!texts_of(&out).iter().any(|x| x == t_go_to()), "no dock");

    // Docked again, then another page: the dock follows the person there.
    for i in 0..80 {
        chat.post(&format!("another newer line, number {i}, that pushes the video away again"));
    }
    chat.state.chat_video.rewind_for_test(&a);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.dock.is_some());
    assert!(held, "docked again");
    chat.state.active_page = crate::gui::GuiPage::Library;
    let mut out = chat.frame_page(Vec::new());
    for _ in 0..3 {
        out = chat.frame_page(Vec::new());
    }
    assert!(texts_of(&out).iter().any(|x| x == t_go_to()), "the dock follows to another page: {:?}", texts_of(&out));
    assert!(chat.state.chat_video.is_playing(&a), "and plays on there");
    let go = words_at(&out, t_go_to()).unwrap();
    chat.frame_page(vec![egui::Event::PointerMoved(go.center())]);
    let m = egui::Modifiers::default();
    chat.frame_page(vec![
        egui::Event::PointerButton { pos: go.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: m },
        egui::Event::PointerButton { pos: go.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: m },
    ]);
    assert_eq!(chat.state.active_page, crate::gui::GuiPage::Chat, "Go to message opens the Chat page");
    chat.state.chat_video.rewind_for_test(&a);
    let (out, held) = chat.frames_until(|s, _| s.chat_video.dock.is_none() && s.chat_video.is_playing(&Chat::url(A)));
    assert!(held, "Go to message scrolls its row into view and the video comes back: {:?}", texts_of(&out));
    let out = chat.frame(Vec::new());
    let picture = chat.state.chat_video.pictures[&a];
    let list = chat.state.chat_video.last_list_rect().unwrap();
    assert!(list.intersects(picture), "its row is in view: {picture:?} in {list:?}");
    assert!(pictures_of(&out).iter().any(|(_, r)| r.intersects(picture) && r.width() > 100.0), "its picture is in its row");
    assert_eq!(chat.state.chat_video.fetches, 1, "nothing fetched again");
    assert_eq!(chat.state.chat_video.temp_copy(&a), Some(copy), "the same player");
}

/// STARTING ANOTHER VIDEO PLAYS IT INLINE, PAUSES THE DOCKED ONE AND CLOSES THE DOCK (the
/// orchestrator's proposal, the operator's "Great ideas", 2026-11-06). With A docked, B (on screen)
/// pressed: B plays in its row, the dock is gone, and A is paused; its player goes as a paused row's
/// does, its still kept.
///
/// Seen red 2026-11-06 with `press_play` leaving the dock alone: "the dock closes" (A stayed docked,
/// paused by the one-at-a-time rule, beside B playing in its row).
#[test]
fn starting_another_video_plays_it_inline_and_closes_the_dock() {
    let mut chat = Chat::new("chat_video_dock_another", &[&format!("watch {A}")], 1600.0, 1000.0);
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(fixture(), chat.dir.to_path_buf()));
    let (a, b) = (Chat::url(A), Chat::url(B));
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    chat.press_picture(&a);
    // Its first picture waited for, so its row has a still to keep (on 2026-11-06, in the full
    // suite, "A's row keeps its still" failed once and passed alone: no picture had come yet).
    let (_, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(A)) && s.chat_video.shows_picture(&Chat::url(A)));
    assert!(held, "A plays, its first picture shown");
    for i in 0..80 {
        chat.post(&format!("a newer line, number {i}, that pushes the video up and out of view"));
    }
    chat.post(&format!("and another video {B}"));
    chat.state.chat_video.rewind_for_test(&a);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.dock.as_ref().is_some_and(|d| d.url == Chat::url(A)));
    assert!(held, "A is docked");
    for _ in 0..2 {
        chat.frame(Vec::new());
    }
    chat.press_picture(&b);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(B)));
    assert!(held, "B plays in its row");
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    assert!(chat.state.chat_video.dock.is_none(), "the dock closes");
    assert!(!chat.state.chat_video.is_playing(&a), "A is paused");
    assert!(!chat.state.chat_video.has_player(&a), "A's player goes, as a paused row's does");
    assert!(chat.state.chat_video.still_texture(&a).is_some(), "A's row keeps its still");
    assert_eq!(chat.state.chat_video.playing(), Some(b.as_str()), "only B plays");
}

/// THE DOCK NEVER COVERS THE MESSAGE BOX OR THE CHANNEL HEADER, at 1280x720 and 2560x1440 and Font
/// Size 100% and 200% (the screen in points is the pixels over the Font Size). Docked, the dock (or
/// its tab, where the room is too small for a picture) lies inside the message list and crosses
/// neither the box (the composer's own response) nor the header's "# global"; at 2560x1440 at 100%
/// it opens (a picture fits), and at 1280x720 at 200% (640 x 360 points) it starts as the tab.
///
/// Seen red 2026-11-06 with the picture's height not held to the list and the room never too small
/// (`picture_h` the width over the shape, `too_small` false): "1280x720 at 200%: list [[220.0 125.5]
/// - [420.0 308.0]], drawn [[226.0 131.5] - [414.0 311.3]] (open true, tab false), box [[249.0 320.0]
/// - [369.0 338.4]] ...: outside the message list" (open in a list 200 points wide, running past its
/// bottom toward the box).
#[test]
fn the_dock_never_covers_the_message_box_or_the_header() {
    let mut bad = Vec::new();
    for (w, h) in [(1280.0_f32, 720.0_f32), (2560.0, 1440.0)] {
        for percent in [100.0_f32, 200.0] {
            let zoom = percent / 100.0;
            let (mut chat, _a) = docked(&format!("chat_video_dock_room_{w}_{percent}"), w / zoom, h / zoom);
            let mut out = chat.frame(Vec::new());
            for _ in 0..3 {
                out = chat.frame(Vec::new());
            }
            let texts = texts_of(&out);
            let open = texts.iter().any(|x| x == t_go_to());
            let tab = texts.iter().any(|x| x == t("chat_video_dock.tab_playing") || x == t("chat_video_dock.tab_paused"));
            let drawn = if open { dock_rect(&chat) } else { chat.ctx.memory(|m| m.area_rect(egui::Id::new(TAB_ID))).unwrap_or(egui::Rect::NOTHING) };
            let list = chat.state.chat_video.last_list_rect().unwrap_or(egui::Rect::NOTHING);
            let composer = chat.ctx.read_response(egui::Id::new("chat_composer_input")).map(|r| r.rect).unwrap_or(egui::Rect::NOTHING);
            let header = words_at(&out, "# global").unwrap_or(egui::Rect::NOTHING);
            let name = format!("{w}x{h} at {percent}%: list {list:?}, drawn {drawn:?} (open {open}, tab {tab}), box {composer:?}, header {header:?}");
            if !(open || tab) {
                bad.push(format!("{name}: neither the dock nor its tab"));
            } else if !list.contains_rect(drawn) {
                bad.push(format!("{name}: outside the message list"));
            } else if drawn.intersects(composer) {
                bad.push(format!("{name}: crosses the message box"));
            } else if drawn.intersects(header) {
                bad.push(format!("{name}: crosses the header"));
            } else if composer == egui::Rect::NOTHING || header == egui::Rect::NOTHING {
                bad.push(format!("{name}: the box or the header was not found"));
            }
            if w == 2560.0 && percent == 100.0 && !open {
                bad.push(format!("{name}: a picture fits, so it opens"));
            }
            if w == 1280.0 && percent == 200.0 && !tab {
                bad.push(format!("{name}: no room for a picture, so it starts as the tab"));
            }
        }
    }
    assert!(bad.is_empty(), "the dock keeps off the box and the header:\n{}", bad.join("\n"));
}

/// ON ANOTHER PAGE THE DOCK STANDS ON THE RIGHT EDGE, CLEAR OF THE PAGE'S CONTROLS, OR AS THE TAB.
/// `free_spot` on its own: an empty right edge puts it at the bottom right; a button there lifts it to
/// just above the button; a column of buttons up the whole edge leaves no spot (the tab). And drawn:
/// a stand-in page with a button at its bottom right, the dock (followed there from the chat) stands
/// clear of it, decided from egui's record of the page's controls the frame before.
///
/// Seen red 2026-11-06 with `page_controls` answering no controls: "clear of the page's button" (the
/// dock stood over it at the bottom right).
#[test]
fn on_another_page_the_dock_keeps_clear_of_the_pages_controls() {
    let room = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1000.0, 800.0));
    let size = egui::vec2(300.0, 250.0);
    assert_eq!(free_spot(room, size, &[], 8.0), Some(egui::pos2(700.0, 550.0)), "an empty edge: the bottom right");
    let button = egui::Rect::from_min_size(egui::pos2(900.0, 760.0), egui::vec2(90.0, 30.0));
    assert_eq!(free_spot(room, size, &[button], 8.0), Some(egui::pos2(700.0, 760.0 - 8.0 - 250.0)), "just above the button");
    let column: Vec<egui::Rect> = (0..20).map(|i| egui::Rect::from_min_size(egui::pos2(950.0, i as f32 * 40.0), egui::vec2(40.0, 30.0))).collect();
    assert_eq!(free_spot(room, size, &column, 8.0), None, "a column of buttons up the edge: no spot");
    assert_eq!(free_spot(room, egui::vec2(1200.0, 100.0), &[], 8.0), None, "wider than the room: no spot");

    // Drawn: followed from the chat onto a stand-in page with a button at its bottom right.
    let (mut chat, a) = docked("chat_video_dock_other_page", 1600.0, 1000.0);
    chat.state.active_page = crate::gui::GuiPage::Library;
    let mut out = chat.frame_page(Vec::new());
    for _ in 0..4 {
        chat.state.chat_video.rewind_for_test(&a);
        out = chat.frame_page(Vec::new());
    }
    let button = words_at(&out, "A page's own button").expect("the stand-in page's button");
    assert!(texts_of(&out).iter().any(|x| x == t_go_to()), "the dock is drawn on the page: {:?}", texts_of(&out));
    let dock = dock_rect(&chat);
    assert!(!dock.intersects(button.expand(4.0)), "clear of the page's button: dock {dock:?}, button {button:?}");
    assert!((1600.0 - dock.right()) < 20.0, "on the right edge: {dock:?}");
}

/// ONE CHAT VIDEO AT A TIME ACROSS THE APP SCREEN AND EVERY WALL SCREEN (the operator, 2026-11-06:
/// "Agreed."). The app's video docked and playing; a wall screen's chat starts its own: the app's
/// pauses at its next end of frame and its dock closes. Then the app's video started again: the
/// wall screen's pauses, without the wall drawing again (as under a page, when no wall screen draws),
/// through `ScreenCore::yield_chat_video`, which the main loop calls for every wall screen. Then the
/// wall screen's own Full window, opened on the wall, plays it: the app's pauses again.
///
/// Seen red 2026-11-06 with `yield_if_elsewhere` doing nothing (each set its own, as before): "the
/// app's video pauses once the wall's starts" (both played, both sounded).
#[test]
fn one_chat_video_at_a_time_across_the_app_and_the_wall_screens() {
    let (mut chat, a) = docked("chat_video_dock_across_sets", 1600.0, 1000.0);
    assert!(chat.state.chat_video.is_playing(&a), "the app's video plays in the dock");
    // A wall screen's chat (its own set), drawing the same chat; its row pressed.
    let mut wall = Wall::new(fetch_copies(fixture(), chat.dir.to_path_buf()));
    let mut other = Chat::new("chat_video_dock_across_sets_wall", &[&format!("on the wall {A}")], 1280.0, 720.0);
    for _ in 0..3 {
        wall.frame(&mut other.state);
    }
    let at = wall.core.chat_videos().pictures.get(&a).expect("the picture is drawn on the wall").center();
    wall.click(&mut other.state, at);
    let (texts, held) = wall.frames_until(&mut other.state, |core, _| core.chat_videos().is_playing(&Chat::url(A)));
    assert!(held, "the wall's video plays: {texts:?}");
    chat.frame(Vec::new());
    assert!(!chat.state.chat_video.is_playing(&a), "the app's video pauses once the wall's starts");
    assert!(chat.state.chat_video.dock.is_none(), "and its dock closes");

    // The app's started again (its row brought back and pressed): the wall's pauses, the wall not
    // drawn again, by the main loop's call for every wall screen.
    chat.state.chat_messages.truncate(1);
    for _ in 0..3 {
        chat.frame(Vec::new());
    }
    chat.press_picture(&a);
    let (_, held) = chat.frames_until(|s, _| s.chat_video.is_playing(&Chat::url(A)));
    assert!(held, "the app's video plays again");
    assert!(wall.core.chat_videos().is_playing(&a), "the wall's still plays until it gives way");
    wall.core.yield_chat_video();
    assert!(!wall.core.chat_videos().is_playing(&a), "the wall's video pauses, the wall not drawn again");

    // A wall screen's own Full window (drawn on the wall, main's e27689b6d) counts too: pressed on
    // the wall, it plays the video there, and the app's pauses at its next end of frame.
    let shown = texts_at(&wall.frame(&mut other.state));
    let full = shown.into_iter().find(|(x, _)| x == t("chat_video.full_window")).expect("Full window is drawn on the wall").1;
    wall.click(&mut other.state, full.center());
    let (texts, held) = wall.frames_until(&mut other.state, |core, _| core.chat_videos().window_open() && core.chat_videos().is_playing(&Chat::url(A)));
    assert!(held, "the wall's Full window plays the video: {texts:?}");
    chat.frame(Vec::new());
    assert!(!chat.state.chat_video.is_playing(&a), "the app's video pauses once the wall's Full window plays");
}

// ── Is the dock in the picture? (2026-11-07) ──
// The three dock snapshots passed on 2026-11-07 and none showed the dock: the snapshot set its dock
// up on the very first frame, frame 0, with `row_seen` 0, so the dock read its row as back on screen
// and closed before it was ever drawn ("a check whose evidence is the setup"). The real app docks
// only at the end of a frame after its row was drawn (frame 1 or later), so it never met that; the
// test below proves the app's own draw order paints the dock, and `dock_painted` now gates the
// snapshots too, so a picture without the dock fails instead of being written.

/// What the dock should show: open, or folded to its tab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shown {
    Open,
    Tab,
}

/// Where the test frames note the bottom strip's top (the strip's own note answers only during the
/// pass that drew it).
const STRIP_TOP_KEY: &str = "chat_video_dock_test_strip_top";

/// The app's frame after its first lines, in lib.rs's order: the top menu on a page (not the title
/// screen or the world), the bottom strip (its top noted for [`strip_top`]), the page through the one
/// dispatch table the in-world screens share, the chat images, then the dock, the Full window and
/// the chat videos' end of frame.
pub(crate) fn app_frame_parts(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    if !matches!(state.active_page, GuiPage::None | GuiPage::MainMenu) {
        crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state);
    }
    crate::gui::bottom_strip::draw_for_version(ctx, theme, state, "0.1550.0");
    let top = crate::gui::bottom_strip::room_above(ctx).bottom();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(STRIP_TOP_KEY), top));
    match state.active_page {
        GuiPage::None => {}
        page => {
            crate::gui::dispatch::draw_tool_page(ctx, page, theme, state);
        }
    }
    state.image_cache.poll(ctx);
    draw_video_dock(ctx, theme, state);
    crate::gui::pages::chat::draw_video_window(ctx, theme, &mut state.chat_video);
}

/// The bottom strip's top as the last test frame drew it.
pub(crate) fn strip_top(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new(STRIP_TOP_KEY))).unwrap_or(f32::INFINITY)
}

/// Every shape of a frame in the order egui paints them (nested lists opened), with its clip.
fn painted_in_order(out: &egui::FullOutput) -> Vec<(egui::Rect, egui::Shape)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, all: &mut Vec<(egui::Rect, egui::Shape)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, all)),
            s => all.push((clip, s.clone())),
        }
    }
    let mut all = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, c.clip_rect, &mut all);
    }
    all
}

/// Whether the dock was painted in the frame `out` (just run on `ctx`), as `shown`, over the page:
/// its Area (or the tab's) shown that frame and nearer than the page's layer in egui's own order;
/// its words drawn inside it and inside their clip (Go to message for the open dock and no tab, the
/// tab's words and no Go to message for the tab); the open dock's picture drawn inside it; and
/// nothing opaque painted after its frame over most of it. The dock's rectangle, or why not.
pub(crate) fn dock_painted(ctx: &egui::Context, out: &egui::FullOutput, theme: &Theme, shown: Shown) -> Result<egui::Rect, String> {
    let id = egui::Id::new(if shown == Shown::Open { AREA_ID } else { TAB_ID });
    let layer = egui::LayerId::new(Layer::Footer.order(), id);
    let rect = ctx.memory(|m| m.area_rect(id)).ok_or("its Area was never laid out")?;
    if !ctx.memory(|m| m.areas().visible_last_frame(&layer)) {
        return Err(format!("its Area ({shown:?}) was not shown in the frame"));
    }
    let order: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
    let (mine, page) = (order.iter().position(|l| *l == layer), order.iter().position(|l| *l == egui::LayerId::background()));
    match (mine, page) {
        (Some(m), Some(p)) if m > p => {}
        _ => return Err(format!("its layer is not over the page's in egui's order (dock at {mine:?}, page at {page:?})")),
    }
    let shapes = painted_in_order(out);
    let text_in = |w: &str| {
        shapes.iter().any(|(clip, s)| match s {
            egui::Shape::Text(t) => {
                let r = egui::Rect::from_min_size(t.pos, t.galley.size());
                t.galley.job.text == w && rect.expand(1.0).contains_rect(r) && clip.intersects(r)
            }
            _ => false,
        })
    };
    let go_to = t("chat_video_dock.go_to");
    let tab_words = [t("chat_video_dock.tab_playing"), t("chat_video_dock.tab_paused")];
    match shown {
        Shown::Open => {
            if !text_in(go_to) {
                return Err(format!("no \"{go_to}\" painted inside the dock {rect:?}"));
            }
            if tab_words.iter().any(|w| text_in(w)) {
                return Err("the tab's words are painted with the open dock".into());
            }
            // A picture: a textured mesh, or (egui's `Image`) a rectangle filled with a texture.
            let picture = shapes.iter().any(|(clip, s)| {
                let r = match s {
                    egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default() => m.calc_bounds(),
                    egui::Shape::Rect(r) if r.fill_texture_id() != egui::TextureId::default() => r.rect,
                    _ => return false,
                };
                rect.expand(1.0).contains_rect(r) && clip.intersects(r) && r.width() > 40.0
            });
            if !picture {
                return Err(format!("no picture painted inside the dock {rect:?}"));
            }
        }
        Shown::Tab => {
            if !tab_words.iter().any(|w| text_in(w)) {
                return Err(format!("no tab words painted inside the tab {rect:?}"));
            }
            if shapes.iter().any(|(_, s)| matches!(s, egui::Shape::Text(t) if t.galley.job.text == go_to)) {
                return Err("the open dock's Go to message is painted with the tab".into());
            }
        }
    }
    // Its frame (the filled box covering its whole Area, in the card colour), then nothing opaque
    // over most of it.
    let fills_it = |r: &egui::epaint::RectShape| r.fill.a() > 0 && rect.expand(2.0).contains_rect(r.rect) && r.rect.contains_rect(rect.shrink(2.0));
    let frame_at = shapes.iter().position(|(_, s)| matches!(s, egui::Shape::Rect(r) if fills_it(r)));
    let Some(frame_at) = frame_at else {
        let near: Vec<String> = shapes.iter().filter_map(|(_, s)| match s {
            egui::Shape::Rect(r) if r.rect.intersects(rect) && r.rect.area() > rect.area() * 0.3 => Some(format!("{:?} {:?}", r.rect, r.fill)),
            _ => None,
        }).take(6).collect();
        return Err(format!("its frame filling {rect:?} was not painted (the big boxes there: {near:?})"));
    };
    // In the card colour (its opacity aside: egui fades an Area in over its animation time, so a few
    // frames in the frame's alpha can still be a little under full, #030303FC on 2026-11-07).
    if let egui::Shape::Rect(r) = &shapes[frame_at].1 {
        let card = theme.bg_card();
        if (r.fill.r(), r.fill.g(), r.fill.b()) != (card.r(), card.g(), card.b()) && r.fill.to_opaque() != card.to_opaque() {
            return Err(format!("its frame is {:?}, not the card colour {:?}", r.fill, card));
        }
    }
    for (clip, s) in &shapes[frame_at + 1..] {
        if let egui::Shape::Rect(r) = s {
            let over = r.rect.intersect(rect).intersect(*clip);
            // Nearly opaque counts: an Area fading in (egui's animation time) paints a little under full.
            if r.fill.a() >= 204 && !rect.expand(1.0).contains_rect(r.rect) && over.is_positive() && over.area() > rect.area() * 0.5 {
                return Err(format!("something opaque was painted over it after its frame: {:?} filled {:?}", r.rect, r.fill));
            }
        }
    }
    Ok(rect)
}

/// THE APP'S OWN FRAME PAINTS THE DOCK OVER THE PAGE, ON THE CHAT PAGE AND ON ANOTHER PAGE (the
/// orchestrator's question after the dockless snapshots, 2026-11-07: is the real app affected?). A
/// video docked the app's way (its row pushed off the screen), then frames drawn in lib.rs's order
/// (`app_frame_parts`: the top menu, the bottom strip, the page through the dispatch table, the dock):
/// on the Chat page the open dock is painted over the page (its layer nearer than the page's, its
/// words and picture inside it, nothing opaque over it) at the top right of the message list; folded,
/// the tab is painted instead; on the Library page the open dock is painted standing on the bottom
/// strip at the right edge. Then the checker itself is seen to fail: an opaque box painted over the
/// dock in a nearer layer is reported.
///
/// Seen red 2026-11-07 with `draw_video_dock` returning before drawing on any page but the Chat page:
/// "on the Library page: its Area (Open) was not shown in the frame".
#[test]
fn the_apps_own_frame_paints_the_dock_over_the_page() {
    let (mut chat, a) = docked("chat_video_dock_painted", 1600.0, 1000.0);
    let run = |chat: &mut Chat, extra: &dyn Fn(&egui::Context, &Theme)| -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, chat.screen)), ..Default::default() };
        let (theme, state) = (&mut chat.theme, &mut chat.state);
        chat.ctx.run(input, |ctx| {
            app_frame_parts(ctx, theme, state);
            extra(ctx, theme);
        })
    };
    let frames = |chat: &mut Chat, n: usize| -> egui::FullOutput {
        let mut out = run(chat, &|_, _| {});
        for _ in 1..n {
            chat.state.chat_video.rewind_for_test(&Chat::url(A));
            out = run(chat, &|_, _| {});
        }
        out
    };
    let theme = chat.theme.clone();

    // The Chat page, drawn as the app draws it (the menu, the strip, the dispatch table).
    let out = frames(&mut chat, 3);
    let rect = dock_painted(&chat.ctx, &out, &theme, Shown::Open).unwrap_or_else(|why| panic!("on the Chat page: {why}"));
    let list = chat.state.chat_video.last_list_rect().expect("the Chat page noted its list");
    assert!(list.contains_rect(rect) && rect.top() - list.top() < 20.0 && list.right() - rect.right() < 20.0, "at the list's top right: {rect:?} in {list:?}");

    // Folded: the tab.
    chat.state.chat_video.dock.as_mut().unwrap().minimized = true;
    let out = frames(&mut chat, 3);
    dock_painted(&chat.ctx, &out, &theme, Shown::Tab).unwrap_or_else(|why| panic!("folded, on the Chat page: {why}"));
    chat.state.chat_video.dock.as_mut().unwrap().minimized = false;

    // Another page: the Library, the dock standing on the bottom strip at the right edge.
    chat.state.active_page = GuiPage::Library;
    let out = frames(&mut chat, 4);
    let rect = dock_painted(&chat.ctx, &out, &theme, Shown::Open).unwrap_or_else(|why| panic!("on the Library page: {why}"));
    let top = strip_top(&chat.ctx);
    assert!(rect.bottom() <= top + 0.5 && chat.screen.x - rect.right() < 24.0, "standing on the strip (its top at {top}) at the right edge: {rect:?}");
    assert!(chat.state.chat_video.is_playing(&a), "and playing there");

    // The checker can fail: an opaque box painted over the dock, nearer than it, is reported. (Two
    // frames at least: an Area's first is egui's sizing pass, drawn unseen; six, so its fade-in is done.)
    let cover_it = |ctx: &egui::Context, theme: &Theme| {
        crate::gui::layers::area(ctx, egui::Id::new("test_cover_over_the_dock"), Layer::PageFloat).fixed_pos(rect.min).show(ctx, |ui| {
            ui.painter().rect_filled(rect.expand(4.0), 0.0, theme.bg_primary());
        });
    };
    for _ in 0..5 {
        run(&mut chat, &cover_it);
    }
    let cover = run(&mut chat, &cover_it);
    let said = dock_painted(&chat.ctx, &cover, &theme, Shown::Open);
    assert!(said.as_ref().is_err_and(|w| w.contains("opaque")), "a box painted over the dock is reported: {said:?}");
}

/// A DOCK SET UP ON THE VERY FIRST FRAME STAYS (the other-page snapshot's setup, 2026-11-07):
/// `row_seen` starts at "never", not at frame 0, so a dock made before any frame has ended is not
/// read as its row back on screen. On another page (no rows drawn, so nothing can bring it back
/// either), with the video playing and docked before the first frame, the dock is still there and
/// drawn three frames later (one waiting for egui's record of the page, one sizing pass unseen).
///
/// Seen red 2026-11-07 with `row_seen` starting at 0: "a dock made on frame 0 stays" (it closed at
/// that frame's end, unseen, and never came back: on that page no row is drawn to dock it again).
#[test]
fn a_dock_made_on_the_first_frame_stays() {
    let mut chat = Chat::new("chat_video_dock_first_frame", &[&format!("watch {A}")], 1600.0, 1000.0);
    chat.state.chat_video.fetch_for_test = Some(fetch_copies(fixture(), chat.dir.to_path_buf()));
    chat.state.active_page = GuiPage::Library;
    let a = Chat::url(A);
    let ffmpeg = chat.state.settings.ffmpeg_path.clone();
    chat.state.chat_video.press_play(&a, "sliding_bar.mp4", &ffmpeg);
    assert!(chat.state.chat_video.wait_for_picture_or_words(&a, &chat.ctx.clone()), "the fixture shows a picture");
    let origin = origin_here(&chat.state);
    chat.state.chat_video.dock_for_test(&a, origin);
    let mut out = chat.frame_page(Vec::new());
    for _ in 0..3 {
        chat.state.chat_video.rewind_for_test(&a);
        out = chat.frame_page(Vec::new());
    }
    assert!(chat.state.chat_video.dock.is_some(), "a dock made on frame 0 stays");
    assert!(texts_of(&out).iter().any(|x| x == t_go_to()), "a dock made on frame 0 is drawn: {:?}", texts_of(&out));
}
