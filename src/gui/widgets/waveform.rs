//! A sound's picture (2026-10-19, the Media page's middle pane for a sound or a song): the whole
//! track's waveform, the part already played in the accent colour and the rest muted, the play
//! position as a line across it, and a click or a drag on it to go there; above it, the spectrum
//! of the sound at the play position, low notes on the left and high on the right.
//!
//! NOTHING HERE READS OR DECODES THE SOUND WHILE IT DRAWS. The sound is read once, on a worker
//! thread (sound_view.rs decodes it), into an [`Analysis`] by a [`Builder`]: the peaks of the
//! whole track in a fixed number of stretches, and the spectrum at evenly spaced moments. Drawing
//! a frame looks up two small arrays by the play position. The builder keeps a fixed amount of
//! memory whatever the length (an hour-long recording costs what a song does): when its lists
//! fill, neighbours merge and each entry covers twice the time.
//!
//! The spectrum is the sound's own, from the same samples the mixer plays: a 1024-point Fourier
//! transform of the moment, its strength in each of [`BANDS`] bands spaced evenly in pitch from
//! 40 Hz up, from 60 dB below full scale (nothing) to full scale (the band's full height).
//!
//! Every colour is the theme's: the accent for the played part, the spectrum and the line,
//! text_muted for the part to come, bg_tertiary behind.
//!
//! Web mirror: none. The website plays a sound in the browser's own `<audio>` element.

use egui::{Color32, Rect, Sense, Ui, Vec2};

use crate::gui::theme::Theme;

/// The stretches of the track the waveform keeps the peaks of.
pub const BUCKETS: usize = 1024;
/// The spectrum's bands.
pub const BANDS: usize = 24;
/// The samples each spectrum is made from.
pub const FFT: usize = 1024;
/// The lowest pitch the spectrum shows (Hz), and the highest (or the half the sample rate allows).
pub const LOWEST_HZ: f32 = 40.0;
pub const HIGHEST_HZ: f32 = 16_000.0;
/// How quiet a band can be before it shows nothing (dB below full scale).
pub const FLOOR_DB: f32 = 60.0;
/// The most the builder keeps before neighbours merge.
const MAX_BLOCKS: usize = 8192;
const MAX_SLICES: usize = 8192;
/// How often a spectrum is taken at first (seconds); it doubles as the list fills.
const FIRST_SLICE_S: f64 = 0.05;

/// What the waveform widget draws, made once from the whole sound.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Analysis {
    /// The lowest and highest sample in each of up to [`BUCKETS`] equal stretches of the track,
    /// over every channel, from -1 to 1.
    pub peaks: Vec<(f32, f32)>,
    /// The spectrum at evenly spaced moments: `slices[i]` holds the [`BANDS`] levels, each 0
    /// (nothing, or [`FLOOR_DB`] below full scale) to 1 (full scale), of the moment ending
    /// `(i + 1) * slice_s` seconds in.
    pub slices: Vec<[f32; BANDS]>,
    pub slice_s: f64,
    /// The track's length, from the samples counted.
    pub duration_s: f64,
    pub sample_rate: u32,
}

impl Analysis {
    /// The spectrum playing at `seconds`, if there is one.
    pub fn spectrum_at(&self, seconds: f64) -> Option<&[f32; BANDS]> {
        if self.slices.is_empty() || self.slice_s <= 0.0 {
            return None;
        }
        let i = (seconds.max(0.0) / self.slice_s) as usize;
        self.slices.get(i.min(self.slices.len() - 1))
    }

    /// The loudest peak, for scaling the drawing (a quiet recording still fills its height).
    pub fn loudest(&self) -> f32 {
        self.peaks.iter().map(|(lo, hi)| lo.abs().max(hi.abs())).fold(0.0, f32::max)
    }
}

/// The band edges in Hz for a sound sampled at `rate`: [`BANDS`] + 1 of them, spaced evenly in
/// pitch from [`LOWEST_HZ`] to [`HIGHEST_HZ`] or half the rate, whichever is lower.
pub fn band_edges_hz(rate: u32) -> Vec<f32> {
    let top = HIGHEST_HZ.min(rate as f32 / 2.0).max(LOWEST_HZ * 2.0);
    let ratio = (top / LOWEST_HZ).powf(1.0 / BANDS as f32);
    (0..=BANDS).map(|i| LOWEST_HZ * ratio.powi(i as i32)).collect()
}

