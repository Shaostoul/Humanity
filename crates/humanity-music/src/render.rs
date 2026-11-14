//! Offline rendering: the real engines, faster than real time, into memory and to a WAV file
//! (`just music-render`, the tests). The WAV goes wherever the caller says (the scratchpad or the
//! system temp folder): a render is never written into the repository.

use crate::bed::{BedContext, BedEngine};
use crate::score::{MusicContext, ScoreEngine};

/// The block kira asks for (128 frames), so offline timing is the audio thread's.
pub const BLOCK: usize = 128;

/// A finished render: stereo frames and how long each block took.
pub struct Rendered {
    pub frames: Vec<[f32; 2]>,
    pub rate: f32,
    /// Microseconds per 128-frame block: the mean and the worst.
    pub block_us_mean: f64,
    pub block_us_worst: f64,
    pub blocks: usize,
}

/// Render `seconds` of a score in 128-frame blocks, timing each.
pub fn render_score(engine: &mut ScoreEngine, rate: f32, seconds: f64) -> Rendered {
    let total = (seconds * rate as f64) as usize;
    let mut frames = vec![[0.0f32; 2]; total];
    let mut sum = 0.0;
    let mut worst = 0.0f64;
    let mut blocks = 0;
    let mut i = 0;
    while i < total {
        let n = BLOCK.min(total - i);
        let t0 = std::time::Instant::now();
        engine.render(&mut frames[i..i + n]);
        let us = t0.elapsed().as_secs_f64() * 1e6;
        engine.note_block_time(us as f32);
        sum += us;
        worst = worst.max(us);
        blocks += 1;
        i += n;
    }
    Rendered { frames, rate, block_us_mean: sum / blocks.max(1) as f64, block_us_worst: worst, blocks }
}

/// Fade the last `fade_s` seconds of `frames` out with a raised cosine (a listening file ends; the
/// game's score never does). `just music-render` and the Music page's export both end this way.
pub fn fade_out(frames: &mut [[f32; 2]], rate: f32, fade_s: f64) {
    let fade_n = (fade_s.max(0.0) * rate as f64) as usize;
    let len = frames.len();
    for (k, f) in frames.iter_mut().enumerate().skip(len.saturating_sub(fade_n)) {
        let x = (k + fade_n - len) as f32 / fade_n.max(1) as f32;
        let g = 0.5 + 0.5 * (std::f32::consts::PI * x).cos();
        f[0] *= g;
        f[1] *= g;
    }
}

/// The fade a listening file of `seconds` ends with: 6 s, or a quarter of a short file.
pub fn default_fade_s(seconds: f64) -> f64 {
    6.0f64.min(seconds / 4.0)
}

/// The WAV a favourite exports to (docs/design/music-page.md section 8): its piece rendered with `res`
/// (the instruments the game plays it with) for `seconds` in the score's 128-frame blocks, the
/// default fade, at 48 kHz, written to `path` AS IT RENDERS (an hour of it would be 1.4 GB held in
/// memory). The file is byte for byte what rendering it all, [`fade_out`] and [`write_wav`] give, as
/// `just music-render` does (a test holds the two paths to it). Returns how many samples clipped (0
/// from a levelled score).
pub fn export_favourite(res: crate::Resolved, lib: &crate::MusicLibrary, fav: &crate::favourite::Favourite, a4: f32, seconds: f64, path: &std::path::Path) -> Result<usize, String> {
    let ctx = fav.context(lib, a4)?;
    let rate = EXPORT_RATE;
    let mut engine = ScoreEngine::new(res, rate, fav.seed, ctx);
    // Faster than real time: a streamed recording waits for its stream (crate::stream).
    engine.set_offline(true);
    let total = (seconds * rate as f64) as usize;
    let fade_n = (default_fade_s(seconds) * rate as f64) as usize;
    let err = |e: std::io::Error| format!("{} could not be written: {e}", path.display());
    let mut w = WavWriter::create(path, rate as u32, total).map_err(err)?;
    let mut buf = [[0.0f32; 2]; BLOCK];
    let mut i = 0;
    while i < total {
        let n = BLOCK.min(total - i);
        engine.render(&mut buf[..n]);
        // The fade, as fade_out gives it over the whole file.
        for (j, f) in buf[..n].iter_mut().enumerate() {
            let k = i + j;
            if k + fade_n >= total {
                let x = (k + fade_n - total) as f32 / fade_n.max(1) as f32;
                let g = 0.5 + 0.5 * (std::f32::consts::PI * x).cos();
                f[0] *= g;
                f[1] *= g;
            }
        }
        w.write(&buf[..n]).map_err(err)?;
        i += n;
    }
    w.finish().map_err(err)
}

