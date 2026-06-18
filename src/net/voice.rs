//! Native voice audio (v0.485). Phase A: real-time mic capture, Opus codec, and
//! speaker playback, proven with a local loopback (you hear yourself, no
//! network). Pure Rust: cpal (WASAPI on Windows) + unsafe-libopus (libopus
//! transpiled to Rust, no C toolchain). Later phases add the WebRTC media
//! transport (str0m), the voice mesh join, and the per-peer controls. This
//! subsystem is separate from src/audio (kira game SFX); WASAPI shared mode
//! mixes the two output streams for us.
#![cfg(feature = "native")]

use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
// dasp's `Sample` trait (re-exported by cpal) provides the `f32::from_sample(s)`
// / `T::from_sample(f)` convenience conversions; the `FromSample` bound on those
// generic fns is what the method requires. Importing `Sample` brings the method
// into scope so the `Type::method` call path resolves.
use cpal::Sample as _;

/// Opus runs at 48 kHz; a 20 ms mono frame is 960 samples.
pub const SR: u32 = 48_000;
pub const FRAME: usize = 960;
const OPUS_APPLICATION_VOIP: i32 = 2048; // OPUS_APPLICATION_VOIP

// ---- Safe Opus wrappers (the only unsafe in the voice path) ----

/// A mono 48 kHz Opus encoder. One instance per sender; not shared across
/// threads without moving ownership.
pub struct Encoder {
    st: *mut unsafe_libopus::OpusEncoder,
}
// The encoder state is a self-contained heap allocation owned solely by this
// struct; moving it between threads is sound (libopus has no thread-locals).
unsafe impl Send for Encoder {}
impl Encoder {
    pub fn new() -> Result<Self, String> {
        let mut err = 0i32;
        let st = unsafe { unsafe_libopus::opus_encoder_create(SR as i32, 1, OPUS_APPLICATION_VOIP, &mut err) };
        if err != 0 || st.is_null() {
            return Err(format!("opus_encoder_create failed (err {err})"));
        }
        Ok(Self { st })
    }
    /// Encode exactly one 960-sample mono frame into `out`; returns the byte len.
    pub fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Option<usize> {
        if pcm.len() < FRAME {
            return None;
        }
        let n = unsafe {
            unsafe_libopus::opus_encode(self.st, pcm.as_ptr(), FRAME as i32, out.as_mut_ptr(), out.len() as i32)
        };
        if n < 0 { None } else { Some(n as usize) }
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { unsafe_libopus::opus_encoder_destroy(self.st) };
    }
}

/// A mono 48 kHz Opus decoder. One instance per remote sender.
pub struct Decoder {
    st: *mut unsafe_libopus::OpusDecoder,
}
unsafe impl Send for Decoder {}
impl Decoder {
    pub fn new() -> Result<Self, String> {
        let mut err = 0i32;
        let st = unsafe { unsafe_libopus::opus_decoder_create(SR as i32, 1, &mut err) };
        if err != 0 || st.is_null() {
            return Err(format!("opus_decoder_create failed (err {err})"));
        }
        Ok(Self { st })
    }
    /// Decode one Opus packet into up to `FRAME` mono samples; returns count.
    /// Pass an empty slice with `fec=false` to invoke packet-loss concealment.
    pub fn decode(&mut self, data: &[u8], out: &mut [i16]) -> Option<usize> {
        let (ptr, len) = if data.is_empty() { (std::ptr::null(), 0) } else { (data.as_ptr(), data.len() as i32) };
        let n = unsafe {
            unsafe_libopus::opus_decode(self.st, ptr, len, out.as_mut_ptr(), FRAME as i32, 0)
        };
        if n < 0 { None } else { Some(n as usize) }
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { unsafe_libopus::opus_decoder_destroy(self.st) };
    }
}

// ---- Phase A: local mic loopback (toggle: runs until stopped) ----

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Mutex;

use crate::config::{VoiceFilterMode, VoiceTransmitMode};