/// Reads a sound a frame at a time and keeps what the waveform widget needs, in a fixed amount of
/// memory. Fed by the decoder on a worker (sound_view.rs), then [`Builder::finish`]ed.
pub struct Builder {
    rate: u32,
    frames: u64,
    /// The peaks so far: each block covers `block` frames.
    block: u64,
    blocks: Vec<(f32, f32)>,
    current: (f32, f32),
    in_current: u64,
    /// The last [`FFT`] frames, mixed to one channel, in a ring.
    ring: Vec<f32>,
    ring_at: usize,
    /// A spectrum every `every` frames.
    every: u64,
    slices: Vec<[f32; BANDS]>,
    window: Vec<f32>,
    /// The transform's turning factors (cos, sin of -2 pi k / FFT), worked out once.
    twiddles: Vec<(f32, f32)>,
    /// The first and last Fourier bin of each band.
    bins: Vec<(usize, usize)>,
}

impl Builder {
    /// A builder for a sound sampled at `rate` frames a second.
    pub fn new(rate: u32) -> Self {
        let rate = rate.max(1);
        let window: Vec<f32> = (0..FFT).map(|n| 0.5 - 0.5 * (std::f32::consts::TAU * n as f32 / FFT as f32).cos()).collect();
        let hz_per_bin = rate as f32 / FFT as f32;
        let edges = band_edges_hz(rate);
        let mut bins = Vec::with_capacity(BANDS);
        let mut last = 0usize;
        for b in 0..BANDS {
            let lo = ((edges[b] / hz_per_bin).floor() as usize).max(last + 1).min(FFT / 2 - 1);
            let hi = ((edges[b + 1] / hz_per_bin).floor() as usize).max(lo).min(FFT / 2 - 1);
            bins.push((lo, hi));
            last = hi;
        }
        Builder {
            rate,
            frames: 0,
            block: 64,
            blocks: Vec::new(),
            current: (0.0, 0.0),
            in_current: 0,
            ring: vec![0.0; FFT],
            ring_at: 0,
            every: ((rate as f64 * FIRST_SLICE_S) as u64).max(1),
            slices: Vec::new(),
            window,
            twiddles: (0..FFT / 2).map(|k| { let (s, c) = (-std::f32::consts::TAU * k as f32 / FFT as f32).sin_cos(); (c, s) }).collect(),
            bins,
        }
    }

    /// Take `samples`, interleaved over `channels` channels.
    pub fn push_interleaved(&mut self, samples: &[f32], channels: usize) {
        let channels = channels.max(1);
        for frame in samples.chunks(channels) {
            self.push_frame(frame);
        }
    }

