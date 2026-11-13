# The media player core (in-world screens ladder, rung 5)

Written 2026-09-16 with the code of `src/media/`. This is the design of record
for video playback inside HumanityOS; update it when the player changes.

## The question and the answer

The operator asked: "could we embed VLC player or would it be better to make
our own purpose built for our app?" The answer we are executing is
purpose-built, and this document is the reasoning, the licence audit, the
measurements and the honest list of what is not done.

**What rung 5 delivers:** open a WebM/Matroska file, decode its AV1 video to
RGBA frames and its Opus audio to PCM, keep a playback clock, offer play,
pause, seek-to-start and "give me the frame that is due", with decoding on a
background thread that is joined on drop. **The display** is the in-world
screen surface, a separate rung; the integration that connects the two (a
`video:<path>` screen playing a clip on loop with its sound at the screen and
click-to-pause) is the "Video sources" section of
[in-world-screens.md](in-world-screens.md), and its provider is
`src/engine/screens/video.rs`.

## Why not embed VLC

libvlc would give us every codec on earth on day one. It was rejected for
four reasons that stack:

1. **Licence.** libvlc is LGPL-2.1. Dynamic linking keeps our own code free of
   the copyleft, but the LGPL obligations (shipping the library's licence,
   letting a user swap the library, no static linking) travel with every
   download and every platform build.
2. **It is a C runtime.** Dozens of DLLs (the core, plus a plugins directory
   of codec, demux, output and filter modules) shipped next to the exe and
   loaded at run time. This project has refused C build and runtime
   dependencies twice already (str0m with `rust-crypto` instead of aws-lc-sys,
   `unsafe-libopus` instead of libopus-sys), and a player is not the place to
   start.
3. **It owns the window.** libvlc's natural mode is "give me an HWND and I
   will draw into it". Getting raw frames instead means the callback-based
   `libvlc_video_set_callbacks` path with pixel format negotiation, which is
   the part of libvlc that is least like a library and most like a fight. We
   need decoded pixels in OUR wgpu texture on an in-game monitor, with the
   3D scene's lighting on top.
4. **It decides the codec policy.** VLC decodes H.264 and AAC without asking.
   Shipping that inside a free download puts us in the patent-pool territory
   the next section is about.

A purpose-built core is two decoders, one demuxer and a clock, which is a
few hundred lines around libraries that already exist. That is the shorter
road to frames in a texture, and it is the road we can keep.

## Container and codecs: what we decode and why

| Layer | Choice | Crate (exact version) | Licence (file opened and read) | Patent status |
|---|---|---|---|---|
| Container | WebM / Matroska | `matroska-demuxer 0.8.1` | Zlib OR MIT OR Apache-2.0 (`LICENSE-ZLIB`, `LICENSE-MIT`, `LICENSE-APACHE` in the crate) | Not applicable (a file format; Matroska is an open specification) |
| Container | MP4 (ISO/IEC 14496-12 and -14), for AV1 inside it (2026-11-03, BUG-228; `src/media/mp4.rs`) | `mp4-atom 0.17.0`, default features (none); it brings `derive_more 2.1.1` (MIT) and `num 0.4.3` (MIT OR Apache-2.0) | MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE` in the crate) | No pool licenses the container; the base-format patents once in the MPEG-4 Systems pool expired in 2018; one open point (ISO's 2026 declarations unread): `docs/reference/findings/2026-11-03-mp4-container.md` |
| Video | AV1 | `rav1d 1.1.0`, `default-features = false`, features `bitdepth_8`, `bitdepth_16` | BSD-2-Clause (`COPYING` in the crate: VideoLAN and dav1d authors, ISRG) | **Royalty-free.** The Alliance for Open Media publishes a royalty-free patent licence for AV1 implementations and content |
| Audio | Opus | `unsafe-libopus 0.2.0` (already in the tree for voice chat) | BSD-3-Clause (`LICENSE` in the crate: Xiph.Org, Skype, Octasic and others) | **Royalty-free.** The Opus licence file itself points at the Xiph, Microsoft and Broadcom royalty-free patent statements filed with the IETF |
| Playback | kira 0.12 `StreamingSoundData::from_decoder` (0.9.6 until 2026-11-06) | already a dependency | MIT | Not applicable |
| One errno | `libc 0.2` | for `EAGAIN` only | MIT OR Apache-2.0 | Not applicable |

Every licence above was verified by opening the file in
`~/.cargo/registry/src/<index>/<crate>/` after `cargo fetch`, not from the
crates.io summary. The one crate whose tarball ships no licence text is
symphonia (kira's file decoder, not used by the player); its manifests say
`MPL-2.0` and nothing else, which corrected a wrong "MPL 2.0 / Apache 2.0"
line in `docs/history/audio-engine.md`.

### What is excluded and why

- **H.264, H.265, AAC: excluded even where a permissive decoder exists.**
  An MIT-licensed H.264 decoder does not change who holds the patents. The
  H.264 and H.265 pools (Via LA, Access Advance) and the AAC pool (Via LA)
  license per unit shipped; a free download with no revenue cannot carry a
  per-unit fee and cannot promise the pool's terms to every mirror and fork.
  AV1 and Opus have royalty-free patent grants from their authors, which is
  the whole reason the web moved to them. This is the same rule
  `docs/design/streaming.md` applies on the encoding side.
- **VP8, VP9, Vorbis: not decoded in this rung.** They are royalty-free, but
  the only VP8/VP9 decoders on crates.io wrap libvpx (a C build) and the
  pure-Rust Vorbis decoders exist only inside symphonia's audio path. The
  task allowed them "only if a pure-Rust decoder exists"; for video none
  does, so a WebM that carries them is refused by name (`V_VP8`, `V_VP9`,
  `A_VORBIS`) rather than half-supported.
- **AV1 in an MP4 is ours too (2026-11-03, BUG-228).** Only the box structure
  was missing: `src/media/mp4.rs` reads it and hands AV1 to rav1d and Opus to
  our decoder; AAC or any other sound in such a file goes to the computer's own
  decoder (`system::probe_sound`), as an ordinary MP4's does. When nothing plays
  a file, `media::probe` says what it is in plain names (`media::kind`), never a
  reader's own message.
- **Everything else** (an MP4 the computer's own decoders cannot play, MKV with H.264, MOV, AVI,
  a disc's MPEG-2) is decoded **by the machine's own ffmpeg as it plays** (2026-11-13,
  `src/media/stream.rs`, "Through ffmpeg as it plays" below): ffmpeg hands the player raw
  pictures and raw sound through two pipes, the same way Windows's own decoders hand over an
  ordinary MP4's. The exe still carries one decoder pair and no patented decoder: the H.264 or
  MPEG-2 decoding happens in the person's ffmpeg, as it did when the file was converted ahead.
  (From 2026-09-18 to 2026-11-13 such a file was converted once to WebM AV1 + Opus first and the
  copy kept in a media cache; that broke the no-caches rule and made a film wait for its whole
  conversion.)

### Video discs: what plays and what does not (2026-09-18)

The operator asked whether a DVD in the PC's drive can play on an in-game
display. The answer has two halves, and the line between them is not
negotiable.

**A disc we can read as files, we play.** A disc the operator burned
themselves, a camcorder disc, a data disc with video files on it, an
unprotected video disc: the `.VOB` files in its `VIDEO_TS` folder are
ordinary MPEG-2 program streams. MPEG-2 video and AC-3 audio are not in the
excluded list above and do not need to be: their patents expired around 2017
and 2018, and in any case nothing new ships in our exe for them, because the
machine's own ffmpeg reads the stream and decodes it as it plays, the road
every file the player does not decode itself takes. A disc is simply another
thing ffmpeg can read.

**A copy-protected commercial disc, we do not touch.** Those discs are
encrypted, and HumanityOS does not break disc protection. There is no
circumvention in this codebase, no optional component that would add one,
and no instruction anywhere that points at one. When a disc turns out to be
protected the screen says so plainly, in one sentence that names what the
app does rather than what someone else could do, and stops.

The code is `src/media/dvd.rs`, and the whole of it is: find the `VIDEO_TS`
folder, choose the main title (the biggest title set, its `_0` menu
excluded, its parts in order), look at the first part's packet headers, and
either hand the chain of parts to ffmpeg as one film or show the honest
message. See "Video sources" in
[in-world-screens.md](in-world-screens.md) for what the player sees.

### AV1 decoder candidates, for the record

| Candidate | Verdict |
|---|---|
| **rav1d 1.1.0** (memorysafety.org's port of dav1d) | **Chosen.** BSD-2, builds on stable Rust 1.79+. Default features enable hand-written assembly that needs the `nasm` binary on x86; turned off. |
| dav1d-rs / dav1d-sys | Binds the C library, which must be installed on the player's machine. We cannot ship that or ask players for it. Rejected. |
| rav1d-safe 0.6.0 (imazen's SIMD fork) | Assembly replaced by safe SIMD intrinsics, so no nasm; but licensed **AGPL-3.0 OR commercial**. Rejected on licence. |
| libaom / libgav1 bindings | C and C++ builds. Rejected. |

### How "no C toolchain, no nasm" was proven, not assumed

rav1d lists `cc` and `nasm-rs` as **non-optional build-dependencies**, which
looks alarming. Reading its `build.rs` settles it: `main()` contains one
statement, `#[cfg(feature = "asm")] { asm::main(); }`. With `asm` off the
two crates are compiled as inert pure-Rust libraries and never called, so no
C compiler and no nasm are invoked. Proof on the dev machine, 2026-09-16:
`where nasm` finds nothing, and a scratch crate depending on
`rav1d = { version = "=1.1.0", default-features = false, features = ["bitdepth_8", "bitdepth_16"] }`
passes `cargo check` in 11 seconds. The relay build (`--features relay
--no-default-features`) never sees any of this: `src/media` is native-gated.

