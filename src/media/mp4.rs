//! The MP4 container (ISO/IEC 14496-12 and -14) for our own decoders: AV1 video in an MP4, the
//! form video sites hand AV1 out in (BUG-228, 2026-11-03). Its sound plays through our Opus decoder
//! when it is Opus, and through the computer's own decoders otherwise (AAC on Windows, the way an
//! ordinary MP4 plays: `system.rs`). An MP4 whose picture is not AV1 is not ours to decode: `probe`
//! says what it is and the caller asks the computer's own decoders, then the person's ffmpeg.
//!
//! **May we?** Researched first: docs/reference/findings/2026-11-03-mp4-container.md. No pool
//! licenses the container, the patents once tied to it expired in 2018, and AOMedia's binding of AV1
//! into it is one of its royalty-free Final Deliverables.
//!
//! **Who reads the boxes.** `mp4-atom` decodes every box to a type, reading only the bytes really
//! present (Cargo.toml says why it was chosen over `re_mp4` and `mp4parse`). Turning a track's
//! tables into a list of samples is the one thing it leaves to the caller, and that is here
//! (`samples_of`). A file from a stranger in chat is untrusted, so every count is checked before
//! it is believed: no sample may lie outside the file, and no track may list more samples than
//! `MAX_SAMPLES` or than the file has bytes. A malformed file is refused in words, never a panic or
//! an allocation without limit.
//!
//! **Time.** Each track counts in its own timescale. A sample's place on the film's timeline is
//! its composition time less the media time where the track's edit list starts it, plus any empty
//! edit before that, in seconds (`Track::pts_s`). For Opus the edit list is also where the encoder's
//! priming samples are cut ("that task falls on the Edit List Box", the Opus-in-ISOBMFF spec), so
//! the samples to drop are the edit's media time, and the OpusSpecificBox's PreSkip only when the
//! track has no edit list.
//!
//! **Samples as the AV1 decoder takes them.** The binding says a sample is one Temporal Unit of
//! OBUs and every sync sample carries the sequence header, which is exactly what our decoder takes
//! from WebM; the `av1C` box's config OBUs are handed over once before the first sample too
//! (harmless when the samples repeat them, and needed when a file has no sync samples).

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use mp4_atom::{Codec, Ftyp, Header, Moof, Moov, ReadAtom, ReadFrom, StszSamples, Trak};

use super::kind::{Container, FileKind};
use super::MediaError;

/// The most samples one track may list. A day of 60 fps video is 5.2 million samples, a day of
/// AAC sound about 4 million; this bounds the list a hostile file can make us build at about
/// 480 MB, and in practice the file's own length bounds it far lower (`samples_of`).
pub const MAX_SAMPLES: usize = 20_000_000;

/// The largest `moov` or `moof` box read into memory. A two-hour film's `moov` is a few megabytes.
pub const MAX_INDEX_BOX: u64 = 256 * 1024 * 1024;

/// The sample's `sample_is_non_sync_sample` flag (ISO/IEC 14496-12, 8.8.3.1): set on a sample that
/// is not a keyframe.
const NON_SYNC_FLAG: u32 = 1 << 16;

/// One sample: where its bytes are, when it shows (in its track's timescale, before the edit list)
/// and whether decoding may start at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub offset: u64,
    pub size: u32,
    pub cts: i64,
    pub sync: bool,
}

/// What a track carries, as far as playing it goes.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackCodec {
    /// AV1 video and its `av1C` config OBUs (usually one sequence header).
    Av1 { config_obus: Vec<u8> },
    /// Opus sound: its channel count, its PreSkip, and whether it uses a channel mapping family
    /// other than 0 (more than two channels: the multistream layout our decoder does not have).
    Opus { channels: u8, pre_skip: u16, multistream: bool },
    /// Anything else, by its plain name ("H.264", "AAC").
    Other(String),
}

impl TrackCodec {
    /// Its plain name ("AV1", "Opus", "AAC").
    pub fn name(&self) -> String {
        match self {
            TrackCodec::Av1 { .. } => "AV1".to_string(),
            TrackCodec::Opus { .. } => "Opus".to_string(),
            TrackCodec::Other(name) => name.clone(),
        }
    }
}