/// True while the mic loopback test is running. The worker thread loops on this.
static MIC_RUNNING: AtomicBool = AtomicBool::new(false);
/// Most recent mic input peak (0.0 to 1.0) as f32 bits, for the level meter.
static MIC_PEAK_BITS: AtomicU32 = AtomicU32::new(0);
/// A human status line shown under the test button.
static MIC_STATUS: Mutex<String> = Mutex::new(String::new());

// ── Live input params (v0.488) ──────────────────────────────────────────
// The UI thread (lib.rs) writes these every frame so the running worker picks
// up gain / filter / transmit-mode / threshold / push-key changes WITHOUT
// restarting the audio streams. The worker reads them per 20 ms frame.
static VOICE_GAIN_BITS: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0_f32
static VOICE_FILTER_MODE: AtomicU8 = AtomicU8::new(1); // 0=Off 1=Light 2=NoiseSuppression
static VOICE_TRANSMIT_MODE: AtomicU8 = AtomicU8::new(0); // 0=OpenMic 1=PTT 2=VAD 3=PushToMute
static VOICE_VAD_THRESH_BITS: AtomicU32 = AtomicU32::new(0x3d4c_cccd); // 0.05_f32
static VOICE_PTT_HELD: AtomicBool = AtomicBool::new(false);
/// Worker -> UI: is cleaned audio actually being transmitted right now (i.e. the
/// transmit gate is open)? Lets the UI show a live "transmitting" indicator.
static VOICE_TRANSMITTING: AtomicBool = AtomicBool::new(false);

fn filter_to_u8(m: VoiceFilterMode) -> u8 {
    match m {
        VoiceFilterMode::Off => 0,
        VoiceFilterMode::Light => 1,
        VoiceFilterMode::NoiseSuppression => 2,
    }
}
fn transmit_to_u8(m: VoiceTransmitMode) -> u8 {
    match m {
        VoiceTransmitMode::OpenMic => 0,
        VoiceTransmitMode::PushToTalk => 1,
        VoiceTransmitMode::VoiceActivated => 2,
        VoiceTransmitMode::PushToMute => 3,
    }
}

/// Push the current input params to the worker. Called every frame from lib.rs.
pub fn set_input_params(
    gain: f32,
    filter: VoiceFilterMode,
    transmit: VoiceTransmitMode,
    vad_threshold: f32,
    ptt_held: bool,
) {
    VOICE_GAIN_BITS.store(gain.to_bits(), Ordering::Relaxed);
    VOICE_FILTER_MODE.store(filter_to_u8(filter), Ordering::Relaxed);
    VOICE_TRANSMIT_MODE.store(transmit_to_u8(transmit), Ordering::Relaxed);
    VOICE_VAD_THRESH_BITS.store(vad_threshold.to_bits(), Ordering::Relaxed);
    VOICE_PTT_HELD.store(ptt_held, Ordering::Relaxed);
}
/// Is the transmit gate currently open (audio passing)? For a UI indicator.
pub fn is_transmitting() -> bool {
    VOICE_TRANSMITTING.load(Ordering::Relaxed)
}

fn set_status(s: &str) {
    if let Ok(mut g) = MIC_STATUS.lock() {
        *g = s.to_string();
    }
}