One consequence worth knowing: an unoptimised rav1d is roughly a thousand
times slower than its release build (a 320x180 clip decoded at a few frames
per second in a debug build). `Cargo.toml` therefore carries
`[profile.dev.package.rav1d] opt-level = 3`, so debug builds of the game and
`cargo test` decode in real time; the rest of a dev build stays unoptimised.

## Architecture

### Files and the public surface

```
src/media/mod.rs    MediaError, MediaInfo, probe(), VideoFrame, VideoPlayer,
                    the clock, the bounded frame queue, the decode thread
src/media/video.rs  Av1Decoder (the only place rav1d's C-shaped API is called),
                    YUV to RGBA conversion, decode_video() for tests and bench
src/media/audio.rs  OpusHead parsing, OpusTrack: a kira streaming Decoder
src/media/mp4.rs    the MP4 container, for AV1 in an MP4
src/media/system.rs the computer's own decoders (Windows Media Foundation)
src/media/ffmpeg.rs finding the person's ffmpeg; begin_ingest(): which road a file takes
src/media/stream.rs a film through ffmpeg as it plays: the look, the picture pipe
                    (YUV4MPEG2), PipeAudio (the sound pipe, a kira streaming Decoder)
src/media/dvd.rs    a video disc's main title, handed to ffmpeg as one film
crates/humanity-core/src/audio/mod.rs    AudioManager::play_stream(): hands a streaming sound to kira
```

```rust
let mut player = VideoPlayer::open("clip.webm")?;   // probes; refuses unsupported codecs by name
player.attach_audio(&mut audio_manager)?;          // optional: Opus through kira's mixer, one-shot, at master volume, centred
player.attach_audio_with(&mut audio_manager, AudioAttach { looping: true, volume, panning })?;
                                                   // the same with the choices spelled out: looping on kira's side (for a
                                                   // clip that repeats) and the ABSOLUTE mix the first sample plays at
player.set_audio_mix(volume, panning, tween_ms);   // move the sound: absolute amplitude + stereo pan 0..1, over a tween
player.audio_position_s(); player.audio_state();   // kira's own reading of the sound (not the clock): wraps on a loop, Stopped after a one-shot's end
player.play();                                     // also .pause(), .seek_to_start()
if let Some(frame) = player.poll() {               // newest frame with pts <= clock, or None
    upload(frame.rgba, frame.width, frame.height); // the surface rung's job
}
player.duration_s(); player.position_s(); player.is_playing(); player.at_end();
// drop(player) stops and JOINS the decode thread and stops the sound
```

### Threads

```
decode thread ("media-decode", ours)         caller thread (the surface, later)
------------------------------------         ----------------------------------
open demuxer + Av1Decoder                    poll() every drawn frame:
loop: next video packet                        clock = audio position if playing
      rav1d decode -> pictures in                    else wall clock
        display order, each tagged with        take the NEWEST queued frame whose
        the packet's pts                         pts <= clock, drop older ones,
      YUV -> RGBA                                never return one ahead of it
      push (waits while 4 frames are queued)   notify the decoder that space opened
at EOF: drain, mark eof, idle until a
        seek (restart from the top) or a stop

kira decode thread (kira's own)              kira audio thread
------------------------------               -----------------
OpusTrack::decode(): next Opus packet        mixes, advances the handle's
  -> unsafe-libopus -> kira Frames             position() in seconds
OpusTrack::seek(): reopen, skip packets
```

- **Bounded queue, `QUEUE_CAPACITY = 4`.** Four frames is 133 ms at 30 fps
  and about 33 MB at 1080p RGBA. The decoder blocks on a condition variable
  when it is full, so a paused player costs no CPU after the first four
  frames.
- **pts survives reordering** because rav1d copies each input packet's
  timestamp (`Dav1dData.m.timestamp`) onto the picture that packet produces,
  and it emits pictures in display order. The queue is therefore in pts
  order by construction and `take_due_frame` only has to look at the front.
- **Seek-to-start** bumps a generation counter, clears the queue and wakes
  the decoder, which abandons its pass and reopens the file. Frames from the
  old generation can never enter the queue (the push checks the generation
  under the same lock the seek uses).
- **Drop joins.** `VideoPlayer::drop` sets the stop flag, notifies, and
  `join()`s the thread; every wait in the decoder has a 20 to 50 ms timeout
  so a stop is seen promptly even if a notify raced. A dropped player waits
  at most one in-flight picture.

### The clock

Monotonic and audio-led:

- `Clock` holds a base position and the instant it was taken; while playing
  the wall time since then is added. It never hands out a value smaller than
  the last one it handed out (a `reported` high-water mark), so a re-base to
  a marginally earlier audio position holds the clock still for a few
  milliseconds instead of stepping it back.
- With a kira sound attached and in the `Playing` state, `poll()` re-bases
  the clock on `handle.position()` every call. The picture follows the sound
  card; an ear notices a 20 ms audio hiccup, an eye forgives a repeated frame.
- A reading more than `AUDIO_TRUST_WINDOW_S` (0.5 s) away from the wall
  estimate is ignored: kira applies seek and pause commands asynchronously,
  so for a moment after `seek_to_start()` its position still reads the old
  place in the file. The wall clock carries on until the two agree.
- Without audio (silent file, or `attach_audio` never called, as in the
  tests) the wall clock alone drives playback.
