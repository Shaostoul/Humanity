//! Tests of Add to Media's upload in pieces (resumable.rs and upload_waits.rs), against a server
//! simulated in memory that answers the way src/relay/resumable.rs does: the same routes' shapes,
//! statuses and refusals. Each was seen to fail once before it passed; what it saw red is in its
//! comment. The loop tests (round 3's findings 2, 3 and 5) drive `work` and count the requests the
//! server saw in a window of time: a loop with no wait shows as hundreds of thousands.

use super::*;
use crate::gui::pages::media::upload_cancel::Told;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// The token the simulated server takes.
pub(crate) const TOKEN: &str = "tok-now";

/// What the simulated server does to a request, scripted by a test.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Trick {
    /// Write the piece, then lose its answer (the connection dropped after the server took it).
    LoseAnswer,
    /// Refuse the piece for the day's allowance, resetting at this time (Unix ms), with this many
    /// seconds to wait by the server's clock (None: the round 2 relay, which sent none).
    Allowance(i64, Option<f64>),
    /// Refuse the finish for room (507), keeping the whole file.
    NoRoom,
    /// Finish, then lose the answer.
    LoseFinish,
}

/// The simulated server's state (the page's tests read it too: page_tests.rs).
#[derive(Default)]
pub(crate) struct Server {
    token: String,
    pub(crate) id: Option<String>,
    pub(crate) size: u64,
    pub(crate) blake3: String,
    pub(crate) data: Vec<u8>,
    pub(crate) done: Option<Value>,
    chunk_max: u64,
    /// Tricks for the non-empty pieces in order (None: answer normally), and for the finish.
    pub(crate) tricks: VecDeque<Option<Trick>>,
    pub(crate) finish_trick: Option<Trick>,
    /// Answers the start gives first, in order, before answering normally; and one it gives every
    /// time, forever (a server error that never clears).
    pub(crate) start_answers: VecDeque<Reply>,
    pub(crate) start_forever: Option<Reply>,
    /// The day's allowance, in bytes, as the relay of round 3 counts it: a piece larger than what
    /// is left is refused with `left_bytes` (R3-6). With `reset_at_zero` the window passes while
    /// the app waits after a refusal with nothing left.
    allowance_left: Option<u64>,
    reset_at_zero: bool,
    /// Every non-empty piece refused for the day's allowance, forever: its `resets_at` and
    /// `resets_in_s`, as [`Trick::Allowance`].
    pub(crate) allowance_forever: Option<(i64, Option<f64>)>,
    pub(crate) log: Vec<String>,
    pub(crate) cancelled: bool,
    start_body: Value,
    /// Answers a cancel gives first, in order (409 busy while the finish is written, round 4),
    /// and one it gives every time, forever.
    pub(crate) cancel_answers: VecDeque<Reply>,
    pub(crate) cancel_forever: Option<Reply>,
    /// The finished file was put on the Media page: a cancel leaves it (`published: true`).
    pub(crate) published: bool,
    /// A cancel removed the finished file stored from the upload (`removed_stored`, round 4).
    pub(crate) stored_removed: bool,
    /// Status requests whose answer is lost, from the next one on (a connection that keeps
    /// dropping), counted down.
    pub(crate) status_lost: usize,
}

/// A server simulated in memory, answering as src/relay/resumable.rs does.
pub(crate) struct Fake(Mutex<Server>);

impl Fake {
    pub(crate) fn new(chunk_max: u64) -> Arc<Fake> {
        Arc::new(Fake(Mutex::new(Server { token: TOKEN.into(), chunk_max, ..Default::default() })))
    }
    pub(crate) fn s(&self) -> std::sync::MutexGuard<'_, Server> {
        self.0.lock().unwrap()
    }
    /// How many requests of `kind` ("start", "status", "piece", "cancel") the server has seen.
    pub(crate) fn count(&self, kind: &str) -> usize {
        self.s().log.iter().filter(|l| l.starts_with(kind)).count()
    }
}

fn ok(v: Value) -> Result<Reply, String> {
    Ok(Reply { status: 200, body: v.to_string() })
}

fn refuse(status: u16, body: impl Into<String>) -> Result<Reply, String> {
    Ok(Reply { status, body: body.into() })
}

impl Transport for Fake {
    fn start(&self, token: &str, body: &Value) -> Result<Reply, String> {
        let mut s = self.s();
        s.log.push("start".into());
        if token != s.token {
            return refuse(403, "Invalid upload token.");
        }
        if let Some(r) = s.start_answers.pop_front().or_else(|| s.start_forever.clone()) {
            return Ok(r);
        }
        // The relay's own StartBody reads what is sent.
        let b: crate::relay::resumable::StartBody = serde_json::from_value(body.clone()).expect("the relay reads the start");
        assert_eq!(b.media, Some(1), "a resumable upload is for the Files page");
        s.size = b.size;
        s.blake3 = b.blake3.unwrap_or_default();
        s.id = Some("0123456789abcdef0123456789abcdef".into());
        s.start_body = body.clone();
        Ok(Reply { status: 201, body: json!({ "id": s.id, "offset": 0, "size": s.size, "chunk_max": s.chunk_max }).to_string() })
    }

