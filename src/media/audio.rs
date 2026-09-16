//! Opus audio for the media player, delivered to kira as a streaming sound.
//!
//! kira 0.9 has exactly the hook we need: `sound::streaming::Decoder`, a pull
//! trait its own decode thread drives (`decode()` for the next chunk of
//! frames, `seek()` to move). So the hookup is direct: `OpusTrack` demuxes
//! the file's Opus track on its own file handle, decodes each packet with
//! unsafe-libopus (libopus transpiled to Rust, already in the tree for voice
//! chat) and hands kira left/right frames. No queued-chunk workaround, no
//! second ring buffer. The kira handle the player keeps reports the playback
//! position, which is what the video clock follows.
//!
//! Opus details that matter here:
//! - Opus always decodes at 48 kHz regardless of the source rate.
//! - The encoder's look-ahead means the first `pre_skip` samples (from the
//!   OpusHead in CodecPrivate, 312 for libopus) are padding and are dropped.
//! - A packet holds at most 120 ms, so 5760 samples per channel is the
//!   largest decode buffer ever needed.
//! - Files with more than two channels use the multistream layout, which
//!   needs a different decoder; `probe()` refuses them with a clear message.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use kira::Frame;
use matroska_demuxer::{Frame as MkvPacket, MatroskaFile};

use super::{open_demuxer, MediaError, MediaInfo};

/// Opus output sample rate: fixed by the codec.
pub const OPUS_RATE: u32 = 48_000;
/// Longest Opus packet, 120 ms at 48 kHz, in samples per channel.
pub const OPUS_MAX_FRAME: usize = 5760;
/// Size of the silence chunk handed out past the end of the stream.
const SILENCE_CHUNK: usize = 960;

/// The fields of an OpusHead (RFC 7845 section 5.1) this player uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpusHead {
    pub channels: u16,
    pub pre_skip: u16,
    /// The rate the audio had before Opus encoding (informational; decoding
    /// is always 48 kHz).
    pub input_sample_rate: u32,
}

/// Parse the 19-byte OpusHead that WebM stores in CodecPrivate.
pub fn parse_opus_head(private: &[u8]) -> Result<OpusHead, MediaError> {
    if private.len() < 19 || &private[0..8] != b"OpusHead" {
        return Err(MediaError::Audio("Opus track has no OpusHead in CodecPrivate".into()));
    }
    Ok(OpusHead {
        channels: private[9] as u16,
        pre_skip: u16::from_le_bytes([private[10], private[11]]),
        input_sample_rate: u32::from_le_bytes([private[12], private[13], private[14], private[15]]),
    })
}

/// A safe wrapper on one unsafe-libopus decoder state.
struct OpusDecoder {
    st: *mut unsafe_libopus::OpusDecoder,
    channels: usize,
    /// Scratch for one decoded packet, interleaved: OPUS_MAX_FRAME * channels.
    pcm: Vec<f32>,
}

// SAFETY: the decoder state is a self-contained heap allocation owned by this
// struct alone; libopus keeps no thread-locals, so moving it to kira's decode
// thread is sound (the same argument `net::voice` makes for its wrappers).
unsafe impl Send for OpusDecoder {}

impl OpusDecoder {
    fn new(channels: usize) -> Result<Self, MediaError> {
        let mut err = 0i32;
        // SAFETY: plain FFI construction; the result is checked before use.
        let st = unsafe { unsafe_libopus::opus_decoder_create(OPUS_RATE as i32, channels as i32, &mut err) };
        if err != 0 || st.is_null() {
            return Err(MediaError::Audio(format!("opus_decoder_create failed (err {err})")));
        }
        Ok(Self { st, channels, pcm: vec![0.0; OPUS_MAX_FRAME * channels] })
    }

    /// Decode one packet; returns the interleaved samples (len = n * channels).
    fn decode(&mut self, packet: &[u8]) -> Result<&[f32], MediaError> {
        // SAFETY: `packet` is a valid slice; `pcm` holds OPUS_MAX_FRAME *
        // channels floats, the most one packet can produce.
        let n = unsafe {
            unsafe_libopus::opus_decode_float(
                self.st,
                packet.as_ptr(),
                packet.len() as i32,
                self.pcm.as_mut_ptr(),
                OPUS_MAX_FRAME as i32,
                0,
            )
        };
        if n < 0 {
            return Err(MediaError::Audio(format!("opus_decode_float failed (code {n})")));
        }
        Ok(&self.pcm[..n as usize * self.channels])
    }
}

impl Drop for OpusDecoder {
    fn drop(&mut self) {
        // SAFETY: created by opus_decoder_create, destroyed exactly once here.
        unsafe { unsafe_libopus::opus_decoder_destroy(self.st) };
    }
}

/// The file's Opus track as a kira `Decoder`. Holds its own demuxer so kira's
/// decode thread never contends with the video thread for the file.
pub struct OpusTrack {
    path: PathBuf,
    mkv: MatroskaFile<BufReader<File>>,
    track: u64,
    decoder: OpusDecoder,
    packet: MkvPacket,
    channels: usize,
    pre_skip: usize,
    /// Pre-skip samples still to drop (counts down from `pre_skip` after
    /// every rewind).
    skip_left: usize,
    /// Total frames kira should expect, from the container duration.
    num_frames: usize,
    /// Index of the first frame the next `decode()` will return.
    cursor: usize,
    /// A chunk `seek()` decoded but could not skip whole; handed out by the
    /// next `decode()`.
    pending: Option<Vec<Frame>>,
    eof: bool,
}

