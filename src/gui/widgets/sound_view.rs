//! A sound or a song played in the app (2026-10-19, the Media page's middle pane), the way a video
//! is (video_view.rs): fetched first to a temporary copy on a worker, then, on that same worker,
//!
//! - OPENED as a stream for the app's mixer (kira's `StreamingSoundData`): the mixer reads and
//!   decodes it a piece at a time on its own thread while it plays, so a long recording never sits
//!   in memory whole. It is handed to the mixer by [`SoundView::sync_sound`], which the engine
//!   calls each frame it has a mixer, at the master volume or silent while muted;
//! - READ ONCE, whole, by the app's own decoders (symphonia, which kira plays with: MP3, Ogg
//!   Vorbis, FLAC and WAV), into the picture the waveform widget draws (waveform.rs `Analysis`:
//!   the peaks of the whole track and its spectrum over time). Nothing is read or decoded again
//!   while it is shown: a frame looks up that picture by the play position.
//!
//! Its buttons: Play here (before anything is fetched), Play or Pause, Start again, Mute, its time,
//! and a press anywhere on the waveform goes there. A file these decoders cannot read says so, and
//! can still be downloaded. The temporary copy goes with the view.
//!
//! Web mirror: the browser's own `<audio>` element (web/pages/media-app.js plays a sound that way).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use egui::RichText;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::sound::{FromFileError, PlaybackState};
use kira::tween::Tween;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::waveform::{self, Analysis, Builder};

/// The view's buttons and lines.
pub const PLAY_HERE: &str = "Play here";
pub const PLAY_HERE_TIP: &str = "Fetch the sound and play it in the app, with its waveform.";
pub const FETCHING: &str = "Fetching the sound...";
pub const READING: &str = "Reading the sound for its waveform...";
pub const WAITING_FOR_SOUND: &str = "Waiting for the app's sound output...";
pub const START_AGAIN: &str = "Start again";
pub const SEEK_TIP: &str = "Press anywhere on the waveform to go there.";

/// What it says when the file will not play: the decoder's reason, then what to do.
pub fn cannot_play_words(why: &str) -> String {
    format!("This sound cannot play in the app: {why}. Download it to hear it in another player.")
}

/// What the worker sends back.
enum Message {
    /// The copy is here and opened (or why it would not open) for the mixer.
    Opened { path: PathBuf, data: Result<StreamingSoundData<FromFileError>, String> },
    /// The whole sound read for its waveform (or why not).
    Analysed(Result<Analysis, String>),
    /// The copy could not be fetched.
    NotFetched(String),
}

/// A sound being fetched, playing, paused, or that could not play.
#[derive(Default)]
pub struct SoundView {
    rx: Option<Receiver<Message>>,
    /// Opened for the mixer, waiting for it (the engine's next `sync_sound`).
    pending: Option<StreamingSoundData<FromFileError>>,
    handle: Option<StreamingSoundHandle<FromFileError>>,
    /// Its waveform and spectrum, once read.
    pub analysis: Option<Arc<Analysis>>,
    /// Why it cannot play, or why the fetch failed.
    pub problem: String,
    /// The temporary copy, deleted with the view.
    pub temp: Option<PathBuf>,
    pub muted: bool,
    pub(crate) volume_sent: Option<f64>,
    /// Where it stands (seconds) while the mixer is not playing it: before it starts, after it
    /// ended, or where a press put it.
    at_s: f64,
    /// Play was asked for: the mixer starts it as soon as it has it.
    want_play: bool,
    /// It played to its end: Play opens the copy again.
    ended: bool,
    /// Its length, from the stream until the waveform's count of samples is in.
    duration_s: f64,
}