    fn status(&self, token: &str, id: &str) -> Result<Reply, String> {
        let mut s = self.s();
        s.log.push("status".into());
        if token != s.token {
            return refuse(403, "Invalid upload token.");
        }
        if s.status_lost > 0 {
            s.status_lost -= 1;
            return Err("The server could not be reached (connection reset).".into());
        }
        if s.id.as_deref() != Some(id) {
            return refuse(404, "There is no upload in progress with that id.");
        }
        let mut v = json!({ "offset": s.data.len(), "size": s.size, "chunk_max": s.chunk_max });
        if let Some(done) = s.done.clone() {
            v["done"] = done;
        }
        ok(v)
    }

    fn piece(&self, token: &str, id: &str, offset: u64, bytes: &[u8]) -> Result<Reply, String> {
        let mut s = self.s();
        s.log.push(format!("piece {offset}+{}", bytes.len()));
        if token != s.token {
            return refuse(403, "Invalid upload token.");
        }
        if s.id.as_deref() != Some(id) {
            return refuse(404, "There is no upload in progress with that id.");
        }
        if let Some(done) = s.done.clone() {
            return ok(done);
        }
        if offset != s.data.len() as u64 {
            let at = s.data.len();
            return refuse(409, json!({ "error": "wrong place", "offset": at, "size": s.size }).to_string());
        }
        assert!(bytes.len() as u64 <= s.chunk_max, "no piece over the server's chunk_max");
        if let Some(left) = s.allowance_left {
            if bytes.len() as u64 > left {
                let (at, size) = (s.data.len(), s.size);
                if left == 0 && s.reset_at_zero {
                    s.allowance_left = None; // the window passes while the app waits
                }
                return refuse(429, json!({ "error": "Your role may upload 1 MB a day on this server.", "allowance": "day", "resets_at": 0, "resets_in_s": 0, "left_bytes": left, "offset": at, "size": size }).to_string());
            }
            s.allowance_left = Some(left - bytes.len() as u64);
        }
        if !bytes.is_empty() {
            let trick = s.tricks.pop_front().flatten().or_else(|| s.allowance_forever.map(|(at, secs)| Trick::Allowance(at, secs)));
            if let Some(Trick::Allowance(resets_at, resets_in_s)) = trick {
                let (at, size) = (s.data.len(), s.size);
                let mut v = json!({ "error": "Your role may upload 100 MB a day on this server.", "allowance": "day", "resets_at": resets_at, "offset": at, "size": size });
                if let Some(secs) = resets_in_s {
                    v["resets_in_s"] = json!(secs);
                }
                return refuse(429, v.to_string());
            }
            s.data.extend_from_slice(bytes);
            if (s.data.len() as u64) < s.size {
                if trick == Some(Trick::LoseAnswer) {
                    return Err("The server could not be reached (connection reset).".into());
                }
                return ok(json!({ "offset": s.data.len(), "size": s.size }));
            }
        }
        if (s.data.len() as u64) < s.size {
            return refuse(409, json!({ "error": "not there yet", "offset": s.data.len(), "size": s.size }).to_string());
        }
        // The finish.
        if s.finish_trick == Some(Trick::NoRoom) {
            s.finish_trick = None;
            return refuse(507, "This server's file space is full. The file you sent is kept here for a day, so once there is room it can be finished without sending it again.");
        }
        if blake3::hash(&s.data).to_hex().to_string() != s.blake3 {
            return refuse(422, "The file that arrived does not match the file you sent (its BLAKE3 hash differs), so nothing was kept. Send it again from the start.");
        }
        let answer = json!({ "url": "/uploads/1700_clip.webm", "filename": "1700_clip.webm", "size": s.size, "type": "video" });
        s.done = Some(answer.clone());
        if s.finish_trick == Some(Trick::LoseFinish) {
            s.finish_trick = None;
            return Err("The server could not be reached (connection reset).".into());
        }
        ok(answer)
    }

