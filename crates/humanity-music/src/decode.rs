//! Decoding recordings for the score, several files at once (BUG-297, docs/design/music.md
//! section 12a), and a packet at a time (2026-11-14, section 23's streaming).
//!
//! A genre change builds a new score whose recordings must be decoded before it can play. One file
//! after another, electro swing's 51 small Ogg files took the build's worker most of its time; the
//! files are independent, so they are decoded on a few threads at once. Nothing is kept here: the
//! caller (the engine's `SampleBank`) decides what, if anything, outlives the build, under the Dev
//! page's "turn off all reuse" switch (CLAUDE.md, the no-caches rule).
//!
//! [`Reader`] decodes one packet at a time, so a long recording need not be decoded whole: the score
//! keeps its first part ([`decode_head`]) and the streaming worker decodes the rest while a note
//! plays ([`crate::stream`]). It converts exactly as kira's `StaticSoundData::from_file` did before
//! it (symphonia's own sample conversion, a mono file played on both sides, then
//! [`crate::instrument::sample16`]'s rounding), so a recording decodes to the same frames either way
//! (`tests_stream::the_reader_decodes_what_kira_decodes`).

use std::path::{Path, PathBuf};

use symphonia::core::audio::conv::{FromSample, IntoSample};
use symphonia::core::audio::sample::Sample;
use symphonia::core::audio::{Audio, AudioBuffer, GenericAudioBufferRef};
use symphonia::core::codecs::audio::AudioDecoder;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;

/// One decoded file: its sample rate and its frames as 16-bit stereo
/// ([`crate::instrument::SAMPLE_SCALE`]).
pub type Decoded = (u32, Vec<[i16; 2]>);

/// An open recording, decoded a packet at a time.
pub struct Reader {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    /// Frames a second.
    pub sample_rate: u32,
    /// Its length in frames, as the file's container says (None when it does not say).
    pub frames: Option<u64>,
}

impl Reader {
    /// Open an audio file (Ogg Vorbis, FLAC, WAV: whatever kira and symphonia read).
    pub fn open(path: &Path) -> Result<Reader, String> {
        let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
        let file = std::fs::File::open(path).map_err(|e| err(&e))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let format = symphonia::default::get_probe().probe(&Hint::default(), mss, Default::default(), Default::default()).map_err(|e| err(&e))?;
        let track = format.default_track(TrackType::Audio).ok_or_else(|| err(&"no audio track"))?;
        let params = match track.codec_params.as_ref() {
            Some(CodecParameters::Audio(p)) => p,
            _ => return Err(err(&"no audio track")),
        };
        let sample_rate = params.sample_rate.ok_or_else(|| err(&"no sample rate"))?;
        let decoder = symphonia::default::get_codecs().make_audio_decoder(params, &Default::default()).map_err(|e| err(&e))?;
        let (track, frames) = (track.id, track.num_frames);
        Ok(Reader { format, decoder, track, sample_rate, frames })
    }

