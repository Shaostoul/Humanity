//! The Music page's pictures: Now playing (the piece in words, the beat pulse, each part moving, the
//! spectrum) and See (the piano roll with chord symbols and section letters). Everything is read
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
/// Bars the piano roll shows: one behind the playhead and three ahead.
const ROLL_BARS: usize = 4;

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
pub(super) fn ensure_sheet(state: &mut GuiState) {
    let Some(playing) = state.music_dev.readout.playing.clone() else {
        state.music_page.sheet = None;
        return;
    };
    let now = state.music_dev.readout.score.map(|s| s.seconds).unwrap_or(0.0);
    let reach = ((now + AHEAD_S) / 60.0).ceil() * 60.0;
    let reach = reach.min(MOST_S);
    let keep = humanity_core::reuse::enabled()
        && state.music_page.sheet.as_ref().is_some_and(|s| {
            let fresh = tag_of(&playing, s.tag.seconds as f64);
            s.tag == fresh && (s.tag.seconds as f64 >= MOST_S || now + AHEAD_S * 0.5 < s.tag.seconds as f64)
        });
    if keep {
        return;
    }
    let sheet = transcribe(&playing.res, playing.seed, &playing.ctx, 48000.0, reach);
    state.music_page.sheet = Some(SheetCache { tag: tag_of(&playing, reach), sheet });
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

/// Now playing: Play here, the piece in words, and the visualizer.
pub(super) fn now_playing(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    ensure_sheet(state);
    let readout = &state.music_dev.readout;
    let status = readout.score;
    widgets::subsection_header(ui, theme, accent, t("music_page.now_playing"), "");
    widgets::card(ui, theme, |ui| {
        ui.checkbox(&mut state.music_page.play_here, t("music_page.play_here")).hint("music_page.play_here_hint");
        widgets::setting_hint(ui, theme, hint, t("music_page.play_here_hint"));
        if !state.settings.music.music_on {
            ui.label(RichText::new(t("music_page.music_off")).color(theme.warning()));
        } else if !readout.device {
            ui.label(RichText::new(t("music_page.no_device")).color(theme.text_muted()));
        }
        if let (Some(c), Some(p)) = (state.music_page.sheet.as_ref(), readout.playing.as_ref()) {
            let (words, leads) = piece_words(&c.sheet, p);
            ui.label(RichText::new(t("music_page.seed_line").replace("{{seed}}", &p.seed.to_string())).strong().color(theme.text_primary()));
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
        painter.text(Pos2::new((x0 + x1) * 0.5, rect.bottom() - label_h * 0.5), Align2::CENTER_CENTER, role_words(k), font.clone(), theme.text_muted());
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

/// See: the piano roll of the bars around the playhead.
pub(super) fn see(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    widgets::subsection_header(ui, theme, accent, t("music_page.see"), "");
    widgets::card(ui, theme, |ui| {
        widgets::setting_hint(ui, theme, hint, t("music_page.roll_hint"));
        let now = state.music_dev.readout.score.map(|s| s.seconds).unwrap_or(0.0);
        match state.music_page.sheet.as_ref() {
            Some(c) if !c.sheet.bars.is_empty() => piano_roll(ui, theme, &c.sheet, now, state.music_page.tune.mute),
            _ => {
                ui.label(RichText::new(t("music_page.roll_empty")).color(theme.text_muted()));
            }
        }
    });
}

/// The piano roll: `ROLL_BARS` bars from the one before the playhead; time across, pitch up, each
/// part in its colour (a muted part faint), chord symbols above each chord, the section letter at
/// each section's start, the playhead.
pub fn piano_roll(ui: &mut egui::Ui, theme: &Theme, sheet: &Sheet, now: f64, muted: u8) {
    let width = ui.available_width().max(320.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 280.0), Sense::hover());
    resp.hint("music_page.roll_hint");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, theme.bg_card());
    let first = sheet.bar_at(now).unwrap_or(0).saturating_sub(1);
    let bars: Vec<usize> = (first..first + ROLL_BARS).filter(|&b| b < sheet.bars.len()).collect();
    let Some(&last) = bars.last() else { return };
    let (t0, _) = sheet.bar_span(first).unwrap_or((0.0, 1.0));
    let (_, t1) = sheet.bar_span(last).unwrap_or((t0, t0 + 1.0));
    // Two lines above the grid: the section letters, then the chord symbols.
    let head_h = theme.font_size_small * 2.8 + 12.0;
    let grid = Rect::from_min_max(Pos2::new(rect.left() + 4.0, rect.top() + head_h), Pos2::new(rect.right() - 4.0, rect.bottom() - 4.0));
    let x_of = |s: f64| grid.left() + ((s - t0) / (t1 - t0).max(1e-6)) as f32 * grid.width();
    // The notes in view and their pitch range.
    let mut notes = Vec::new();
    for &b in &bars {
        let (a, end) = sheet.bar_span(b).unwrap_or((0.0, 0.0));
        let beat_s = (end - a) / sheet.beats_per_bar.max(1) as f64;
        for n in sheet.bar_notes(b) {
            if n.role as usize >= ROLES {
                continue;
            }
            notes.push((a + n.start_beat as f64 * beat_s, n.len_beats as f64 * beat_s, n));
        }
    }
    let (lo, hi) = notes.iter().fold((f32::MAX, f32::MIN), |(lo, hi), (_, _, n)| (lo.min(n.midi), hi.max(n.midi)));
    let (lo, hi) = if lo > hi { (48.0, 72.0) } else { ((lo - 2.0).floor(), (hi + 2.0).ceil()) };
    let row = grid.height() / (hi - lo + 1.0).max(1.0);
    let y_of = |m: f32| grid.bottom() - (m - lo + 1.0) * row;
    // Octave lines (each C) and the bar and beat lines.
    let mut c = (lo / 12.0).ceil() * 12.0;
    while c <= hi {
        painter.line_segment([Pos2::new(grid.left(), y_of(c) + row), Pos2::new(grid.right(), y_of(c) + row)], Stroke::new(1.0f32, theme.border().gamma_multiply(0.5)));
        c += 12.0;
    }
    let font = FontId::proportional(theme.font_size_small);
    let flats = sheet.flats();
    for &b in &bars {
        let (a, end) = sheet.bar_span(b).unwrap_or((0.0, 0.0));
        let bpb = sheet.beats_per_bar.max(1);
        for k in 0..bpb {
            let x = x_of(a + (end - a) * k as f64 / bpb as f64);
            let stroke = if k == 0 { Stroke::new(1.5f32, theme.border()) } else { Stroke::new(1.0f32, theme.border().gamma_multiply(0.35)) };
            painter.line_segment([Pos2::new(x, grid.top()), Pos2::new(x, grid.bottom())], stroke);
        }
    }
    // Section letters and chord symbols above the grid.
    for s in sheet.sections.iter().filter(|s| s.at_s >= t0 - 1e-6 && s.at_s < t1 && s.letter > 0) {
        let x = x_of(s.at_s);
        painter.text(Pos2::new(x + 3.0, rect.top() + 4.0), Align2::LEFT_TOP, ((b'A' + s.letter - 1) as char).to_string(), FontId::proportional(theme.font_size_small * 1.3), theme.accent());
    }
    // The section already playing when the view begins, at its left edge (fainter).
    if let Some(s) = sheet.section_at(t0 + 1e-6).filter(|s| s.letter > 0 && s.at_s < t0 - 1e-6) {
        painter.text(Pos2::new(grid.left() + 3.0, rect.top() + 4.0), Align2::LEFT_TOP, ((b'A' + s.letter - 1) as char).to_string(), FontId::proportional(theme.font_size_small * 1.3), theme.text_secondary());
    }
    // The chord already sounding when the view begins, then each chord that starts in it.
    if let Some(ch) = sheet.chord_at(t0 + 1e-6).filter(|c| c.at_s < t0 - 1e-6) {
        painter.text(Pos2::new(grid.left() + 3.0, rect.top() + head_h - 4.0), Align2::LEFT_BOTTOM, chord_symbol(ch.root, ch.pcs, flats), font.clone(), theme.text_secondary());
    }
    for ch in sheet.chords.iter().filter(|c| c.at_s >= t0 - 1e-6 && c.at_s < t1) {
        let x = x_of(ch.at_s);
        painter.text(Pos2::new(x + 3.0, rect.top() + head_h - 4.0), Align2::LEFT_BOTTOM, chord_symbol(ch.root, ch.pcs, flats), font.clone(), theme.text_primary());
    }
    // The notes.
    for (at, len, n) in &notes {
        let x0 = x_of(*at);
        let x1 = x_of(at + len.max(0.05)).max(x0 + 2.0).min(grid.right());
        let y = y_of(n.midi);
        let faint = muted & (1 << n.role) != 0;
        let colour = role_colour(theme, n.role).gamma_multiply(if faint { 0.15 } else { 0.45 + 0.55 * n.vel.clamp(0.0, 1.0) });
        painter.rect_filled(Rect::from_min_max(Pos2::new(x0, y + 1.0), Pos2::new(x1, y + row.max(2.0) - 1.0)), 2.0, colour);
    }
    // The playhead.
    if now >= t0 && now <= t1 {
        let x = x_of(now);
        painter.line_segment([Pos2::new(x, grid.top()), Pos2::new(x, grid.bottom())], Stroke::new(2.0f32, theme.accent()));
    }
}
