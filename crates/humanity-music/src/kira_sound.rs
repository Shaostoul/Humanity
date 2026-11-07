//! The score and the bed on the sound card: kira 0.12 `Sound`s over the pure engines
//! (docs/design/music.md, section 4.2).
//!
//! Each is a [`kira::sound::SoundData`] the game hands to `AudioManager::play_data`
//! (humanity_core::audio). kira calls `process` on its audio thread for each block (128 frames by
//! default, 2.67 ms at 48 kHz); `process` renders the engine into a buffer made at start and copies
//! it out, allocating nothing (kira: "should not allocate or deallocate memory"). The game talks to a
//! playing sound through kira's latest-value channels (`command_writer_and_reader`): a `Copy`
//! context in, a `Copy` status out, both triple buffers, so neither side ever waits. Dropping the
//! handle (or `stop`) fades the sound out over about two seconds, then kira unloads it.
//!
//! The sample rate comes from kira's own `dt` (1 / rate) on every block, so a device at 44.1 or 96 kHz
//! renders right without the game knowing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use kira::command::{command_writer_and_reader, CommandReader, CommandWriter};
use kira::info::Info;
use kira::sound::{Sound, SoundData};
use kira::Frame;

use crate::bed::{BedContext, BedEngine, BedStatus};
use crate::score::{MusicContext, MusicStatus, ScoreEngine};

/// The most frames kira asks for at once that the buffer holds (it asks 128; larger asks are split).
const BUF: usize = 1024;

/// A score ready to play.
pub struct ScoreSoundData {
    pub engine: ScoreEngine,
}

/// The game's handle on a playing score.
pub struct ScoreHandle {
    ctx: CommandWriter<MusicContext>,
    status: CommandReader<MusicStatus>,
    last: MusicStatus,
    stop: Arc<AtomicBool>,
}

impl ScoreHandle {
    /// Send the latest context (kept if the audio thread has not read the last one yet: only the
    /// newest matters).
    pub fn set_context(&mut self, ctx: MusicContext) {
        self.ctx.write(ctx);
    }

    /// The latest status the audio thread sent.
    pub fn status(&mut self) -> MusicStatus {
        if let Some(s) = self.status.read() {
            self.last = s;
        }
        self.last
    }

    /// Fade out and end.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for ScoreHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct ScoreSound {
    engine: ScoreEngine,
    ctx: CommandReader<MusicContext>,
    status: CommandWriter<MusicStatus>,
    stop: Arc<AtomicBool>,
    buf: Vec<[f32; 2]>,
}

impl SoundData for ScoreSoundData {
    type Error = std::convert::Infallible;
    type Handle = ScoreHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let (ctx_w, ctx_r) = command_writer_and_reader();
        let (st_w, st_r) = command_writer_and_reader();
        let stop = Arc::new(AtomicBool::new(false));
        let sound = ScoreSound { engine: self.engine, ctx: ctx_r, status: st_w, stop: stop.clone(), buf: vec![[0.0; 2]; BUF] };
        Ok((Box::new(sound), ScoreHandle { ctx: ctx_w, status: st_r, last: MusicStatus::default(), stop }))
    }
}

impl Sound for ScoreSound {
    fn process(&mut self, out: &mut [Frame], dt: f64, _info: &Info) {
        let t0 = std::time::Instant::now();
        if let Some(c) = self.ctx.read() {
            self.engine.set_context(c);
        }
        if self.stop.load(Ordering::Relaxed) {
            self.engine.stop();
        }
        if dt > 0.0 {
            self.engine.set_rate((1.0 / dt) as f32);
        }
        for chunk in out.chunks_mut(BUF) {
            let buf = &mut self.buf[..chunk.len()];
            self.engine.render(buf);
            for (o, s) in chunk.iter_mut().zip(buf.iter()) {
                *o = Frame { left: s[0], right: s[1] };
            }
        }
        self.engine.note_block_time(t0.elapsed().as_secs_f32() * 1e6);
        self.status.write(self.engine.status());
    }

    fn finished(&self) -> bool {
        self.engine.finished()
    }
}

/// A bed (the hum and the tones) ready to play.
pub struct BedSoundData {
    pub engine: BedEngine,
}

/// The game's handle on a playing bed.
pub struct BedHandle {
    ctx: CommandWriter<BedContext>,
    status: CommandReader<BedStatus>,
    last: BedStatus,
    stop: Arc<AtomicBool>,
}

impl BedHandle {
    /// Send the latest context.
    pub fn set_context(&mut self, ctx: BedContext) {
        self.ctx.write(ctx);
    }

    /// The latest status.
    pub fn status(&mut self) -> BedStatus {
        if let Some(s) = self.status.read() {
            self.last = s;
        }
        self.last
    }

    /// Fade out and end.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for BedHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct BedSound {
    engine: BedEngine,
    ctx: CommandReader<BedContext>,
    status: CommandWriter<BedStatus>,
    stop: Arc<AtomicBool>,
    buf: Vec<[f32; 2]>,
}

impl SoundData for BedSoundData {
    type Error = std::convert::Infallible;
    type Handle = BedHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let (ctx_w, ctx_r) = command_writer_and_reader();
        let (st_w, st_r) = command_writer_and_reader();
        let stop = Arc::new(AtomicBool::new(false));
        let sound = BedSound { engine: self.engine, ctx: ctx_r, status: st_w, stop: stop.clone(), buf: vec![[0.0; 2]; BUF] };
        Ok((Box::new(sound), BedHandle { ctx: ctx_w, status: st_r, last: BedStatus::default(), stop }))
    }
}