/// A 16-bit PCM WAV written frame by frame, with triangular dither as [`write_wav`] does (the export).
pub struct WavWriter {
    out: std::io::BufWriter<std::fs::File>,
    rng: crate::rng::Rng,
    clipped: usize,
}

impl WavWriter {
    /// A WAV at `path` that will hold `frames` stereo frames at `rate` Hz (the header says so).
    pub fn create(path: &std::path::Path, rate: u32, frames: usize) -> std::io::Result<WavWriter> {
        use std::io::Write;
        let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
        let data_len = (frames * 4) as u32;
        out.write_all(b"RIFF")?;
        out.write_all(&(36 + data_len).to_le_bytes())?;
        out.write_all(b"WAVEfmt ")?;
        out.write_all(&16u32.to_le_bytes())?;
        out.write_all(&1u16.to_le_bytes())?;
        out.write_all(&2u16.to_le_bytes())?;
        out.write_all(&rate.to_le_bytes())?;
        out.write_all(&(rate * 4).to_le_bytes())?;
        out.write_all(&4u16.to_le_bytes())?;
        out.write_all(&16u16.to_le_bytes())?;
        out.write_all(b"data")?;
        out.write_all(&data_len.to_le_bytes())?;
        Ok(WavWriter { out, rng: crate::rng::Rng::new(0xd1_7e4), clipped: 0 })
    }

    /// Write `frames`, dithered to 16 bits.
    pub fn write(&mut self, frames: &[[f32; 2]]) -> std::io::Result<()> {
        use std::io::Write;
        for f in frames {
            for &x in f {
                let d = self.rng.f32() - self.rng.f32();
                let mut s = (x * 32767.0 + d).round();
                if !(-32768.0..=32767.0).contains(&s) {
                    self.clipped += 1;
                    s = s.clamp(-32768.0, 32767.0);
                }
                self.out.write_all(&(s as i16).to_le_bytes())?;
            }
        }
        Ok(())
    }

    /// Finish the file; how many samples clipped.
    pub fn finish(mut self) -> std::io::Result<usize> {
        use std::io::Write;
        self.out.flush()?;
        Ok(self.clipped)
    }
}

/// The rate exports are written at (what the game plays at, and every player reads).
pub const EXPORT_RATE: f32 = 48000.0;

/// Render `seconds` of a bed in 128-frame blocks.
pub fn render_bed(engine: &mut BedEngine, rate: f32, seconds: f64) -> Rendered {
    let total = (seconds * rate as f64) as usize;
    let mut frames = vec![[0.0f32; 2]; total];
    let mut sum = 0.0;
    let mut worst = 0.0f64;
    let mut blocks = 0;
    let mut i = 0;
    while i < total {
        let n = BLOCK.min(total - i);
        let t0 = std::time::Instant::now();
        engine.render(&mut frames[i..i + n]);
        let us = t0.elapsed().as_secs_f64() * 1e6;
        sum += us;
        worst = worst.max(us);
        blocks += 1;
        i += n;
    }
    Rendered { frames, rate, block_us_mean: sum / blocks.max(1) as f64, block_us_worst: worst, blocks }
}

/// How loud one note of instrument `inst` is (RMS over `seconds`, velocity 1, no reverb): the
/// instrument library levels a recording to the synthesised voice it replaces with it, so the
/// mood's balance holds whichever plays.
pub fn note_rms(res: &crate::Resolved, inst: u16, midi: f32, a4: f32, seconds: f32, rate: f32) -> f32 {
    note_rms_between(res, inst, midi, a4, 0.0, seconds, rate)
}