/// Is the mic loopback test currently running?
pub fn mic_test_running() -> bool {
    MIC_RUNNING.load(Ordering::Relaxed)
}
/// The most recent mic input level (0.0 to 1.0), for a meter. Decays to 0 when stopped.
pub fn mic_level() -> f32 {
    f32::from_bits(MIC_PEAK_BITS.load(Ordering::Relaxed))
}
/// The current status line ("Listening...", "Failed: ...", etc.).
pub fn mic_status() -> String {
    MIC_STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Names of the available input (mic) devices.
pub fn list_input_devices() -> Vec<String> {
    device_names(true)
}
/// Names of the available output (speaker) devices.
pub fn list_output_devices() -> Vec<String> {
    device_names(false)
}
fn device_names(input: bool) -> Vec<String> {
    let host = cpal::default_host();
    let iter = if input { host.input_devices() } else { host.output_devices() };
    iter.map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// Find a device by name; empty name or no match falls back to the system default.
fn find_device(name: &str, input: bool) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if !name.is_empty() {
        let iter = if input { host.input_devices() } else { host.output_devices() };
        if let Ok(devs) = iter {
            for d in devs {
                if d.name().map(|n| n == name).unwrap_or(false) {
                    return Some(d);
                }
            }
        }
    }
    if input { host.default_input_device() } else { host.default_output_device() }
}

/// Start the mic -> Opus -> speaker loopback on the chosen devices. Runs until
/// stop_mic_test(). Use headphones to avoid feedback. Best-effort: sets a Failed
/// status if a device or the 48 kHz format is unavailable.
pub fn start_mic_test(input_name: String, output_name: String) {
    if MIC_RUNNING.swap(true, Ordering::SeqCst) {
        return; // already running
    }
    set_status("Starting...");
    std::thread::spawn(move || {
        if let Err(e) = run_loopback(&input_name, &output_name) {
            log::warn!("Mic test failed: {e}");
            set_status(&format!("Failed: {e}"));
            crate::debug::push_debug(format!("Mic test failed: {e}"));
        }
        MIC_RUNNING.store(false, Ordering::SeqCst);
        MIC_PEAK_BITS.store(0, Ordering::Relaxed);
    });
}

/// Stop the running mic loopback (the worker exits and drops its streams).
pub fn stop_mic_test() {
    if MIC_RUNNING.load(Ordering::Relaxed) {
        set_status("Stopped");
    }
    MIC_RUNNING.store(false, Ordering::SeqCst);
}

/// Minimal streaming linear resampler (mono f32). Opus needs exactly 48 kHz, but
/// a mic/speaker in WASAPI shared mode is whatever the user picked in Windows
/// Sound settings (commonly 44.1 or 48 kHz, i16 or f32). This converts the device
/// rate to/from 48 kHz so any device works. Linear interpolation is plenty for
/// speech; a higher-quality (sinc) resampler can drop in later without changing
/// callers. When in_rate == out_rate it is an exact passthrough.
struct Resampler {
    /// Input samples consumed per output sample (in_rate / out_rate).
    step: f64,
    /// Fractional read position into `pending`.
    pos: f64,
    /// Unconsumed input, with one sample of lookahead kept for interpolation.
    pending: Vec<f32>,
}
impl Resampler {
    fn new(in_rate: u32, out_rate: u32) -> Self {
        Self {
            step: in_rate as f64 / out_rate.max(1) as f64,
            pos: 0.0,
            pending: Vec::new(),
        }
    }
    /// Feed input samples; append the resampled result to `out`.
    fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.pending.extend_from_slice(input);
        // Need x[i] and x[i+1] to interpolate, so stop one short of the end.
        while (self.pos as usize) + 1 < self.pending.len() {
            let i = self.pos as usize;
            let f = (self.pos - i as f64) as f32;
            out.push(self.pending[i] * (1.0 - f) + self.pending[i + 1] * f);
            self.pos += self.step;
        }
        // Drop fully-consumed input; keep the fractional remainder + lookahead.
        let drop = self.pos as usize;
        if drop > 0 {
            self.pending.drain(0..drop);
            self.pos -= drop as f64;
        }
    }
}

/// Build a mic input stream for sample type `T`, downmixing to mono f32, pushing
/// to `mic_tx`, and publishing this buffer's peak to the level meter. Works for
/// i16 / u16 / f32 mics (cpal converts each sample via `f32::from_sample`).
fn build_input<T>(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    in_ch: usize,
    mut mic_tx: rtrb::Producer<f32>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    device
        .build_input_stream(
            cfg,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let mut peak = 0.0f32;
                let mut i = 0;
                while i + in_ch <= data.len() {
                    let mut s = 0.0f32;
                    for c in 0..in_ch {
                        s += f32::from_sample(data[i + c]);
                    }
                    let mono = s / in_ch as f32;
                    peak = peak.max(mono.abs());
                    let _ = mic_tx.push(mono);
                    i += in_ch;
                }
                MIC_PEAK_BITS.store(peak.min(1.0).to_bits(), Ordering::Relaxed);
            },
            |e| log::warn!("mic stream error: {e}"),
            None,
        )
        .map_err(|e| format!("open microphone: {e}"))
}