    /// The relay's cancel as round 4 states it: an unfinished upload `{ok: true}`; a finished one
    /// not on Media `{ok: true, removed_stored: true}`, its stored file removed; a published one
    /// `{ok: true, published: true}`, left as it is; 409 `{busy: true}` while busy (scripted).
    fn cancel(&self, token: &str, id: &str) -> Result<Reply, String> {
        let mut s = self.s();
        s.log.push("cancel".into());
        if token != s.token {
            return refuse(403, "Invalid upload token.");
        }
        if let Some(r) = s.cancel_answers.pop_front().or_else(|| s.cancel_forever.clone()) {
            return Ok(r);
        }
        if s.id.as_deref() != Some(id) {
            return refuse(404, "There is no upload in progress with that id.");
        }
        if s.published {
            return ok(json!({ "ok": true, "published": true }));
        }
        s.cancelled = true;
        s.id = None;
        if s.done.is_some() {
            s.stored_removed = true;
            return ok(json!({ "ok": true, "removed_stored": true }));
        }
        ok(json!({ "ok": true }))
    }
}

static FILES: AtomicUsize = AtomicUsize::new(0);

/// A file of `len` patterned bytes, and the plan to upload it.
pub(crate) fn file(len: usize) -> (crate::test_temp::TempPath, Plan, Vec<u8>) {
    let n = FILES.fetch_add(1, Ordering::SeqCst);
    let path = crate::test_temp::file(&format!("media_resumable_{n}"), "webm");
    let bytes: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let plan = Plan { path: path.to_path_buf(), name: "clip.webm".into(), size: len as u64, content_type: "video/webm".into(), resume: None, file_changed: false, clean: false, keep_metadata: false };
    (path, plan, bytes)
}

/// Run the upload on a worker thread with `ctl`, with `before` given the control and the events
/// as they come (to press Pause, Resume, Retry or Cancel), until it ends. Returns every event.
pub(crate) fn run_on(fake: Arc<Fake>, plan: Plan, ctl: Arc<Control>, mut before: impl FnMut(&Control, &Event)) -> Vec<Event> {
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_ctl = ctl.clone();
    let t: Arc<dyn Transport> = fake;
    let handle = std::thread::spawn(move || work(t.as_ref(), &worker_ctl, &plan, &tx));
    let mut events = Vec::new();
    loop {
        let e = rx.recv_timeout(Duration::from_secs(20)).unwrap_or_else(|_| panic!("the upload stalled; events so far: {events:?}"));
        before(&ctl, &e);
        let last = matches!(e, Event::Done { .. } | Event::Failed { .. } | Event::Cancelled { .. });
        events.push(e);
        if last {
            break;
        }
    }
    handle.join().unwrap();
    events
}

/// [`run_on`] with a control whose waits after a failure start at a millisecond.
pub(crate) fn run_with(fake: Arc<Fake>, plan: Plan, token: &str, before: impl FnMut(&Control, &Event)) -> Vec<Event> {
    run_on(fake, plan, Arc::new(Control::with_pace(Some(token.to_string()), Duration::from_millis(1))), before)
}

/// Start the upload on a worker thread and hand back its control, its events and the thread,
/// for the tests that look at the server between events.
pub(crate) fn spawn_work(fake: &Arc<Fake>, plan: Plan, ctl: Control) -> (Arc<Control>, std::sync::mpsc::Receiver<Event>, std::thread::JoinHandle<()>) {
    let ctl = Arc::new(ctl);
    let (tx, rx) = std::sync::mpsc::channel();
    let (worker_ctl, t): (Arc<Control>, Arc<dyn Transport>) = (ctl.clone(), fake.clone());
    let handle = std::thread::spawn(move || work(t.as_ref(), &worker_ctl, &plan, &tx));
    (ctl, rx, handle)
}

/// Wait for the first event `want` matches (at most ten seconds), and return it.
pub(crate) fn wait_for(rx: &std::sync::mpsc::Receiver<Event>, want: impl Fn(&Event) -> bool) -> Event {
    until(rx, want).pop().expect("the matching event")
}

/// Every event up to and including the first `want` matches (at most ten seconds).
pub(crate) fn until(rx: &std::sync::mpsc::Receiver<Event>, want: impl Fn(&Event) -> bool) -> Vec<Event> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(left) {
            Ok(e) => {
                let found = want(&e);
                seen.push(e);
                if found {
                    return seen;
                }
            }
            Err(_) => panic!("the event never came; events so far: {} of them", seen.len()),
        }
    }
}

/// The window the loop tests count requests in: long enough that a loop with no wait sends
/// hundreds of thousands, short enough to keep the run quick.
pub(crate) const WINDOW: Duration = Duration::from_millis(500);

pub(crate) fn done_answer(events: &[Event]) -> Option<Value> {
    events.iter().find_map(|e| if let Event::Done { answer } = e { Some(answer.clone()) } else { None })
}