- At the container's declared end, once the decoder has delivered its last
  frame, the clock stops and `is_playing()` turns false; `play()` after that
  is a no-op until `seek_to_start()`.

### Audio through kira

kira (0.9 then, 0.12 since 2026-11-06, the trait unchanged) exposes exactly the hook needed: `sound::streaming::Decoder`, a pull
trait its own decode thread drives (`sample_rate`, `num_frames`, `decode()`
for the next chunk, `seek(index)`). `OpusTrack` implements it over its own
file handle, so the video thread and kira's thread never contend for the
file. No queued-chunk workaround was needed, so `take_audio` does not exist.
Two details that matter:

- kira's scheduler calls `decode()` in a loop until the chunk containing the
  frame it wants arrives; a decoder that returned an EMPTY chunk at end of
  stream would spin it forever. `OpusTrack` returns silence past the end,
  and `num_frames` is derived from the container duration minus the Opus
  pre-skip, which errs short, so kira stops before that path is reached.
- `seek(index)` reopens the file with a fresh decoder state and decodes,
  discarding whole packets up to (never past) the target; a packet that
  straddles it is kept for the next `decode()`. Linear from the start, which
  is fine for seek-to-start and correct for anything else.

Opus facts the code relies on: output is always 48 kHz; the first `pre_skip`
samples (312 for libopus, read from the OpusHead in CodecPrivate) are encoder
look-ahead and are dropped; a packet is at most 120 ms (5760 samples per
channel); more than two channels means the multistream layout, which needs a
different decoder and is refused at `probe()` with a message saying so.

The attach takes an `AudioAttach`: `looping` (a loop region over the whole
stream, for a clip that repeats; a kira stream that runs off its end leaves
the mixer for good and can no longer be resumed or seeked), and the
ABSOLUTE `volume` and `panning` the sound starts at. Absolute means the
final amplitude, master and bus already multiplied in by the caller (hence
`AudioManager::master_volume()` / `sfx_volume()` are readable), and it is
the same convention `set_audio_mix` uses for later updates, so the attach
and the updates agree and the first sample plays at the placed level. That
was a real defect: a stream attached at master volume and then tweened to
master x sfx x falloff played its first 60 ms up to 33 times too loud with
the sfx slider at 10 percent. `AudioManager::play_stream` therefore takes
the amplitude and pan as given and does NOT multiply master in, unlike the
one-shot paths. The plain `attach_audio` is the one-shot default: master
volume at the moment of the call, centred, no loop. `audio_position_s()`
and `audio_state()` expose kira's own reading of the sound (its position,
which wraps on a loop and is not the monotonic clock, and its
`PlaybackState`), read-only, for status and for the device tests.

**Volume and loudness (2026-11-06): `docs/design/loudness.md`.** A video's own volume starts at
Settings > Audio's choice ("My last volume", kept across restarts, or "A set volume"), the same for a
chat video, the Media page and a screen. Levelling a video's loudness as it plays (a wrapper around
this `Decoder` on kira's decode thread, Short-term loudness driving a slow gain, a true-peak limiter)
is that design's rung 2; rung 1 levels the shipped sound files.

### Colour

