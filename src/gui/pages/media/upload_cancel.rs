//! Cancelling an upload on the server (round 4 of docs/design/media-showcase.md, "a cancel means
//! nothing stays", and the review's D4-2). `DELETE /api/upload/resumable/<id>`, the token in the
//! `X-Upload-Token` header, answers:
//! - `{ok: true}`: an unfinished upload, dropped with every piece that arrived;
//! - `{ok: true, removed_stored: true}`: a finished upload not yet on the Media page, its stored
//!   file removed too (so a finish whose answer was lost, or the copy a 507 keeps, goes as well);
//! - `{ok: true, published: true}`: its file is on the Media page already, and stays there
//!   (putting it there was the person's own choice): Remove on its tile takes it off;
//! - 409 `{busy: true}` while a piece or the finish is being written: asked again, a little later
//!   each time, at most [`BUSY_ASKS`] times;
//! - 404: the server has no such upload (one left alone for a day is dropped, or it was cancelled
//!   already), so nothing of it is there.
//!
//! Round 3 took ANY answer as done, a 409 included, and the page then said "Cancelled: nothing of
//! it stays on the server." Now that is said only when the server confirmed it ([`Told::Gone`]);
//! anything else is said plainly ([`not_cancelled_words`]) and the upload stays listed with its
//! Cancel, so the person can try again.

use std::time::Duration;

use serde_json::Value;

use super::resumable::{Reply, Transport};

/// What the server said to a cancel.
#[derive(Debug, Clone, PartialEq)]
pub enum Told {
    /// Confirmed: nothing of the upload stays on the server (or there was nothing of it there).
    Gone,
    /// Its file is on the Media page already, and stays there.
    Published,
    /// Not confirmed: why, in words with no token in them.
    NotConfirmed(String),
}

/// How many times a cancel is asked while the server says it is busy writing: about fifteen
/// minutes at the app's pace (1 s doubling to 30 s), what nginx allows a finish.
pub const BUSY_ASKS: u32 = 35;
/// How many times a cancel is asked when no answer comes at all, before saying so.
pub const LOST_ASKS: u32 = 3;
/// The first wait between asks in the app (each one after doubles, up to thirty times it).
pub const CANCEL_PACE: Duration = Duration::from_secs(1);

/// Said when the server was still busy writing the file every time it was asked.
pub const STILL_STORING: &str = "the server was still storing the file each time it was asked.";
/// Said when there is no upload token for this server to send the cancel with.
pub const NOT_SIGNED_IN: &str = "this server has not signed you in on this connection yet.";
/// What is said when the upload's file was already on the Media page.
pub const ALREADY_ON_MEDIA: &str = "It was already on the Media page, so it stays there: Remove on its tile takes it off.";

/// What one answer to a cancel means: Some when it settles the cancel, None when it is asked again
/// (busy, a server error, a 429).
pub fn settle(r: &Reply) -> Option<Told> {
    match r.status {
        200..=299 => Some(if r.json().get("published").and_then(Value::as_bool) == Some(true) { Told::Published } else { Told::Gone }),
        404 => Some(Told::Gone),
        _ if r.token_refused() => Some(Told::NotConfirmed(NOT_SIGNED_IN.to_string())),
        409 | 429 | 500..=599 => None,
        _ => Some(Told::NotConfirmed(r.words())),
    }
}

/// Ask the server at `t` to drop upload `id` with `token`, again while it is busy writing (waiting
/// `pace`, then twice that, up to thirty times it) and when no answer came, until it settles.
/// Blocking: run it on a worker thread.
pub fn tell(t: &dyn Transport, token: &str, id: &str, pace: Duration) -> Told {
    let mut wait = pace;
    let (mut busy, mut lost) = (0u32, 0u32);
    loop {
        match t.cancel(token, id) {
            Ok(r) => match settle(&r) {
                Some(told) => return told,
                None => {
                    busy += 1;
                    if busy >= BUSY_ASKS {
                        return Told::NotConfirmed(if r.status == 409 { STILL_STORING.to_string() } else { r.words() });
                    }
                }
            },
            Err(words) => {
                lost += 1;
                if lost >= LOST_ASKS {
                    return Told::NotConfirmed(words);
                }
            }
        }
        std::thread::sleep(wait);
        wait = (wait * 2).min(pace * 30);
    }
}