/// A FILE GOES IN PIECES AND FINISHES: hashed, started with the relay's own start body (media: 1
/// and the file's BLAKE3), sent in pieces no larger than the server's `chunk_max`, and finished
/// with the single request's answer; what the server holds is the file exactly.
///
/// Seen red 2026-10-11 with each piece half the server's `chunk_max`: "three pieces" (left: 5 /
/// right: 3).
#[test]
fn a_file_goes_in_pieces_and_finishes() {
    let fake = Fake::new(8192);
    let (_f, plan, bytes) = file(20_000);
    let events = run_with(fake.clone(), plan, TOKEN, |_, _| {});
    let answer = done_answer(&events).unwrap_or_else(|| panic!("it finished: {events:?}"));
    assert_eq!(stored_name_of(&answer), "1700_clip.webm");
    let s = fake.s();
    assert_eq!(s.data, bytes, "the server holds the file exactly");
    assert_eq!(s.blake3, blake3::hash(&bytes).to_hex().to_string(), "started with the file's hash");
    assert_eq!(s.log.iter().filter(|l| l.starts_with("piece")).count(), 3, "three pieces: {:?}", s.log);
    assert!(events.iter().any(|e| matches!(e, Event::Started { .. })), "the page is told the id to keep");
}

/// A DROPPED CONNECTION CARRIES ON FROM WHERE THE SERVER GOT: the second piece's answer is lost
/// after the server took it, so the app asks the server where it is (status) and sends the third,
/// never the second again; and a finish whose answer was lost is found in the status answer's
/// `done` (the relay's R2).
///
/// Seen red 2026-10-11 with a lost answer treated as a sent piece without asking (the offset moved
/// on by the piece): it still finished (the piece's answer told it the finish's), so the checks
/// that count are where the status was asked: "the status is asked after the lost answer" (the
/// log was ["start", "piece 0+8192", "piece 8192+8192", "piece 16384+3616", "piece 20000+0"]).
#[test]
fn a_dropped_connection_carries_on_from_where_the_server_got() {
    let fake = Fake::new(8192);
    fake.s().tricks = VecDeque::from([None, Some(Trick::LoseAnswer), None]);
    fake.s().finish_trick = Some(Trick::LoseFinish);
    let (_f, plan, bytes) = file(20_000);
    let events = run_with(fake.clone(), plan, TOKEN, |_, _| {});
    assert!(done_answer(&events).is_some(), "it finished: {events:?}");
    let s = fake.s();
    assert_eq!(s.data, bytes, "no piece twice, none missing");
    let after_lost: Vec<&str> = s.log.iter().skip_while(|l| l.as_str() != "piece 8192+8192").skip(1).take(1).map(String::as_str).collect();
    assert_eq!(after_lost, vec!["status"], "the status is asked after the lost answer: {:?}", s.log);
    assert_eq!(s.log.last().map(String::as_str), Some("status"), "a lost finish is found in the status answer's `done`: {:?}", s.log);
    assert!(events.iter().any(|e| matches!(e, Event::Reconnecting { .. })), "the person is told the connection dropped");
}

/// THE DAY'S ALLOWANCE IS SAID IN ITS OWN WORDS, AND THE UPLOAD CARRIES ON BY ITSELF AFTER THE
/// SERVER'S OWN WAIT: a piece refused 429 shows the server's words, nothing of it is written, and
/// once the wait has passed the upload carries on from where it was.
///
/// Seen red 2026-10-11 with the 429 handled as any other refusal: "its own words" (left: None; the
/// run ended Failed with the server's words).
#[test]
fn an_allowance_is_waited_out_in_its_own_words() {
    let fake = Fake::new(8192);
    fake.s().tricks = VecDeque::from([None, Some(Trick::Allowance(waits::now_ms() + 300, Some(0.3)))]);
    let (_f, plan, bytes) = file(20_000);
    // The least wait made short, so the test waits the server's 0.3 s and not 5 s.
    let ctl = Arc::new(Control::with_waits(Some(TOKEN.into()), Duration::from_millis(1), Duration::from_millis(1)));
    let events = run_on(fake.clone(), plan, ctl, |_, _| {});
    let waiting = events.iter().find_map(|e| if let Event::Waiting { words, .. } = e { Some(words.clone()) } else { None });
    assert_eq!(waiting.as_deref(), Some("Your role may upload 100 MB a day on this server."), "its own words: {events:?}");
    assert!(done_answer(&events).is_some(), "the allowance is waited out: {events:?}");
    assert_eq!(fake.s().data, bytes);
}

