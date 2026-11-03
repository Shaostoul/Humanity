//! What a video file is, in a person's words (BUG-228, 2026-11-03).
//!
//! When nothing here can play a file, the person is told what it IS: its container and its codecs
//! in plain names ("an MP4 file with H.265 video and AAC sound"), never a reader's own message. The
//! operator's report was a screen saying "not a WebM/Matroska file we can read: unexpected element
//! found. Expected: Ebml Found: Unknown", which is the Matroska reader talking about an MP4.
//!
//! Each reader fills in what it learned: the Matroska reader its codec ids (`matroska_codec_name`),
//! the MP4 reader its sample entries (`mp4::codec_name`). A file neither recognises is "a kind of
//! file this app does not read itself", and its name is not guessed from its ending. Every word
//! comes from `data/i18n/en.json` (the `video.*` keys).

use humanity_core::i18n::{t, tf};

/// A container one of our own readers recognised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Mp4,
    /// An MP4's QuickTime parent (`ftyp` brand `qt  `, a `.mov` from a camera or a Mac).
    QuickTime,
    WebM,
    Matroska,
}

impl Container {
    /// "an MP4 file", "a WebM file", ... with its article.
    pub fn words(self) -> &'static str {
        match self {
            Container::Mp4 => t("video.file_mp4"),
            Container::QuickTime => t("video.file_mov"),
            Container::WebM => t("video.file_webm"),
            Container::Matroska => t("video.file_mkv"),
        }
    }
}

/// What a file is: its container, if one of our readers knows it, and the plain names of its first
/// picture and its first sound.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileKind {
    pub container: Option<Container>,
    pub video: Option<String>,
    pub sound: Option<String>,
}

impl FileKind {
    /// "an MP4 file with AV1 video and AAC sound", or `None` when no reader recognised the file.
    pub fn words(&self) -> Option<String> {
        let file = self.container?.words();
        Some(match (&self.video, &self.sound) {
            (Some(video), Some(sound)) => tf("video.kind_video_sound", &[("file", &file), ("video", video), ("sound", sound)]),
            (Some(video), None) => tf("video.kind_video", &[("file", &file), ("video", video)]),
            (None, Some(sound)) => tf("video.kind_sound", &[("file", &file), ("sound", sound)]),
            (None, None) => file.to_string(),
        })
    }

    /// The middle of a sentence that starts with the file's name: "is an MP4 file with H.265 video,
    /// which this app cannot play as it is", or "is a kind of file this app does not read itself".
    pub fn why(&self) -> String {
        match self.words() {
            Some(kind) => tf("video.why_known", &[("kind", &kind)]),
            None => t("video.why_unknown").to_string(),
        }
    }
}

/// A Matroska codec id in plain words: `V_MPEGH/ISO/HEVC` is "H.265", `A_VORBIS` is "Vorbis". An id
/// not listed loses its `V_` or `A_` and reads as it is.
pub fn matroska_codec_name(id: &str) -> String {
    let name = match id {
        "V_AV1" => "AV1",
        "V_VP8" => "VP8",
        "V_VP9" => "VP9",
        "V_MPEG4/ISO/AVC" => "H.264",
        "V_MPEGH/ISO/HEVC" => "H.265",
        "V_MPEGI/ISO/VVC" => "H.266",
        "V_MPEG4/ISO/SP" | "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/AP" | "V_MPEG4/MS/V3" => "MPEG-4 Part 2",
        "V_MPEG2" => "MPEG-2",
        "V_MPEG1" => "MPEG-1",
        "V_THEORA" => "Theora",
        "V_PRORES" => "ProRes",
        "V_FFV1" => "FFV1",
        "V_UNCOMPRESSED" => "uncompressed",
        "V_MS/VFW/FOURCC" => "Video for Windows",
        "A_OPUS" => "Opus",
        "A_VORBIS" => "Vorbis",
        "A_AC3" => "AC-3",
        "A_EAC3" => "E-AC-3",
        "A_MPEG/L3" => "MP3",
        "A_MPEG/L2" => "MP2",
        "A_FLAC" => "FLAC",
        "A_ALAC" => "ALAC",
        "A_TRUEHD" => "Dolby TrueHD",
        id if id.starts_with("A_AAC") => "AAC",
        id if id.starts_with("A_DTS") => "DTS",
        id if id.starts_with("A_PCM") => "PCM",
        other => return other.strip_prefix("V_").or_else(|| other.strip_prefix("A_")).unwrap_or(other).to_string(),
    };
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words a person reads for each shape of file, and never a reader's own message. Seen red
    /// 2026-11-03 with the sound left out of a file that has both: left "an MP4 file with AV1
    /// video", right "an MP4 file with AV1 video and AAC sound".
    #[test]
    fn a_file_is_named_by_its_container_and_codecs() {
        let mp4 = FileKind { container: Some(Container::Mp4), video: Some("AV1".into()), sound: Some("AAC".into()) };
        assert_eq!(mp4.words().as_deref(), Some("an MP4 file with AV1 video and AAC sound"));
        assert_eq!(mp4.why(), "is an MP4 file with AV1 video and AAC sound, which this app cannot play as it is");
        let silent = FileKind { container: Some(Container::WebM), video: Some("VP8".into()), sound: None };
        assert_eq!(silent.words().as_deref(), Some("a WebM file with VP8 video"));
        let unknown = FileKind::default();
        assert_eq!(unknown.words(), None);
        assert_eq!(unknown.why(), "is a kind of file this app does not read itself");
        assert_eq!(matroska_codec_name("V_MPEGH/ISO/HEVC"), "H.265");
        assert_eq!(matroska_codec_name("A_AAC/MPEG4/LC"), "AAC");
        assert_eq!(matroska_codec_name("V_SOMETHING"), "SOMETHING");
    }
}