rav1d hands out planar YUV (I420 in every file ffmpeg produces by default;
I422, I444 and I400 are handled too). `video.rs` converts with fixed-point
integer math using the matrix the sequence header names (BT.601, BT.709,
BT.2020 non-constant-luminance, or AV1's identity for RGB-coded 4:4:4), the
declared range (limited or full), and the usual convention for "unknown":
BT.709 for 720 rows and up, BT.601 below. 10- and 12-bit pictures are shifted
down to 8 bits; HDR tone mapping is not this rung's job. Chroma is sampled
nearest-neighbour, which is what the conversion will do when it moves into a
shader on the surface rung (the CPU conversion is the placeholder-free rung of
the real architecture: the same math, later on the GPU).

## Test fixtures: provenance

`ffmpeg -version` on the dev machine reports the gyan.dev 2025-01-22 full
build with libaom, libopus, libvpx and libvorbis, so the fixtures were made
with it. `scripts/make-media-fixtures.sh` regenerates everything under
`tests/fixtures/media/` from SYNTHETIC lavfi sources; nothing was recorded
and nothing downloaded, so the files carry no third-party rights:

- `colour-bar-av1-opus.webm` (19,629 bytes): 2 s, 320x180, 30 fps, AV1
  (libaom, `-cpu-used 6 -crf 32 -g 30`) plus stereo Opus at 48 kbit/s. A
  40 px red bar slides right at 140 px/s over a dark grey (0x202020) field;
  the audio is a 440 Hz sine. The bar is an `overlay` of a second source,
  not a `drawbox`, because drawbox evaluates its position once at start and
  the first attempt produced a bar that never moved.
- `unsupported-vp8-vorbis.webm` (4,692 bytes): 0.2 s of 64x36 VP8 plus mono
  Vorbis, so the refusal path has a real file to refuse.
- `moving-box-h264-aac.mp4` (22,798 bytes): 2 s of 320x180 H.264 plus AAC,
  the format a phone writes, so the ffmpeg road (and, on Windows, the
  computer's own decoders) has a real file to play.
- `VIDEO_TS/` (69,632 bytes for the tree): a whole unencrypted video disc,
  so the disc path is tested with no disc in the drive.
  `VTS_01_1.VOB` and `VTS_01_2.VOB` are ONE 2 s MPEG-2 program stream
  (300 kbit/s video, AC-3 audio, a cyan square sliding and bobbing) cut in
  half at a 2048 byte pack boundary, which is exactly what a disc author
  does at the VOB size limit: the timestamps run on across the cut, so the
  two parts really do play as one film and a test that only got the
  first part fails on the duration. `VTS_01_0.VOB` is the title set's menu
  (a still green field) and `VTS_02_1.VOB` a smaller second title set, so
  "ignore the menu, pick the biggest set" has something to be right about.
  The protected-disc case needs no fixture of its own: the test marks a
  copy of the first part's packet headers as scrambled, which is how an
  encrypted disc declares itself, and checks the refusal.

An observation, not a defect: ffmpeg and ffprobe print
`Error parsing Opus packet header` once when they read their own Opus WebM
output. unsafe-libopus decodes all 101 packets of the fixture without error
and the sample count is right, so the message is ffmpeg's own probing quirk.

## Tests and what each one can catch

All in `src/media/tests.rs`, run with `cargo test --features native --lib media::`.
Each asserts on the decoded RESULT; none is satisfied by the setup it wrote.

| Test | Proves |
|---|---|
| `probe_reports_tracks_codecs_and_duration` | 2 tracks, `V_AV1` + `A_OPUS`, 320x180, 48 kHz stereo, pre-skip 312, duration 2.008 s |
| `refuses_unsupported_codec_naming_it` | the VP8 file is refused with `UnsupportedCodec { kind: "video", codec_id: "V_VP8" }` and a message naming both the codec and the fix |
| `decodes_every_frame_in_order_at_the_right_size_with_the_bar_moving` | 60 frames of 320x180x4 bytes, strictly increasing pts from 0 to 1.967 s, and on every frame the red bar's left edge is where `140 * pts` puts it (tolerance 3 px), the field is neutral dark grey and the bar pixels are red: a garbage frame, a swapped frame, or a wrong matrix or range fails here |
| `opus_sample_count_matches_duration_within_5_percent` | 193,296 interleaved samples against 2.008 s x 48000 x 2 (ratio 1.003), an audible tone (RMS), left equals right |
| `opus_track_feeds_kira_in_order_and_rewinds_sample_exactly` | the kira `Decoder` contract: first chunk is 960 minus pre-skip, 100+ chunks to `num_frames`, `seek(0)` reaches 0 and the next chunk equals the first sample for sample, `seek(10000)` lands packet-aligned at 9288 and the straddling packet is not lost |
| `take_due_frame_never_hands_out_a_frame_ahead_of_the_clock` | the pure selection rule on synthetic frames: newest at-or-before wins, older due frames drop, nothing ahead of the clock leaves |
| `live_playback_delivers_frames_in_order_and_never_ahead_of_the_clock` | with the real thread and wall clock: paused at 0 only frame 0 is ever due; playing, every polled frame has pts <= the clock read after the poll and pts strictly increases; on every poll the clock never reads ahead of the wall and lags it by under 0.3 s (the real-time proof, on the clock, load-invariant); the run lasts until the clock stops at the declared end (deadline 15 s), which the player only does once the last frame was delivered, so the last frame arrives and the end cannot come before 2.008 s of wall; at least half the 59 frames were delivered (`real_time_floor`: `poll` drops older due frames by design, so a parked test thread loses frames legitimately, while the defects the count is for deliver a handful; a fixed 40 failed under load at 37, and a fixed 2.6 s window failed under load with the last frame at 1.233 s when a debug-build decoder sharing the machine with the suite ran at about 15 fps) |
| `seek_to_start_rewinds_and_replays` | after passing 0.4 s, `seek_to_start()` puts the clock near 0 and the next delivered frame is from the start |
| `drop_joins_the_decode_thread` | a liveness flag shared with the thread's stack guard is true while the player lives and false the instant `drop()` returns |
| `audio_led_clock_follows_kira_when_a_device_exists` (ignored: needs an audio device, run by hand) | the kira hookup end to end through the ONE-SHOT attach: attached while paused the clock holds at 0; playing muted, the AUDIO-LED clock reads 1.494 s after 1.5 s of wall time with 44 frames delivered and none ahead of it; pause holds; a rewind restarts both clock and frames. Observed on the dev machine 2026-09-16 |
| `looping_audio_keeps_playing_past_the_end_when_a_device_exists` (ignored: needs an audio device, run by hand) | the LOOPING attach (`AudioAttach { looping: true }`, the `loop_region` branch every video screen uses; nothing executed it before this test): attached paused, kira reads `Paused`; phase A lets the stream run off its end on its own and kira's own position wraps (drops back toward 0) with the sound still `Playing`; phase B drives the provider's loop rule (rewind at the declared end) through a second pass, frames keep flowing after the wrap and the sound is still `Playing`, no error; phase C is the control: the same clip attached WITHOUT looping reads a state other than `Playing` after its end. Observed 2026-09-17: phase A 51 frames, 1 audio wrap, max kira position 2.000 s of 2.008; phase B 46 frames, 2 picture loops, 46 after the wrap; 4.82 s total; control one-shot `Stopped` |
| `bench_decode_fps` (ignored) | the measurement below |

Negative proofs (a gate that cannot fail is not a gate): see the section
"Proven to fail" at the end of this document for the two deliberate breaks
that were made and observed before the code was restored.

## Measurements

Measured, not guessed. Method: `cargo test --release --features native --lib
media::tests::bench -- --ignored --nocapture` runs `decode_video()` on the
calling thread, which is the whole pipeline for every frame (demux, AV1
decode, YUV to RGBA conversion) plus decoder open, and reports frames per
elapsed second. Run once per row on 2026-09-16 on the dev machine (Intel
i7-8700K, 6 cores / 12 logical processors, 32 GB, Windows 10, release
profile). The 1080p clip is a 5 s synthetic `testsrc2` pattern encoded by
SVT-AV1 (preset 10, CRF 35, a keyframe every 60 frames, 4.65 MB); it lives in
the session scratchpad and is not committed. `HUMANITY_MEDIA_BENCH_FILE`
points the benchmark at any other AV1 WebM.

| Clip | rav1d threads | Frames | Wall time | fps | Mpx/s |
|---|---|---|---|---|---|
| 320x180 fixture | 1 | 60 | 0.052 s | 1154 | 66.5 |
| 320x180 fixture | auto (12) | 60 | 0.033 s | 1802 | 103.8 |
| 1920x1080 | 1 | 150 | 5.283 s | **28.4** | 58.9 |
| 1920x1080 | auto (12) | 150 | 2.168 s | **69.2** | 143.5 |

What the numbers say:

- **Pure-Rust AV1 reaches 30 fps at 1080p on this machine only with rav1d's
  threads on**: 69 fps with them (a 2.3x margin over 30, above 60), 28.4 fps
  on one thread, just under real time. Threads are on by default
  (`n_threads = 0` means one per logical processor).
- **Extrapolating from the small fixture would have overstated it.** The
  fixture's 66.5 Mpx/s single-thread rate, scaled by the 36x pixel count of
  1080p, predicts 32 fps; the direct measurement is 28.4. The doc reports
  the measurement.
- **The CPU colour conversion is now a visible share.** A decode-only
  prototype of the same loop measured 85 fps on the 1080p clip with threads;
  the full pipeline measures 69, and the difference is the serial RGBA pass
  on the calling thread (8.3 MB written per frame). Moving that to a shader
  on the surface rung is the next lever, and it is already planned there.
- **Caveat on content.** A synthetic pattern is easier than real footage
  (flat regions, no film grain, moderate motion). Expect real 1080p content
  to decode slower than these rows, and treat 4K as out of reach without
  assembly.

Mitigations, in the order they would be reached for: (1) rav1d's frame and
tile threads, on by default; (2) YUV to RGBA in a shader, removing the serial
CPU pass; (3) for a machine that cannot keep up, handing an AV1 file to its
ffmpeg as well (its dav1d has the assembly; until 2026-11-13 every converted
file was capped at 720p for this reason, 2.25x fewer pixels, and the cap went
with the conversion); (4) an assembly build of rav1d later. On that last one: nasm is a BUILD-time
tool on the CI machine, never something a player installs, so
`features = ["asm"]` in the release workflow stays an honest option if real
content demands it; dav1d's assembly is typically 2 to 4x faster than the
plain build. It is not turned on now because it is not needed for 1080p and
the no-toolchain build is worth keeping simple.

### Through ffmpeg as it plays (2026-11-13)

Measured on the dev machine (i7-8700K, 12 logical processors, ffmpeg 2025-01-22
gyan.dev full build), with other agents compiling, so read the numbers as an
upper bound. The sources: one minute of a real film made into a 1280x692
H.264 + AAC MKV (a "short local clip") and into an NTSC video-disc title
(720x480 MPEG-2 + AC-3, 16:9) cut into two VOB parts at a pack boundary; the
2 s fixture; and the 20 s 1280x720 MPEG-4 Part 2 + AC-3 clip the screen test
makes for itself. BEFORE is the whole-file conversion that had to finish
before the first picture (the pinned SVT-AV1 preset 8 command, ffmpeg run at
below-normal priority under the operator's share rules); AFTER is the time from
the open to the first picture on the ffmpeg road (`bench_first_picture`, and
the screen test's own timing).

| Source | BEFORE: first picture after | AFTER: first picture after | AFTER: seek to the middle |
|---|---|---|---|
| 1 min 1280x692 H.264 + AAC MKV | 16.4 s (3.7x real time; a two-hour film about 32 min) | 0.21 s (the look 0.06 s) | 0.26 s |
| 1 min disc title, 2 VOB parts | 5.9 s (10x real time; a two-hour title about 12 min) | 0.15 s (the look 0.05 s) | 0.14 s |
| the 2 s 320x180 fixture | 0.3 s | 0.11 s | |
| 20 s 1280x720 MPEG-4 (the screen test) | 5.97 s (the test's red run) | 0.25 s (bound in the test: 2 s) | |

And on disk: BEFORE kept every converted film (the dev machine's
`media/cache` held one two-hour film, 595 MB); AFTER writes nothing. Decoding
alone is far faster than real time, and so is the pipe: ffmpeg wrote raw pictures into a
reading process at about 1.5 GB a second, 1,121 pictures a second at 1280x692, 2,204 at the
disc's 854x480 and 506 at 1920x1038 (a script counting the bytes ffmpeg wrote with the picture arguments). What sets the pace
while a film plays is the colour conversion on the decode thread, the same one the AV1 path
uses (about 2.7 ms a 1080p picture in the release build, "Measurements" above). A debug test
build runs that converter unoptimised and shows only about 13 pictures a second at 1280x692
(`bench_first_picture`), which says nothing about the release exe.

### A video playing in a chat row (2026-11-06)

Method: `chat::inline_video::tests::bench_one_playing_video_frame_cost`, run in release
(`cargo test --release --features native --lib chat::inline_video::tests::bench -- --ignored
--nocapture`, `HUMANITY_CHAT_VIDEO_BENCH_FILE` pointing at a 10 s 1920x1080 30 fps AV1 MP4 made
with SVT-AV1, preset 10, CRF 35, kept in the session scratchpad). It times the frame loop's own
work, the chat page with twenty video rows laid out at 2560x1440 plus egui's tessellation, over
240 frames paced at 120 Hz, with nothing playing and then with one row playing. Same machine as
above; other agents' builds were running, so the baselines spread between runs.

| | mean per frame | p95 |
|---|---|---|
| Twenty video rows, nothing playing (four runs) | 2.4 to 5.3 ms | 3.6 to 17.9 ms |
| One row playing 1080p, picture made unmultiplied (the code before) | 9.07 ms | 23.4 ms |
| One row playing 1080p, picture made premultiplied (three runs) | 3.8 to 4.0 ms | 8.0 to 8.8 ms |

The step that mattered is turning each decoded picture into an egui image: measured alone at
1920x1080, `ColorImage::from_rgba_unmultiplied` took 8.9 to 10.8 ms a picture and
`from_rgba_premultiplied` 2.6 to 2.9 ms. Every decoder here writes opaque pictures (alpha 255), for
which the two are the same bytes, so `VideoView::poll` now uses the premultiplied form, for the
Media page's player as well. So one playing 1080p video costs the frame loop about 2.7 ms on each
frame a new picture arrives (30 a second), about 1 ms averaged over 120 Hz frames, plus what this
headless run cannot see: the 8.3 MB upload of each picture to the GPU, and the decoder's own threads
(the table above). A row that is not playing costs what any row does: no fetch, no decoder, no
repaint (its bars are `track_bar`, which asks for none).

## Videos inside chat messages (2026-11-06)

The operator, 2026-11-04, with a screenshot of a `.mp4` card in #global: "Can we make the videos
show in chat and playable in chat?" Before this a posted video was a file card whose Play here
opened the video in a window of its own. Now the player sits inside the message, in both apps.

**The desktop app** (`src/gui/pages/chat/inline_video.rs`, drawn in the card by
`chat/public_files.rs`; the controls are `widgets/video_view.rs`, the Media page's player):

- The card of a video on the message's own server holds a picture box (16:9 until the video says
  its shape, at most 560 points wide), a seek bar, Play or Pause, the time, Mute, the volume and
  Full window. Full window is the old window, kept as the bigger view of the same player (nothing
  is fetched again); closing it puts the video back in its message.
- **Nothing costs anything until Play.** A row fetches nothing and decodes nothing when it is
  drawn. The first Play fetches the whole file (the player opens a file; streaming is not built,
  "Streaming from the relay" below) on the one road every video takes: `VideoView::start` hands
  the copy to `ffmpeg::begin_ingest_as`, which plays it, hands it to the person's ffmpeg to play
  as it goes, or names it in the plain words BUG-228 built. Those words are drawn in the row, under the
  player, with the card's Download beside the name.
- **The still frame is drawn after the first press, not before** (the brief asked which). A still
  before Play would need, for every video row on screen, a range read of the MP4's index box
  (phones and many tools write it at the END of the file, so a second read), then the first
  keyframe, then a picture decoded: a fetch and a decode for every video nobody pressed, which is
  exactly the cost this design refuses. So the box is black with a Play mark and "Press Play to
  fetch and watch it here." until the first Play; after that, the picture last shown stays as the
  row's still (made small, at most 480 pixels wide, when the player goes) with the place it
  reached, and Play carries on from there. The better still, a poster picture made once when the
  video is posted and served beside it, is the next rung, built the same day: "Rung 2: the poster
  picture" below. Before Play a row now shows that picture under its Play mark; a video posted
  before it (no poster picture) keeps the black box.
- **One video plays at a time.** The last video Play was pressed on is the only one allowed to
  play; every other is paused at the end of the frame (`ChatVideos::end_frame`), which also
  catches a video whose fetch finishes after another was started.
- **A row off the screen keeps nothing running.** A player whose row was not drawn inside the
  messages' view this frame or the last one (and is not in Full window) is dropped: its decoder,
  its decode thread, its sound and its temporary copy go, and only its still and its place stay.
  The chat is not a virtual list (every row is laid out every frame), so "on screen" is the row's
  picture box intersecting the scroll view's clip (`ui.is_rect_visible`). Switching page or
  channel drops them all the same way. Since rung 3 (below) the video PLAYING whose row leaves
  goes into the dock instead, the same player.
- **Nothing is kept across runs.** The temporary copies go with their players (and the next start
  sweeps any a crash left); the stills, the places and the words are memory only. The stills obey
  the Dev page's "turn off all reuse" switch: with reuse off none is kept, and a row goes back to
  its Play mark. At most 24 stills are kept; the oldest goes first.
- Below Verified, a poster's files are plain words (chat access rung 1); no player is drawn for
  them, as no card was before.
- **On a wall screen showing the chat** (an in-world screen with `page:chat`, the console room's
  `desk_monitor_2`), the screen is the bigger view, so no Full window is offered (BUG-258). The
  window is the app screen's, drawn by the engine for the app screen's set of videos only, and a
  person at a wall screen points with the look ray while the view holds the pointer, so a window
  opened over the app screen could not be closed from there; offered anyway, it stranded the row
  ("Playing in its own window."), never polled again, its temporary copy never deleted, the
  screen's reaction popups shut. A wall screen's videos play in their rows, and their sound comes
  from the screen (`engine::screens::place_page_sound`, every frame, framed or not): fading to
  silence at 50 m, panned by the screen's bearing, on the world-sound bus under the master volume
  like a film on an in-world screen, with the row's volume bar and Mute on top. They had played
  silent, their Mute and volume doing nothing.
- **Full window keeps Close on the screen.** Its body is held to the room a dialog has
  (`widgets::dialog::body_room`, BUG-251's rule) and laid out from the bottom up: Close the window
  and the controls (with any words under them) are measured first and the picture's box gets the
  height they leave, so Close sits inside the window at every window size and Font Size (BUG-258:
  the picture was 0.72 of the screen's height with everything under it, so on a screen under about
  620 points tall (1280x720 at Font Size 125%, 1920x1080 at 175%), with a video's words under its
  picture, Close fell off).

**The website** (`web/chat/app.js` `fileCardHtml`): the browser's own `<video controls>` inline in
the card, `preload="none"` on every video (it was `metadata` for this server's files, a read of
every video row as the chat loaded), so nothing is fetched until Play. Its poster picture is set
once the card is on screen (rung 2, below). A capture-phase `play` listener pauses the card that
played before, so one plays at a time, and a playing card scrolled out of view is paused (an
IntersectionObserver); since rung 3 a playing video goes into the dock instead (a sound is still
paused). A file the browser cannot play says so in its card, as since 2026-10-17.

### Rung 2: the poster picture (2026-11-06)

The first rung's own recommendation, which the orchestrator took: the app that posts a video makes
a small still of it once, sends it with the video, and the server keeps it beside the file. A row
then shows a picture of the video before Play at the cost of one small picture, and no row ever
reads the video itself before Play.

- **What it is.** A JPEG, its longest side at most 640 pixels, made at quality 80 and stepped down
  (65, 50, 35) until it is under 96 KB; a few tens of KB for an ordinary frame. JPEG and not WebP:
  both apps can make a JPEG everywhere (the `image` crate's WebP encoder is lossless only, and
  Safari's canvas does not make WebP), and one type keeps the address predictable.
- **Which frame.** About one second in (a first frame is often black or a fade), or the first frame
  of a video shorter than that. The desktop app takes it with its own player (`media::VideoPlayer`,
  which also opens an ordinary MP4 through Windows's own decoders): `chat/video_poster.rs`. The
  website takes it with the browser's own `<video>` and a canvas, seeked a second in:
  `web/shared/video-poster.js`. A video the app cannot decode gets no poster picture (no ffmpeg run
  for one); the video still goes, and its row keeps the black box.
- **How it travels.** In the same `POST /api/upload` as the video: a form part named `poster`,
  before the `file` part, so the relay holds the small picture before the large file starts. Only
  with a video (mp4 or webm); never with a file from a private conversation (an `encrypted=1`
  upload is ciphertext, and a plain picture of it would be readable by anyone with its address:
  the relay refuses one, and neither app makes one). The relay refuses a poster picture that is
  not a JPEG, that does not decode, that is over 640 pixels on a side or over 128 KB, or a second
  one, with words saying which. It is cleaned like every picture (`strip_image_metadata`), whatever
  `keep_metadata` says: the app made it, so it has nothing of the person's to keep.
- **Where it is kept.** As `<the video's stored name>.poster.jpg`, beside the video, in the same
  folder (`storage::files::poster_name`). A stored name never holds two dots (its stem has none),
  so no upload can ever be given a poster picture's name. Its size is the video's row's
  `user_uploads.poster_bytes` (added by ALTER, BUG-046's rule), and it counts where the video
  counts: the server's room (the folder's bytes), the day's allowance (the bytes that arrived) and,
  for a video on the Media page, its uploader's share.
- **Its address and who may fetch it.** `/uploads/<stored name>.poster.jpg`, the same service as
  the video, so exactly as public as the video and never more. A Media item made members only
  moves its file to the members' folder, and its poster picture moves with it
  (`Folders::move_to`): the public address answers 404 to everyone, as the video's does. The
  members' route serves an item's own files only, not poster pictures.
- **It goes when its video goes, on every road.** `Folders::delete` (an erased account, a removed
  Files item or file, a decided copyright report, a Media upload never published, a private file's
  lifetime), the chat files' keep-N limit (`delete_pruned`), a removed Shared File, and, new with
  this rung, a deleted message: deleting a message (its author, or a moderator) now deletes the
  chat files it posted (its author's own, on the chat shelf; never a Shared File or a Media
  upload), unless another message still links them. Before this a deleted message's video stayed
  at its address until the keep-N limit pushed it out. The six-hour pass (and the start) removes
  any poster picture whose video is no longer beside it, for a relay stopped between the two.
  Still open: a message that leaves by the retention sweep or an admin's channel clear leaves its
  video (and so its poster picture) until the keep-N limit (BUG-256).
- **Drawn before Play.** The desktop app: a video row on screen asks for its poster picture once,
  on a worker (at most 256 KB read, decoded there), and draws it in the picture box under the Play
  mark; nothing else is fetched, and no player is made. A 404 (a video from before this rung)
  leaves the black box. Kept in memory only, never on disk: at most 48 while reuse is on; with the
  Dev page's "turn off all reuse" switch, a poster picture is kept only while its row stays on
  screen and asked for again when it comes back. After a Play, the row's still (where it was left)
  takes the poster picture's place. The website: a video card on this server carries
  `data-poster`, and its `<video>` gets `poster="..."` when the card comes within a screen's
  height of the view (an IntersectionObserver; at once in a browser without one); `preload="none"`
  stays, so the video itself is still not read before Play.
- **Below Verified** a person's files are plain words (chat access rung 1): no card, so no poster
  picture is fetched for them.
- **Not built:** a poster picture for a file in a private conversation (it would have to be sealed
  with the file's own key and opened by the receiving app), and one for a Media item's own page
  (an item has its cover).

### Rung 3: the anchored dock (2026-11-06)

The operator, 2026-11-06: "Oh, for playing videos in chat. When we scroll them off page can we add
like an anchored picture-in-picture mode that's stuck to the side of the chat it's at? It could be
like a collapsible tap that has minimize (keeps playing unless paused) and close (closes anchored
section.)" On the orchestrator's three proposals for the edges (another video started, a channel
or server switched, the chat page left), his answer the same day: "Great ideas. Let's have it
follow around app. Like how we have the chat footer on nonchat pages." And on one video across the
app and the wall screens together: "Agreed."

**What changes.** Before this rung a PLAYING video whose row left the view was paused and its
player dropped (its still and place kept). Now it moves into the dock: the same player, nothing
fetched again, nothing restarted. A paused or never-played video whose row leaves keeps the old
behaviour (its player goes, its still and place stay). "Its row leaves" covers every way: scrolled
away, a channel or server switched (the rows go), or the chat page left.

**Where it sits.**

- **On the Chat page**, inside the message list, at its top right corner, just under the channel
  header (and any notice under the header), a small margin in from both edges. Right, because a
  row's name and words start at the left, so the right edge covers the least of them; top, because
  the message box and the newest lines are at the bottom, where the person reads and types, and a
  row that leaves as the person reads on leaves at the top. It is laid out inside the message
  list's own rectangle, so it can never cover the header or the message box.
- **On every other app page** it follows the person, in the app-wide layer of the bottom strip
  (gui/layers.rs layer 30, the chat footer's, app-bottom-strip.md), at the right edge standing on
  the strip (the warning, and the chat footer once built). It is placed when it arrives on a page
  and when the window changes size: from the bottom of the right edge upward, the lowest spot where
  it covers none of the page's own controls, read from egui's record of what the page drew the
  frame before (anything that takes a press or the keyboard, smaller than a quarter of the page;
  a bigger one, a map or a long list, is a surface a picture-in-picture may stand over). With no
  such spot it shows as the slim tab. Opened from the tab anyway, it stands at the bottom right
  over what is there, the person's choice, until they leave that page. It is not moved as the page
  scrolls under it.
- **Its layer.** An egui Area in layer 30, Background like the footer's panel: over the page,
  under the page's own windows and drop-downs, every popup and its shade, and tooltips. (By the
  operator's numbers 30 is nearer than 40, which holds for panels that never overlap; a page's
  window is Middle in egui and is drawn over the strip's layer, the warning's and the footer's
  included. A window the person opened is what they are working in.)
- **Not in the first-person world.** The dock is an app layer: entering the world (GuiPage::None)
  pauses the video and hides the dock. Its player goes, as a paused video's does (a person may stay
  in the world for hours), its still and place staying, and the dock comes back with them on the
  next page; Play carries on from the place, fetching it again. Not resumed by itself.
- **Never inside a wall screen.** A wall screen showing the chat (`ChatVideos::on_a_wall_screen`)
  plays its videos in their rows, or in the Full window drawn on that wall (the same day's change:
  "the whole display would be dedicated to the whole page"); its set never docks. The dock is the
  person's, not a screen's.

**How big.** The width is 40% of the room (the message list on the Chat page, the page between the
menu and the strip elsewhere), between 240 and 420 points and never wider than the room; the
picture keeps the video's shape (16:9 until it says its own); the parts round the picture are
measured, and the whole is held inside the room. When the picture would be under 72 points tall
there (a small window, Font Size 200% on 1280x720), the dock starts as the slim tab. Every size is
in points, so Font Size 100% to 200% scales it like the rest of the app. Theme tokens only.

**What it shows.** A line saying where the video came from ("From #global on united-humanity.us")
with Minimize and Close; the picture (pressed, it plays or pauses, as in a row); then Play or
Pause, the time, Mute, and Go to message. No seek bar or volume bar: the row has them, and Go to
message is one press away. Every control has its words and hint by key (`chat_video_dock.*`).

- **Minimize** folds it to a slim tab on the same edge ("Video playing" or "Video paused"). The
  video plays on, sound included, unless paused. Pressing the tab opens it again.
- **Close** stops the video (paused) and closes the dock; its player goes and the row keeps its
  place and its still.
- **Go to message** opens the Chat page at the video's channel, switching to its server first when
  it is a saved one and not the one open (the history's own way back to a channel, `At::Chat`),
  and scrolls its row into view, where the video returns inline. It waits up to 15 s for the row
  to be drawn (a switched channel loads its history first); a row older than the history loaded is
  not reached, and the dock stays.
- **Scrolling the row back into view** returns the video inline, the same player.

**One video at a time, everywhere.** Starting another video plays the new one in its row, pauses
the docked one and closes the dock (the docked one's player goes; its row keeps its still).
Since the operator's "Agreed." this holds across the app screen and every wall screen together, a
wall screen's own Full window included (it is that screen's set's player): the last set whose
Play was pressed holds the claim (a thread-local, since every set is drawn on
the app's one GUI thread), and any other set gives way: at its end of frame, at its wall screen's
sound step (every frame in the world, framed or not), and for every wall screen every frame while
a page covers the world (when none of them draws).

**Switching channels or servers** keeps the dock playing, labelled with where the video came from.

**What it costs.** A docked, playing video costs what a row's does (about 1 ms averaged at 120 Hz on
the frame loop, "A video playing in a chat row" below, plus its decoder threads and the picture
upload). Minimized, its picture is not drawn, so the frame loop takes each due picture and keeps
it for the still without making it an image or uploading it (the 2.6 to 2.9 ms per 1080p picture
measured below, skipped); when it opens again the last picture is made an image once. The decoder
cannot skip pictures: every AV1 picture (and H.264's or H.265's through Windows) is decoded from the
ones before it, and the player's queue holds four, so a player not polled stops its decoder and the
picture would fall behind the sound. A minimized video therefore keeps its decoder threads and
their colour conversion (1080p at 30 fps is about 43% of what the threaded decoder manages on the
dev machine, 69 fps in "Measurements") and its sound.

**The website** (`web/shared/video-dock.js`, wired in `web/chat/app.js`): the same dock in the
page's own HTML and CSS, at the top right of the message list under the channel header, in the
chat page's column; the browser's own controls on the video give Play or Pause, the time and Mute,
with Minimize, Close and Go to message beside them. The IntersectionObserver that paused a playing
video scrolled out of view now moves the `<video>` element itself into the dock and leaves a line
in its card saying where it is. Moving it does not restart it: HTML's removing steps for a media
element wait for a stable state and do nothing if the element is back in a document by then, and
inserting one whose network state is not empty runs no load, so an element moved in one task plays
on. Checked on 2026-11-06 in Chromium 152 (the desktop app's browser pane), with a page playing the
AV1 WebM fixture and doing the dock's moves: before the move the clock read 0.362 s and the video
played; right after it, 0.362 s and playing; 300 ms later 0.624 s, with no `loadstart`, `emptied`,
`pause` or `seeking` event since; hidden with `display: none` on the box (Minimize), 0.624 s to
0.794 s, still playing, no event; put back in its card in its line's place, 0.794 s to 1.069 s, no
event; then on to its end at 2.008 s. Firefox and Safari were not checked; they follow the same
HTML steps. A card for that video drawn again (the channel opened again) takes the element back
when it comes into view. On a phone-sized window (640 px wide or less), or a message list too small
for a picture (narrower than 240 px, or under 72 px of picture), the dock starts as the tab; the
picture gets the height the line above it and the buttons leave, at most 315 px.
Where the browser offers its own floating picture-in-picture (`document.pictureInPictureEnabled`:
Chromium and Safari; Firefox has its own button on the video and no such call), the dock carries one
more button for it, one line of script. The website's pages are separate documents, so the dock
lives on the chat page only (it follows channel switches there); following the person to another
page would need the site to stop reloading on every page, which is not built.

**What one playing video costs** is measured in "Measurements" below ("A video playing in a chat
row").

## Not done yet (deliberately, this rung)

- **Display integration: DONE** for in-world screens (`video:<path>` in
  `data/machines/home.ron`, provider `src/engine/screens/video.rs`; the
  frame is a GPU texture drawn by egui into the display's rectangle, the
  sound is placed at the screen, a click pauses, the clip loops).
- **Choosing a file: DONE (2026-09-18).** The operator, in the console room:
  "should we play a video that's stored on my PC?" The screen carries a
  control strip (Open, Play/Pause, the name, the time) that shows while the
  screen is looked at, paused or idle and hides over a playing film, and
  Open raises the in-app file picker on the screen itself, filtered to the
  video extensions in `data/media/ingest.json` and starting in the Videos
  folder. The choice is remembered PER SCREEN in `AppConfig::screen_media`
  (keyed by the placed instance id), so a data file's `video:` source is
  only the default and the remembered file wins on the next boot.
- **Through ffmpeg as it plays: DONE (2026-11-13).** A chosen file the
  player refuses is decoded by the machine's ffmpeg WHILE it plays: nothing
  is converted ahead, nothing is written to disk, the source is never
  touched. ffmpeg is looked for at the Settings > Media path, then `PATH`,
  then the per-platform list in `data/media/ingest.json`; a short look at
  the file (`ffmpeg -i`, on a worker) gives its picture size, length and
  sound, then one ffmpeg sends raw pictures (YUV4MPEG2, 8-bit 4:2:0, square
  pixels, at most 1080 rows, deinterlaced when the file is interlaced) and a
  second raw sound (32-bit float, 48 kHz stereo). A seek starts both again at
  the new place. No ffmpeg or an unreadable file is an on-screen message
  naming the fix, never a panic and never a silent black screen. Both
  command lines are pinned by `the_command_lines_are_pinned`. A crash leaves
  nothing behind: the pipes are the only link, and ffmpeg exits on its next
  write once they close (`a_crash_leaves_no_ffmpeg_running`).
  Implementation: `src/media/ffmpeg.rs` and `src/media/stream.rs`.
  It replaced **transcode on ingest** (2026-09-18 to 2026-11-13), which
  converted the whole file to WebM AV1 + Opus first and kept the copy in
  `<data dir>/media/cache`: it broke the no-caches rule (CLAUDE.md, operator
  2026-07-16, "Every cache is a chance to encounter a bug."), kept every film
  on disk (595 MB for one film on the dev machine) and showed nothing until
  the whole conversion was done (the numbers are in "Measurements", "Through
  ffmpeg as it plays"). An old cache folder is deleted once at start, with a
  log line (`ffmpeg::remove_old_cache_once`).
  Still to come on that side: synchronised playback between players (a
  one-shot Sync: load the same source, seek to the same moment, then play
  independently; `docs/design/in-world-screens.md`, "Screens are per
  viewer"), subtitles, and true 3D spatial audio once the
  engine has a kira spatial scene (today the stream's volume and pan are set
  per frame from distance and bearing).
- **Loop-point audio alignment.** A looping stream wraps at the Opus sample
  count while the clock wraps at the container duration, a few milliseconds
  later; `seek_to_start` re-seeks the sound to 0, replaying those
  milliseconds (under 20 ms on the fixture). Following the AUDIO wrap
  instead (detecting `position()` falling and restarting the picture then)
  would remove the glitch; not done because it is below what the ear
  catches on a tone and the fixture is the only clip so far.
- **Arbitrary seek: DONE, v0.1325.0.** `VideoPlayer::seek_to(seconds)`. The
  decode thread is told where to start by writing the target and THEN bumping
  the generation, in that order, so it can never pair a new generation with an
  old target. A pass rewinds `SEEK_PREROLL_S` (12 s) before the target using
  matroska-demuxer's `seek()`, skips packets until a keyframe (rav1d cannot be
  handed a mid-GOP packet), and refuses to QUEUE anything older than the
  target; the frames between are decoding work deliberately thrown away.
  If the window holds no keyframe, or the file has no index, the pass reports
  `RetryFromStart` and decodes from the top, which always arrives - count it
  with `seek_fallbacks()`. Audio gets the same position and kira seeks itself.
  Seeking while PAUSED shows the frame it landed on, which needs its own rule
  because a paused clock never advances to make anything due. A seek lands on
  the first frame at or after the target: the NEXT frame when the target falls
  between two (the one a player would show for that instant is thrown away
  with the rest of the pre-target frames), and no frame at all when the target
  is after the clip's last one, so a paused screen keeps the picture it had.
  The fallback from the top lands the same way (since BUG-158; before it, the
  fallback showed the clip from its first frame on the way).
- **Subtitles and chapters.** The demuxer exposes them; nothing reads them.
- **Streaming from the relay.** The player reads a local file through a
  `BufReader<File>`. Playing a WebM as it downloads means a reader that
  blocks on the not-yet-arrived range, which is a transport question for the
  storage rungs.
- **Through ffmpeg as it plays: DONE** (it replaced transcode on ingest), see
  the display-integration entry above.
- **GPU colour conversion.** The CPU converter is correct and measured; the
  shader version belongs with the surface.
- **More than two audio channels** (Opus multistream), **HDR tone mapping**.

## Known limits

- **Through ffmpeg as it plays (2026-11-13): what it does not do yet.** A
  film with HDR (PQ or HLG) colour comes through as plain 8-bit BT.2020 and
  looks washed out: no tone mapping (the conversion before it did none
  either; ffmpeg's `zscale` and `tonemap` filters would do it, but
  `zscale` is not in every build). The picture is capped at 1080 rows
  (`stream::MAX_ROWS`, the pipe and the colour conversion per picture). A
  seek restarts both ffmpegs at the new place, about a tenth to a quarter of
  a second; a disc whose timestamps jump between its cells seeks where
  ffmpeg's estimate puts it. Subtitles and a disc's other audio tracks are
  not chosen (the first picture stream that is not cover art, the first
  sound stream). A file's uneven frame rate is evened to the rate ffmpeg
  prints (`fps=`), because the pipe carries no times.

- **FIXED 2026-09-27 (BUG-093): the abort below was a real over-wide borrow
  in rav1d's CDEF, not only its instrumentation.** `padding` in rav1d 1.1.0's
  `src/cdef.rs` borrowed two line-buffer columns left of a block at the
  frame's left edge that it never reads, and another postfilter thread could
  hold a mutable borrow of them (the previous row's end, `backup2lines`). The
  build now uses `vendor/rav1d`, the published crate with that borrow
  narrowed to the columns read (vendor/README.md). Measured: the new
  `media::tests::rav1d_threaded_decode_stress` aborted 8 of 20 runs on the
  published crate and 0 of 20 on the patched one, and the decoded fixture is
  byte-identical either way. The rest of this entry is the 2026-09-17 account,
  kept for the history; its conclusion that the code was sound and release
  unaffected was wrong: release kept the overlapping borrow unchecked.
- **rav1d's own debug-only borrow checker can abort a debug test run.**
  rav1d guards its threaded frame buffers with `DisjointMut`, whose
  disjointness is checked at runtime only under `debug_assertions` (its
  `src/disjoint_mut.rs` says so: "checked at runtime in debug mode, while in
  release mode disjointness must be manually guaranteed"). Under `cargo test`
  (a debug build, with rav1d at `opt-level = 3` but its assertions on) and
  with the screens and media tests opening several decoders at once, each
  with rav1d's automatic thread count on a 12-thread machine, the check
  fired once in 16 completed runs of the filtered suite on 2026-09-17, on
  one of rav1d's own worker threads, with nothing of ours on the stack:
  `thread 'rav1d-worker-3' panicked at ...\rav1d-1.1.0\src\disjoint_mut.rs:837:13`
  (the `check_overlaps` panic, "overlapping DisjointMut"), followed by
  `thread 'rav1d-worker-3' panicked at library\core\src\panicking.rs:226:5`,
  a panic while panicking, which ABORTS the whole test process (no
  `test result` line; every other test in that run is lost with it). The
  other 19 completed runs that day passed 40 of 40, and one further run
  ended before its first result line with its output lost to a grep filter
  (not attributable; recorded so nobody thinks the rate is exactly known).
  It is rav1d's instrumentation, not this code: the wrapper (`src/media/video.rs`) drives one context from one
  thread, releases every picture inside `drain` before returning, unrefs the
  packet on every path, and reads picture planes through raw pointers the
  checker never sees. Decided 2026-09-17 not to touch the build profile for
  it (the release exe has no checker, so the product is unaffected). The
  device tests, which are the longest-running decoders in the suite, are
  therefore meant to be run with `cargo test --release --features native
  --lib -- --ignored <name>`; a debug run of them is still fine when it
  completes, only the abort is possible.

## Proven to fail

Two deliberate breaks were made on 2026-09-16, each observed to fail the
suite, then reverted; the commit history holds only the restored code.

1. **Drop detached instead of joining** (`let _ = self.thread.take();` in
   place of the `join()`): `drop_joins_the_decode_thread` failed with "Drop
   must join the decode thread; a leaked thread would still be alive here".
2. **`take_due_frame` returned the front frame regardless of the clock:**
   `take_due_frame_never_hands_out_a_frame_ahead_of_the_clock` failed at its
   first assertion ("nothing is due before the first pts") and the live test
   failed with "frame at 0.1 s handed out when the clock read 0.091 s".

The pixel test earned its keep before the code was even finished: the first
fixture recipe used `drawbox` with a time expression, which ffmpeg evaluates
once at start, and the bar-edge check reported the same position on every
frame. The recipe was changed to an `overlay`, which is re-evaluated per
frame, and the check then tracked `140 * pts` as intended. A frame-count
test would have passed both fixtures.

One test bug was also caught and fixed on the way: the live test first
demanded the last pts equal 59/30 to a microsecond, but WebM stores
timestamps in millisecond ticks, so frame 59 sits at 1.967 s. The tolerance
is now the container's granularity.