impl SoundView {
    /// Fetch the sound with `fetch` on a worker (it returns the temporary copy), open it for the
    /// mixer and play it, and read it once for its waveform.
    pub fn start(&mut self, fetch: impl FnOnce() -> Result<PathBuf, String> + Send + 'static) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let path = match fetch() {
                Ok(path) => path,
                Err(e) => {
                    let _ = tx.send(Message::NotFetched(e));
                    return;
                }
            };
            let data = open_stream(&path);
            // The view went while it was fetching: nobody will play it, so the copy goes now.
            if tx.send(Message::Opened { path: path.clone(), data }).is_err() {
                let _ = std::fs::remove_file(&path);
                return;
            }
            let _ = tx.send(Message::Analysed(analyse(&path)));
        });
        self.rx = Some(rx);
        self.want_play = true;
        self.problem.clear();
    }

    /// Whether anything was fetched or is on its way (Play here is not offered then).
    pub fn started(&self) -> bool {
        self.rx.is_some() || self.temp.is_some() || self.handle.is_some() || self.pending.is_some()
    }

    /// Whether the copy is still on its way.
    pub fn fetching(&self) -> bool {
        self.rx.is_some() && self.temp.is_none()
    }

    /// Whether the mixer is playing it now.
    pub fn playing(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| matches!(h.state(), PlaybackState::Playing))
    }

    /// Its length in seconds (0 until known).
    pub fn duration_s(&self) -> f64 {
        self.analysis.as_ref().map(|a| a.duration_s).filter(|d| *d > 0.0).unwrap_or(self.duration_s)
    }

    /// Where it is now, in seconds.
    pub fn position_s(&self) -> f64 {
        match &self.handle {
            Some(h) if !matches!(h.state(), PlaybackState::Stopped) => h.position(),
            _ => self.at_s,
        }
    }

    /// Take what the worker sent, and notice the end. Every frame the view is drawn.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut done = false;
        if let Some(rx) = self.rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(Message::Opened { path, data }) => {
                        self.temp = Some(path); // deleted with the view from here on
                        match data {
                            Ok(data) => {
                                self.duration_s = data.duration().as_secs_f64();
                                self.pending = Some(data);
                            }
                            Err(e) => self.problem = e,
                        }
                    }
                    Ok(Message::Analysed(result)) => {
                        match result {
                            Ok(a) => self.analysis = Some(Arc::new(a)),
                            // Played but not drawn: say why under it, unless it would not play at all.
                            Err(e) if self.problem.is_empty() && self.pending.is_none() && self.handle.is_none() => self.problem = e,
                            Err(e) => log::warn!("media: a sound plays but its waveform could not be read: {e}"),
                        }
                        done = true;
                        break;
                    }
                    Ok(Message::NotFetched(e)) => {
                        self.problem = e;
                        done = true;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint_after(Duration::from_millis(200));
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            self.rx = None;
        }
        if let Some(h) = self.handle.as_ref() {
            match h.state() {
                PlaybackState::Stopped => {
                    // Played to its end (or stopped): it stands at the end until Play.
                    self.handle = None;
                    self.ended = true;
                    self.at_s = self.duration_s();
                }
                // The line moves while it plays: thirty times a second is smooth enough.
                PlaybackState::Playing => ctx.request_repaint_after(Duration::from_millis(33)),
                _ => {}
            }
        }
        if self.want_play && self.pending.is_some() && self.handle.is_none() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Play from where it stands: resumed, or opened again from the copy when it ended.
    pub fn play(&mut self) {
        if let Some(h) = self.handle.as_mut() {
            h.resume(Tween::default());
            return;
        }
        self.want_play = true;
        if self.ended || (self.pending.is_none() && self.rx.is_none()) {
            if self.ended && self.at_s >= self.duration_s() - 0.05 {
                self.at_s = 0.0;
            }
            self.ended = false;
            if let Some(path) = self.temp.clone() {
                match open_stream(&path) {
                    Ok(data) => self.pending = Some(data),
                    Err(e) => self.problem = e,
                }
            }
        }
    }

    pub fn pause(&mut self) {
        if let Some(h) = self.handle.as_mut() {
            h.pause(Tween::default());
        }
        self.want_play = false;
    }

    /// Go to `fraction` (0 to 1) of the way through.
    pub fn seek_fraction(&mut self, fraction: f32) {
        let to = fraction.clamp(0.0, 1.0) as f64 * self.duration_s();
        self.seek_to(to);
    }

    /// Go to `seconds` in.
    pub fn seek_to(&mut self, seconds: f64) {
        self.at_s = seconds.max(0.0);
        match self.handle.as_mut() {
            Some(h) => h.seek_to(self.at_s),
            None => {
                // Not playing: it starts from there when played.
                if self.ended {
                    self.ended = false;
                    if let Some(path) = self.temp.clone() {
                        if let Ok(data) = open_stream(&path) {
                            self.pending = Some(data);
                        }
                    }
                    self.want_play = false;
                }
            }
        }
    }

    /// The sound in a `size` area: its waveform and spectrum, then its buttons and time; Play here
    /// while nothing is fetched. Returns true when Play here was pressed (the caller then calls
    /// [`SoundView::start`]).
    pub fn draw(&mut self, ui: &mut egui::Ui, theme: &Theme, size: egui::Vec2) -> bool {
        let small = |s: &str| RichText::new(s).size(theme.font_size_small).color(theme.text_muted());
        if !self.started() {
            if !self.problem.is_empty() {
                ui.add(egui::Label::new(RichText::new(&self.problem).size(theme.font_size_small).color(theme.danger())).wrap());
                return false;
            }
            return widgets::Button::primary(PLAY_HERE).tooltip(PLAY_HERE_TIP).show(ui, theme);
        }
        let controls = theme.button_height + theme.spacing_sm * 2.0 + theme.font_size_small;
        let area = egui::vec2(size.x.max(1.0), (size.y - controls).max(theme.button_height * 2.0));
        match self.analysis.clone() {
            Some(a) => {
                let duration = self.duration_s();
                let at = self.position_s();
                let played = if duration > 0.0 { (at / duration) as f32 } else { 0.0 };
                if let Some(to) = waveform::show(ui, theme, &a, played, at, area) {
                    self.seek_fraction(to);
                }
            }
            None => {
                let (rect, _) = ui.allocate_exact_size(area, egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::CornerRadius::same(theme.border_radius as u8), theme.bg_tertiary());
                let words = if self.fetching() { FETCHING } else if self.rx.is_some() { READING } else { "" };
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, words, egui::FontId::proportional(theme.font_size_small), theme.text_muted());
            }
        }
        if !self.problem.is_empty() {
            ui.add(egui::Label::new(RichText::new(&self.problem).size(theme.font_size_small).color(theme.danger())).wrap());
            return false;
        }
        ui.horizontal(|ui| {
            if self.playing() {
                if widgets::Button::secondary("Pause").show(ui, theme) {
                    self.pause();
                }
            } else if widgets::Button::secondary("Play").show(ui, theme) {
                self.play();
            }
            if widgets::Button::ghost(START_AGAIN).show(ui, theme) {
                self.seek_to(0.0);
            }
            if widgets::Button::ghost(if self.muted { widgets::video_view::UNMUTE } else { widgets::video_view::MUTE }).show(ui, theme) {
                self.muted = !self.muted;
            }
            ui.label(small(&format!("{} of {}", widgets::video_view::clock(self.position_s()), widgets::video_view::clock(self.duration_s()))));
            if self.want_play && self.handle.is_none() && self.pending.is_some() {
                ui.label(small(WAITING_FOR_SOUND));
            }
        });
        false
    }

    /// The sound's place in the app's mixer: handed over once Play is asked for, then kept at the
    /// master volume, or silent while muted.
    pub fn sync_sound(&mut self, audio: &mut crate::audio::AudioManager) {
        let volume = if self.muted { 0.0 } else { audio.master_volume() };
        if self.handle.is_none() && self.want_play {
            if let Some(data) = self.pending.take() {
                let data = data.start_position(self.at_s);
                match audio.play_stream(data, volume, 0.5) {
                    Ok(h) => {
                        self.handle = Some(h);
                        self.volume_sent = Some(volume);
                    }
                    Err(e) => self.problem = cannot_play_words(&e),
                }
                return;
            }
        }
        if let Some(h) = self.handle.as_mut() {
            if self.volume_sent != Some(volume) {
                h.set_volume(kira::Volume::Amplitude(volume), Tween { duration: Duration::from_millis(30), ..Default::default() });
                self.volume_sent = Some(volume);
            }
        }
    }

    /// A view already holding `analysis`, standing at `at_s` seconds (the snapshots and tests draw
    /// only a few frames, with no mixer).
    #[cfg(test)]
    pub fn with_analysis(analysis: Analysis, at_s: f64) -> SoundView {
        let mut view = SoundView::default();
        view.duration_s = analysis.duration_s;
        view.analysis = Some(Arc::new(analysis));
        view.at_s = at_s;
        view
    }

    /// For the tests: pretend the copy is here (so the view counts as started).
    #[cfg(test)]
    pub fn set_temp_for_test(&mut self, path: PathBuf) {
        self.temp = Some(path);
    }
}

