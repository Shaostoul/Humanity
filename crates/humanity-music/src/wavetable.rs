//! The wavetable (docs/design/music.md section 7f, rung E5): single-cycle waves (the core set's AKWF
//! waves, `data/music/waves/`, or an installed pack's) read band-limited, so a high note never
//! aliases, and morphed from one wave to the next.
//!
//! The method is Nigel Redmon's ("A wavetable oscillator", earlevel.com, Part 2, 2012-05-08, read
//! 2026-11-14): one table per octave of pitch, each higher table keeping only the harmonics that stay
//! under the Nyquist frequency there ("I'll continue to develop this oscillator in octaves"). Here
//! the tables step by HALF octaves of harmonic count (1, 2, 3, 4, 6, 8, 11, 16 ... up to the wave's
//! own limit), so a note loses at most half an octave of its top harmonics where an octave table
//! would lose a whole one, and the oscillator always picks the table whose top harmonic is under
//! Nyquist at the note's pitch: no harmonic folds back at all (Redmon's octave tables accept a small
//! fold in the lowest; these do not). Each wave's harmonics come from a discrete Fourier transform of
//! its one cycle, and each table is that sum of sines written out again at a power-of-two length
//! (at least eight samples a cycle of its top harmonic, so reading between samples in a straight
//! line adds little), with a guard sample on the end. All of it on the game thread when the data is
//! read (`MusicLibrary::load`), never kept across runs (CLAUDE.md, the no-caches rule): the audio
//! thread only reads the finished tables.

use std::path::{Path, PathBuf};

/// The harmonic counts of the tables, rising by about half an octave (each a little over 1.41 times
/// the last), capped at a wave's own limit (half its length: a 600-sample AKWF cycle holds 300).
const COUNTS: [u32; 17] = [1, 2, 3, 4, 6, 8, 11, 16, 23, 32, 45, 64, 91, 128, 181, 256, 362];
/// The longest table, samples.
const MAX_LEN: usize = 2048;
/// The shortest table, samples.
const MIN_LEN: usize = 256;

/// One table: its top harmonic, its length (a power of two) and where it starts in the wave's run.
#[derive(Clone, Copy, Debug)]
struct Level {
    count: u32,
    len: usize,
    mask: usize,
    off: usize,
}

/// Every wave of one instrument, band-limited: `levels` describes the tables every wave has (the
/// same counts and lengths for each), `data` holds them, wave after wave.
pub struct WaveTable {
    levels: Vec<Level>,
    stride: usize,
    waves: usize,
    data: Vec<f32>,
}

impl std::fmt::Debug for WaveTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WaveTable({} waves, {} tables each, {} KiB)", self.waves, self.levels.len(), self.data.len() * 4 / 1024)
    }
}

impl WaveTable {
    /// Band-limit `cycles` (each one cycle of a wave, any length of 4 samples or more). Each wave is
    /// scaled so its loudness (RMS) is a sawtooth's, 1/sqrt(3), so a wavetable sits at a saw synth's
    /// level and a morph does not swell or sag; its offset (harmonic 0) is dropped.
    pub fn build(cycles: &[Vec<f32>]) -> Result<WaveTable, String> {
        if cycles.is_empty() {
            return Err("a wavetable needs at least one wave".into());
        }
        let limit = cycles.iter().map(|c| c.len() as u32 / 2).min().unwrap_or(0);
        if limit < 1 {
            return Err("a wave is shorter than 4 samples".into());
        }
        // The tables' counts: every step under the limit, then the limit itself.
        let mut counts: Vec<u32> = COUNTS.iter().copied().filter(|&c| c < limit).collect();
        counts.push(limit);
        let mut levels = Vec::with_capacity(counts.len());
        let mut off = 0;
        for &count in &counts {
            let len = ((count as usize * 8).next_power_of_two()).clamp(MIN_LEN, MAX_LEN);
            levels.push(Level { count, len, mask: len - 1, off });
            off += len + 1;
        }
        let stride = off;
        let mut data = vec![0f32; stride * cycles.len()];
        for (w, cyc) in cycles.iter().enumerate() {
            let (a, b) = harmonics(cyc, limit as usize);
            let power: f64 = a.iter().zip(&b).map(|(x, y)| (x * x + y * y) * 0.5).sum();
            let scale = if power > 1e-12 { (1.0 / 3f64).sqrt() / power.sqrt() } else { 0.0 };
            for lv in &levels {
                let out = &mut data[w * stride + lv.off..w * stride + lv.off + lv.len + 1];
                synthesise(&a, &b, lv.count as usize, scale, &mut out[..lv.len]);
                out[lv.len] = out[0];
            }
        }
        Ok(WaveTable { levels, stride, waves: cycles.len(), data })
    }