/// One track: its codec, its clock and its samples in decode order.
#[derive(Clone, Debug)]
pub struct Track {
    pub id: u32,
    pub codec: TrackCodec,
    /// Ticks per second of `Sample::cts`.
    pub timescale: u32,
    /// The media time the edit list starts the track at, in ticks (0 without an edit list).
    pub start_ticks: i64,
    /// How long the track waits before it starts (the edit list's leading empty edits), seconds.
    pub empty_s: f64,
    /// Whether the track has an edit list at all (Opus: whether its priming is cut there).
    pub edited: bool,
    /// Where the track ends on the film's timeline, seconds.
    pub end_s: f64,
    /// The picture's size from the sample entry (0 for sound).
    pub width: u32,
    pub height: u32,
    /// The sound's channels and rate from the sample entry (0 for a picture).
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<Sample>,
}

impl Track {
    /// Sample `s`'s place on the film's timeline, in seconds.
    pub fn pts_s(&self, s: &Sample) -> f64 {
        (s.cts - self.start_ticks) as f64 / self.timescale as f64 + self.empty_s
    }
}

/// A whole MP4: what it is, how long it runs, and its first picture and first sound.
#[derive(Clone, Debug)]
pub struct Movie {
    pub container: Container,
    pub duration_s: f64,
    pub track_count: usize,
    pub video: Option<Track>,
    pub audio: Option<Track>,
}

impl Movie {
    /// What the file is, in plain names, for a person.
    pub fn kind(&self) -> FileKind {
        FileKind {
            container: Some(self.container),
            video: self.video.as_ref().map(|t| t.codec.name()),
            sound: self.audio.as_ref().map(|t| t.codec.name()),
        }
    }
}

/// What `open` makes of a file that is not an MP4 at all (its first box is not one an MP4 or a
/// QuickTime file starts with): the caller tries the next reader.
pub fn not_mp4(e: &MediaError) -> bool {
    matches!(e, MediaError::Demux(d) if d == NOT_MP4)
}

const NOT_MP4: &str = "not an MP4 file";

fn bad(why: impl std::fmt::Display) -> MediaError {
    MediaError::Demux(format!("MP4: {why}"))
}