    /// Decode the next packet onto the end of `out`. False at the end of the file.
    pub fn next_into(&mut self, out: &mut Vec<[i16; 2]>) -> Result<bool, String> {
        loop {
            match self.format.next_packet() {
                Ok(Some(packet)) if packet.track_id == self.track => {
                    let buf = self.decoder.decode(&packet).map_err(|e| e.to_string())?;
                    push_frames(&buf, out)?;
                    return Ok(true);
                }
                Ok(Some(_)) => continue,
                Ok(None) => return Ok(false),
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}

/// A packet's samples as 16-bit stereo frames: the conversion kira 0.12's `load_frames_from_buffer`
/// makes (symphonia's `IntoSample` to f32; one channel on both sides), then
/// [`crate::instrument::sample16`].
fn push_frames(buf: &GenericAudioBufferRef, out: &mut Vec<[i16; 2]>) -> Result<(), String> {
    fn push<S: Sample>(b: &AudioBuffer<S>, out: &mut Vec<[i16; 2]>) -> Result<(), String>
    where
        f32: FromSample<S>,
    {
        let q = crate::instrument::sample16;
        match (b.plane(0), b.plane(1), b.num_planes()) {
            (Some(m), _, 1) => out.extend(m.iter().map(|s| {
                let v: f32 = (*s).into_sample();
                [q(v), q(v)]
            })),
            (Some(l), Some(r), 2) => out.extend(l.iter().zip(r.iter()).map(|(a, b)| {
                let (a, b): (f32, f32) = ((*a).into_sample(), (*b).into_sample());
                [q(a), q(b)]
            })),
            (_, _, n) => return Err(format!("{n} channels: only mono and stereo recordings are read")),
        }
        Ok(())
    }
    match buf {
        GenericAudioBufferRef::U8(b) => push(b, out),
        GenericAudioBufferRef::U16(b) => push(b, out),
        GenericAudioBufferRef::U24(b) => push(b, out),
        GenericAudioBufferRef::U32(b) => push(b, out),
        GenericAudioBufferRef::S8(b) => push(b, out),
        GenericAudioBufferRef::S16(b) => push(b, out),
        GenericAudioBufferRef::S24(b) => push(b, out),
        GenericAudioBufferRef::S32(b) => push(b, out),
        GenericAudioBufferRef::F32(b) => push(b, out),
        GenericAudioBufferRef::F64(b) => push(b, out),
    }
}

/// Decode one audio file whole.
pub fn decode_file(path: &Path) -> Result<Decoded, String> {
    let mut r = Reader::open(path)?;
    let mut frames = Vec::with_capacity(r.frames.unwrap_or(0).min(1 << 26) as usize);
    while r.next_into(&mut frames).map_err(|e| format!("{}: {e}", path.display()))? {}
    Ok((r.sample_rate, frames))
}

/// The start of a recording: its first `max_frames` frames, and whether that is all of it. A file no
/// longer than `max_frames` is decoded whole (true); a longer one stops decoding a packet past
/// `max_frames` and is cut there (false: the rest is streamed while a note plays).
pub fn decode_head(path: &Path, max_frames: usize) -> Result<(u32, Vec<[i16; 2]>, bool), String> {
    let mut r = Reader::open(path)?;
    let mut frames = Vec::with_capacity(max_frames.min(r.frames.unwrap_or(u64::MAX).min(1 << 26) as usize) + 4096);
    loop {
        if frames.len() > max_frames {
            frames.truncate(max_frames);
            frames.shrink_to_fit();
            return Ok((r.sample_rate, frames, false));
        }
        if !r.next_into(&mut frames).map_err(|e| format!("{}: {e}", path.display()))? {
            frames.shrink_to_fit();
            return Ok((r.sample_rate, frames, true));
        }
    }
}

/// A recording a note plays straight through, for streaming (crate::stream): decoded WHOLE when it
/// is no longer than `longer_than_s` (true), else only its first `attack_s` (false: the rest is
/// streamed). The file's own length decides when its container gives one, so a long recording is
/// decoded no further than its attack; otherwise it is decoded until it passes `longer_than_s`.
pub fn decode_attack(path: &Path, longer_than_s: f32, attack_s: f32) -> Result<(u32, Vec<[i16; 2]>, bool), String> {
    let mut r = Reader::open(path)?;
    let rate = r.sample_rate as f32;
    let limit = (longer_than_s.max(0.0) * rate) as usize;
    let attack = ((attack_s.max(0.01) * rate) as usize).clamp(1, limit.max(1));
    let stop_past = match r.frames {
        Some(n) if n as usize <= limit => usize::MAX,
        Some(_) => attack,
        None => limit,
    };
    let mut frames = Vec::with_capacity(stop_past.min(limit).min(1 << 26) + 4096);
    loop {
        if frames.len() > stop_past {
            frames.truncate(attack);
            frames.shrink_to_fit();
            return Ok((r.sample_rate, frames, false));
        }
        if !r.next_into(&mut frames).map_err(|e| format!("{}: {e}", path.display()))? {
            // Decoded to its end (a short recording, or one its container undercounted): whole.
            frames.shrink_to_fit();
            return Ok((r.sample_rate, frames, true));
        }
    }
}

/// A recording's sample rate and length in frames, from its container's header alone (nothing
/// decoded): what Settings counts a pack's memory from. None for the length when the file does not
/// say.
pub fn frame_count(path: &Path) -> Result<(u32, Option<u64>), String> {
    let r = Reader::open(path)?;
    Ok((r.sample_rate, r.frames))
}

/// How many threads a batch of decodes uses: all but one of the machine's cores, at most four (the
/// game's own threads keep running beside it; four already brings electro swing's set from 120 ms
/// to about 40 in an optimised build, and more threads gain little on files this small).
pub fn decode_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get().saturating_sub(1)).unwrap_or(1).clamp(1, 4)
}

/// Run `f` on every job (a file to decode, whole or its attack) on up to `threads` threads; the
/// results in the same order. The jobs are shared out by an atomic counter, so a long file does not
/// hold up the others.
pub fn each<J: Sync, T: Send>(jobs: &[J], threads: usize, f: impl Fn(&J) -> Result<T, String> + Sync) -> Vec<Result<T, String>> {
    let threads = threads.clamp(1, jobs.len().max(1));
    if threads <= 1 {
        return jobs.iter().map(&f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<Option<Result<T, String>>> = (0..jobs.len()).map(|_| None).collect();
    let parts: Vec<Vec<(usize, Result<T, String>)>> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(j) = jobs.get(i) else { break };
                        mine.push((i, f(j)));
                    }
                    mine
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap_or_default()).collect()
    });
    for (i, r) in parts.into_iter().flatten() {
        out[i] = Some(r);
    }
    out.into_iter().enumerate().map(|(i, r)| r.unwrap_or_else(|| Err(format!("decode job {i}: the decode thread stopped")))).collect()
}

/// Decode every file in `paths` whole on up to `threads` threads; the results in the same order.
pub fn decode_files(paths: &[PathBuf], threads: usize) -> Vec<Result<Decoded, String>> {
    each(paths, threads, |p| decode_file(p))
}