    /// Take one frame: one sample per channel.
    pub fn push_frame(&mut self, frame: &[f32]) {
        if frame.is_empty() {
            return;
        }
        let (mut lo, mut hi, mut sum) = (f32::MAX, f32::MIN, 0.0f32);
        for s in frame {
            let s = if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 };
            lo = lo.min(s);
            hi = hi.max(s);
            sum += s;
        }
        if self.in_current == 0 {
            self.current = (lo, hi);
        } else {
            self.current = (self.current.0.min(lo), self.current.1.max(hi));
        }
        self.in_current += 1;
        if self.in_current >= self.block {
            self.blocks.push(self.current);
            self.in_current = 0;
            if self.blocks.len() >= MAX_BLOCKS {
                // Full: each two neighbours become one, covering twice the time.
                self.blocks = self.blocks.chunks(2).map(|p| p.iter().fold((f32::MAX, f32::MIN), |a, b| (a.0.min(b.0), a.1.max(b.1)))).collect();
                self.block *= 2;
            }
        }
        self.ring[self.ring_at] = sum / frame.len() as f32;
        self.ring_at = (self.ring_at + 1) % FFT;
        self.frames += 1;
        if self.frames % self.every == 0 {
            let levels = self.spectrum();
            self.slices.push(levels);
            if self.slices.len() >= MAX_SLICES {
                // Full: keep every second one (they stay on the new, doubled, spacing) and take
                // them half as often from here on.
                self.slices = self.slices.iter().skip(1).step_by(2).copied().collect();
                self.every *= 2;
            }
        }
    }

    /// The spectrum of the last [`FFT`] frames.
    fn spectrum(&self) -> [f32; BANDS] {
        let mut re: Vec<f32> = (0..FFT).map(|n| self.ring[(self.ring_at + n) % FFT] * self.window[n]).collect();
        let mut im = vec![0.0f32; FFT];
        fft(&mut re, &mut im, &self.twiddles);
        // A full-scale sine through the Hann window peaks at FFT / 4.
        let full = FFT as f32 / 4.0;
        let mut out = [0.0f32; BANDS];
        for (b, (lo, hi)) in self.bins.iter().enumerate() {
            let peak = (*lo..=*hi).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).fold(0.0f32, f32::max);
            let db = 20.0 * (peak / full).max(1e-9).log10();
            out[b] = ((db + FLOOR_DB) / FLOOR_DB).clamp(0.0, 1.0);
        }
        out
    }

    /// The analysis of everything taken.
    pub fn finish(mut self) -> Analysis {
        if self.in_current > 0 {
            self.blocks.push(self.current);
        }
        let group = self.blocks.len().div_ceil(BUCKETS).max(1);
        let peaks = self.blocks.chunks(group).map(|g| g.iter().fold((f32::MAX, f32::MIN), |a, b| (a.0.min(b.0), a.1.max(b.1)))).collect();
        Analysis { peaks, slices: self.slices, slice_s: self.every as f64 / self.rate as f64, duration_s: self.frames as f64 / self.rate as f64, sample_rate: self.rate }
    }
}

/// An in-place radix-2 Fourier transform of `re` + i`im` (their length a power of two, `twiddles`
/// the cos and sin of -2 pi k / n for k below n / 2).
fn fft(re: &mut [f32], im: &mut [f32], twiddles: &[(f32, f32)]) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let stride = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (c, s) = twiddles[k * stride];
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Where along the waveform drawn in `rect` a press at `x` is, from 0 (the start) to 1 (the end).
pub fn seek_fraction(rect: Rect, x: f32) -> f32 {
    if rect.width() <= 0.0 {
        return 0.0;
    }
    ((x - rect.left()) / rect.width()).clamp(0.0, 1.0)
}

/// Draw `a` in a `size` area: the spectrum at `at_s` seconds over the top part, the waveform
/// under it with the first `played` (0 to 1) of it in the accent colour and a line where it is.
/// A press or a drag on the waveform comes back as where it is, 0 to 1 (the caller goes there).
pub fn show(ui: &mut Ui, theme: &Theme, a: &Analysis, played: f32, at_s: f64, size: Vec2) -> Option<f32> {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::CornerRadius::same(theme.border_radius as u8), theme.bg_tertiary());
    let pad = theme.spacing_sm;
    let inner = rect.shrink(pad);
    let spectrum_h = if a.slices.is_empty() { 0.0 } else { (inner.height() * 0.4).floor() };
    let spectrum_rect = Rect::from_min_size(inner.min, egui::vec2(inner.width(), spectrum_h));
    let wave = Rect::from_min_max(egui::pos2(inner.left(), inner.top() + spectrum_h + if spectrum_h > 0.0 { pad } else { 0.0 }), inner.max);
    if let Some(levels) = a.spectrum_at(at_s) {
        paint_spectrum(&painter, theme, spectrum_rect, levels);
    }
    paint_waveform(&painter, theme, wave, a, played.clamp(0.0, 1.0));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let pressed = resp.clicked() || resp.dragged();
    pressed.then(|| resp.interact_pointer_pos()).flatten().map(|p| seek_fraction(wave, p.x))
}