/// Read the file's index: its `ftyp`, its `moov`, and the `moof` of every fragment. The media
/// itself (`mdat`) is skipped, never read.
pub fn open(path: &Path) -> Result<Movie, MediaError> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut r = BufReader::new(file);
    let mut ftyp: Option<Ftyp> = None;
    let mut moov: Option<Moov> = None;
    let mut moofs: Vec<(u64, Moof)> = Vec::new();
    let mut at = 0u64;
    let mut first = true;
    while at + 8 <= file_len {
        r.seek(SeekFrom::Start(at))?;
        let header = match <Option<Header> as ReadFrom>::read_from(&mut r) {
            Ok(Some(h)) => h,
            Ok(None) => break,
            Err(e) if first => return Err(bad(e)),
            // Bytes after the last whole box (a file cut short, padding): the index is already read.
            Err(_) => break,
        };
        let body = r.stream_position()?;
        // Only a file that starts the way an MP4 or a QuickTime file does is taken for one; anything
        // else goes on to the next reader without a word from this one.
        if first && !matches!(&header.kind.to_string()[..], "ftyp" | "moov" | "mdat" | "free" | "skip" | "wide" | "pnot" | "styp" | "sidx") {
            return Err(MediaError::Demux(NOT_MP4.to_string()));
        }
        first = false;
        let size = match header.size {
            Some(s) => s as u64,
            None => file_len.saturating_sub(body),
        };
        let end = body.saturating_add(size);
        match &header.kind.to_string()[..] {
            "ftyp" if ftyp.is_none() && size <= 4096 => ftyp = Some(Ftyp::read_atom(&header, &mut r).map_err(bad)?),
            "moov" if moov.is_none() => {
                if size > MAX_INDEX_BOX || end > file_len {
                    return Err(bad(format!("its index is {size} bytes, past the file's end or too large")));
                }
                moov = Some(Moov::read_atom(&header, &mut r).map_err(bad)?);
            }
            "moof" => {
                if size > MAX_INDEX_BOX || end > file_len {
                    break; // a fragment cut short: play what came before it
                }
                // Kept with where the box starts: a run's data offsets may count from there.
                moofs.push((at, Moof::read_atom(&header, &mut r).map_err(bad)?));
            }
            _ => {}
        }
        if header.size.is_none() || end <= at {
            break;
        }
        at = end;
    }
    let moov = moov.ok_or_else(|| bad("it has no index (no moov box)"))?;
    let container = match &ftyp {
        Some(f) if f.major_brand.to_string() == "qt  " => Container::QuickTime,
        None => Container::QuickTime, // an old QuickTime file starts without one
        Some(_) => Container::Mp4,
    };
    let movie_scale = moov.mvhd.timescale.max(1) as f64;
    let mut video = None;
    let mut audio = None;
    for trak in &moov.trak {
        let handler = trak.mdia.hdlr.handler.to_string();
        let slot = match &handler[..] {
            "vide" if video.is_none() => &mut video,
            "soun" if audio.is_none() => &mut audio,
            _ => continue,
        };
        *slot = Some(track_of(trak, &moov, &moofs, movie_scale, file_len)?);
    }
    let declared = |d: u64| if d == 0 || d == u32::MAX as u64 || d == u64::MAX { 0.0 } else { d as f64 / movie_scale };
    let mut duration_s = declared(moov.mvhd.duration);
    if duration_s <= 0.0 {
        duration_s = moov.mvex.as_ref().and_then(|m| m.mehd.as_ref()).map(|m| declared(m.fragment_duration)).unwrap_or(0.0);
    }
    if duration_s <= 0.0 {
        duration_s = video.iter().chain(audio.iter()).map(|t: &Track| t.end_s).fold(0.0, f64::max);
    }
    Ok(Movie { container, duration_s, track_count: moov.trak.len(), video, audio })
}

/// A sample entry's plain name: "H.264" for `avc1`, "AAC" for `mp4a`, the four letters otherwise.
pub fn codec_name(codec: &Codec) -> String {
    match codec {
        Codec::Av01(_) => "AV1".into(),
        Codec::Avc1(_) | Codec::Avc3(_) => "H.264".into(),
        Codec::Hev1(_) | Codec::Hvc1(_) => "H.265".into(),
        Codec::Vp08(_) => "VP8".into(),
        Codec::Vp09(_) => "VP9".into(),
        Codec::Jpeg(_) => "Motion JPEG".into(),
        Codec::Uncv(_) => "uncompressed".into(),
        Codec::Mp4a(m) => match m.esds.es_desc.dec_config.object_type_indication {
            0x69 | 0x6B => "MP3".into(),
            _ => "AAC".into(),
        },
        Codec::Mp3(_) => "MP3".into(),
        Codec::Opus(_) => "Opus".into(),
        Codec::Flac(_) => "FLAC".into(),
        Codec::Ac3(_) => "AC-3".into(),
        Codec::Eac3(_) => "E-AC-3".into(),
        Codec::Samr(_) => "AMR".into(),
        Codec::Ipcm(_) | Codec::Fpcm(_) | Codec::Sowt(_) | Codec::Twos(_) | Codec::Lpcm(_) | Codec::In24(_) | Codec::In32(_) | Codec::Fl32(_) | Codec::Fl64(_) | Codec::S16l(_) => "PCM".into(),
        Codec::Unknown(fourcc, _) => match &fourcc.to_string()[..] {
            "mp4v" => "MPEG-4 Part 2".into(),
            "apcn" | "apch" | "apcs" | "apco" | "ap4h" | "ap4x" => "ProRes".into(),
            "alac" => "ALAC".into(),
            "dtsc" | "dtsh" | "dtsl" | "dtse" => "DTS".into(),
            "vvc1" | "vvi1" => "H.266".into(),
            "dvh1" | "dvhe" | "dvav" | "dva1" => "Dolby Vision".into(),
            "ac-4" => "AC-4".into(),
            other => {
                let s = other.trim();
                if !s.is_empty() && s.chars().all(|c| c.is_ascii_graphic()) {
                    s.to_string()
                } else {
                    "unknown".into()
                }
            }
        },
        _ => "unknown".into(),
    }
}