/// What is said when a cancel was not confirmed: why, and what happens to the upload meanwhile.
pub fn not_cancelled_words(why: &str) -> String {
    let why = why.trim();
    let why = if why.ends_with('.') { why.to_string() } else { format!("{why}.") };
    format!("Not cancelled on the server yet: {why} What arrived stays there until a cancel reaches it, or for a day after its last piece. Press Cancel again to try.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// A server that answers each cancel with the next of its answers, then the last forever.
    struct Answers(Mutex<(VecDeque<Result<Reply, String>>, usize)>);

    impl Answers {
        fn new(answers: Vec<Result<Reply, String>>) -> Self {
            Answers(Mutex::new((answers.into(), 0)))
        }
        fn asks(&self) -> usize {
            self.0.lock().unwrap().1
        }
    }

    impl Transport for Answers {
        fn start(&self, _: &str, _: &Value) -> Result<Reply, String> {
            unreachable!()
        }
        fn status(&self, _: &str, _: &str) -> Result<Reply, String> {
            unreachable!()
        }
        fn piece(&self, _: &str, _: &str, _: u64, _: &[u8]) -> Result<Reply, String> {
            unreachable!()
        }
        fn cancel(&self, _: &str, _: &str) -> Result<Reply, String> {
            let mut s = self.0.lock().unwrap();
            s.1 += 1;
            if s.0.len() > 1 {
                s.0.pop_front().unwrap()
            } else {
                s.0.front().cloned().unwrap()
            }
        }
    }

    fn reply(status: u16, body: Value) -> Result<Reply, String> {
        Ok(Reply { status, body: body.to_string() })
    }

    fn busy() -> Result<Reply, String> {
        reply(409, json!({ "error": "Another piece of this upload is being written.", "busy": true }))
    }

    /// THE NEW CANCEL ANSWER, READ AS THE SPEC STATES IT (round 4): `removed_stored` and a plain
    /// `ok` are done, `published` is done with the item kept, a 404 is nothing there; a 409 busy is
    /// asked again until it settles, and one that never settles stops at the ceiling saying the
    /// server was still storing it; no answer at all is said plainly after three tries; a refusal
    /// is said in the server's words.
    ///
    /// Seen red 2026-10-11 with `settle` taking any answer as done (round 3's cancel): "a 409 busy
    /// is asked again" (left: 1 ask / right: 3).
    #[test]
    fn the_cancel_answer_is_read_and_busy_is_asked_again() {
        let pace = Duration::from_millis(1);
        let gone = Answers::new(vec![busy(), busy(), reply(200, json!({ "ok": true, "removed_stored": true }))]);
        assert_eq!(tell(&gone, "t", "id", pace), Told::Gone);
        assert_eq!(gone.asks(), 3, "a 409 busy is asked again (left: asks / right: 3)");
        assert_eq!(tell(&Answers::new(vec![reply(200, json!({ "ok": true }))]), "t", "id", pace), Told::Gone);
        assert_eq!(tell(&Answers::new(vec![reply(200, json!({ "ok": true, "published": true }))]), "t", "id", pace), Told::Published);
        assert_eq!(tell(&Answers::new(vec![reply(404, json!({ "error": "There is no upload in progress with that id." }))]), "t", "id", pace), Told::Gone);

        let always = Answers::new(vec![busy()]);
        assert_eq!(tell(&always, "t", "id", pace), Told::NotConfirmed(STILL_STORING.to_string()), "busy for ever stops at the ceiling");
        assert_eq!(always.asks(), BUSY_ASKS as usize);

        let lost = Answers::new(vec![Err("The server could not be reached (connection refused).".into())]);
        assert_eq!(tell(&lost, "t", "id", pace), Told::NotConfirmed("The server could not be reached (connection refused).".into()));
        assert_eq!(lost.asks(), LOST_ASKS as usize);

        let refused = Answers::new(vec![reply(403, json!({ "error": "You are not a member of this server." }))]);
        assert_eq!(tell(&refused, "t", "id", pace), Told::NotConfirmed("You are not a member of this server.".into()));
        assert_eq!(refused.asks(), 1, "a refusal is not asked again");
        let words = not_cancelled_words("the server could not be reached");
        assert!(words.starts_with("Not cancelled on the server yet: the server could not be reached. What arrived"), "{words}");
    }
}
