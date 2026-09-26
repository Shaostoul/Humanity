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
| Video | AV1 | `rav1d 1.1.0`, `default-features = false`, features `bitdepth_8`, `bitdepth_16` | BSD-2-Clause (`COPYING` in the crate: VideoLAN and dav1d authors, ISRG) | **Royalty-free.** The Alliance for Open Media publishes a royalty-free patent licence for AV1 implementations and content |
| Audio | Opus | `unsafe-libopus 0.2.0` (already in the tree for voice chat) | BSD-3-Clause (`LICENSE` in the crate: Xiph.Org, Skype, Octasic and others) | **Royalty-free.** The Opus licence file itself points at the Xiph, Microsoft and Broadcom royalty-free patent statements filed with the IETF |
| Playback | kira 0.9.6 `StreamingSoundData::from_decoder` | already a dependency | MIT | Not applicable |
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
- **Everything else** (MP4, MKV with H.264, MOV, AVI, MP3 audio) is the job
  of **transcode-on-ingest**: when a file enters the platform's storage, a
  later rung converts it once to WebM AV1 + Opus (with the machine's ffmpeg
  where present, or a pure-Rust rav1e + Opus encoder path when we have one
  fast enough), and the player only ever sees the one format. One decoder
  pair in the exe, no codec sprawl, no patent exposure.

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
machine's own ffmpeg reads the stream and the existing transcode-on-ingest
path converts it once into the one format the player decodes. A disc is
simply another thing ffmpeg can read.

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
src/audio/mod.rs    AudioManager::play_stream(): hands a streaming sound to kira
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

kira 0.9 exposes exactly the hook needed: `sound::streaming::Decoder`, a pull
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
  the format a phone writes, so transcode-on-ingest has a real file to
  convert.
- `VIDEO_TS/` (69,632 bytes for the tree): a whole unencrypted video disc,
  so the disc path is tested with no disc in the drive.
  `VTS_01_1.VOB` and `VTS_01_2.VOB` are ONE 2 s MPEG-2 program stream
  (300 kbit/s video, AC-3 audio, a cyan square sliding and bobbing) cut in
  half at a 2048 byte pack boundary, which is exactly what a disc author
  does at the VOB size limit: the timestamps run on across the cut, so the
  two parts really do convert as one film and a test that only got the
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
CPU pass; (3) transcode-on-ingest choosing 720p for machines that need it
(2.25x fewer pixels: roughly 64 fps single-thread at the 1080p rate above);
(4) an assembly build of rav1d later. On that last one: nasm is a BUILD-time
tool on the CI machine, never something a player installs, so
`features = ["asm"]` in the release workflow stays an honest option if real
content demands it; dav1d's assembly is typically 2 to 4x faster than the
plain build. It is not turned on now because it is not needed for 1080p and
the no-toolchain build is worth keeping simple.

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
- **Transcode on ingest: DONE (2026-09-18).** A chosen file the player
  refuses is converted ONCE by the machine's ffmpeg into
  `<data dir>/media/cache/<blake3 of path, size and date>.webm` and the
  converted copy plays; the source is never touched. ffmpeg is looked for at
  the Settings > Media path, then `PATH`, then the per-platform list in
  `data/media/ingest.json`. The percentage shows on the screen while it
  runs; a cache hit skips ffmpeg entirely; no ffmpeg, a missing encoder or an
  unreadable file is an on-screen message naming the fix, never a panic and
  never a silent black screen. The command line is pinned by
  `transcode_args`, so a later edit cannot quietly change the quality or
  drop the audio. Implementation: `src/media/transcode.rs`.
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
  because a paused clock never advances to make anything due.
- **Subtitles and chapters.** The demuxer exposes them; nothing reads them.
- **Streaming from the relay.** The player reads a local file through a
  `BufReader<File>`. Playing a WebM as it downloads means a reader that
  blocks on the not-yet-arrived range, which is a transport question for the
  storage rungs.
- **Transcode on ingest: DONE**, see the display-integration entry above.
- **GPU colour conversion.** The CPU converter is correct and measured; the
  shader version belongs with the surface.
- **More than two audio channels** (Opus multistream), **HDR tone mapping**.

## Known limits

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