/// NO ROOM AT THE FINISH KEEPS THE UPLOAD, AND RETRY FINISHES IT WITHOUT SENDING IT AGAIN: a 507
/// at the finish shows its words, and Retry sends the empty piece at the end (the relay keeps the
/// file for a day), which finishes it.
///
/// Seen red 2026-10-11 with Retry asking for the file again from byte 0: "only the empty piece
/// follows the 507" (left: ["piece 0+4096", "piece 20000+0"], with pieces of 4096 that run).
#[test]
fn no_room_at_the_finish_is_retried_with_the_empty_piece() {
    let fake = Fake::new(8192);
    fake.s().finish_trick = Some(Trick::NoRoom);
    let (_f, plan, bytes) = file(20_000);
    let events = run_with(fake.clone(), plan, TOKEN, |ctl, e| {
        if matches!(e, Event::NoRoom { .. }) {
            ctl.retry();
        }
    });
    assert!(events.iter().any(|e| matches!(e, Event::NoRoom { words } if words.contains("kept here for a day"))), "the 507's words: {events:?}");
    assert!(done_answer(&events).is_some(), "Retry finished it: {events:?}");
    let s = fake.s();
    assert_eq!(s.data, bytes);
    let after: Vec<&String> = s.log.iter().skip_while(|l| l.as_str() != "piece 16384+3616").skip(1).filter(|l| l.starts_with("piece")).collect();
    assert_eq!(after, vec!["piece 20000+0"], "only the empty piece follows the 507: {:?}", s.log);
}

/// PAUSE STOPS THE PIECES AND RESUME CARRIES ON; CANCEL TELLS THE SERVER AND NOTHING FINISHES.
///
/// Seen red 2026-10-11 with Cancel ending the worker without telling the server: "the server was
/// told" (no "cancel" in its log).
#[test]
fn pause_resume_and_cancel() {
    // Paused before the first piece: none goes until Resume, then all of it.
    let fake = Fake::new(8192);
    let (_f, plan, bytes) = file(20_000);
    let paused_pieces = Arc::new(Mutex::new(None));
    let seen = paused_pieces.clone();
    let probe = fake.clone();
    let events = run_with(fake.clone(), plan, TOKEN, |ctl, e| match e {
        Event::Started { .. } => ctl.pause(true),
        Event::Paused => {
            std::thread::sleep(Duration::from_millis(50));
            *seen.lock().unwrap() = Some(probe.s().log.iter().filter(|l| l.starts_with("piece")).count());
            ctl.pause(false);
        }
        _ => {}
    });
    assert!(*paused_pieces.lock().unwrap() <= Some(1), "no piece while paused (at most the one already under way)");
    assert!(events.contains(&Event::Paused) && done_answer(&events).is_some(), "{events:?}");
    assert_eq!(fake.s().data, bytes);

    // Cancelled while paused: the server is told, and nothing is published.
    let fake = Fake::new(8192);
    let (_g, plan, _) = file(20_000);
    let events = run_with(fake.clone(), plan, TOKEN, |ctl, e| match e {
        Event::Started { .. } => ctl.pause(true),
        Event::Paused => ctl.cancel(),
        _ => {}
    });
    assert_eq!(events.last(), Some(&Event::Cancelled { told: Told::Gone }));
    assert!(done_answer(&events).is_none(), "nothing finishes");
    assert!(fake.s().cancelled && fake.s().log.contains(&"cancel".to_string()), "the server was told");
}

/// PAUSE STOPS EVERY REQUEST IN EVERY PHASE (round 3, finding 2): paused before anything is sent,
/// the server sees nothing, not even the start; paused while the start is refused (an allowance, a
/// server error), the start is not asked again until Resume. The review measured 1,008,232 starts
/// in half a second here: every wait returned at once while paused, and the start's loop never
/// looked at Pause.
///
/// Seen red 2026-10-11 with the `hold` calls in the start's loop replaced by round 2's cancelled
/// check: "no start while paused after a 429 (left: 41315 starts in the window / right: at most
/// 1)".
#[test]
fn pause_stops_every_request_in_every_phase() {
    // Paused from the first moment: nothing reaches the server.
    let fake = Fake::new(8192);
    let (_f, plan, _) = file(20_000);
    let ctl = Control::with_pace(Some(TOKEN.into()), Duration::from_millis(1));
    ctl.pause(true);
    let (ctl, rx, handle) = spawn_work(&fake, plan, ctl);
    std::thread::sleep(WINDOW);
    assert_eq!(fake.s().log.len(), 0, "nothing is sent while paused before the start: {:?}", fake.s().log);
    ctl.cancel();
    handle.join().unwrap();
    assert!(rx.try_iter().any(|e| matches!(e, Event::Cancelled { .. })));

    // Paused while the start is refused, by an allowance and by a server error.
    let refusals = [
        Reply { status: 429, body: json!({ "error": "Your role may start 5 uploads an hour.", "allowance": "hour", "resets_at": 0, "resets_in_s": 0 }).to_string() },
        Reply { status: 500, body: "storage: disk full".into() },
    ];
    for refusal in refusals {
        let fake = Fake::new(8192);
        fake.s().start_forever = Some(refusal.clone());
        let (_g, plan, _) = file(20_000);
        // The least allowance wait made a millisecond, so the old loop would spin at once.
        let (ctl, rx, handle) = spawn_work(&fake, plan, Control::with_waits(Some(TOKEN.into()), Duration::from_millis(1), Duration::from_millis(1)));
        wait_for(&rx, |e| matches!(e, Event::Waiting { .. } | Event::Reconnecting { .. }));
        ctl.pause(true);
        std::thread::sleep(Duration::from_millis(50)); // what was under way when Pause came
        let before = fake.count("start");
        std::thread::sleep(WINDOW);
        let during = fake.count("start") - before;
        assert!(during <= 1, "no start while paused after a {} (left: {during} starts in the window / right: at most 1)", refusal.status);
        ctl.pause(false);
        std::thread::sleep(Duration::from_millis(50));
        assert!(fake.count("start") > before, "Resume asks again");
        ctl.cancel();
        handle.join().unwrap();
    }
}