/// One `trak` made playable: its codec, clock, edit list and samples.
fn track_of(trak: &Trak, moov: &Moov, moofs: &[(u64, Moof)], movie_scale: f64, file_len: u64) -> Result<Track, MediaError> {
    let stbl = &trak.mdia.minf.stbl;
    let entry = stbl.stsd.codecs.first().ok_or_else(|| bad("a track has no sample description"))?;
    let codec = match entry {
        Codec::Av01(a) => TrackCodec::Av1 { config_obus: a.av1c.config_obus.clone() },
        Codec::Opus(o) => TrackCodec::Opus {
            channels: o.dops.output_channel_count,
            pre_skip: o.dops.pre_skip,
            multistream: o.dops.channel_mapping.is_some(),
        },
        other => TrackCodec::Other(codec_name(other)),
    };
    let (width, height) = entry.visual().map(|v| (v.width as u32, v.height as u32)).unwrap_or((0, 0));
    let (channels, sample_rate) = entry.audio().map(|a| (a.channel_count, a.sample_rate.integer() as u32)).unwrap_or((0, 0));
    let timescale = trak.mdia.mdhd.timescale;
    if timescale == 0 {
        return Err(bad("a track has no timescale"));
    }
    // The edit list: leading empty edits delay the track; the first real edit says which media time
    // shows first.
    let mut empty_s = 0.0;
    let mut start_ticks = 0i64;
    let mut edited = false;
    if let Some(elst) = trak.edts.as_ref().and_then(|e| e.elst.as_ref()) {
        for e in &elst.entries {
            edited = true;
            match e.media_time {
                None => empty_s += e.segment_duration as f64 / movie_scale,
                Some(t) => {
                    start_ticks = i64::try_from(t).map_err(|_| bad("an edit starts past any time"))?;
                    break;
                }
            }
        }
    }
    let (samples, end_ticks) = samples_of(trak, moov, moofs, file_len)?;
    let end_s = (end_ticks - start_ticks) as f64 / timescale as f64 + empty_s;
    Ok(Track { id: trak.tkhd.track_id, codec, timescale, start_ticks, empty_s, edited, end_s, width, height, channels, sample_rate, samples })
}

/// Walks a run-length table one sample at a time: `(count, value)` runs, the value of the run each
/// sample falls in (a sample past the last run keeps the last value, or `default`).
struct Runs<'a, T: Copy> {
    runs: &'a [(u32, T)],
    at: usize,
    left: u32,
    last: T,
}

impl<'a, T: Copy> Runs<'a, T> {
    fn new(runs: &'a [(u32, T)], default: T) -> Self {
        Self { runs, at: 0, left: runs.first().map_or(0, |r| r.0), last: runs.first().map_or(default, |r| r.1) }
    }

    fn next(&mut self) -> T {
        while self.left == 0 && self.at + 1 < self.runs.len() {
            self.at += 1;
            self.left = self.runs[self.at].0;
            self.last = self.runs[self.at].1;
        }
        self.left = self.left.saturating_sub(1);
        self.last
    }
}