/// Build a speaker output stream for sample type `T`, popping mono f32 from
/// `spk_rx` and duplicating it across channels. Silence on underrun.
fn build_output<T>(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    out_ch: usize,
    mut spk_rx: rtrb::Consumer<f32>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + Send + 'static + cpal::FromSample<f32>,
{
    device
        .build_output_stream(
            cfg,
            move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut i = 0;
                while i + out_ch <= out.len() {
                    let v = T::from_sample(spk_rx.pop().unwrap_or(0.0));
                    for c in 0..out_ch {
                        out[i + c] = v;
                    }
                    i += out_ch;
                }
            },
            |e| log::warn!("speaker stream error: {e}"),
            None,
        )
        .map_err(|e| format!("open speaker: {e}"))
}

// ── DSP: the mic input chain (v0.488) ───────────────────────────────────
// Order: user gain -> high-pass -> noise filter -> (transmit gate) -> Opus.
// All mono f32 at 48 kHz. Pure Rust, no deps. The "NoiseSuppression" mode adds
// a learned denoiser (RNNoise) on top; until that is wired it falls back to a
// stronger gate, which the comment in `InputProcessor::process` notes.

/// A Direct-Form-I biquad. We use it as a high-pass to remove DC offset, desk
/// rumble, and AC-hum fundamentals below speech (RBJ cookbook coefficients).
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}
impl Biquad {
    fn highpass(sr: f32, fc: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * fc / sr;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos) / 2.0) / a0,
            b1: (-(1.0 + cos)) / a0,
            b2: ((1.0 + cos) / 2.0) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// A simple envelope-following noise gate: when the frame energy sits below the
/// threshold the gain ramps smoothly to zero (killing between-word hiss + faint
/// background), and ramps back up fast when you speak. Attack is fast so word
/// onsets are not chopped; release is slow so tails decay naturally.
struct NoiseGate {
    thresh: f32,
    gain: f32,
    attack: f32,
    release: f32,
}
impl NoiseGate {
    fn new(thresh: f32) -> Self {
        Self { thresh, gain: 0.0, attack: 0.02, release: 0.0008 }
    }
    fn process(&mut self, frame: &mut [f32], rms: f32) {
        let target = if rms > self.thresh { 1.0 } else { 0.0 };
        for s in frame.iter_mut() {
            let coeff = if target > self.gain { self.attack } else { self.release };
            self.gain += (target - self.gain) * coeff;
            *s *= self.gain;
        }
    }
}

fn frame_rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f32 = frame.iter().map(|s| s * s).sum();
    (sum / frame.len() as f32).sqrt()
}

/// RNNoise (nnnoiseless) wrapper for the NoiseSuppression mode. RNNoise is
/// hard-wired to 48 kHz and processes exactly 480-sample (10 ms) frames, and it
/// expects samples in i16 amplitude range (~[-32768, 32767]) NOT [-1, 1] — so we
/// scale up going in and back down coming out. Stateful (holds the recurrent +
/// overlap-add memory), so one instance per stream, reused across frames.
struct Denoiser {
    st: Box<nnnoiseless::DenoiseState<'static>>,
    sin: [f32; 480],
    sout: [f32; 480],
}
impl Denoiser {
    fn new() -> Self {
        Self { st: nnnoiseless::DenoiseState::new(), sin: [0.0; 480], sout: [0.0; 480] }
    }
    /// Denoise a buffer whose length is a multiple of 480, in place. Our caller
    /// passes exactly FRAME (960 = 2 RNNoise frames), so there is no remainder.
    fn process(&mut self, buf: &mut [f32]) {
        let mut off = 0;
        while off + 480 <= buf.len() {
            for i in 0..480 {
                self.sin[i] = buf[off + i] * 32768.0;
            }
            let _vad = self.st.process_frame(&mut self.sout, &self.sin);
            for i in 0..480 {
                buf[off + i] = (self.sout[i] / 32768.0).clamp(-1.0, 1.0);
            }
            off += 480;
        }
    }
}

