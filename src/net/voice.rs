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

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

/// True while the mic loopback test is running. The worker thread loops on this.
static MIC_RUNNING: AtomicBool = AtomicBool::new(false);
/// Most recent mic input peak (0.0 to 1.0) as f32 bits, for the level meter.
static MIC_PEAK_BITS: AtomicU32 = AtomicU32::new(0);
/// A human status line shown under the test button.
static MIC_STATUS: Mutex<String> = Mutex::new(String::new());

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

/// Pick a stream config at 48 kHz from the device, preferring it; returns the
/// config + channel count. Errors if the device cannot do 48 kHz f32 (the MVP
/// does not resample yet).
fn config_48k(supported: impl Iterator<Item = cpal::SupportedStreamConfigRange>) -> Option<cpal::StreamConfig> {
    let mut best: Option<cpal::SupportedStreamConfigRange> = None;
    for c in supported {
        if c.sample_format() != cpal::SampleFormat::F32 {
            continue;
        }
        if c.min_sample_rate() <= SR && SR <= c.max_sample_rate() {
            // Prefer the fewest channels (mono ideally).
            let take = match &best {
                Some(b) => c.channels() < b.channels(),
                None => true,
            };
            if take {
                best = Some(c);
            }
        }
    }
    best.map(|c| c.with_sample_rate(SR).config())
}

fn run_loopback(input_name: &str, output_name: &str) -> Result<(), String> {
    let input = find_device(input_name, true).ok_or("no input (microphone) device")?;
    let output = find_device(output_name, false).ok_or("no output (speaker) device")?;

    let in_cfg = input
        .supported_input_configs()
        .map_err(|e| e.to_string())
        .ok()
        .and_then(config_48k)
        .ok_or("microphone does not support 48 kHz f32 (resampling not built yet)")?;
    let out_cfg = output
        .supported_output_configs()
        .map_err(|e| e.to_string())
        .ok()
        .and_then(config_48k)
        .ok_or("speaker does not support 48 kHz f32 (resampling not built yet)")?;
    let in_ch = in_cfg.channels as usize;
    let out_ch = out_cfg.channels as usize;

    // Mic callback pushes mono f32 here; the worker pops 960-sample frames.
    let (mut mic_tx, mut mic_rx) = rtrb::RingBuffer::<f32>::new(SR as usize);
    // The worker pushes decoded mono f32 here; the output callback pops it.
    let (mut spk_tx, mut spk_rx) = rtrb::RingBuffer::<f32>::new(SR as usize);

    // Input: downmix to mono, push to the mic ring (drop when full), and update
    // the live level meter with this buffer's peak so the UI shows the mic working.
    let in_stream = input
        .build_input_stream(
            &in_cfg,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut peak = 0.0f32;
                let mut i = 0;
                while i + in_ch <= data.len() {
                    let mut s = 0.0f32;
                    for c in 0..in_ch {
                        s += data[i + c];
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
        .map_err(|e| format!("open microphone: {e}"))?;

    // Output: pop mono, duplicate across channels; silence on underrun.
    let out_stream = output
        .build_output_stream(
            &out_cfg,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut i = 0;
                while i + out_ch <= out.len() {
                    let s = spk_rx.pop().unwrap_or(0.0);
                    for c in 0..out_ch {
                        out[i + c] = s;
                    }
                    i += out_ch;
                }
            },
            |e| log::warn!("speaker stream error: {e}"),
            None,
        )
        .map_err(|e| format!("open speaker: {e}"))?;

    in_stream.play().map_err(|e| e.to_string())?;
    out_stream.play().map_err(|e| e.to_string())?;
    set_status("Listening, speak into your mic (use headphones)");

    // Worker: mic frames -> Opus encode -> Opus decode -> speaker. This proves
    // the full codec round-trip, not just a raw passthrough. Runs until stopped.
    let mut enc = Encoder::new()?;
    let mut dec = Decoder::new()?;
    let mut frame_i16 = [0i16; FRAME];
    let mut frame_f32 = [0f32; FRAME];
    let mut pkt = [0u8; 4000];
    let mut out_i16 = [0i16; FRAME];
    while MIC_RUNNING.load(Ordering::Relaxed) {
        if mic_rx.slots() >= FRAME {
            for s in frame_i16.iter_mut() {
                let v = mic_rx.pop().unwrap_or(0.0);
                *s = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
            }
            if let Some(n) = enc.encode(&frame_i16, &mut pkt) {
                if let Some(m) = dec.decode(&pkt[..n], &mut out_i16) {
                    for k in 0..m {
                        frame_f32[k] = out_i16[k] as f32 / 32768.0;
                    }
                    for &v in frame_f32.iter().take(m) {
                        let _ = spk_tx.push(v);
                    }
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    // Streams stop when dropped here.
    drop(in_stream);
    drop(out_stream);
    Ok(())
}
