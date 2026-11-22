//! ONE LIST OF MUSIC (docs/design/music-page.md section 28, version 3; the operator, 2026-11-21: "We
//! need a playlist of music. We have the premade music and we have procedural music which are kind of
//! the same but, different." and "I think one list would be better.").
//!
//! Everything that can play is an [`Item`] of one of three kinds: a WRITTEN piece (a score, or a MIDI
//! file the person imported), a COMPOSED piece (a genre, mood, seed and tuning with a length: one fixed
//! piece, any bar of which the composer computes from its seed) and a STATION (a genre and a mood
//! composed without end, a new piece every "A new piece" minutes, like radio). The [`Queue`] holds the
//! item playing, Up next (any mix of the three), the station that plays when Up next runs out, the
//! history Previous walks back through, shuffle and repeat. Next and Previous behave the same for
//! every kind.
//!
//! HOW IT REACHES THE MUSIC: the item playing is written into the person's settings
//! ([`Queue::apply`]: `MusicSettings::written`, `genre`, `mood`, `seed`, `tune`, `minutes`), the one
//! state the score reads every frame (section 4). The queue also OBSERVES the settings every frame
//! ([`Queue::observe`]), so any control that writes them (a genre or mood card, the seed row, Tune, a
//! written piece's Play) is adopted as "play now" without a second path to keep in step: a new seed
//! is a composed piece, a new genre or mood with the same seed is a station, a written id is a written
//! piece, a tuning change retunes the item playing.
//!
//! It is the person's own list, kept in `music/queue.json` beside their favourites (written through a
//! temporary file when it changes, read once when the music first runs), never a cache: nothing here is
//! made again from anything else.

use std::path::{Path, PathBuf};

use humanity_music::favourite::SavedTune;
use humanity_music::MusicSettings;
use serde::{Deserialize, Serialize};

use crate::gui::pages::dev_music::MusicReadout;

/// The most items history keeps (Previous walks back through them).
pub const HISTORY_MOST: usize = 50;

/// Something that can play.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    /// A written piece: its id in `data/music/scores/index.ron`, or an imported file's (`own:`).
    Written { id: String },
    /// One composed piece: fixed and reproducible, `minutes` long.
    Composed {
        genre: String,
        mood: String,
        seed: u64,
        #[serde(default)]
        tune: SavedTune,
        minutes: f32,
    },
    /// A station: the genre and mood composed without end; `seed` is the piece it plays now.
    Station {
        genre: String,
        mood: String,
        seed: u64,
        #[serde(default)]
        tune: SavedTune,
    },
}

impl Item {
    /// The words key naming its kind (`music_page.item_kind_*`).
    pub fn kind_key(&self) -> &'static str {
        match self {
            Item::Written { .. } => "music_page.item_kind_written",
            Item::Composed { .. } => "music_page.item_kind_composed",
            Item::Station { .. } => "music_page.item_kind_station",
        }
    }

    /// Whether two items are the same piece of music to a person (a station by its genre and mood,
    /// whatever piece it is on; a composed piece by its genre, mood and seed, whatever its tuning).
    pub fn same_as(&self, other: &Item) -> bool {
        match (self, other) {
            (Item::Written { id: a }, Item::Written { id: b }) => a == b,
            (Item::Composed { genre: g1, mood: m1, seed: s1, .. }, Item::Composed { genre: g2, mood: m2, seed: s2, .. }) => g1 == g2 && m1 == m2 && s1 == s2,
            (Item::Station { genre: g1, mood: m1, .. }, Item::Station { genre: g2, mood: m2, .. }) => g1 == g2 && m1 == m2,
            _ => false,
        }
    }

    /// The station's piece playing now, as the composed piece it is (`minutes` long), for history:
    /// Previous then brings back exactly what was heard. Any other item is itself.
    fn as_heard(&self, station_minutes: f32, endless: f32) -> Item {
        match self {
            Item::Station { genre, mood, seed, tune } => Item::Composed { genre: genre.clone(), mood: mood.clone(), seed: *seed, tune: tune.clone(), minutes: if station_minutes > 0.0 { station_minutes } else { endless } },
            other => other.clone(),
        }
    }
}

/// Repeat: off (Up next drains, then the station), all (the list comes round again), one (the item
/// playing plays again when it ends).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    /// The repeat button's next setting: off, all, one, off.
    pub fn next(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// What the settings said last frame (so a change is seen once).
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    written: String,
    game_chooses: bool,
    genre: String,
    mood: String,
    seed: u64,
    tune: SavedTune,
}