    /// How many waves it morphs through.
    pub fn waves(&self) -> usize {
        self.waves
    }

    /// The table to read at a phase step of `dt` cycles a sample: the one with the most harmonics
    /// whose top harmonic stays under Nyquist (`count * dt < 0.5`), so nothing folds back.
    #[inline]
    pub fn level_for(&self, dt: f32) -> usize {
        let room = 0.5 / dt.max(1e-9);
        let mut best = 0;
        for (i, l) in self.levels.iter().enumerate() {
            if (l.count as f32) < room {
                best = i;
            } else {
                break;
            }
        }
        best
    }

    /// The top harmonic of table `level`.
    pub fn level_count(&self, level: usize) -> u32 {
        self.levels[level.min(self.levels.len() - 1)].count
    }

    /// Wave `wave`'s table `level` at phase `ph` (0 .. 1), read between its samples in a straight line.
    #[inline]
    pub fn read(&self, wave: usize, level: usize, ph: f32) -> f32 {
        let l = &self.levels[level];
        let x = ph * l.len as f32;
        let i = x as usize;
        let fr = x - i as f32;
        let base = wave * self.stride + l.off + (i & l.mask);
        let (a, b) = (self.data[base], self.data[base + 1]);
        a + (b - a) * fr
    }

    /// The morph: waves `pos` (0 .. 1 across them) read at table `level` and phase `ph`, the two
    /// waves either side of it crossfaded.
    #[inline]
    pub fn morph(&self, pos: f32, level: usize, ph: f32) -> f32 {
        if self.waves == 1 {
            return self.read(0, level, ph);
        }
        let x = pos.clamp(0.0, 1.0) * (self.waves - 1) as f32;
        let i = (x as usize).min(self.waves - 2);
        let t = x - i as f32;
        let a = self.read(i, level, ph);
        if t <= 0.0 {
            return a;
        }
        let b = self.read(i + 1, level, ph);
        a + (b - a) * t
    }
}

/// The cosine and sine amplitudes of harmonics 1 ..= `limit` of one cycle (a discrete Fourier
/// transform; the angles from one table of the cycle's length, so no angle drifts).
fn harmonics(cyc: &[f32], limit: usize) -> (Vec<f64>, Vec<f64>) {
    let n = cyc.len();
    let tw: Vec<(f64, f64)> = (0..n).map(|k| {
        let w = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
        (w.cos(), w.sin())
    }).collect();
    let mut a = vec![0f64; limit];
    let mut b = vec![0f64; limit];
    for h in 1..=limit {
        let (mut sa, mut sb) = (0.0, 0.0);
        let mut idx = 0usize;
        for &x in cyc {
            let (c, s) = tw[idx];
            sa += x as f64 * c;
            sb += x as f64 * s;
            idx += h;
            if idx >= n {
                idx -= n;
            }
        }
        a[h - 1] = sa * 2.0 / n as f64;
        b[h - 1] = sb * 2.0 / n as f64;
    }
    (a, b)
}