/// AN ALLOWANCE WAITS THE SERVER'S OWN NUMBER, AT LEAST FIVE SECONDS (round 3, finding 3): a 429
/// whose `resets_at` is not ahead of this PC's clock (a clock a little fast, or the round 2 relay
/// with no `resets_in_s`) was retried at once, in a loop with no wait.
///
/// Seen red 2026-10-11 with the wait taken from `resets_at` alone and no least wait (as in round
/// 2): "no piece in the window after the refusal (resets_in_s None ...)" (left: 5561 / right: 0).
#[test]
fn an_allowance_already_past_by_this_clock_still_waits() {
    for resets_in_s in [None, Some(0.0)] {
        let fake = Fake::new(8192);
        // Every piece refused the same way, with a reset a minute in the past by this clock.
        fake.s().allowance_forever = Some((waits::now_ms() - 60_000, resets_in_s));
        let (_f, plan, _) = file(20_000);
        let (ctl, rx, handle) = spawn_work(&fake, plan, Control::with_pace(Some(TOKEN.into()), Duration::from_millis(1)));
        let waiting = wait_for(&rx, |e| matches!(e, Event::Waiting { .. }));
        let before = fake.count("piece");
        std::thread::sleep(WINDOW);
        let during = fake.count("piece") - before;
        assert_eq!(during, 0, "no piece in the window after the refusal (resets_in_s {resets_in_s:?}; left: pieces sent / right: none)");
        let Event::Waiting { resets_at, .. } = waiting else { unreachable!() };
        assert!(resets_at >= waits::now_ms() + 4_000, "the page is told it waits at least five seconds");
        ctl.cancel();
        handle.join().unwrap();
    }
}

/// AN ALLOWANCE SMALLER THAN A PIECE STILL MOVES (round 3, finding 5): the app sent whole
/// `chunk_max` pieces, so a day with 3,000 bytes left refused every one and the upload never
/// moved. Now a refused piece is cut to the server's `left_bytes` (R3-6), and the rest follows
/// once the window resets.
///
/// Seen red 2026-10-11 with the piece never cut (as in round 2): "a piece cut to what is left" (the
/// log was ["start", "piece 0+8192", "piece 0+8192", ... 52 of them, "cancel"]: the test cancelled
/// it after 50 waits).
#[test]
fn an_allowance_smaller_than_a_piece_still_moves() {
    let fake = Fake::new(8192);
    fake.s().allowance_left = Some(3000);
    fake.s().reset_at_zero = true;
    let (_f, plan, bytes) = file(20_000);
    let ctl = Arc::new(Control::with_waits(Some(TOKEN.into()), Duration::from_millis(1), Duration::from_millis(1)));
    let mut waits_seen = 0;
    let events = run_on(fake.clone(), plan, ctl, |ctl, e| {
        if matches!(e, Event::Waiting { .. }) {
            waits_seen += 1;
            if waits_seen > 50 {
                ctl.cancel(); // never moving: stop, and let the assertions say why
            }
        }
    });
    let log = fake.s().log.clone();
    assert!(log.contains(&"piece 0+3000".to_string()), "a piece cut to what is left: {log:?}");
    assert!(done_answer(&events).is_some(), "and the rest after the reset: {events:?}");
    assert_eq!(fake.s().data, bytes);
}