/// A track's samples in decode order, from its sample tables and then from every fragment, and
/// where its last sample ends (composition time, ticks). Every sample lies inside the file.
fn samples_of(trak: &Trak, moov: &Moov, moofs: &[(u64, Moof)], file_len: u64) -> Result<(Vec<Sample>, i64), MediaError> {
    let limit = MAX_SAMPLES.min(usize::try_from(file_len).unwrap_or(usize::MAX));
    let stbl = &trak.mdia.minf.stbl;
    let count = match &stbl.stsz.samples {
        StszSamples::Identical { count, .. } => *count as usize,
        StszSamples::Different { sizes } => sizes.len(),
    };
    // Every sample is at least a byte of the file, so a count above the file's length is a lie.
    if count > limit {
        return Err(bad(format!("a track lists {count} samples, more than the file could hold")));
    }
    let size_of = |i: usize| -> u32 {
        match &stbl.stsz.samples {
            StszSamples::Identical { size, .. } => *size,
            StszSamples::Different { sizes } => sizes[i],
        }
    };
    let chunks: Vec<u64> = match (&stbl.stco, &stbl.co64) {
        (Some(stco), _) => stco.entries.iter().map(|&o| o as u64).collect(),
        (None, Some(co64)) => co64.entries.clone(),
        (None, None) if count == 0 => Vec::new(),
        (None, None) => return Err(bad("a track has samples but no chunk offsets")),
    };
    let deltas: Vec<(u32, u32)> = stbl.stts.entries.iter().map(|e| (e.sample_count, e.sample_delta)).collect();
    let offsets: Vec<(u32, i64)> = stbl.ctts.as_ref().map(|c| c.entries.iter().map(|e| (e.sample_count, e.sample_offset)).collect()).unwrap_or_default();
    let mut delta = Runs::new(&deltas, 0);
    let mut shift = Runs::new(&offsets, 0);
    let stsc = &stbl.stsc.entries;
    let mut out: Vec<Sample> = Vec::with_capacity(count);
    let mut dts = 0i64;
    let mut end = 0i64;
    let mut run = 0usize;
    'chunks: for (c, &chunk_offset) in chunks.iter().enumerate() {
        // Which run of the sample-to-chunk table this chunk is in (chunks are numbered from 1).
        let chunk_no = c as u64 + 1;
        while run + 1 < stsc.len() && stsc[run + 1].first_chunk as u64 <= chunk_no {
            run += 1;
        }
        let Some(entry) = stsc.get(run) else { break };
        if entry.first_chunk as u64 > chunk_no {
            return Err(bad("the sample-to-chunk table does not start at the first chunk"));
        }
        let mut at = chunk_offset;
        for _ in 0..entry.samples_per_chunk {
            if out.len() == count {
                break 'chunks;
            }
            let size = size_of(out.len());
            let after = at.checked_add(size as u64).filter(|&e| e <= file_len).ok_or_else(|| bad("a sample lies past the end of the file"))?;
            let d = delta.next() as i64;
            let cts = dts.checked_add(shift.next()).ok_or_else(|| bad("its times run past any clock"))?;
            out.push(Sample { offset: at, size, cts, sync: stbl.stss.is_none() });
            at = after;
            dts = dts.checked_add(d).ok_or_else(|| bad("its times run past any clock"))?;
            end = end.max(cts.saturating_add(d));
        }
    }
    // (A table that lists more samples than its chunks hold keeps the samples that are there.)
    // No sync table means every sample is one; with one, only those it lists (numbered from 1).
    if let Some(stss) = &stbl.stss {
        for &n in &stss.entries {
            if let Some(s) = (n as usize).checked_sub(1).and_then(|i| out.get_mut(i)) {
                s.sync = true;
            }
        }
    }

    // Fragments: each `traf` of this track adds its runs, in file order.
    let id = trak.tkhd.track_id;
    let trex_of = |track: u32| moov.mvex.as_ref().and_then(|m| m.trex.iter().find(|t| t.track_id == track));
    for (moof_start, moof) in moofs {
        // Where the previous track fragment's data ended: the base of a next one that states none.
        let mut traf_end: Option<u64> = None;
        for traf in &moof.traf {
            let tfhd = &traf.tfhd;
            let trex = trex_of(tfhd.track_id);
            // The base every run's data offset counts from (ISO/IEC 14496-12, 8.8.7.1): stated, or the
            // moof itself, or (with neither) the moof for the first track fragment and the end of the
            // previous one's data after that. Another track's fragment is walked for that end alone.
            let base = match tfhd.base_data_offset {
                Some(b) => b,
                None if tfhd.default_base_is_moof => *moof_start,
                None => traf_end.unwrap_or(*moof_start),
            };
            let mine = tfhd.track_id == id;
            if mine {
                if let Some(tfdt) = &traf.tfdt {
                    dts = i64::try_from(tfdt.base_media_decode_time).map_err(|_| bad("a fragment starts past any time"))?;
                }
            }
            let default_duration = tfhd.default_sample_duration.or(trex.map(|t| t.default_sample_duration)).unwrap_or(0);
            let default_size = tfhd.default_sample_size.or(trex.map(|t| t.default_sample_size)).unwrap_or(0);
            let default_flags = tfhd.default_sample_flags.or(trex.map(|t| t.default_sample_flags)).unwrap_or(0);
            let mut at = base;
            for trun in &traf.trun {
                // A run with no data offset follows straight on from the one before it.
                if let Some(off) = trun.data_offset {
                    at = base.checked_add_signed(off as i64).ok_or_else(|| bad("a fragment's data lies before the file"))?;
                }
                let n = trun.samples.len();
                if mine && out.len().saturating_add(n) > limit {
                    return Err(bad("a track lists more samples than the file could hold"));
                }
                if !mine && n > limit {
                    return Err(bad("a fragment lists more samples than the file could hold"));
                }
                for i in 0..n {
                    // `get` hands the first sample its run's first-sample flags, when the run has them.
                    let e = trun.samples.get(i).unwrap_or_default();
                    let size = e.size.unwrap_or(default_size);
                    let after = at.checked_add(size as u64).filter(|&e| e <= file_len).ok_or_else(|| bad("a sample lies past the end of the file"))?;
                    if mine {
                        let flags = e.flags.unwrap_or(default_flags);
                        let d = e.duration.unwrap_or(default_duration) as i64;
                        let cts = dts.checked_add(e.cts.unwrap_or(0)).ok_or_else(|| bad("its times run past any clock"))?;
                        out.push(Sample { offset: at, size, cts, sync: flags & NON_SYNC_FLAG == 0 });
                        dts = dts.checked_add(d).ok_or_else(|| bad("its times run past any clock"))?;
                        end = end.max(cts.saturating_add(d));
                    }
                    at = after;
                }
            }
            traf_end = Some(at);
        }
    }
    Ok((out, end))
}

