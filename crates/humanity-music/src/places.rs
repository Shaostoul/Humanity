//! Which mood plays where and when (docs/design/music.md, sections 7 and 8): `data/music/places.ron`.
//!
//! A rule matches a zone purpose (residence, commons, street, bay, agriculture, corridor), a room
//! (by its id or name), and a stretch of the ship's day; the most specific matching rule wins (a
//! room beats a zone beats the hour alone), and the composer moves to its mood at the next phrase.
//! The same file lists the places that may suggest a livelier GENRE (a gym, a workshop), which the
//! game uses only if the person allows it in Settings > Music.

use serde::{Deserialize, Serialize};

/// One rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaceRule {
    /// A zone purpose it matches (none: any zone).
    #[serde(default)]
    pub zone: Option<String>,
    /// A room it matches: a room whose id or name contains this word, any case (none: any room).
    #[serde(default)]
    pub room: Option<String>,
    /// The ship's hours it holds, (from, to) on a 24-hour dial, wrapping past midnight (none: all day).
    #[serde(default)]
    pub hours: Option<(f32, f32)>,
    /// The mood it chooses.
    pub mood: String,
}

/// A place that suggests a genre.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenreSuggestion {
    #[serde(default)]
    pub zone: Option<String>,
    #[serde(default)]
    pub room: Option<String>,
    pub genre: String,
}

/// The places file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Places {
    /// The mood where no rule matches.
    pub default_mood: String,
    pub rules: Vec<PlaceRule>,
    #[serde(default)]
    pub genre_suggestions: Vec<GenreSuggestion>,
}

/// Where the player is, as the places rules read it.
#[derive(Clone, Copy, Debug)]
pub struct Place<'a> {
    /// The zone purpose ("residence", "commons", ...; "" when unknown).
    pub zone: &'a str,
    /// The room's id and name, lower case ("" outside a room).
    pub room_id: &'a str,
    pub room_name: &'a str,
    /// The ship's hour, 0..24.
    pub hour: f32,
}

fn hour_in(h: f32, (from, to): (f32, f32)) -> bool {
    let h = h.rem_euclid(24.0);
    if from <= to {
        h >= from && h < to
    } else {
        h >= from || h < to
    }
}

fn room_matches(word: &str, place: &Place) -> bool {
    let w = word.to_ascii_lowercase();
    !w.is_empty() && (place.room_id.contains(&w) || place.room_name.contains(&w))
}

impl Places {
    /// The mood id for a place: the most specific matching rule's (room 4, zone 2, hours 1; the
    /// first written wins a tie), else the default.
    pub fn mood_for(&self, place: &Place) -> &str {
        let mut best: Option<(u32, &str)> = None;
        for r in &self.rules {
            let mut score = 0;
            if let Some(z) = &r.zone {
                if !z.eq_ignore_ascii_case(place.zone) {
                    continue;
                }
                score += 2;
            }
            if let Some(room) = &r.room {
                if !room_matches(room, place) {
                    continue;
                }
                score += 4;
            }
            if let Some(h) = r.hours {
                if !hour_in(place.hour, h) {
                    continue;
                }
                score += 1;
            }
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, r.mood.as_str()));
            }
        }
        best.map(|(_, m)| m).unwrap_or(self.default_mood.as_str())
    }

    /// The genre a place suggests, if any (a room's beats a zone's).
    pub fn genre_for(&self, place: &Place) -> Option<&str> {
        let room = self.genre_suggestions.iter().find(|s| s.room.as_deref().is_some_and(|r| room_matches(r, place)));
        let zone = || self.genre_suggestions.iter().find(|s| s.room.is_none() && s.zone.as_deref().is_some_and(|z| z.eq_ignore_ascii_case(place.zone)));
        room.or_else(zone).map(|s| s.genre.as_str())
    }
}
