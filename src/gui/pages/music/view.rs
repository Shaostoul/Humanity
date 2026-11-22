//! What plays in words on Now playing: the kind of music, the piece in words, the section and chord
//! (the beat pulse, the parts and the spectrum are the parts row: parts.rs; the piano roll is roll.rs).
//! Everything is read
//! from the score itself (its status: the beat, the role meters, the bands) or from the sheet, the
//! same piece written down by the composer run dry (docs/design/music-page.md section 9).

use egui::{Color32, RichText};
use humanity_core::i18n::t;
use humanity_music::sheet::{chord_symbol, key_uses_flats, pc_name, transcribe, Sheet};
use humanity_music::tune::ROLES;

use super::{role_words, voice_words, SheetCache, SheetTag};
use crate::gui::pages::dev_music::Playing;
use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};

/// How far ahead of the playhead the transcription reaches, seconds (it is made again, longer, when
/// the playhead comes within half of this of its end).
const AHEAD_S: f64 = 40.0;
/// The longest transcription kept, seconds.
const MOST_S: f64 = 1200.0;

/// The tag a transcription of `playing` reaching `seconds` would carry.
fn tag_of(playing: &Playing, seconds: f64) -> SheetTag {
    let c = &playing.ctx;
    SheetTag {
        seed: playing.seed,
        res: std::sync::Arc::as_ptr(&playing.res) as usize,
        steer: [c.mood as u32, c.genre_a as u32, c.genre_b as u32, c.blend.to_bits(), c.intensity.to_bits(), c.a4_hz.to_bits()],
        tune: humanity_music::tune::Tune { level_db: [0.0; ROLES], mute: 0, solo: 0, ..c.tune },
        seconds: seconds as u32,
    }
}

/// Keep the sheet of the playing piece current: made again when the piece changes, when the playhead
/// nears its end, and every frame with "turn off all reuse" off (CLAUDE.md, the no-caches rule).
pub(crate) fn ensure_sheet(state: &mut GuiState) {
    // A written piece is drawn from its own timeline (score-player.md section 16, rung 2): made again
    // when the piece changes, and every frame with "turn off all reuse" off. It is not the composer's,
    // so no transcription of a seed's piece stands in for it.
    if let Some(piece) = state.music_dev.readout.written_piece.clone() {
        let tag = written_tag(&piece);
        let keep = humanity_core::reuse::enabled() && state.music_page.sheet.as_ref().is_some_and(|s| s.tag == tag);
        if !keep {
            state.music_page.sheet = Some(SheetCache { tag, sheet: Sheet::from_timeline(&piece.timeline, &piece.staff_roles()) });
        }
        return;
    }
    let Some(playing) = state.music_dev.readout.playing.clone().filter(|_| state.music_dev.readout.written.is_none()) else {
        state.music_page.sheet = None;
        return;
    };
    let now = state.music_dev.readout.score.map(|s| s.seconds).unwrap_or(0.0);
    // A planned piece (music.md section 7h): its plan for its whole length (every bar line, section and
    // stage, a few hundred phrases of arithmetic), and its notes written down only for the bars in view,
    // each found from the piece's seed without playing what comes before ("how the piece looks X
    // minutes out"). A piece outside the plan (a danger mood) is written down from its start as before.
    if planned_sheet(state, &playing, now) {
        return;
    }
    // A piece with a set length (the game's "A new piece" every few minutes: the score says how long
    // until the next one) is written down to its end, so the piano roll can draw it whole
    // (docs/design/music-page.md section 7); an endless one (Never, or the person's own choice) 40
    // seconds past the playhead, made again further on as the playhead nears the end.
    let whole = state.music_dev.readout.next_piece_s.map(|rem| (now + rem as f64).min(MOST_S));
    let reach = match whole {
        Some(len) => len.ceil().max(1.0),
        None => (((now + AHEAD_S) / 60.0).ceil() * 60.0).min(MOST_S),
    };
    let keep = humanity_core::reuse::enabled()
        && state.music_page.sheet.as_ref().is_some_and(|s| {
            let fresh = tag_of(&playing, s.tag.seconds as f64);
            let long_enough = match whole {
                Some(len) => s.tag.seconds as f64 >= len - 0.5,
                None => s.tag.seconds as f64 >= MOST_S || now + AHEAD_S * 0.5 < s.tag.seconds as f64,
            };
            s.tag == fresh && long_enough
        });
    if keep {
        return;
    }
    let sheet = transcribe(&playing.res, playing.seed, &playing.ctx, 48000.0, reach);
    state.music_page.sheet = Some(SheetCache { tag: tag_of(&playing, reach), sheet });
}

/// The longest a planned piece's plan is drawn for when it plays on, seconds past the playhead.
const ENDLESS_AHEAD_S: f64 = 1200.0;