impl Seen {
    fn of(s: &MusicSettings) -> Seen {
        Seen { written: s.written.clone(), game_chooses: s.game_chooses, genre: s.genre.clone(), mood: s.mood.clone(), seed: s.seed, tune: s.tune.clone() }
    }
}

/// How the item playing was last seen ending, so an end acts once.
#[derive(Clone, Debug, PartialEq)]
enum Mark {
    /// A written piece and the times through it had finished when first seen.
    Loops(String, u64),
    /// A composed piece (its seed) already past its length.
    Ended(u64),
}

/// The queue (see the module's words).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Queue {
    /// The item playing (None before anything was chosen).
    pub now: Option<Item>,
    /// The items after it, in order.
    pub up_next: Vec<Item>,
    /// The station that plays when Up next runs out (None: the station of the genre and mood last
    /// chosen).
    pub station: Option<Item>,
    /// The items played, oldest first, at most [`HISTORY_MOST`].
    pub history: Vec<Item>,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// The shuffle's own seed (each draw moves it on), so a shuffled order can be reproduced.
    pub rng: u64,
    #[serde(skip)]
    seen: Option<Seen>,
    #[serde(skip)]
    mark: Option<Mark>,
}

/// What a frame of the queue did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    /// The queue changed (its file is written again).
    pub changed: bool,
    /// The person's settings changed (they are saved, and the score plays the new item).
    pub settings: bool,
    /// The item playing starts again from its first bar (repeat one on a composed piece).
    pub restart: bool,
}

/// A composed piece's length when it is made (section 28.13, question 3): "A new piece" minutes, or
/// the endless round (`endless`, minutes) when that is Never.
pub fn composed_minutes(s: &MusicSettings, endless: f32) -> f32 {
    if s.piece_minutes > 0.0 {
        s.piece_minutes
    } else {
        endless.max(1.0)
    }
}

/// How long a station's pieces are: "A new piece" minutes (0: one piece that plays on).
pub fn station_minutes(s: &MusicSettings) -> f32 {
    s.piece_minutes.max(0.0)
}

/// The station's next piece: a new seed from the last, easy to read out (under a million).
pub fn next_station_seed(seed: u64) -> u64 {
    humanity_music::piece::mix(seed, 0x57a7_1011) % super::RANDOM_SEED_MAX + 1
}

impl Queue {
    /// Write `item` into the person's settings, so the score plays it from the next bar.
    pub fn apply(&mut self, item: &Item, s: &mut MusicSettings) {
        match item {
            Item::Written { id } => s.written = id.clone(),
            Item::Composed { genre, mood, seed, tune, minutes } => {
                s.written.clear();
                s.game_chooses = false;
                s.genre = genre.clone();
                s.mood = mood.clone();
                s.seed = (*seed).max(1);
                s.tune = tune.clone();
                s.minutes = minutes.max(0.0);
            }
            Item::Station { genre, mood, seed, tune } => {
                s.written.clear();
                s.game_chooses = false;
                s.genre = genre.clone();
                s.mood = mood.clone();
                s.seed = (*seed).max(1);
                s.tune = tune.clone();
                s.minutes = station_minutes(s);
            }
        }
        self.seen = Some(Seen::of(s));
        self.mark = None;
    }

    /// Note an item that has played, newest last, at most [`HISTORY_MOST`].
    fn remember(&mut self, item: Item, s: &MusicSettings, endless: f32) {
        self.history.push(item.as_heard(station_minutes(s), endless));
        let over = self.history.len().saturating_sub(HISTORY_MOST);
        self.history.drain(..over);
    }

    /// The station at the end of the list: the one set, else the station of the genre and mood last
    /// chosen.
    pub fn station_or_default(&self, s: &MusicSettings) -> Item {
        self.station.clone().unwrap_or_else(|| Item::Station { genre: s.genre.clone(), mood: if s.mood.is_empty() { humanity_music::library::DEFAULT_MOOD.to_string() } else { s.mood.clone() }, seed: s.seed.max(1), tune: SavedTune::default() })
    }

    /// Play `item` now: the item playing goes into history, Up next is kept, and a station becomes the
    /// station at the end too.
    pub fn play_now(&mut self, item: Item, s: &mut MusicSettings, endless: f32) {
        self.observe(s, endless);
        if let Some(cur) = self.now.take() {
            if !cur.same_as(&item) {
                self.remember(cur, s, endless);
            }
        }
        if matches!(item, Item::Station { .. }) {
            self.station = Some(item.clone());
        }
        self.apply(&item, s);
        self.now = Some(item);
    }

    /// Add `item` to the end of Up next.
    pub fn add(&mut self, item: Item) {
        self.up_next.push(item);
    }