/// Harmonics 1 ..= `count` written out over one cycle of `out.len()` samples (a power of two), times `scale`.
fn synthesise(a: &[f64], b: &[f64], count: usize, scale: f64, out: &mut [f32]) {
    let len = out.len();
    let tw: Vec<(f64, f64)> = (0..len).map(|k| {
        let w = 2.0 * std::f64::consts::PI * k as f64 / len as f64;
        (w.cos(), w.sin())
    }).collect();
    let mut acc = vec![0f64; len];
    for h in 1..=count.min(a.len()) {
        let (ah, bh) = (a[h - 1], b[h - 1]);
        if ah * ah + bh * bh < 1e-16 {
            continue;
        }
        let mut idx = 0usize;
        for v in acc.iter_mut() {
            let (c, s) = tw[idx];
            *v += ah * c + bh * s;
            idx = (idx + h) & (len - 1);
        }
    }
    for (o, v) in out.iter_mut().zip(acc) {
        *o = (v * scale) as f32;
    }
}

/// One cycle from a WAV file: 8, 16, 24 or 32-bit integer PCM or 32-bit float, its channels averaged.
/// The AKWF waves are 16-bit mono, 600 samples; the set's `waves.ron` (written by
/// scripts/instrument-packs.js) lists them.
pub fn read_wav_cycle(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let mut pos = 12;
    let mut fmt: Option<(u16, u16, u16)> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32_at(pos + 4) as usize;
        let body = pos + 8;
        let end = (body + size).min(bytes.len());
        if id == b"fmt " && size >= 16 {
            fmt = Some((u16_at(body), u16_at(body + 2), u16_at(body + 14)));
        } else if id == b"data" {
            let (format, channels, bits) = fmt.ok_or("the data comes before its format")?;
            let channels = channels.max(1) as usize;
            let width = (bits as usize).div_ceil(8);
            // 1: integer PCM, 3: float, 0xFFFE: the extensible header (read as integer PCM).
            let float = format == 3;
            if !(format == 1 || format == 3 || format == 0xFFFE) || !(1..=4).contains(&width) {
                return Err(format!("an unsupported WAV format ({format}, {bits} bits)"));
            }
            let frame = width * channels;
            let data = &bytes[body..end];
            let mut out = Vec::with_capacity(data.len() / frame.max(1));
            for f in data.chunks_exact(frame) {
                let mut sum = 0f32;
                for c in 0..channels {
                    let s = &f[c * width..(c + 1) * width];
                    sum += match (width, float) {
                        (1, _) => (s[0] as f32 - 128.0) / 128.0,
                        (2, _) => i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0,
                        (3, _) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0,
                        (4, true) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
                        _ => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
                    };
                }
                out.push(sum / channels as f32);
            }
            if out.len() < 4 {
                return Err("the wave has fewer than 4 samples".into());
            }
            return Ok(out);
        }
        pos = body + size + (size & 1);
    }
    Err("no data in the WAV file".into())
}

/// Where wave `name` (`<set>/<file>`) is: the core set's `data/music/waves/<set>/<file>`, else the
/// first installed pack that holds the set (`data/music/packs/<pack>/waves/<set>/<file>`).
pub fn find_wave(data_dir: &Path, name: &str) -> Option<PathBuf> {
    let (set, file) = name.split_once('/')?;
    if set.is_empty() || file.is_empty() || name.contains("..") {
        return None;
    }
    let music = data_dir.join("music");
    let core = music.join("waves").join(set).join(file);
    if core.is_file() {
        return Some(core);
    }
    let mut packs: Vec<PathBuf> = std::fs::read_dir(music.join("packs")).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    packs.sort();
    packs.into_iter().map(|p| p.join("waves").join(set).join(file)).find(|p| p.is_file())
}

/// Read and band-limit the waves `names` from `data_dir` (an error names the first wave missing or
/// unreadable).
pub fn load(data_dir: &Path, names: &[String]) -> Result<WaveTable, String> {
    let mut cycles = Vec::with_capacity(names.len());
    for n in names {
        let path = find_wave(data_dir, n).ok_or_else(|| format!("wave {n:?} is in neither data/music/waves nor an installed pack"))?;
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        cycles.push(read_wav_cycle(&bytes).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    WaveTable::build(&cycles)
}
