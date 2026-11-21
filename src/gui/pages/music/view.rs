//! The Music page's pictures: the piece in words, the beat pulse, each part moving and the spectrum
//! (the piano roll is roll.rs). Everything is read
//! from the score itself (its status: the beat, the role meters, the bands) or from the sheet, the
//! same piece written down by the composer run dry (docs/design/music-page.md section 9).

use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use humanity_core::i18n::t;
use humanity_music::sheet::{chord_symbol, key_uses_flats, pc_name, transcribe, Sheet};
use humanity_music::tune::ROLES;

use super::{role_colour, role_words, voice_words, SheetCache, SheetTag};
use crate::gui::pages::dev_music::Playing;
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
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

/// The piece in words and the visualizer (what plays and who chose it is the strip's sentence along
/// the top: strip.rs).
pub(super) fn now_playing(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, _accent: Color32) {
    let readout = &state.music_dev.readout;
    let status = readout.score;
    widgets::card(ui, theme, |ui| {
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
        ui.add_space(theme.spacing_xs);
        visualizer(ui, theme, status.as_ref());
    });
}

/// The visualizer: a beat pulse (the bar's first beat strongest), each part's activity, and the
/// spectrum in octave bands.
fn visualizer(ui: &mut egui::Ui, theme: &Theme, status: Option<&humanity_music::MusicStatus>) {
    let width = ui.available_width().max(320.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
    resp.hint("music_page.visualizer_hint");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, theme.bg_card());
    let font = FontId::proportional(theme.font_size_small);
    let label_h = theme.font_size_small + 6.0;
    // The beat pulse.
    let pulse_w = 150.0f32.min(width * 0.25);
    let centre = Pos2::new(rect.left() + pulse_w * 0.5, rect.center().y - label_h * 0.5);
    let (beat, bpb) = status.map(|s| (s.beat, s.beats_per_bar.max(1) as f32)).unwrap_or((0.0, 4.0));
    let phase = beat.fract();
    let down = beat < 1.0;
    let swell = (1.0 - phase).powi(2) * if down { 1.0 } else { 0.6 };
    let base = (pulse_w * 0.22).min(40.0);
    let colour = if down { theme.accent() } else { theme.text_secondary() };
    if status.is_some() {
        painter.circle_filled(centre, base + swell * base * 0.6, colour.gamma_multiply(0.25 + 0.5 * swell));
        painter.circle_stroke(centre, base, Stroke::new(2.0f32, colour));
    } else {
        painter.circle_stroke(centre, base, Stroke::new(1.0f32, theme.text_muted()));
    }
    let bar_n = status.map(|s| s.bar + 1).unwrap_or(0);
    let beat_n = beat.floor() as u32 + 1;
    painter.text(Pos2::new(centre.x, rect.bottom() - label_h * 0.5), Align2::CENTER_CENTER, t("music_page.beat").replace("{{bar}}", &bar_n.to_string()).replace("{{beat}}", &format!("{beat_n}/{bpb:.0}")), font.clone(), theme.text_secondary());
    // Each part's activity.
    let parts = Rect::from_min_max(Pos2::new(rect.left() + pulse_w, rect.top() + 8.0), Pos2::new(rect.left() + pulse_w + (width - pulse_w) * 0.45, rect.bottom() - label_h));
    let col_w = parts.width() / ROLES as f32;
    for k in 0..ROLES {
        let v = status.map(|s| s.roles[k]).unwrap_or(0.0).clamp(0.0, 1.0);
        let x0 = parts.left() + k as f32 * col_w + col_w * 0.2;
        let x1 = x0 + col_w * 0.6;
        let h = parts.height() * v;
        painter.rect_filled(Rect::from_min_max(Pos2::new(x0, parts.top()), Pos2::new(x1, parts.bottom())), 2.0, theme.bg_panel());
        painter.rect_filled(Rect::from_min_max(Pos2::new(x0, parts.bottom() - h), Pos2::new(x1, parts.bottom())), 2.0, role_colour(theme, k as u8));
        if col_w >= theme.font_size_small * 4.0 {
            painter.text(Pos2::new((x0 + x1) * 0.5, rect.bottom() - label_h * 0.5), Align2::CENTER_CENTER, role_words(k), font.clone(), theme.text_muted());
        }
    }
    // The spectrum: each octave band's level, -60 to 0 dB.
    let spec = Rect::from_min_max(Pos2::new(parts.right() + 12.0, rect.top() + 8.0), Pos2::new(rect.right() - 8.0, rect.bottom() - label_h));
    let n = humanity_music::score::BANDS;
    let bw = spec.width() / n as f32;
    for k in 0..n {
        let amp = status.map(|s| s.bands[k]).unwrap_or(0.0);
        let db = 20.0 * amp.max(1e-6).log10();
        let v = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let x0 = spec.left() + k as f32 * bw + bw * 0.12;
        let x1 = x0 + bw * 0.76;
        painter.rect_filled(Rect::from_min_max(Pos2::new(x0, spec.bottom() - spec.height() * v), Pos2::new(x1, spec.bottom())), 2.0, theme.accent().gamma_multiply(0.4 + 0.6 * v));
        if k % 3 == 0 || k == n - 1 {
            let hz = humanity_music::score::band_hz(k);
            let label = if hz >= 1000.0 { format!("{:.0}k", hz / 1000.0) } else { format!("{hz:.0}") };
            painter.text(Pos2::new((x0 + x1) * 0.5, rect.bottom() - label_h * 0.5), Align2::CENTER_CENTER, label, font.clone(), theme.text_muted());
        }
    }
}