    /// NEXT, the same for every kind: the item playing goes into history (with repeat all, to the end of
    /// Up next as well); the next item of Up next plays (with shuffle, one drawn from it); with Up next
    /// empty, the station's next piece.
    pub fn next(&mut self, s: &mut MusicSettings, endless: f32) {
        self.observe(s, endless);
        let cur = self.now.take();
        let mut came_round = false;
        if let Some(c) = &cur {
            self.remember(c.clone(), s, endless);
            if self.repeat == Repeat::All && !matches!(c, Item::Station { .. }) {
                self.up_next.push(c.clone());
                came_round = true;
            }
        }
        let item = if self.up_next.is_empty() {
            self.station_next(cur.as_ref(), s)
        } else {
            // With shuffle, never the item that just went to the back (unless it is all there is).
            let n = self.up_next.len() - usize::from(came_round && self.up_next.len() > 1);
            let k = if self.shuffle { (self.draw() % n as u64) as usize } else { 0 };
            self.up_next.remove(k)
        };
        // A station from Up next becomes the station at the end too.
        if matches!(item, Item::Station { .. }) {
            self.station = Some(item.clone());
        }
        self.apply(&item, s);
        self.now = Some(item);
    }

    /// The station's next piece (its seed moved on from the piece it played, or from the station's own
    /// when another item was playing).
    fn station_next(&mut self, cur: Option<&Item>, s: &MusicSettings) -> Item {
        let mut st = self.station_or_default(s);
        if let Item::Station { seed, .. } = &mut st {
            let from = match cur {
                Some(Item::Station { seed: playing, .. }) => *playing,
                _ => *seed,
            };
            *seed = next_station_seed(from);
        }
        self.station = Some(st.clone());
        st
    }

    /// PREVIOUS, the same for every kind: the item playing goes back to the front of Up next (a station
    /// stays at the end) and the last item of history plays again. False with no history.
    pub fn previous(&mut self, s: &mut MusicSettings, endless: f32) -> bool {
        self.observe(s, endless);
        let Some(prev) = self.history.pop() else { return false };
        // With repeat all the item went to the back of Up next as it finished: it is playing again now.
        if self.repeat == Repeat::All && self.up_next.last().is_some_and(|x| x.same_as(&prev)) {
            self.up_next.pop();
        }
        if let Some(cur) = self.now.take() {
            if !matches!(cur, Item::Station { .. }) {
                self.up_next.insert(0, cur);
            }
        }
        self.apply(&prev, s);
        self.now = Some(prev);
        true
    }