/// The full input chain. Holds the per-stream filter state (biquad memory, gate
/// envelope, RNNoise model) so it is allocated once per session and reused per
/// frame.
struct InputProcessor {
    hp: Biquad,
    gate: NoiseGate,
    denoiser: Denoiser,
}
impl InputProcessor {
    fn new() -> Self {
        Self {
            // ~85 Hz high-pass at Q 0.707 (Butterworth) — transparent for voice.
            hp: Biquad::highpass(SR as f32, 85.0, 0.707),
            // Light: a gentle gate (low threshold) just to kill silence hiss.
            gate: NoiseGate::new(0.012),
            denoiser: Denoiser::new(),
        }
    }
    /// Apply gain + the selected filter to a 48 kHz mono frame, in place.
    fn process(&mut self, frame: &mut [f32], gain: f32, mode: u8) {
        if (gain - 1.0).abs() > f32::EPSILON {
            for s in frame.iter_mut() {
                *s = (*s * gain).clamp(-1.0, 1.0);
            }
        }
        match mode {
            0 => {} // Off
            2 => {
                // NoiseSuppression: high-pass, then RNNoise — which removes
                // keyboard clicks, coughs, fans, and background noise even while
                // you speak (a gate only helps between words). No separate gate
                // here: double-suppression sounds pumpy.
                for s in frame.iter_mut() {
                    *s = self.hp.process(*s);
                }
                self.denoiser.process(frame);
            }
            _ => {
                // Light (default): high-pass + a gentle gate.
                for s in frame.iter_mut() {
                    *s = self.hp.process(*s);
                }
                let rms = frame_rms(frame);
                self.gate.process(frame, rms);
            }
        }
    }
}

/// Decide whether the cleaned frame should be transmitted this moment, given the
/// transmit mode, the (post-filter) level, the activation threshold, the push
/// key state, and a mutable VAD hangover counter (in frames). VAD keeps the gate
/// open for a short tail after the level drops, so word endings are not clipped.
fn transmit_decision(mode: u8, rms: f32, vad_thresh: f32, ptt_held: bool, vad_hold: &mut u32) -> bool {
    const VAD_HANGOVER_FRAMES: u32 = 25; // ~500 ms at 20 ms/frame
    match mode {
        1 => ptt_held,           // PushToTalk
        3 => !ptt_held,          // PushToMute
        2 => {
            // VoiceActivated
            if rms > vad_thresh {
                *vad_hold = VAD_HANGOVER_FRAMES;
                true
            } else if *vad_hold > 0 {
                *vad_hold -= 1;
                true
            } else {
                false
            }
        }
        _ => true, // OpenMic
    }
}