/// A planned piece's sheet: its plan for its whole length, its notes for the bars the roll shows
/// (made again when the piece changes, when the bars in view are not written down, and every frame
/// with "turn off all reuse" off). False when the piece is not planned.
fn planned_sheet(state: &mut GuiState, playing: &Playing, now: f64) -> bool {
    let length = match state.music_dev.readout.next_piece_s {
        Some(rem) => (now + rem as f64).max(1.0),
        None => ((now + ENDLESS_AHEAD_S) / 60.0).ceil() * 60.0,
    };
    let tag = tag_of(playing, length);
    let reuse = humanity_core::reuse::enabled();
    let fresh = reuse && state.music_page.sheet.as_ref().is_some_and(|s| s.tag == tag && !s.sheet.plan.is_empty());
    let mut sheet = match (fresh, state.music_page.sheet.take()) {
        (true, Some(c)) => c.sheet,
        _ => match humanity_music::sheet::skeleton(&playing.res, playing.seed, &playing.ctx, 48000.0, length) {
            Some(s) => s,
            None => return false,
        },
    };
    // The bars in view (the roll's own reckoning), written down if they are not.
    let p = &state.music_page;
    let total = sheet.bars.len();
    if total > 0 {
        let n = super::roll::bars_shown(p.roll_bars, total);
        let first = super::roll::first_bar(&sheet, now, n, p.roll_follow, p.roll_first);
        let a = sheet.bars[first];
        let b = sheet.bar_span(first + n - 1).map(|s| s.1).unwrap_or(sheet.seconds).min(a + MOST_S);
        let covered = reuse && sheet.notes_span.0 <= a + 1e-6 && sheet.notes_span.1 >= b - 1e-6;
        if !covered {
            let w = humanity_music::sheet::transcribe_from(&playing.res, playing.seed, &playing.ctx, 48000.0, first as u64, (b - a).max(0.1));
            sheet.notes = w.notes;
            sheet.chords = w.chords;
            sheet.notes_span = w.notes_span;
            sheet.hash = w.hash;
            sheet.leads = w.leads;
        }
    }
    state.music_page.sheet = Some(SheetCache { tag, sheet });
    true
}

/// The tag of a written piece's sheet: the piece's timeline (by address: a new read is a new piece).
fn written_tag(piece: &humanity_music::scores::WrittenPiece) -> SheetTag {
    let tune = humanity_music::tune::Tune { level_db: [0.0; ROLES], mute: 0, solo: 0, ..Default::default() };
    SheetTag { seed: u64::MAX, res: std::sync::Arc::as_ptr(&piece.timeline) as usize, steer: [0; 6], tune, seconds: u32::MAX }
}

/// The piece in words: "D harmonic minor, AABA, 128 beats a minute", the lead and the answer.
fn piece_words(sheet: &Sheet, playing: &Playing) -> (String, Option<String>) {
    let Some(first) = sheet.sections.first() else { return (String::new(), None) };
    let flats = key_uses_flats(first.key_pc, first.mode);
    let form: String = if sheet.form.is_empty() { t("music_page.no_form").to_string() } else { sheet.form.iter().map(|&l| (b'A' + l) as char).collect() };
    let words = t("music_page.piece_words")
        .replace("{{key}}", pc_name(first.key_pc, flats))
        .replace("{{mode}}", humanity_core::i18n::t_dyn(&format!("music.mode.{}", first.mode.id())))
        .replace("{{form}}", &form)
        .replace("{{tempo}}", &format!("{:.0}", first.tempo_bpm));
    let dominant = if playing.ctx.blend >= 0.5 { playing.ctx.genre_b } else { playing.ctx.genre_a };
    let leads: Vec<String> = playing
        .lib
        .genres
        .get(dominant as usize)
        .map(|g| sheet.leads.iter().enumerate().filter_map(|(k, &i)| g.leads.get(i as usize).map(|e| (k, voice_words(e.part.as_deref().unwrap_or(&e.instrument))))).map(|(k, v)| format!("{} ({})", v, role_words(k))).collect())
        .unwrap_or_default();
    (words, (!leads.is_empty()).then(|| t("music_page.leads_line").replace("{{leads}}", &leads.join(", "))))
}

/// What plays in words: the kind of music, the piece in words, its section and chord (who chose it is
/// the transport bar's sentence: transport.rs).
pub(super) fn now_playing(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, _accent: Color32) {
    let readout = &state.music_dev.readout;
    let status = readout.score;
    widgets::card(ui, theme, |ui| {
        // Which kind of music is the person's, said plainly, and while it is a written piece, how to
        // hear composed music again (BUG-343).
        ui.label(RichText::new(super::kind_words(&state.settings.music, readout)).strong().color(theme.text_primary()));
        if super::written_title(&state.settings.music, readout).is_some() {
            ui.label(RichText::new(t("music_page.kind_back")).size(theme.font_size_small).color(theme.text_secondary()));
        }
        ui.add_space(theme.spacing_xs);
        // A written piece has no chords or sections of the composer's to name (its title is above).
        if let (Some(c), Some(p), None) = (state.music_page.sheet.as_ref(), readout.playing.as_ref(), readout.written.as_ref()) {
            let (words, leads) = piece_words(&c.sheet, p);
            ui.label(RichText::new(words).color(theme.text_primary()));
            if let Some(l) = leads {
                ui.label(RichText::new(l).color(theme.text_secondary()));
            }
            if let Some(s) = status {
                let flats = c.sheet.flats();
                let chord = c.sheet.chord_at(s.seconds).map(|ch| chord_symbol(ch.root, ch.pcs, flats)).unwrap_or_default();
                let letter = c.sheet.section_at(s.seconds).filter(|x| x.letter > 0).map(|x| ((b'A' + x.letter - 1) as char).to_string()).unwrap_or_else(|| "-".into());
                ui.label(RichText::new(t("music_page.section_now").replace("{{letter}}", &letter).replace("{{chord}}", &chord)).color(theme.text_secondary()));
            }
        }
    });
}