impl Drop for SoundView {
    /// The mixer lets go of it first (a stream handed to the mixer plays on when its handle is
    /// dropped), then the temporary copy goes.
    fn drop(&mut self) {
        if let Some(mut h) = self.handle.take() {
            h.stop(Tween::default());
        }
        self.pending = None;
        if let Some(path) = self.temp.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("media: a temporary sound could not be deleted yet ({e}); it goes at the next start.");
                }
            }
        }
    }
}

/// `path` opened as a stream for the mixer (reads its header only).
fn open_stream(path: &Path) -> Result<StreamingSoundData<FromFileError>, String> {
    #[cfg(test)]
    tests::note("open", path);
    StreamingSoundData::from_file(path).map_err(|e| cannot_play_words(&e.to_string()))
}

/// Read the whole sound at `path` once, a packet at a time, into its waveform and spectrum.
pub fn analyse(path: &Path) -> Result<Analysis, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    #[cfg(test)]
    tests::note("decode", path);

    let file = std::fs::File::open(path).map_err(|e| format!("The sound could not be opened ({e})."))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| cannot_play_words(&e.to_string()))?;
    let mut format = probed.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).cloned().ok_or_else(|| cannot_play_words("it holds no sound"))?;
    let rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| cannot_play_words(&e.to_string()))?;
    let mut builder = Builder::new(rate);
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(cannot_play_words(&e.to_string())),
        };
        if packet.track_id() != track.id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let channels = spec.channels.count();
                let need = decoded.capacity() * channels;
                if buffer.as_ref().is_none_or(|b| b.capacity() < need) {
                    buffer = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
                }
                if let Some(b) = buffer.as_mut() {
                    b.copy_interleaved_ref(decoded);
                    builder.push_interleaved(b.samples(), channels);
                }
            }
            // A damaged packet is passed over, as a player passes over it.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(cannot_play_words(&e.to_string())),
        }
    }
    Ok(builder.finish())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Every open and every whole read of a file, by the tests' own paths: what the "nothing is
    /// read per frame" test counts.
    static SEEN: Mutex<Vec<(&'static str, PathBuf)>> = Mutex::new(Vec::new());

    pub(super) fn note(what: &'static str, path: &Path) {
        if let Ok(mut s) = SEEN.lock() {
            s.push((what, path.to_path_buf()));
        }
    }

    fn times(what: &str, path: &Path) -> usize {
        SEEN.lock().map(|s| s.iter().filter(|(w, p)| *w == what && p == path).count()).unwrap_or(0)
    }

    /// The fixture: one second of 440 Hz at 0.9 of full scale, then one of 2000 Hz at 0.25, a
    /// mono WAV at 8000 samples a second (made by a short script, 2026-10-19).
    pub(crate) fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/media/two-tones-440-then-2000.wav")
    }

    /// THE WAVEFORM'S PEAKS AND SPECTRUM ARE THE SOUND'S OWN (2026-10-19): the fixture read by the
    /// app's decoders gives two seconds, peaks near 0.9 through the first second and near 0.25
    /// through the second, and a spectrum whose strongest band holds 440 Hz half a second in and
    /// 2000 Hz a second and a half in.
    ///
    /// Seen red 2026-10-19 with each stretch's peak taken from its last frame alone (`push_frame`
    /// replacing the stretch's lowest and highest instead of keeping them): "the first second peaks
    /// near 0.9: 0.028259277 to 0.89953613".
    #[test]
    fn the_waveform_is_the_sounds_own() {
        let a = analyse(&fixture()).expect("the fixture reads");
        assert!((a.duration_s - 2.0).abs() < 0.01, "two seconds: {}", a.duration_s);
        assert_eq!(a.sample_rate, 8000);
        let n = a.peaks.len();
        assert!(n > 100, "{n} stretches");
        let first = &a.peaks[..n / 2 - 2];
        let second = &a.peaks[n / 2 + 2..];
        let loudest = |p: &[(f32, f32)]| p.iter().map(|(lo, hi)| lo.abs().max(hi.abs())).fold(0.0f32, f32::max);
        let quietest = |p: &[(f32, f32)]| p.iter().map(|(lo, hi)| lo.abs().max(hi.abs())).fold(1.0f32, f32::min);
        assert!((loudest(first) - 0.9).abs() < 0.02 && quietest(first) > 0.8, "the first second peaks near 0.9: {} to {}", quietest(first), loudest(first));
        assert!((loudest(second) - 0.25).abs() < 0.02 && quietest(second) > 0.2, "the second near 0.25: {} to {}", quietest(second), loudest(second));
        let edges = waveform::band_edges_hz(8000);
        let band_of = |hz: f32| edges.windows(2).position(|w| hz >= w[0] && hz < w[1]).expect("a band holds it");
        let strongest = |at: f64| a.spectrum_at(at).map(|s| s.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).map(|(i, _)| i).unwrap()).expect("a spectrum");
        assert_eq!(strongest(0.5), band_of(440.0), "440 Hz half a second in");
        assert_eq!(strongest(1.5), band_of(2000.0), "2000 Hz a second and a half in");
    }

    /// NOTHING IS READ OR DECODED WHILE IT IS SHOWN (2026-10-19): the sound is fetched, opened for
    /// the mixer and read for its waveform once each; then, with its copy deleted from the disk,
    /// a hundred frames still draw its waveform and spectrum, and nothing more was opened or read.
    ///
    /// Seen red 2026-10-19 with the waveform read again in `draw` whenever a frame drew it
    /// (`analyse` of the copy on each draw): "read once" (left: 103, right: 1).
    #[test]
    fn nothing_is_read_or_decoded_while_it_is_shown() {
        let copy = crate::test_temp::file("sound_view_once", "wav");
        std::fs::copy(fixture(), &copy).unwrap();
        let path = copy.to_path_buf();
        let fetches = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut view = SoundView::default();
        let (counted, from) = (fetches.clone(), path.clone());
        view.start(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(from)
        });
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        let frame = |ctx: &egui::Context, view: &mut SoundView| {
            ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0))), ..Default::default() }, |ctx| {
                view.poll(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    view.draw(ui, &theme, egui::vec2(700.0, 300.0));
                });
            })
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while view.analysis.is_none() {
            frame(&ctx, &mut view);
            assert!(std::time::Instant::now() < deadline, "the waveform arrives: {}", view.problem);
            std::thread::sleep(Duration::from_millis(5));
        }
        let analysis = view.analysis.clone().unwrap();
        // The copy deleted from the disk under the view (which still names it): anything that
        // tried to read it again would be counted, and would fail.
        assert_eq!(view.temp.as_deref(), Some(path.as_path()), "the view holds its copy");
        let _ = std::fs::remove_file(&path);
        let mut out = frame(&ctx, &mut view);
        for _ in 0..99 {
            out = frame(&ctx, &mut view);
        }
        assert_eq!(fetches.load(std::sync::atomic::Ordering::SeqCst), 1, "fetched once");
        assert_eq!(times("open", &path), 1, "opened for the mixer once");
        assert_eq!(times("decode", &path), 1, "read once");
        assert!(Arc::ptr_eq(&analysis, view.analysis.as_ref().unwrap()), "the same waveform throughout");
        assert!(view.problem.is_empty(), "{}", view.problem);
        let meshes = out.shapes.iter().filter(|c| matches!(&c.shape, egui::Shape::Mesh(m) if m.vertices.len() > 400)).count();
        assert!(meshes >= 1, "the waveform is still drawn");
    }

    /// THE SOUND PLAYS THROUGH THE APP'S MIXER, GOES WHERE IT IS SENT, FOLLOWS MUTE AND STOPS AT ITS
    /// END (2026-10-19). Needs a sound device, so ignored by default; run by hand after touching
    /// `sync_sound`: node scripts/lib-tests.js sound_view::tests::a_sound_plays --ignored --nocapture.
    /// Silent throughout (muted, then a master volume no one can hear), as the video's is.
    ///
    /// Seen red 2026-10-19 with `sync_sound` handing the stream over without its start position
    /// (`start_position` left out): "it starts where it was sent: 0.14" (from the start).
    #[test]
    #[ignore = "needs an audio output device; run: node scripts/lib-tests.js sound_view::tests::a_sound_plays --ignored --nocapture"]
    fn a_sound_plays_through_the_mixer_goes_where_sent_and_stops_at_its_end() {
        let mut audio = match crate::audio::AudioManager::try_new() {
            Ok(a) => a,
            Err(e) => {
                println!("skipped: no audio device ({e})");
                return;
            }
        };
        audio.set_master_volume(0.0001);
        let copy = crate::test_temp::file("sound_view_mixer", "wav");
        std::fs::copy(fixture(), &copy).unwrap();
        let path = copy.to_path_buf();
        let ctx = egui::Context::default();
        let mut view = SoundView::default();
        view.muted = true;
        view.start(move || Ok(path));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while view.analysis.is_none() || view.pending.is_none() {
            view.poll(&ctx);
            assert!(std::time::Instant::now() < deadline, "the sound arrives: {}", view.problem);
            std::thread::sleep(Duration::from_millis(5));
        }
        view.seek_to(1.0);
        view.sync_sound(&mut audio);
        assert!(view.handle.is_some(), "the mixer has it: {}", view.problem);
        assert_eq!(view.volume_sent, Some(0.0), "muted: silence");
        std::thread::sleep(Duration::from_millis(150));
        assert!(view.position_s() >= 0.99, "it starts where it was sent: {}", view.position_s());
        view.muted = false;
        view.sync_sound(&mut audio);
        assert_eq!(view.volume_sent, Some(0.0001), "unmuted: the master volume");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while view.handle.is_some() {
            view.poll(&ctx);
            assert!(std::time::Instant::now() < deadline, "it stops at its end (at {})", view.position_s());
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!((view.position_s() - 2.0).abs() < 0.05, "and stands at its end: {}", view.position_s());
    }

    /// A PRESS ON THE WAVEFORM GOES THERE (2026-10-19): with a two-second sound standing at its
    /// start, a press a quarter of the way along the waveform puts it half a second in, and the time
    /// under it says so.
    ///
    /// Seen red 2026-10-19 with the press's place measured across the whole area, its padding
    /// included, rather than across the waveform (`seek_fraction(rect, ...)` in waveform.rs
    /// `show`): "half a second in: 0.5099999904632568".
    #[test]
    fn a_press_on_the_waveform_goes_there() {
        let a = analyse(&fixture()).expect("the fixture reads");
        let mut view = SoundView::with_analysis(a, 0.0);
        view.set_temp_for_test(std::env::temp_dir().join("HumanityOS-not-a-real-copy.wav"));
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        let frame = |events: Vec<egui::Event>, view: &mut SoundView| {
            let mut wave = egui::Rect::NOTHING;
            ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0))), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let top = ui.cursor().min;
                    view.draw(ui, &theme, egui::vec2(600.0, 300.0));
                    // The waveform: the area less its padding, under the spectrum (waveform.rs `show`).
                    let pad = theme.spacing_sm;
                    let area_h = 300.0 - (theme.button_height + theme.spacing_sm * 2.0 + theme.font_size_small);
                    let inner = egui::Rect::from_min_size(top, egui::vec2(600.0, area_h)).shrink(pad);
                    let spectrum_h = (inner.height() * 0.4).floor();
                    wave = egui::Rect::from_min_max(egui::pos2(inner.left(), inner.top() + spectrum_h + pad), inner.max);
                });
            });
            wave
        };
        let r = frame(Vec::new(), &mut view);
        let at = egui::pos2(r.left() + r.width() * 0.25, r.center().y);
        let m = egui::Modifiers::default();
        frame(vec![egui::Event::PointerMoved(at)], &mut view);
        frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], &mut view);
        frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], &mut view);
        assert!((view.position_s() - 0.5).abs() < 0.002, "half a second in: {}", view.position_s());
        let out = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0))), ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.draw(ui, &theme, egui::vec2(600.0, 300.0));
            });
        });
        assert!(crate::gui::screen_surface::find_text_in_shapes(&out.shapes, "0:00 of 0:02").is_some(), "the time under it");
        view.temp = None;
    }
}