#[cfg(test)]
mod device_tests {
    //! On a REAL sound device, through kira 0.12: a script's boot of the game opens no audio device
    //! (engine/launch_focus.rs), so `just verify-runtime` cannot hear the music; these carry the
    //! audio proof. Silent (gain 0: the operator may be using his PC) but fully rendered: the engine
    //! runs on kira's audio thread and reports back through the status channel. Run by hand:
    //! `cargo test -p humanity-music --lib -- --ignored device`.
    use super::*;
    use crate::tests::shipped;
    use crate::{MusicContext, Resolved, SampleLibrary};

    fn manager() -> Option<kira::AudioManager> {
        match kira::AudioManager::<kira::DefaultBackend>::new(kira::AudioManagerSettings::default()) {
            Ok(m) => Some(m),
            Err(e) => {
                println!("skipped: no audio device ({e})");
                None
            }
        }
    }

    /// THE SCORE PLAYS ON A DEVICE (rung 1): started through kira, it renders on the audio thread
    /// (its status moves: seconds, blocks, a mood and a key), takes a new context through the
    /// latest-value channel (the danger intensity reaches the composer: the tempo rises), costs within
    /// the budget on the real audio thread, and fades out and ends when its handle is dropped. RED
    /// first (2026-11-06) with `process` reading no context: "the context reached the score (tempo
    /// 56.1 bpm after the intensity rose)". Green the same day: 0.085 ms a block on the real audio
    /// thread in calm (the operator's game running beside it), 129.7 bpm in action after the rise.
    #[test]
    #[ignore = "needs an audio output device; run: cargo test -p humanity-music --lib -- --ignored device"]
    fn device_the_score_plays_and_takes_its_context() {
        let Some(mut m) = manager() else { return };
        let lib = shipped();
        let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
        // -60 dB: inaudible on the operator's speakers, but enough for the fade to show.
        let ctx = MusicContext { gain: 0.001, intensity: 0.0, ..Default::default() };
        let mut h = m.play(ScoreSoundData { engine: ScoreEngine::new(res, 48000.0, 9, ctx) }).expect("plays");
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let s = h.status();
        println!("after 1.5 s: {s:?}");
        assert!(s.seconds > 1.0 && s.blocks > 100, "it renders on the audio thread: {s:?}");
        assert!((56.0..=64.0).contains(&s.tempo_bpm), "calm tempo {}", s.tempo_bpm);
        assert!(s.block_us_mean / 1000.0 < 0.25, "mean {:.4} ms a block on the audio thread", s.block_us_mean / 1000.0);
        h.set_context(MusicContext { intensity: 0.95, ..ctx });
        std::thread::sleep(std::time::Duration::from_millis(12000));
        let s = h.status();
        println!("after the intensity rose: {s:?}");
        assert!(s.tempo_bpm > 100.0, "the context reached the score (tempo {:.1} bpm after the intensity rose)", s.tempo_bpm);
        assert!(s.gain > 5e-4, "playing at its gain: {}", s.gain);
        h.stop();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let fading = h.status().gain;
        std::thread::sleep(std::time::Duration::from_millis(12000));
        let gone = h.status().gain;
        println!("gain {:.6} -> {fading:.6} -> {gone:.8} after the stop", s.gain);
        assert!(fading < s.gain && gone < 1e-4, "it fades out after a stop: {fading} then {gone}");
    }

    /// THE HUM PLAYS ON A DEVICE (rung 2), at its level aboard, and fades away off the ship. RED first
    /// (2026-11-06) with `aboard` ignored by the bed: "off the ship it fades: 0.00046837216 from
    /// 0.00042681626" (a first version at gain 0 could not fail: nothing to fade from).
    #[test]
    #[ignore = "needs an audio output device; run: cargo test -p humanity-music --lib -- --ignored device"]
    fn device_the_hum_plays_and_fades_off_the_ship() {
        let Some(mut m) = manager() else { return };
        // -60 dB: inaudible on the operator's speakers, but a level the fade can be read from.
        let ctx = BedContext { gain: 0.001, ..Default::default() };
        let mut h = m.play(BedSoundData { engine: BedEngine::new(48000.0, 3, ctx) }).expect("plays");
        std::thread::sleep(std::time::Duration::from_millis(6000));
        let s = h.status();
        println!("hum aboard: {s:?}");
        assert!(s.blocks > 100, "it renders on the audio thread: {s:?}");
        assert!((s.root_midi - 50.0).abs() < 0.01, "its resonances sit on D3: {}", s.root_midi);
        assert!(s.level > 2e-4, "aboard it plays at its level: {}", s.level);
        h.set_context(BedContext { aboard: false, ..ctx });
        std::thread::sleep(std::time::Duration::from_millis(6000));
        let off = h.status();
        println!("hum off the ship: {off:?}");
        assert!(off.level < s.level * 0.02, "off the ship it fades: {} from {}", off.level, s.level);
    }
}

impl Sound for BedSound {
    fn process(&mut self, out: &mut [Frame], dt: f64, _info: &Info) {
        let t0 = std::time::Instant::now();
        if let Some(c) = self.ctx.read() {
            self.engine.set_context(c);
        }
        if self.stop.load(Ordering::Relaxed) {
            self.engine.stop();
        }
        if dt > 0.0 {
            self.engine.set_rate((1.0 / dt) as f32);
        }
        for chunk in out.chunks_mut(BUF) {
            let buf = &mut self.buf[..chunk.len()];
            self.engine.render(buf);
            for (o, s) in chunk.iter_mut().zip(buf.iter()) {
                *o = Frame { left: s[0], right: s[1] };
            }
        }
        self.engine.note_block_time(t0.elapsed().as_secs_f32() * 1e6);
        self.status.write(self.engine.status());
    }

    fn finished(&self) -> bool {
        self.engine.finished()
    }
}