/// A SERVER ERROR THAT NEVER CLEARS STOPS AFTER TEN, WITH ITS OWN WORDS AND RETRY (round 3,
/// finding 5): a full disk's 500 was resent forever and shown as a dropped connection. Now each
/// is waited out a little longer, said as a server problem, and the tenth of the same in a row
/// shows the server's words and waits for Retry, sending nothing meanwhile.
///
/// Seen red 2026-10-11 with the count of the same error switched off (as in round 2): "the event
/// never came; events so far: 321 of them" (ten seconds of starts, never stopping).
#[test]
fn a_server_error_that_never_clears_waits_for_retry() {
    let fake = Fake::new(8192);
    fake.s().start_forever = Some(Reply { status: 500, body: "storage: disk full".into() });
    let (_f, plan, _) = file(20_000);
    let (ctl, rx, handle) = spawn_work(&fake, plan, Control::with_pace(Some(TOKEN.into()), Duration::from_millis(1)));
    let said = until(&rx, |e| matches!(e, Event::Stuck { .. }));
    assert_eq!(said.last(), Some(&Event::Stuck { words: "storage: disk full".into() }), "the server's own words");
    assert!(!said.iter().any(|e| matches!(e, Event::Reconnecting { words } if words == RECONNECTING)), "never called a dropped connection: {said:?}");
    assert!(said.contains(&Event::Reconnecting { words: waits::SERVER_TROUBLE.into() }), "a server problem, said as one");
    assert_eq!(fake.count("start"), waits::STUCK_AFTER as usize, "ten of the same, then it stops");
    std::thread::sleep(WINDOW);
    assert_eq!(fake.count("start"), waits::STUCK_AFTER as usize, "nothing is sent while it waits for Retry");
    ctl.retry();
    wait_for(&rx, |e| matches!(e, Event::Stuck { .. }));
    assert_eq!(fake.count("start"), 2 * waits::STUCK_AFTER as usize, "Retry tries again");
    ctl.cancel();
    handle.join().unwrap();
}

/// A 429 WITHOUT THE ALLOWANCE'S JSON, OR A 5XX, IS WAITED OUT, NEVER THE END OF THE UPLOAD
/// (round 3, both apps): nginx's rate limit (an HTML page) and the relay's "You have 3 uploads in
/// progress" are waited out with back-off, saying so, and the upload goes on.
///
/// Seen red 2026-10-11 with a 429 at the start that had no `resets_at` ending the upload (as in
/// round 2): "it carried on after the 429s: [Hashing { done: 10000 }, Failed { words: \"The server
/// refused it (429).\", gone: true }]".
#[test]
fn a_429_without_an_allowance_is_waited_out() {
    let fake = Fake::new(8192);
    fake.s().start_answers = VecDeque::from([
        Reply { status: 429, body: "<html><body>429 Too Many Requests</body></html>".into() },
        Reply { status: 429, body: "You have 3 uploads in progress on this server, the most at once. Finish or cancel one first.".into() },
        Reply { status: 503, body: String::new() },
    ]);
    let (_f, plan, bytes) = file(10_000);
    let events = run_with(fake.clone(), plan, TOKEN, |_, _| {});
    assert!(done_answer(&events).is_some(), "it carried on after the 429s: {events:?}");
    assert!(events.contains(&Event::Reconnecting { words: waits::SERVER_BUSY.into() }), "nginx's page is not shown: {events:?}");
    assert!(events.iter().any(|e| matches!(e, Event::Reconnecting { words } if words.starts_with("You have 3 uploads"))), "the relay's own words are");
    assert_eq!(fake.s().data, bytes);
}

/// A REPLACED TOKEN WAITS FOR THE NEW ONE: after a reconnect the server refuses the old token;
/// the upload waits, says so, and carries on with the page's new one.
///
/// Seen red 2026-10-11 with a refused token treated as a refusal of the upload: "it carried on with
/// the new token" (the run ended Failed with "Invalid upload token.").
#[test]
fn a_replaced_token_waits_for_the_new_one() {
    let fake = Fake::new(8192);
    let (_f, plan, bytes) = file(10_000);
    let events = run_with(fake.clone(), plan, "tok-before-reconnect", |ctl, e| {
        if matches!(e, Event::Reconnecting { words } if words == WAITING_FOR_SIGN_IN) {
            ctl.set_token(Some(TOKEN.to_string()));
        }
    });
    assert!(done_answer(&events).is_some(), "it carried on with the new token: {events:?}");
    assert_eq!(fake.s().data, bytes);
}

/// A RESUME AFTER A RESTART STARTS NOTHING NEW: with the kept id it asks the server where it got
/// and sends only the rest; and it is offered only for the same file, unchanged.
///
/// Seen red 2026-10-11 with `file_unchanged` comparing the size alone: "a file rewritten to the
/// same size is not resumed" (it said unchanged).
#[test]
fn a_resume_after_a_restart_sends_only_the_rest() {
    let fake = Fake::new(8192);
    let (_f, mut plan, bytes) = file(20_000);
    {
        let mut s = fake.s();
        s.id = Some("feedfacefeedfacefeedfacefeedface".into());
        s.size = 20_000;
        s.blake3 = blake3::hash(&bytes).to_hex().to_string();
        s.data = bytes[..8192].to_vec(); // what arrived before the restart
    }
    plan.resume = Some("feedfacefeedfacefeedfacefeedface".into());
    let events = run_with(fake.clone(), plan.clone(), TOKEN, |_, _| {});
    assert!(done_answer(&events).is_some(), "{events:?}");
    let s = fake.s();
    assert_eq!(s.data, bytes);
    assert!(!s.log.contains(&"start".to_string()), "nothing new started: {:?}", s.log);
    assert_eq!(s.log.first().map(String::as_str), Some("status"), "it asked where the server got first");
    drop(s);

    let kept = Unfinished { path: plan.path.clone(), size: 20_000, modified_ms: modified_ms(&plan.path), ..Default::default() };
    assert!(kept.file_unchanged(), "the same file, unchanged");
    std::thread::sleep(Duration::from_millis(20));
    let mut other = bytes.clone();
    other[0] ^= 0xff;
    std::fs::write(&plan.path, &other).unwrap();
    // Some file systems keep coarse times: make the change visible to them as well.
    let later = std::time::SystemTime::now() + Duration::from_secs(5);
    let _ = std::fs::File::options().write(true).open(&plan.path).and_then(|f| f.set_modified(later));
    assert!(!kept.file_unchanged(), "a file rewritten to the same size is not resumed");
}