/// The RMS of one held note of instrument `inst` between `from_s` and `to_s` after it starts: a
/// pad's or a drone's sustained level, past its slow attack (the instrument library levels a
/// sustained recording against it, BUG-266: measured over the first second, a slow pad read 5 to 8
/// dB under its sustain, and the strings that replaced it played that much too quiet).
pub fn note_rms_between(res: &crate::Resolved, inst: u16, midi: f32, a4: f32, from_s: f32, to_s: f32, rate: f32) -> f32 {
    use crate::dsp::SvfCoefs;
    use crate::voice::{BlockParams, NoteOn, VoicePool};
    let mut pool = VoicePool::new(1);
    pool.note_on(NoteOn { inst, role: 0, midi, a4, vel: 1.0, pan: 0.0, hold_s: 0.0, tag: 1, level: 1.0 }, &res.instruments, &res.banks, rate);
    let vowel = [(SvfCoefs::new(800.0, 10.0, rate), 1.0); 5];
    let p = BlockParams::plain(rate, 1.0, &vowel);
    let total = (to_s * rate) as usize;
    let skip = (from_s.max(0.0) * rate) as usize;
    let (mut sum, mut n, mut counted) = (0.0f64, 0usize, 0usize);
    let (mut l, mut r, mut sl, mut sr) = ([0f32; 64], [0f32; 64], [0f32; 64], [0f32; 64]);
    while n < total {
        l.fill(0.0);
        r.fill(0.0);
        pool.render(&res.instruments, &res.banks, &p, &mut l, &mut r, &mut sl, &mut sr, 64);
        for i in 0..64 {
            if n + i >= skip {
                sum += (l[i] as f64).powi(2) + (r[i] as f64).powi(2);
                counted += 1;
            }
        }
        n += 64;
    }
    (sum / (2 * counted.max(1)) as f64).sqrt() as f32
}

/// A default context for offline renders: full gain, A = 432, the given mood and genre.
pub fn context(mood: u16, genre: u16) -> MusicContext {
    MusicContext { mood, genre_a: genre, genre_b: genre, ..Default::default() }
}

/// A default bed context: aboard, in an open space, D, A = 432, gain 1.
pub fn bed_context() -> BedContext {
    BedContext::default()
}