/// The bands as bars across `rect`, each as tall as its level.
fn paint_spectrum(painter: &egui::Painter, theme: &Theme, rect: Rect, levels: &[f32; BANDS]) {
    let gap = theme.spacing_xs;
    let w = ((rect.width() - gap * (BANDS as f32 - 1.0)) / BANDS as f32).max(1.0);
    let color = theme.accent().gamma_multiply(0.85);
    let mut mesh = egui::Mesh::default();
    for (b, level) in levels.iter().enumerate() {
        let h = (level * rect.height()).max(1.0);
        let x = rect.left() + b as f32 * (w + gap);
        mesh.add_colored_rect(Rect::from_min_max(egui::pos2(x, rect.bottom() - h), egui::pos2(x + w, rect.bottom())), color);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// The waveform across `rect`, one column a point wide, played columns in the accent colour.
fn paint_waveform(painter: &egui::Painter, theme: &Theme, rect: Rect, a: &Analysis, played: f32) {
    let mid = rect.center().y;
    let half = rect.height() / 2.0;
    painter.line_segment([egui::pos2(rect.left(), mid), egui::pos2(rect.right(), mid)], egui::Stroke::new(1.0, theme.border()));
    if a.peaks.is_empty() {
        return;
    }
    // A quiet recording still fills its height.
    let scale = 1.0 / a.loudest().max(0.05);
    let columns = rect.width().max(1.0) as usize;
    let n = a.peaks.len();
    let (on, off): (Color32, Color32) = (theme.accent(), theme.text_muted());
    let mut mesh = egui::Mesh::default();
    for c in 0..columns {
        let from = c * n / columns;
        let to = ((c + 1) * n / columns).max(from + 1).min(n);
        let (lo, hi) = a.peaks[from..to].iter().fold((f32::MAX, f32::MIN), |acc, p| (acc.0.min(p.0), acc.1.max(p.1)));
        let top = mid - (hi * scale).clamp(-1.0, 1.0) * half;
        let bottom = (mid - (lo * scale).clamp(-1.0, 1.0) * half).max(top + 1.0);
        let x = rect.left() + c as f32;
        let color = if (c as f32 + 0.5) / columns as f32 <= played { on } else { off };
        mesh.add_colored_rect(Rect::from_min_max(egui::pos2(x, top), egui::pos2(x + 1.0, bottom)), color);
    }
    painter.add(egui::Shape::mesh(mesh));
    let x = rect.left() + played * rect.width();
    painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(2.0, theme.text_primary()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A press on the waveform comes back as where along it the press was: the start at its left
    /// edge, the end at its right, a quarter a quarter of the way along; outside it, clamped.
    ///
    /// Seen red 2026-10-19 with the rect's left edge left out of `seek_fraction` (x over the
    /// width): "the left edge is the start" (left: 0.5, right: 0.0).
    #[test]
    fn a_press_maps_to_where_along_the_waveform_it_is() {
        let r = Rect::from_min_size(egui::pos2(200.0, 50.0), egui::vec2(400.0, 100.0));
        assert_eq!(seek_fraction(r, 200.0), 0.0, "the left edge is the start");
        assert_eq!(seek_fraction(r, 600.0), 1.0, "the right edge is the end");
        assert!((seek_fraction(r, 300.0) - 0.25).abs() < 1e-6, "a quarter of the way along");
        assert_eq!(seek_fraction(r, 10.0), 0.0, "left of it, the start");
        assert_eq!(seek_fraction(r, 900.0), 1.0, "right of it, the end");
    }

    /// The Fourier transform puts a sine's strength in its own bin.
    ///
    /// Seen red 2026-10-19 with the transform's bit reversal left out: (left: Some(16), right:
    /// Some(32)).
    #[test]
    fn the_transform_finds_a_sine_in_its_bin() {
        let mut re: Vec<f32> = (0..FFT).map(|n| (std::f32::consts::TAU * 32.0 * n as f32 / FFT as f32).sin()).collect();
        let mut im = vec![0.0; FFT];
        fft(&mut re, &mut im, &Builder::new(8000).twiddles);
        let mags: Vec<f32> = (0..FFT / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).collect();
        let best = mags.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(k, _)| k);
        assert_eq!(best, Some(32));
        assert!((mags[32] - FFT as f32 / 2.0).abs() < 1.0, "{}", mags[32]);
    }
}