/// A FILE THAT CAN NO LONGER BE READ ENDS THE UPLOAD, SAYING SO, and the server is told.
///
/// Seen red 2026-10-11 with round 2's words put back: "the words say the file could not be read:
/// The file could not be read: it may have changed (failed to fill whole buffer)." (they did not
/// say the upload stopped).
///
/// Round 4 holds the upload in an allowance's wait while the file is cut short: cut at `Started`,
/// the simulated server could take every piece before the test wrote the file, and the upload
/// then finished (seen once, 2026-10-11, "it ended: [..., Done { .. }]").
#[test]
fn a_file_that_can_no_longer_be_read_ends_the_upload() {
    let fake = Fake::new(8192);
    fake.s().tricks = VecDeque::from([Some(Trick::Allowance(waits::now_ms() + 3_600_000, Some(3600.0)))]);
    let (f, plan, _) = file(20_000);
    let events = run_with(fake.clone(), plan, TOKEN, |ctl, e| {
        if matches!(e, Event::Waiting { .. }) {
            std::fs::write(&f, b"cut short").unwrap();
            ctl.retry();
        }
    });
    let failed = events.iter().find_map(|e| if let Event::Failed { words, gone } = e { Some((words.clone(), *gone)) } else { None });
    let (words, gone) = failed.unwrap_or_else(|| panic!("it ended: {events:?}"));
    assert!(words.contains("could not be read any more"), "the words say the file could not be read: {words}");
    assert!(gone && fake.s().log.contains(&"cancel".to_string()), "and the server is told");
}

/// THE TOKEN RIDES IN A HEADER ON THE RESUMABLE ROUTES (R3-9), never in an address, which ends up
/// in error words, logs and lists.
///
/// Seen red 2026-10-11 with the status request putting `token=` in its address as in round 2: "no
/// token in the address: get /api/upload/resumable/abc?token=secret-r39 http/1.1".
#[test]
fn the_token_rides_in_a_header_on_the_resumable_routes() {
    use std::io::{Read as _, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        while !got.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        let body = r#"{"offset":0,"size":1,"chunk_max":8}"#;
        let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        String::from_utf8_lossy(&got).to_ascii_lowercase()
    });
    let r = Http::new(&format!("http://127.0.0.1:{port}")).status("SECRET-R39", "abc").expect("answered");
    assert_eq!(r.status, 200);
    let head = server.join().unwrap();
    assert!(head.starts_with("get /api/upload/resumable/abc http/1.1"), "no token in the address: {head}");
    assert!(head.contains("x-upload-token: secret-r39"), "it rides in the header: {head}");
}

/// THE UNFINISHED UPLOADS SURVIVE A RESTART: kept in the app's config, read back the same.
///
/// Seen red 2026-10-11 with `apply_to_gui_state` not reading them back: "the unfinished uploads
/// survive a restart" (left: []).
#[test]
fn the_unfinished_uploads_survive_a_restart() {
    let mut state = crate::gui::GuiState::default();
    let kept = Unfinished { server: "https://h".into(), key: "me".into(), id: "abc".into(), path: "C:/x/clip.webm".into(), name: "clip.webm".into(), size: 9, modified_ms: 5, title: "Dusk".into(), ..Default::default() };
    let other = Unfinished { id: "def".into(), name: "b.webm".into(), ..kept.clone() };
    state.media.unfinished = vec![kept.clone(), other.clone()];
    let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).expect("config serializes");
    let back: crate::config::AppConfig = serde_json::from_str(&json).expect("and reads back");
    let mut fresh = crate::gui::GuiState::default();
    back.apply_to_gui_state(&mut fresh);
    assert_eq!(fresh.media.unfinished, vec![kept, other], "the unfinished uploads survive a restart");
    let old: crate::config::AppConfig = serde_json::from_str("{}").unwrap();
    assert!(old.media_unfinished_uploads.is_empty(), "an older config has none");
}