/// A track's samples read one by one from the file: what the decode thread and the Opus sound pull
/// from, each on a file handle of its own (the Matroska path's rule).
pub struct SampleReader {
    file: BufReader<File>,
    pub track: Track,
    next: usize,
}

impl SampleReader {
    /// The file's first picture (`video`) or first sound, read again from the file.
    pub fn open(path: &Path, video: bool) -> Result<Self, MediaError> {
        let movie = open(path)?;
        let track = if video { movie.video } else { movie.audio }.ok_or(if video { MediaError::NoVideoTrack } else { MediaError::Audio("the file has no sound".into()) })?;
        Ok(Self { file: BufReader::new(File::open(path)?), track, next: 0 })
    }

    /// The next sample's bytes into `buf`, with its time in seconds and whether it is a sync sample;
    /// `None` after the last.
    pub fn next(&mut self, buf: &mut Vec<u8>) -> Result<Option<(f64, bool)>, MediaError> {
        let Some(s) = self.track.samples.get(self.next).copied() else { return Ok(None) };
        self.next += 1;
        self.file.seek(SeekFrom::Start(s.offset))?;
        buf.clear();
        buf.resize(s.size as usize, 0);
        self.file.read_exact(buf)?;
        Ok(Some((self.track.pts_s(&s), s.sync)))
    }

    /// Back to the first sample.
    pub fn rewind(&mut self) {
        self.next = 0;
    }

    /// The next sample read will be the last sync sample that shows at or before `seconds` (the
    /// first sample when none does), so decoding can start there.
    pub fn seek_to_sync(&mut self, seconds: f64) {
        let t = &self.track;
        self.next = t.samples.iter().enumerate().filter(|(_, s)| s.sync && t.pts_s(s) <= seconds + 1.0e-6).map(|(i, _)| i).last().unwrap_or(0);
    }
}

#[cfg(test)]
#[path = "mp4_tests.rs"]
mod tests;