fn run_loopback(input_name: &str, output_name: &str) -> Result<(), String> {
    let input = find_device(input_name, true).ok_or("no input (microphone) device")?;
    let output = find_device(output_name, false).ok_or("no output (speaker) device")?;

    // Use each device's actual shared-mode format (rate + sample format + channel
    // count) rather than demanding a specific one. On WASAPI this is whatever the
    // user set in Windows Sound settings, so building this config is the reliable
    // path; we adapt to it (any format, any rate) instead of failing.
    let in_def = input
        .default_input_config()
        .map_err(|e| format!("microphone has no default format: {e}"))?;
    let out_def = output
        .default_output_config()
        .map_err(|e| format!("speaker has no default format: {e}"))?;
    let in_rate: u32 = in_def.sample_rate();
    let out_rate: u32 = out_def.sample_rate();
    let in_fmt = in_def.sample_format();
    let out_fmt = out_def.sample_format();
    let in_cfg = in_def.config();
    let out_cfg = out_def.config();
    let in_ch = in_cfg.channels as usize;
    let out_ch = out_cfg.channels as usize;

    // Mic callback pushes mono f32 (at in_rate) here; the worker drains it.
    let (mic_tx, mut mic_rx) = rtrb::RingBuffer::<f32>::new(SR as usize);
    // The worker pushes decoded mono f32 (at out_rate) here; output callback pops.
    let (mut spk_tx, spk_rx) = rtrb::RingBuffer::<f32>::new(SR as usize);

    let in_stream = match in_fmt {
        cpal::SampleFormat::F32 => build_input::<f32>(&input, &in_cfg, in_ch, mic_tx),
        cpal::SampleFormat::I16 => build_input::<i16>(&input, &in_cfg, in_ch, mic_tx),
        cpal::SampleFormat::U16 => build_input::<u16>(&input, &in_cfg, in_ch, mic_tx),
        other => return Err(format!("microphone sample format {other:?} not supported")),
    }?;
    let out_stream = match out_fmt {
        cpal::SampleFormat::F32 => build_output::<f32>(&output, &out_cfg, out_ch, spk_rx),
        cpal::SampleFormat::I16 => build_output::<i16>(&output, &out_cfg, out_ch, spk_rx),
        cpal::SampleFormat::U16 => build_output::<u16>(&output, &out_cfg, out_ch, spk_rx),
        other => return Err(format!("speaker sample format {other:?} not supported")),
    }?;

    in_stream.play().map_err(|e| e.to_string())?;
    out_stream.play().map_err(|e| e.to_string())?;
    set_status("Listening, speak into your mic (use headphones)");

    // Worker: mic (in_rate) -> resample to 48 kHz -> gain + filter (DSP) ->
    // transmit gate -> Opus encode -> Opus decode -> resample to out_rate ->
    // speaker. Proves the full input chain + codec round-trip, for any device
    // rate. The DSP params are read live each frame from the UI. Runs until stopped.
    let mut up = Resampler::new(in_rate, SR);
    let mut down = Resampler::new(SR, out_rate);
    let mut proc = InputProcessor::new();
    let mut vad_hold: u32 = 0;
    let mut enc = Encoder::new()?;
    let mut dec = Decoder::new()?;
    let mut frame_f32 = [0f32; FRAME];
    let mut frame_i16 = [0i16; FRAME];
    let mut pkt = [0u8; 4000];
    let mut out_i16 = [0i16; FRAME];
    let mut drained: Vec<f32> = Vec::with_capacity(2048);
    let mut buf48: Vec<f32> = Vec::with_capacity(SR as usize);
    let mut dec_f32: Vec<f32> = Vec::with_capacity(FRAME);
    let mut resampled: Vec<f32> = Vec::with_capacity(FRAME * 2);
    while MIC_RUNNING.load(Ordering::Relaxed) {
        // Drain everything the mic callback has produced and resample to 48 kHz.
        drained.clear();
        while let Ok(s) = mic_rx.pop() {
            drained.push(s);
        }
        let idle = drained.is_empty();
        if !idle {
            up.process(&drained, &mut buf48);
        }
        // Read the live input params once per drain.
        let gain = f32::from_bits(VOICE_GAIN_BITS.load(Ordering::Relaxed));
        let filter_mode = VOICE_FILTER_MODE.load(Ordering::Relaxed);
        let transmit_mode = VOICE_TRANSMIT_MODE.load(Ordering::Relaxed);
        let vad_thresh = f32::from_bits(VOICE_VAD_THRESH_BITS.load(Ordering::Relaxed));
        let ptt_held = VOICE_PTT_HELD.load(Ordering::Relaxed);
        // Process every full 48 kHz frame, gate it, then codec + resample to speaker.
        while buf48.len() >= FRAME {
            frame_f32.copy_from_slice(&buf48[..FRAME]);
            buf48.drain(0..FRAME);
            // Gain + filter (always run so the filter state stays warm), then the
            // transmit decision on the cleaned level.
            proc.process(&mut frame_f32, gain, filter_mode);
            let rms = frame_rms(&frame_f32);
            let transmit = transmit_decision(transmit_mode, rms, vad_thresh, ptt_held, &mut vad_hold);
            VOICE_TRANSMITTING.store(transmit, Ordering::Relaxed);
            if !transmit {
                // Gate closed: emit nothing; the output callback plays silence.
                continue;
            }
            for (s, &v) in frame_i16.iter_mut().zip(frame_f32.iter()) {
                *s = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
            }
            if let Some(n) = enc.encode(&frame_i16, &mut pkt) {
                if let Some(m) = dec.decode(&pkt[..n], &mut out_i16) {
                    dec_f32.clear();
                    for &v in out_i16.iter().take(m) {
                        dec_f32.push(v as f32 / 32768.0);
                    }
                    resampled.clear();
                    down.process(&dec_f32, &mut resampled);
                    for &v in &resampled {
                        let _ = spk_tx.push(v);
                    }
                }
            }
        }
        if idle {
            std::thread::sleep(Duration::from_millis(3));
        }
    }
    VOICE_TRANSMITTING.store(false, Ordering::Relaxed);
    // Streams stop when dropped here.
    drop(in_stream);
    drop(out_stream);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_identity_passthrough() {
        // 48k -> 48k is an exact passthrough (one sample of lookahead latency).
        let mut r = Resampler::new(48_000, 48_000);
        let input: Vec<f32> = (0..480).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut out = Vec::new();
        r.process(&input, &mut out);
        // All but the last (held for lookahead) samples come out unchanged.
        assert!(out.len() >= input.len() - 1);
        for (a, b) in input.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-6, "passthrough altered a sample");
        }
    }

    #[test]
    fn resampler_halves_on_downsample() {
        // 48k -> 24k yields ~half the samples.
        let mut r = Resampler::new(48_000, 24_000);
        let input = vec![0.0f32; 4800];
        let mut out = Vec::new();
        r.process(&input, &mut out);
        let ratio = out.len() as f32 / input.len() as f32;
        assert!((ratio - 0.5).abs() < 0.02, "expected ~half, got {ratio}");
    }

    #[test]
    fn gain_stays_in_range() {
        // Boosting a hot signal must not exceed [-1, 1] (clip protection).
        let mut proc = InputProcessor::new();
        let mut frame = vec![0.9f32; FRAME];
        proc.process(&mut frame, 2.0, 0); // gain 200%, filter Off
        assert!(frame.iter().all(|s| s.abs() <= 1.0), "gain clipped out of range");
    }

    #[test]
    fn highpass_removes_dc_offset() {
        // A constant (DC) input must decay toward zero through the high-pass.
        let mut hp = Biquad::highpass(SR as f32, 85.0, 0.707);
        let mut last = 0.0;
        for _ in 0..4000 {
            last = hp.process(1.0);
        }
        assert!(last.abs() < 0.05, "DC not removed: {last}");
    }

    #[test]
    fn denoiser_runs_and_stays_finite() {
        // RNNoise must accept a 960-sample (2x480) frame and return finite audio.
        let mut d = Denoiser::new();
        let mut frame: Vec<f32> = (0..FRAME).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        d.process(&mut frame);
        assert_eq!(frame.len(), FRAME);
        assert!(frame.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    #[test]
    fn transmit_modes_gate_correctly() {
        let mut hold = 0u32;
        // Open mic (0): always on.
        assert!(transmit_decision(0, 0.0, 0.05, false, &mut hold));
        // Push-to-talk (1): on only while held.
        assert!(transmit_decision(1, 0.5, 0.05, true, &mut hold));
        assert!(!transmit_decision(1, 0.5, 0.05, false, &mut hold));
        // Push-to-mute (3): off only while held.
        assert!(!transmit_decision(3, 0.5, 0.05, true, &mut hold));
        assert!(transmit_decision(3, 0.5, 0.05, false, &mut hold));
        // Voice-activated (2): opens above threshold, holds briefly, then closes.
        hold = 0;
        assert!(transmit_decision(2, 0.10, 0.05, false, &mut hold)); // above -> on
        assert!(transmit_decision(2, 0.0, 0.05, false, &mut hold)); // hangover keeps it on
        for _ in 0..30 {
            transmit_decision(2, 0.0, 0.05, false, &mut hold);
        }
        assert!(!transmit_decision(2, 0.0, 0.05, false, &mut hold)); // hangover expired -> off
    }
}