impl OpusTrack {
    pub fn open(path: &Path, info: &MediaInfo) -> Result<Self, MediaError> {
        let track = info
            .audio_track
            .ok_or_else(|| MediaError::Audio("the file has no audio track".into()))?;
        let channels = info.channels as usize;
        if channels == 0 || channels > 2 {
            return Err(MediaError::Audio(format!("{channels} channels; mono and stereo only")));
        }
        let mkv = open_demuxer(path)?;
        // kira asks for frames up to num_frames and never past it, so this
        // must not exceed what the stream really holds. The container
        // duration counts from the first packet's start (the pre-skip
        // padding included), so subtracting pre_skip errs on the short side.
        let num_frames = ((info.duration_s * OPUS_RATE as f64).floor() as usize).saturating_sub(info.pre_skip as usize);
        Ok(Self {
            path: path.to_path_buf(),
            mkv,
            track,
            decoder: OpusDecoder::new(channels)?,
            packet: MkvPacket::default(),
            channels,
            pre_skip: info.pre_skip as usize,
            skip_left: info.pre_skip as usize,
            num_frames,
            cursor: 0,
            pending: None,
            eof: false,
        })
    }

    /// Decode the next audio packet into kira frames. Ok(None) at end of file.
    /// A packet that lies entirely inside the pre-skip yields an empty Vec.
    fn decode_packet(&mut self) -> Result<Option<Vec<Frame>>, MediaError> {
        if self.eof {
            return Ok(None);
        }
        loop {
            if !self.mkv.next_frame(&mut self.packet)? {
                self.eof = true;
                return Ok(None);
            }
            if self.packet.track != self.track {
                continue;
            }
            let ch = self.channels;
            let pcm = self.decoder.decode(&self.packet.data)?;
            let n = pcm.len() / ch;
            let drop = self.skip_left.min(n);
            self.skip_left -= drop;
            let frames: Vec<Frame> = pcm[drop * ch..]
                .chunks_exact(ch)
                .map(|s| if ch == 2 { Frame::new(s[0], s[1]) } else { Frame::from_mono(s[0]) })
                .collect();
            return Ok(Some(frames));
        }
    }

    /// Back to the first sample: reopen the file and start a fresh decoder
    /// (a fresh state is exactly what a decoder reset gives, and it is what
    /// makes the samples after a rewind identical to the first pass).
    fn rewind(&mut self) -> Result<(), MediaError> {
        self.mkv = open_demuxer(&self.path)?;
        self.decoder = OpusDecoder::new(self.channels)?;
        self.skip_left = self.pre_skip;
        self.cursor = 0;
        self.pending = None;
        self.eof = false;
        Ok(())
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Decode the whole track to interleaved f32 (tests and tools). Returns
    /// the samples, the channel count and the sample rate.
    pub fn decode_all_pcm(path: &Path, info: &MediaInfo) -> Result<(Vec<f32>, usize, u32), MediaError> {
        let mut track = Self::open(path, info)?;
        let mut out = Vec::new();
        while let Some(frames) = track.decode_packet()? {
            for f in frames {
                if track.channels == 2 {
                    out.push(f.left);
                    out.push(f.right);
                } else {
                    out.push(f.left);
                }
            }
        }
        Ok((out, track.channels, OPUS_RATE))
    }
}

impl kira::sound::streaming::Decoder for OpusTrack {
    type Error = MediaError;

    fn sample_rate(&self) -> u32 {
        OPUS_RATE
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn decode(&mut self) -> Result<Vec<Frame>, MediaError> {
        if let Some(chunk) = self.pending.take() {
            self.cursor += chunk.len();
            return Ok(chunk);
        }
        loop {
            match self.decode_packet()? {
                Some(frames) if frames.is_empty() => continue,
                Some(frames) => {
                    self.cursor += frames.len();
                    return Ok(frames);
                }
                None => {
                    // Past the end. kira's scheduler keeps calling decode()
                    // until it reaches the frame index it wants, so an empty
                    // chunk here would spin it forever; silence keeps the
                    // index moving. It is only reached if the container
                    // duration overstated the stream, which num_frames guards.
                    self.cursor += SILENCE_CHUNK;
                    return Ok(vec![Frame::ZERO; SILENCE_CHUNK]);
                }
            }
        }
    }

    /// Rewind, then decode and discard whole packets up to (never past) the
    /// requested frame. Returns the index actually reached; kira accepts an
    /// earlier one and skips the rest itself. Linear from the start, which
    /// is fine for the seek-to-start this rung supports and correct (if
    /// slow) for anything else.
    fn seek(&mut self, index: usize) -> Result<usize, MediaError> {
        self.rewind()?;
        loop {
            match self.decode_packet()? {
                Some(frames) if frames.is_empty() => continue,
                Some(frames) => {
                    if self.cursor + frames.len() > index {
                        // This packet straddles the target: keep it whole for
                        // the next decode() rather than losing its samples.
                        self.pending = Some(frames);
                        return Ok(self.cursor);
                    }
                    self.cursor += frames.len();
                }
                None => return Ok(self.cursor),
            }
        }
    }
}
