//! TUNING A PIECE (docs/design/music-page.md section 6): what a person may change about a piece on
//! the Music page without changing which piece it is. The operator, 2026-11-13: "We'll want to add
//! all the features we can for like visualizing and tuning the music."
//!
//! A [`Tune`] rides in the score's `Copy` context (and the composer's steer), so it is fixed-size
//! and allocation-free. Two halves act in two places:
//! - the COMPOSER's half (the key, the tempo, the swing, the lead voices) is read when a phrase is
//!   planned, so the runtime starts the same seed again with a change, at the next bar;
//! - the SCORE's half (each role's level, mute and solo) is applied as the composer's notes reach the
//!   voices, so it is heard at once and never changes what is composed: a muted bass leaves the tune
//!   and the chords exactly as they were.

/// A lead voice left to the piece's own draw.
pub const PIECE: u16 = u16::MAX;

/// The roles a tune addresses: the voice pool's eight (`mood::RoleKind::ALL`, in that order).
pub const ROLES: usize = 8;

/// Each role's id, in that order: what the favourites file and share codes name a role by, and its
/// words (`music_page.role.<id>`).
pub const ROLE_IDS: [&str; ROLES] = ["pad", "bass", "drone", "choir", "bells", "melody", "percussion", "texture"];

/// How far either way a genre written at one tempo (electro swing's 128 and only 128) may be moved,
/// as a fraction (the design's question 1, recommended to the operator).
pub const NARROW_TEMPO_SPAN: f32 = 0.10;

/// What the person changed about a piece. The default changes nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tune {
    /// Semitones the whole piece moves (its home key and any key its bridge moves to), -6..=6.
    pub transpose: i8,
    /// Beats a minute in place of the piece's own (0: the piece's).
    pub tempo_bpm: f32,
    /// How late the off-beats lean, 0.5 straight to 0.75 heavy (negative: the genre's own).
    pub swing: f32,
    /// Per role, the genre's lead entry (an index in its `leads`) that plays it ([`PIECE`]: the
    /// piece's draw).
    pub lead: [u16; ROLES],
    /// Per role, a level in dB on top of the genre's.
    pub level_db: [f32; ROLES],
    /// Per role, one bit: muted; and soloed (any solo silences every role not soloed).
    pub mute: u8,
    pub solo: u8,
}

impl Default for Tune {
    fn default() -> Self {
        Tune { transpose: 0, tempo_bpm: 0.0, swing: -1.0, lead: [PIECE; ROLES], level_db: [0.0; ROLES], mute: 0, solo: 0 }
    }
}

impl Tune {
    /// Role `role`'s gain (0: silent), from its level, its mute and the solos. Roles outside the
    /// eight (the stinger and the riser, 254) are never touched.
    pub fn role_gain(&self, role: usize) -> f32 {
        if role >= ROLES {
            return 1.0;
        }
        let bit = 1u8 << role;
        if self.mute & bit != 0 || (self.solo != 0 && self.solo & bit == 0) {
            return 0.0;
        }
        crate::dsp::db_to_amp(self.level_db[role].clamp(-60.0, 12.0))
    }

    /// Every role's gain (the score's sink reads this once a block).
    pub fn gains(&self) -> [f32; ROLES] {
        let mut g = [1.0; ROLES];
        for (k, x) in g.iter_mut().enumerate() {
            *x = self.role_gain(k);
        }
        g
    }

    /// Whether the composer's half equals `other`'s: the same seed with these two tunes composes the
    /// same notes (a level or a mute never changes what is composed).
    pub fn composes_like(&self, other: &Tune) -> bool {
        self.transpose == other.transpose && self.tempo_bpm.to_bits() == other.tempo_bpm.to_bits() && self.swing.to_bits() == other.swing.to_bits() && self.lead == other.lead
    }

    /// The tempo range the page offers for a genre whose range is `range` (bpm): the range itself,
    /// or for a genre written at one tempo (a range under a tenth of its tempo wide), that tempo
    /// [`NARROW_TEMPO_SPAN`] either way.
    pub fn tempo_range(range: (f32, f32)) -> (f32, f32) {
        let (lo, hi) = (range.0.min(range.1), range.0.max(range.1));
        if hi - lo < lo * NARROW_TEMPO_SPAN {
            let mid = (lo + hi) * 0.5;
            ((mid * (1.0 - NARROW_TEMPO_SPAN)).round(), (mid * (1.0 + NARROW_TEMPO_SPAN)).round())
        } else {
            (lo, hi)
        }
    }

    /// The tempo the composer plays: the person's (held to the range the page offers) or `drawn`.
    pub fn tempo_for(&self, drawn: f32, range: (f32, f32)) -> f32 {
        if self.tempo_bpm > 0.0 {
            let (lo, hi) = Tune::tempo_range(range);
            self.tempo_bpm.clamp(lo, hi)
        } else {
            drawn
        }
    }
}
