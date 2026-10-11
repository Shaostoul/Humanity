//! Tests of Add to Media's upload in pieces (resumable.rs), against a server simulated in memory
//! that answers the way src/relay/resumable.rs does: the same routes' shapes, statuses and
//! refusals. Each was seen to fail once before it passed; what it saw red is in its comment.

use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The token the simulated server takes.
pub(crate) const TOKEN: &str = "tok-now";

/// What the simulated server does to a request, scripted by a test.
#[derive(Debug, Clone, PartialEq)]
enum Trick {
    /// Write the piece, then lose its answer (the connection dropped after the server took it).
    LoseAnswer,
    /// Refuse the piece for the day's allowance, resetting at this time (Unix ms).
    Allowance(i64),
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
    done: Option<Value>,
    chunk_max: u64,
    /// Tricks for the non-empty pieces in order (None: answer normally), and for the finish.
    tricks: VecDeque<Option<Trick>>,
    finish_trick: Option<Trick>,
    pub(crate) log: Vec<String>,
    pub(crate) cancelled: bool,
    start_body: Value,
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
        // The relay's own StartBody reads what is sent.
        let b: crate::relay::resumable::StartBody = serde_json::from_value(body.clone()).expect("the relay reads the start");
        assert_eq!(b.media, Some(1), "a resumable upload is for the Media page");
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
        if !bytes.is_empty() {
            let trick = s.tricks.pop_front().flatten();
            if let Some(Trick::Allowance(resets_at)) = trick {
                let (at, size) = (s.data.len(), s.size);
                return refuse(429, json!({ "error": "Your role may upload 100 MB a day on this server.", "allowance": "day", "resets_at": resets_at, "offset": at, "size": size }).to_string());
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
            return refuse(507, "This server's Media space is full. The file you sent is kept here for a day, so once there is room it can be finished without sending it again.");
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

    fn cancel(&self, token: &str, _id: &str) -> Result<Reply, String> {
        let mut s = self.s();
        s.log.push("cancel".into());
        if token != s.token {
            return refuse(403, "Invalid upload token.");
        }
        s.cancelled = true;
        s.id = None;
        ok(json!({ "ok": true }))
    }
}

static FILES: AtomicUsize = AtomicUsize::new(0);

/// A file of `len` patterned bytes, and the plan to upload it.
fn file(len: usize) -> (crate::test_temp::TempPath, Plan, Vec<u8>) {
    let n = FILES.fetch_add(1, Ordering::SeqCst);
    let path = crate::test_temp::file(&format!("media_resumable_{n}"), "webm");
    let bytes: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let plan = Plan { path: path.to_path_buf(), name: "clip.webm".into(), size: len as u64, content_type: "video/webm".into(), resume: None };
    (path, plan, bytes)
}

/// Run the upload on a worker thread, with `before` given the control and the events as they come
/// (to press Pause, Resume, Retry or Cancel), until it ends. Returns every event.
fn run_with(fake: Arc<Fake>, plan: Plan, token: &str, mut before: impl FnMut(&Control, &Event)) -> Vec<Event> {
    let ctl = Arc::new(Control::with_pace(Some(token.to_string()), Duration::from_millis(1)));
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_ctl = ctl.clone();
    let t: Arc<dyn Transport> = fake;
    let handle = std::thread::spawn(move || work(t.as_ref(), &worker_ctl, &plan, &tx));
    let mut events = Vec::new();
    loop {
        let e = rx.recv_timeout(Duration::from_secs(20)).unwrap_or_else(|_| panic!("the upload stalled; events so far: {events:?}"));
        before(&ctl, &e);
        let last = matches!(e, Event::Done { .. } | Event::Failed { .. } | Event::Cancelled);
        events.push(e);
        if last {
            break;
        }
    }
    handle.join().unwrap();
    events
}

fn done_answer(events: &[Event]) -> Option<Value> {
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

/// THE DAY'S ALLOWANCE IS SAID IN ITS OWN WORDS, WITH WHEN IT RESETS, AND THE UPLOAD CARRIES ON
/// THEN: a piece refused 429 shows the server's words and its `resets_at`, nothing of it is
/// written, and once that time has passed the upload carries on by itself from where it was.
///
/// Seen red 2026-10-11 with the 429 handled as any other refusal: "its own words and when it
/// resets" (left: None; the run ended Failed with the server's words).
#[test]
fn an_allowance_is_waited_out_in_its_own_words() {
    let fake = Fake::new(8192);
    let soon = now_ms() + 300;
    fake.s().tricks = VecDeque::from([None, Some(Trick::Allowance(soon))]);
    let (_f, plan, bytes) = file(20_000);
    let events = run_with(fake.clone(), plan, TOKEN, |_, _| {});
    let waiting = events.iter().find_map(|e| if let Event::Waiting { words, resets_at } = e { Some((words.clone(), *resets_at)) } else { None });
    assert_eq!(waiting, Some(("Your role may upload 100 MB a day on this server.".to_string(), soon)), "its own words and when it resets: {events:?}");
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
    assert_eq!(events.last(), Some(&Event::Cancelled));
    assert!(done_answer(&events).is_none(), "nothing finishes");
    assert!(fake.s().cancelled && fake.s().log.contains(&"cancel".to_string()), "the server was told");
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

/// THE UNFINISHED UPLOAD SURVIVES A RESTART: kept in the app's config, read back the same.
///
/// Seen red 2026-10-11 with `apply_to_gui_state` not reading it back: "the unfinished upload
/// survives a restart" (left: None).
#[test]
fn the_unfinished_upload_survives_a_restart() {
    let mut state = crate::gui::GuiState::default();
    let kept = Unfinished { server: "https://h".into(), key: "me".into(), id: "abc".into(), path: "C:/x/clip.webm".into(), name: "clip.webm".into(), size: 9, modified_ms: 5, title: "Dusk".into(), ..Default::default() };
    state.media.unfinished = Some(kept.clone());
    let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).expect("config serializes");
    let back: crate::config::AppConfig = serde_json::from_str(&json).expect("and reads back");
    let mut fresh = crate::gui::GuiState::default();
    back.apply_to_gui_state(&mut fresh);
    assert_eq!(fresh.media.unfinished, Some(kept), "the unfinished upload survives a restart");
    let old: crate::config::AppConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(old.media_unfinished_upload, None, "an older config has none");
}