    /// The shuffle's next draw (splitmix64).
    fn draw(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// See what the settings say plays and adopt a change made anywhere as "play now" (the module's
    /// words). True when the queue changed. A composed piece adopted gets its length written into the
    /// settings too, so the score knows it.
    pub fn observe(&mut self, s: &mut MusicSettings, endless: f32) -> bool {
        let seen = Seen::of(s);
        if self.seen.as_ref() == Some(&seen) {
            return false;
        }
        let first = self.seen.is_none();
        let prev = self.seen.replace(seen);
        let item = if !s.written.is_empty() {
            Item::Written { id: s.written.clone() }
        } else if s.game_chooses {
            // The game chooses: the queue waits, keeping its item for when the person's music plays.
            return false;
        } else {
            match &self.now {
                Some(Item::Station { genre, mood, seed, .. }) if *genre == s.genre && *mood == s.mood && *seed == s.seed => Item::Station { genre: genre.clone(), mood: mood.clone(), seed: *seed, tune: s.tune.clone() },
                Some(Item::Composed { genre, mood, seed, minutes, .. }) if *genre == s.genre && *mood == s.mood && *seed == s.seed => Item::Composed { genre: genre.clone(), mood: mood.clone(), seed: *seed, tune: s.tune.clone(), minutes: *minutes },
                _ => {
                    // A new seed is one piece; a genre or a mood chosen with the seed unchanged, a station.
                    let seed_changed = prev.as_ref().is_some_and(|p| p.seed != s.seed && p.written.is_empty());
                    if seed_changed {
                        Item::Composed { genre: s.genre.clone(), mood: s.mood.clone(), seed: s.seed.max(1), tune: s.tune.clone(), minutes: composed_minutes(s, endless) }
                    } else {
                        Item::Station { genre: s.genre.clone(), mood: s.mood.clone(), seed: s.seed.max(1), tune: s.tune.clone() }
                    }
                }
            }
        };
        if self.now.as_ref() == Some(&item) {
            return false;
        }
        let retune = self.now.as_ref().is_some_and(|n| n.same_as(&item));
        if let (Some(cur), false, false) = (self.now.take(), retune, first) {
            self.remember(cur, s, endless);
        }
        // The length the score plans the piece over, written beside it.
        match &item {
            Item::Composed { minutes, .. } => s.minutes = *minutes,
            Item::Station { .. } => s.minutes = station_minutes(s),
            Item::Written { .. } => {}
        }
        if matches!(item, Item::Station { .. }) {
            self.station = Some(item.clone());
        }
        self.seen = Some(Seen::of(s));
        self.now = Some(item);
        if !retune {
            self.mark = None;
        }
        true
    }

    /// One frame (the engine's music frame, page open or not): adopt a change of the settings, and move
    /// on when the item playing has ended (a written piece's last bar played, a composed piece's
    /// minutes, a station piece's "A new piece" minutes). Nothing ends while the game chooses, while an
    /// audition or the danger plays in place of the person's music, or before the score plays the item.
    pub fn tick(&mut self, s: &mut MusicSettings, r: &MusicReadout, endless: f32) -> Tick {
        let mut out = Tick { changed: self.observe(s, endless), ..Default::default() };
        let Some(now) = self.now.clone() else { return out };
        let Some(st) = r.score else { return out };
        if (s.game_chooses && s.written.is_empty()) || r.audition_s.is_some() || r.danger {
            return out;
        }
        let ended = match &now {
            Item::Written { id } => {
                let Some(piece) = r.written_piece.as_ref().filter(|_| r.written.as_deref() == Some(id.as_str())) else { return out };
                let bars = piece.timeline.bars.len().max(1) as u64;
                let loops = st.bars.saturating_sub(1) / bars;
                // Ended when the times through it went up since it was first seen playing.
                let before = match &self.mark {
                    Some(Mark::Loops(seen, at)) if seen == id => Some(*at),
                    _ => None,
                };
                if before != Some(loops) {
                    self.mark = Some(Mark::Loops(id.clone(), loops));
                }
                before.is_some_and(|at| loops > at)
            }
            Item::Composed { genre, seed, .. } | Item::Station { genre, seed, .. } => {
                let (genre, seed) = (genre.clone(), *seed);
                let len_min = match &now {
                    Item::Composed { minutes, .. } => *minutes,
                    _ => station_minutes(s),
                };
                let len = len_min * 60.0;
                let plays = r.playing.as_ref().and_then(|p| p.choice.as_ref()).is_some_and(|c| c.written.is_none() && c.seed == seed && c.genre == genre && c.length_s.is_some_and(|l| (l - len).abs() < 0.5));
                if len <= 0.0 || !plays {
                    return out;
                }
                let over = st.seconds as f32 >= len;
                if !over {
                    if self.mark == Some(Mark::Ended(seed)) {
                        self.mark = None;
                    }
                    false
                } else if self.mark == Some(Mark::Ended(seed)) {
                    false
                } else {
                    self.mark = Some(Mark::Ended(seed));
                    true
                }
            }
        };
        if !ended {
            return out;
        }
        match (&now, self.repeat) {
            // Repeat one: a written piece goes round by itself; a composed piece starts again.
            (Item::Written { .. }, Repeat::One) => {}
            (Item::Composed { .. }, Repeat::One) => out.restart = true,
            (Item::Station { .. }, _) => {
                // A station piece ended: the station's next piece, still the station.
                self.remember(now.clone(), s, endless);
                let st = self.station_next(Some(&now), s);
                self.apply(&st, s);
                self.now = Some(st);
                out.changed = true;
                out.settings = true;
            }
            _ => {
                self.next(s, endless);
                out.changed = true;
                out.settings = true;
            }
        }
        out
    }

    /// Move the Up next item at `from` to `to` (Up and Down).
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from < self.up_next.len() && to < self.up_next.len() {
            let x = self.up_next.remove(from);
            self.up_next.insert(to, x);
        }
    }
}

/// The person's queue file: `music/queue.json` beside their favourites.
pub fn queue_path() -> PathBuf {
    super::favourites_path().with_file_name("queue.json")
}

/// Read the queue (none yet: an empty one). An unreadable file is set aside and said in words.
pub fn load(path: &Path) -> Result<Queue, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Queue::default()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    match serde_json::from_str::<Queue>(&text) {
        Ok(mut q) => {
            let over = q.history.len().saturating_sub(HISTORY_MOST);
            q.history.drain(..over);
            Ok(q)
        }
        Err(e) => {
            let aside = path.with_file_name("queue.unreadable.json");
            let _ = std::fs::rename(path, &aside);
            Err(format!("{} could not be read ({e}); it was set aside as {}", path.display(), aside.display()))
        }
    }
}

/// Write the queue (through a temporary file, so a crash mid-write never leaves half a file).
pub fn save(path: &Path, q: &Queue) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(q).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}
