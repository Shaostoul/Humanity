//! The two channels a system speaks to the person on (BUG-258, 2026-11-06). A line is either the
//! WORLD speaking on its own (a bed flowering, the feeder running dry, a craft finishing, "while
//! you were away"), or a REPLY to something the person just did on a page (a craft refused at an
//! unpowered station, a Plant press with no seed, a drone's service short of parts). The system that
//! writes the line says which it is, by the channel it writes to; nothing downstream reads the
//! words to tell them apart.
//!
//! Why two: before the person enters a world, the world does not run or speak (BUG-253, the
//! engine's gui/world_entry.rs), so its lines wait for Enter. A reply is not the world speaking:
//! held, a click on a page got no answer at all. The engine (engine/world_hold.rs `take_notices`)
//! shows a reply at once and holds a world line until the person enters.
//!
//! Both are a `Mutex<Vec<String>>` in the `DataStore`, drained once a frame by the engine;
//! [`register`] puts both in. A system with no store slot (a headless test that did not register
//! them) says nothing, as before.

use std::sync::Mutex;

use humanity_core::hot_reload::data_store::DataStore;

/// The world's own lines: held until the person enters a world.
pub const WORLD: &str = "player_notices";
/// Replies to what the person just did on a page: shown at once, entered or not.
pub const REPLIES: &str = "player_replies";

/// Put both channels in the store (the engine's start, and a test's store).
pub fn register(data: &mut DataStore) {
    data.insert(WORLD, Mutex::new(Vec::<String>::new()));
    data.insert(REPLIES, Mutex::new(Vec::<String>::new()));
}

/// A line from the world, said on its own.
pub fn world(data: &DataStore, line: String) {
    push(data, WORLD, line);
}

/// A reply to what the person just did on a page (a request a page sent this frame).
pub fn reply(data: &DataStore, line: String) {
    push(data, REPLIES, line);
}

/// Take what was said on `channel` since the last take.
pub fn take(data: &DataStore, channel: &str) -> Vec<String> {
    data.get::<Mutex<Vec<String>>>(channel).and_then(|slot| slot.lock().ok().map(|mut n| std::mem::take(&mut *n))).unwrap_or_default()
}

fn push(data: &DataStore, channel: &str, line: String) {
    if let Some(Ok(mut n)) = data.get::<Mutex<Vec<String>>>(channel).map(|m| m.lock()) {
        n.push(line);
    }
}

/// Everything said on both channels so far, the world's lines first (tests read what a system
/// said without caring which channel it went on).
#[cfg(any(test, feature = "test-support"))]
pub fn said(data: &DataStore) -> Vec<String> {
    let read = |c: &str| data.get::<Mutex<Vec<String>>>(c).map(|m| m.lock().unwrap().clone()).unwrap_or_default();
    let mut all = read(WORLD);
    all.extend(read(REPLIES));
    all
}

/// [`said`], taken: both channels emptied.
#[cfg(any(test, feature = "test-support"))]
pub fn take_said(data: &DataStore) -> Vec<String> {
    let mut all = take(data, WORLD);
    all.extend(take(data, REPLIES));
    all
}