/// One instrument by itself, to listen to (docs/design/music.md section 7f; `pieces_to_listen_to`'s
/// `instrument:<id>` items): `seconds` at 120 bpm in D minor at `a4`, through the score's reverb. A
/// drum-machine voice (or a membrane or noise drum) plays a groove (quarters, eighths with accents,
/// sixteenths, a syncopated bar; a kick at A1, a tom stepping through three pitches, the rest at D3);
/// a growl or a bass a riff of chord tones in eighths low down; anything else the chords D minor, B
/// flat, F and C two bars each, held the first time through and arpeggiated the next. Levelled to
/// about -20 dBFS RMS, its peaks under -1 dBFS. A recorded instrument plays its synthesised fallback.
pub fn render_demo(lib: &crate::MusicLibrary, id: &str, a4: f32, seconds: f64, rate: f32) -> Result<Rendered, String> {
    use crate::instrument::InstrumentKind;
    use crate::voice::{BlockParams, NoteOn, VoicePool};
    let mut idx = lib.instrument_index(id).ok_or_else(|| format!("{id:?} is not in data/music/instruments"))?;
    if let InstrumentKind::Sampler(s) = &lib.instruments[idx].kind {
        idx = lib.instrument_index(&s.fallback).ok_or_else(|| format!("{id}: its fallback {:?} is not an instrument", s.fallback))?;
    }
    let inst = lib.instruments[idx].clone();
    let drum = matches!(inst.kind, InstrumentKind::DrumMachine(_) | InstrumentKind::Membrane(_) | InstrumentKind::Noise(_));
    let low = id.contains("growl") || id.contains("bass");
    let beat = 0.5f64;
    let bar = 4.0 * beat;
    // (start s, midi, held s, velocity), for 16 bars, then repeated.
    let mut notes: Vec<(f64, f32, f32, f32)> = Vec::new();
    if drum {
        let midi = |k: usize| if id.contains("kick") { 33.0 } else if id.contains("tom") { [41.0, 45.0, 50.0][k % 3] } else { 50.0 };
        let bars: [&[(usize, f32)]; 4] = [
            &[(0, 1.0), (4, 0.8), (8, 1.0), (12, 0.8)],
            &[(0, 1.0), (2, 0.5), (4, 0.8), (6, 0.5), (8, 1.0), (10, 0.5), (12, 0.8), (14, 0.5)],
            &[(0, 1.0), (1, 0.4), (2, 0.6), (3, 0.4), (4, 0.9), (5, 0.4), (6, 0.6), (7, 0.4), (8, 1.0), (9, 0.4), (10, 0.6), (11, 0.4), (12, 0.9), (13, 0.5), (14, 0.7), (15, 0.6)],
            &[(0, 1.0), (3, 0.7), (6, 0.8), (10, 0.7), (12, 0.9), (14, 0.6)],
        ];
        let mut k = 0;
        for b in 0..16 {
            for &(step, vel) in bars[b % 4] {
                notes.push((b as f64 * bar + step as f64 * beat / 4.0, midi(k), 0.1, vel));
                k += 1;
            }
        }
    } else {
        // D minor, B flat, F, C (as MIDI notes in the third octave).
        let chords: [[f32; 3]; 4] = [[50.0, 53.0, 57.0], [46.0, 50.0, 53.0], [53.0, 57.0, 60.0], [48.0, 52.0, 55.0]];
        for b in 0..16 {
            let c = chords[(b / 2) % 4];
            let t0 = b as f64 * bar;
            if low {
                let riff = [0usize, 0, 2, 0, 1, 0, 2, 1];
                for (s, &n) in riff.iter().enumerate() {
                    notes.push((t0 + s as f64 * beat / 2.0, c[n] - 24.0, 0.2, if s % 2 == 0 { 0.9 } else { 0.7 }));
                }
            } else if b < 8 {
                if b % 2 == 0 {
                    for &n in &c {
                        notes.push((t0, n, (2.0 * bar - 0.15) as f32, 0.8));
                    }
                }
            } else {
                let arp = [c[0], c[1], c[2], c[1] + 12.0, c[2], c[1], c[0] + 12.0, c[2]];
                for (s, &n) in arp.iter().enumerate() {
                    notes.push((t0 + s as f64 * beat / 2.0, n, 0.22, 0.75));
                }
            }
        }
    }
    let span = 16.0 * bar;
    let insts = vec![inst];
    let banks = vec![None];
    let mut pool = VoicePool::new(7);
    let mut reverb = crate::dsp::Fdn::new(2.4, rate);
    let vowel = [(crate::dsp::SvfCoefs::new(800.0, 10.0, rate), 1.0); 5];
    let total = (seconds * rate as f64) as usize;
    let mut frames = Vec::with_capacity(total);
    let (mut dl, mut dr, mut sl, mut sr) = ([0f32; 64], [0f32; 64], [0f32; 64], [0f32; 64]);
    let mut next = 0usize;
    let mut tag = 0u32;
    while frames.len() < total {
        let t = frames.len() as f64 / rate as f64;
        while notes[next % notes.len()].0 + (next / notes.len()) as f64 * span <= t {
            let (_, midi, hold, vel) = notes[next % notes.len()];
            tag += 1;
            pool.note_on(NoteOn { inst: 0, role: 1, midi, a4, vel, pan: 0.0, hold_s: hold, tag, level: 1.0 }, &insts, &banks, rate);
            next += 1;
        }
        let beats = t / beat;
        let mut p = BlockParams::plain(rate, (1.0 / beat) as f32, &vowel);
        p.bar = (beats / 4.0).floor() as u64;
        p.beats = beats;
        for b in [&mut dl, &mut dr, &mut sl, &mut sr] {
            b.fill(0.0);
        }
        pool.render(&insts, &banks, &p, &mut dl, &mut dr, &mut sl, &mut sr, 64);
        reverb.process(&sl, &sr, &mut dl, &mut dr, 0.5);
        frames.extend(dl.iter().zip(dr.iter()).map(|(l, r)| [*l, *r]));
    }
    frames.truncate(total);
    let rms = (frames.iter().map(|f| (f[0] * f[0] + f[1] * f[1]) as f64).sum::<f64>() / (2 * frames.len().max(1)) as f64).sqrt();
    let peak = frames.iter().map(|f| f[0].abs().max(f[1].abs())).fold(0.0, f32::max) as f64;
    let gain = if rms > 1e-9 { (0.1 / rms).min(0.89 / peak.max(1e-9)) } else { 1.0 } as f32;
    for f in frames.iter_mut() {
        f[0] *= gain;
        f[1] *= gain;
    }
    fade_out(&mut frames, rate, 2.0);
    Ok(Rendered { frames, rate, block_us_mean: 0.0, block_us_worst: 0.0, blocks: 0 })
}

/// Write stereo frames as a 16-bit PCM WAV with triangular dither.
pub fn write_wav(path: &std::path::Path, frames: &[[f32; 2]], rate: u32) -> std::io::Result<usize> {
    let n = frames.len();
    let mut buf = Vec::with_capacity(44 + n * 4);
    let data_len = (n * 4) as u32;
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&rate.to_le_bytes());
    buf.extend_from_slice(&(rate * 4).to_le_bytes());
    buf.extend_from_slice(&4u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    let mut rng = crate::rng::Rng::new(0xd1_7e4);
    let mut clipped = 0;
    for f in frames {
        for &x in f {
            let d = rng.f32() - rng.f32();
            let mut s = (x * 32767.0 + d).round();
            if !(-32768.0..=32767.0).contains(&s) {
                clipped += 1;
                s = s.clamp(-32768.0, 32767.0);
            }
            buf.extend_from_slice(&(s as i16).to_le_bytes());
        }
    }
    std::fs::write(path, buf)?;
    Ok(clipped)
}
