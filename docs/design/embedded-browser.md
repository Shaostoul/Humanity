# An embedded browser engine on an in-world screen: the measurement

A spike, run 2026-09-18. It answers one question with numbers and stops. It is
not a feature, nothing in it is wired into the game, and the default build is
byte for byte what it was before.

## The question

> Can we get an embedded browser engine's pixels onto an in-world screen on
> this machine, at 1280 x 720, thirty times a second, at a frame cost the game
> can afford?

**Yes.** One 720p browser screen costs the game **0.064 ms of GPU** and
**0.26 to 0.63 ms of CPU** out of a 33 ms frame. Three of them on the walls at
once cost 0.19 ms of GPU and 1.2 to 2.6 ms of CPU. That puts a browser screen
in the same class as the egui page screens already on those walls, whose
`cpu.screen_ui` measured 1.366 ms in the console room the same day for
however many were framed that tick (at most four).

The browser itself sustains 1280 x 720 at a true 60 frames a second on this
machine while using **54% of one core**, or 30 frames a second using **16% of
one core**, in a separate process that never touches the game's frame.

Two things the answer does NOT include, and they matter more than the frame
cost:

- **Twitch video cannot decode in an official CEF build**, because those builds
  ship without H.264 and AAC. Measured, not assumed: see [Codecs](#codecs).
  This is a PATENT licensing limit, not a law and not a Twitch rule.
- **Paid streaming services (Netflix, Disney+, Prime Video) should not be
  expected to work at all**, because they need a signed Widevine path that only
  Google grants. See [Widevine](#widevine). This is a PLATFORM HOLDER'S rule.

YouTube plays. Rumble plays. Twitch's embed is *accepted* and its player runs;
only the video stream cannot be decoded.

## Why the project wants this

The operator wants to watch video platforms on the monitors in the console
room, through those platforms' own embedded players with their advertising
intact, which is the condition under which every one of them permits embedding.
Our own HTML reader (`src/web_reader`) cannot run those players, because they
are JavaScript applications rather than documents.

The position this serves is already written down in
[`docs/reference/media-stance.md`](../reference/media-stance.md): official
embeds, adverts untouched, no stream extraction. Worth stating plainly, because
it is easy to assume otherwise: **Twitch's own developer agreement forbids
covering, obscuring or interfering with the player, including the
advertisements inside it.** Our stance and their terms agree. We are not
tolerating their rule; we wrote the same one first.

The screen surface this would draw into already exists and did not change for
this spike. See [`in-world-screens.md`](in-world-screens.md) for
`ScreenProvider`, `ScreenSurface::write_pixels` and the existing `web:`
provider.

### This does not replace our own reader, and it cuts against a standing note

`CLAUDE.md` carries a direction that the wanted web-browsing surface is **our
own lightweight browser, explicitly not Chromium or CEF**, so that real sites
render on in-game monitors. That direction is not disturbed by this spike and
should not be read as overturned by it.

The two answer different questions. Our own reader (`src/web_reader`) renders
documents: it fetches, parses and lays out a page, and it is the right thing for
reading the open web inside the game, on our own terms, at a size we can
maintain. What it cannot do is run a platform's video player, because those
players are JavaScript applications with their own networking, adaptive
streaming and advertising, and reimplementing one is not a reader feature. It
is a second browser engine.

So the honest framing is: **the reader keeps the web, and this route is only
for the video case**, where the platform's own player is both the technical
requirement and the licence condition. If the project would rather not carry a
Chromium at all, that is a legitimate answer and the cost of it is simply that
the monitors show our reader and not YouTube. This document exists so that
choice can be made against numbers instead of guesses.

## What was built, and where it lives

Two standalone crates under `tools/`, neither a workspace member, neither
referenced by `src/`, both invisible to `cargo check` at the repository root:

| Path | What it is |
|---|---|
| `tools/cef-probe/` | The browser host. Starts Chromium offscreen, loads a URL at a fixed size, counts frames, writes PNGs, publishes into shared memory, records the console and the page title. |
| `tools/frame-sink/` | The game's side, without the game. Takes frames out of shared memory and puts them into a wgpu texture, timing both. |
| `tools/frame_shm.rs` | The shared-memory ring, one file included by both, so the two processes can never disagree about the layout. |
| `tools/cef-probe/pages/` | `anim.html` (the animated test page), `embed.html` (hosts each platform's official player), `codecs.html` (what this build can decode). |
| `tools/cef-probe/serve.js` | A loopback-only static server, standing in for the app's own HTTP server. |
| `tools/cef-probe/run-embed-tests.js` | Runs the six platform scenarios and diffs the before and after frames. |

The CEF distribution itself is **not in the repository**. It is a 164 MiB
download that expands to 431 MB, and it lives at
`%LOCALAPPDATA%\cef-sdk\cef` on this machine. `/tools/*/target/` is gitignored.

## 1. Does it build here at all?

**Yes, in about 25 minutes of a three-hour budget, with no source patching.**

**The binding chosen: the `cef` crate from tauri-apps, version
`152.3.0+152.0.6`.** It was the first candidate tried and nothing gave a reason
to try a second. The reasoning:

- It is the only actively maintained option. Published 2026-09-14, four days
  before this spike. Its CI runs a `windows-latest` job that builds and tests.
  Every alternative is dead or niche: `cef-sys` last published 2015,
  `dungeonfog/cef` unpublished and last touched 2023, `hytopiagg/cef-ui` never
  on crates.io. `wef` and `wew` exist but are early and lightly used.
- It targets CEF **152.0.6**, matching Chromium 152.0.7977.83, and the crate's
  major version tracks the CEF major, so which SDK it wants is never a guess.
- It downloads the SDK rather than vendoring it, through a companion binary
  (`export-cef-dir`) that also verifies the archive's SHA1.
- Driving the C API by hand was never needed. The crate generates safe wrappers
  and `wrap_render_handler!` style macros; the whole host process is about 600
  lines of ordinary Rust.

### What was downloaded, exactly

```
https://cef-builds.spotifycdn.com/
  cef_binary_152.0.6+g708dc14+chromium-152.0.7977.83_windows64_minimal.tar.bz2
```

171,619,649 bytes (163.7 MiB), SHA1 `e5e3020627f4528bd43e22f4c4970000b0458e99`,
verified by the downloader. Exported to `C:\Users\Shaos\AppData\Local\cef-sdk\cef`,
431 MB on disk. Fetched with:

```bash
cargo install export-cef-dir --version "152.3.0+152.0.6" --root <toolingdir>
export-cef-dir --target x86_64-pc-windows-msvc --save-archive <sdkdir>/cef
```

The `minimal` distribution is what the Rust binding asks for and it is
sufficient: it is the release binaries and headers. The `standard` distribution
(343.0 MiB) adds debug binaries and the sample client, and the `client` one
(161.0 MiB) is just the demo application.

### What the build needs

`CEF_PATH` pointing at the exported directory, and **CMake plus Ninja on
PATH**, because the binding compiles CEF's own C++ wrapper library. Both are
already on this machine inside the Visual Studio Build Tools that the MSVC Rust
toolchain requires anyway:

```
.../BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin
.../BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja
```

Clean build of `cef-dll-sys` + `cef` + the probe: **1 minute 39 seconds**.

### The one non-obvious thing

Modern CEF versions its C interface, and every process must declare which
version it was built against **before it touches anything else**:

```rust
let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
```

Without that, the very first object handed to CEF is rejected and the process
dies with `CefApp_0_CToCpp called with invalid version -1`. That was the only
false start of the whole exercise.

## 2. Offscreen frames at all

`tools/cef-probe/pages/anim.html` is a local page that changes nearly every
pixel every frame: a stripe field that slides one pixel per frame, a bar
sweeping across, an orbiting disc and the frame number in large digits. It also
publishes its own animation rate in its title, so the page's opinion of its
frame rate and the embedding's opinion can be compared without either trusting
the other.

The first frame it produced, read back at 1280 x 720 and written to PNG,
showed frame 150 with the blue bar at 83% across, the orange disc at
(709, 549), and the digits upright. Those are exactly the positions the page's
own arithmetic puts them at for frame 150. So: real pixels, correct colours
(so the BGRA to RGBA conversion is right way round), correct orientation.

Chromium delivers **BGRA**. The engine's screen textures are RGBA
(`SURFACE_FORMAT` is `Rgba8UnormSrgb`), so something has to swap the red and
blue byte of every pixel. That swap was measured at **0.56 to 0.74 ms per
frame** and it is **entirely avoidable**: `ScreenProvider::surface_format`
already lets a provider choose its own texture format, so a browser provider
can ask for `Bgra8UnormSrgb` and write Chromium's bytes in with no conversion
at all. Recorded here so the next increment does not pay it by accident.

## 3. Rate and cost of the source

Measured over 30 to 45 second windows, at 1280 x 720. CPU is summed over the
**whole** process family (Chromium relaunches the host executable for its
renderer, GPU and utility processes), which is six to nine processes.

| Page | Requested | Delivered | p50 interval | CPU (one core = 100%) | Working set | Private |
|---|---|---|---|---|---|---|
| anim.html (every pixel changes) | 60 fps | **60.02 fps** over 45 s | 16.57 ms | **53.7%**, 6 procs | 338 MB | 229 MB |
| anim.html (every pixel changes) | 30 fps | **30.02 fps** over 30 s | 32.04 ms | **16.2%**, 6 procs | 305 MB | 214 MB |
| YouTube official embed, playing | 30 fps | 16.38 fps over 45 s | 66.54 ms | **14.7%**, 9 procs | 625 MB | 488 MB |

Tail behaviour on the synthetic page at 60 fps: p95 26.11 ms, p99 30.51, worst
39.47. At 30 fps: p95 37.10, p99 37.57, worst 41.05. The page's own reported
rate was 60.0 and 30.0 exactly, so the embedding lost nothing.

**The YouTube row is not a shortfall.** Chromium only hands over a frame when
something changed, and the clip used (the first video ever uploaded to YouTube,
chosen because it has been publicly embeddable for twenty years) is 2005
footage at about 15 frames a second. The embedding delivered 16.38, which is
the content's rate plus the player's own small animations. The practical
consequence is worth keeping: **a browser screen costs what its content costs.**
A 24 fps film costs 24 frames of pipe. A paused video or a still page costs
almost nothing.

Per-frame work inside the host's own paint callback, which is host CPU and not
the game's:

| | 30 fps run | 60 fps run |
|---|---|---|
| copy out of Chromium's buffer | 1.435 ms | 1.763 ms |
| BGRA to RGBA swap (avoidable) | 0.556 ms | 0.737 ms |
| publish into shared memory | 0.338 ms | 0.445 ms |

The copy out of Chromium's buffer runs at about 2 GB/s, roughly a quarter of
this machine's plain memory speed, which is what reading from a mapped,
uncached buffer costs. It is paid by the host process, not the game.

## 4. The pipe

Shared memory, which is the obvious choice: the two processes map the same
block and no bytes travel through the operating system. `tools/frame_shm.rs`
is a ring of four slots, 14.1 MB in total at 1280 x 720, with a sequence number
per slot that the writer makes odd while it writes and even when the slot is
whole. The reader checks that number before and after copying; if it moved, the
frame is skipped. Nothing locks, and the writer pays two atomic stores a frame.

Each slot carries the Windows performance counter at the moment the writer
finished it. That counter is the same clock in every process on the machine, so
the reader's subtraction is real elapsed time rather than an estimate.

| | anim.html at 60 fps | YouTube at 30 fps |
|---|---|---|
| published / received | 1919 / 1915 over 32 s | 519 / 520 over 40 s |
| frames skipped | 4 | 0 |
| read retries (writer landed on the reader) | **0** | **0** |
| throughput | **210.4 MB/s** | 45.7 MB/s |
| copy out of shared memory | 0.480 ms/frame (7.7 GB/s) | 0.421 ms/frame (8.7 GB/s) |
| frame age at pickup, p50 | **0.69 ms** | 0.71 ms |
| p95 / p99 / worst | 1.32 / 3.07 / 11.34 ms | 1.72 / 4.60 / 54.61 ms |

The brief asked whether 105 MB/s is comfortable or marginal. At 60 fps this ran
at **210 MB/s, twice that, and the honest answer is comfortable**: the copy out
of shared memory is under half a millisecond, 4 frames out of 1,919 were
skipped because the consumer deliberately takes the newest rather than every
one, and not one read out of about 2,400 had to be retried. The worst observed
staleness was 11 ms at 60 fps, which is under one frame at 30.

(The YouTube row shows 520 received against 519 published because the consumer
attached after the producer and picked up the frame already sitting in the ring.
Both counts are within the consumer's own window, not the producer's.)

## 5. Onto the GPU

`tools/frame-sink` creates a wgpu device with the same shape the engine uses
(`wgpu` 24, `Limits::default()`, features requested as an intersection with
what the adapter has, which is the "can never fail device creation" rule from
v0.784.2) and a 1280 x 720 `Rgba8UnormSrgb` texture with the same usages a
screen surface has. It then does exactly what `ScreenSurface::write_pixels`
does.

Three ways of asking, because the first two can mislead:

**The GPU's own view.** A timestamp either side of the buffer-to-texture copy:

> **0.0635 to 0.0696 ms**, with p95 at 0.0645 to 0.0707 across runs. This is
> the most stable number in the whole spike.

**A frame loop's view**, which is the one the budget cares about: upload every
screen, submit **once**, never block waiting for the GPU. Run three times under
different machine loads, with a plain RAM-to-RAM memcpy of the same 3.52 MB as
the control:

| Machine state (memcpy control) | 1 screen | 2 screens | 3 screens |
|---|---|---|---|
| quiet (30.9 GB/s) | **+0.261 ms** | +0.636 ms | +1.174 ms |
| middling (11.0 GB/s) | +0.627 ms | +1.341 ms | +1.955 ms |
| busy, three other builds running (7.6 GB/s) | +0.570 ms | +1.136 ms | +2.569 ms |

The control moves with the result, which is how we know the spread is the
machine and not the measurement. Three other agents were compiling on this
machine throughout; the quiet row is the fair one for a player's machine, and
the busy row is a fair worst case.

**A misleading way, recorded so nobody repeats it.** Submitting and waiting for
the GPU after every single upload reported anywhere from 0.50 to 7.16 ms per
upload, and `write_texture` alone from 0.52 to 2.27 ms. Both numbers are
artefacts: forcing a full synchronisation every iteration starves wgpu's
staging memory, which no real frame does. They are ten to thirty times the
truth. If a future measurement of this quotes a figure near a millisecond for a
single `write_texture`, check whether it synchronised.

**Is there a cheaper shape?** Writing the frame straight into a persistently
mapped GPU buffer took 0.122 ms (quiet) to 0.282 ms, against 0.261 ms for
`write_texture` in the realistic loop. So no: `write_texture` is already at the
speed of memory, and there is no win worth the complexity. Good to know it was
checked.

## 6. The verdict, in frame-budget terms

The target is 30 frames a second, so a frame is 33.3 ms.

**The baseline to compare against** is the engine's own, measured on this
machine on the same day (2026-09-18 16:11, the console room with wall screens
in view, real GPU timestamps, from the screens rig's own capture):

```
frame_ms         33.28      (the 30 fps vsync cap)
gpu.screen_ui     0.035 ms  (every framed page screen's egui pass, summed)
cpu.screen_ui     1.366 ms  (the same screens' egui runs and submits, summed)
```

At most four surfaces re-run their page per frame, so that 1.366 ms is shared
across up to four page screens.

**One 720p browser screen, put beside that:**

| | GPU | CPU | share of a 33.3 ms frame |
|---|---|---|---|
| one browser screen | 0.064 ms | 0.26 ms (quiet) to 0.63 ms (busy) | **0.2% GPU, 0.8% to 1.9% CPU** |
| two browser screens | 0.127 ms | 0.64 to 1.34 ms | 0.4% GPU, 1.9% to 4.0% CPU |
| three browser screens | 0.191 ms | 1.17 to 2.57 ms | 0.6% GPU, 3.5% to 7.7% CPU |

The CPU column is measured directly, one row per screen count. The GPU column
is measured for one screen and multiplied: a buffer-to-texture copy of a fixed
size has no shared setup to amortise, so N of them cost N times one. Worth
knowing which is which.

The "In-world screens" budget in `data/performance/budget_systems.ron` is 3% of
the frame, which is **1.0 ms**. One browser screen uses 6% of that budget and
three use 19%.

For scale against the things this project actually fights: the cloud pass in
the console room costs 26 ms after a whole increment of work to get it down
from 211, the scene pass costs 14.24 ms, and the celestial pass 17.82. Three
browser screens cost 0.19 ms of GPU. The upload is not where a frame goes.

**The host process is charged separately and does not come out of the frame.**
At the 30 fps target it used 16% of one core on a six core, twelve thread
i7-8700K: a sixth of one core out of twelve threads. Its memory, 305 MB for a
plain page and 625 MB for YouTube's, is the real cost of the route and should
be stated as such: a browser is not a cheap thing to have running, it is a
cheap thing to have *on a screen*.

So: **yes, comfortably, with room for three screens.** The frame cost was never
the risk. The risks are the two licensing constraints below.

## Platform embeds: what each one actually did

Every scenario ran the platform's **own** published embed, unmodified, for 20
to 26 seconds, saving a frame at 4 seconds and another at the end. The share of
pixels that changed between the two is the check that cannot be fooled by a
page claiming success: a player that is really playing changes most of the
picture, a still refusal notice changes none of it.

### From a page opened as a file on disk (`file://`)

| Platform | Result | Pixels changed |
|---|---|---|
| YouTube | **Refused**, player error **153** | 0.0% |
| Twitch | **Refused** by the browser, before the request | 0.0% (2 colours: blank) |
| Rumble | Loaded, poster frame shown, did not autoplay | 0.0% |

YouTube error 153 is the documented code for a request with no `HTTP Referer`
header or equivalent client identification, added in July 2025. A file has no
origin to send, so it gets 153.

Twitch's refusal, verbatim from the browser console:

```
Framing 'https://player.twitch.tv/' violates the following Content Security
Policy directive: "frame-ancestors http://localhost:* https://localhost:*".
The request has been blocked.
```

This one is structural rather than a policy anyone could argue with. Twitch
turns the `parent` parameter into a `frame-ancestors` header and the **browser**
does the matching. A file document has an opaque origin with no host name, and
`frame-ancestors` only ever names hosts, so no value of `parent` could ever
match. There is nothing to fix here and no point trying.

### From the same page served over local HTTP (`http://localhost:8788`)

| Platform | Result | Pixels changed |
|---|---|---|
| YouTube | **PLAYING.** Reported state 1, then played to the end | **100.0%** |
| Twitch | **Embed accepted.** Player loaded, reported READY then OFFLINE | 0.0% (nothing was live) |
| Rumble | Loaded; after one click, **playing** | **99.9%** |

The YouTube end frame shows the video at 0:19 / 0:19 with the replay button up.
The Rumble end frame is a frame from inside the video. Neither is a poster.

**This is the architectural finding, and it is worth stating loudly: the watch
page does not need to come from our website.** Serving it from
`http://localhost` satisfied every platform that had anything to say about
where it was hosted. The app already contains an HTTP server (`src/relay`), so
the page can ship inside the app and be served by the app itself. That means
zero bandwidth from united-humanity.us, no dependency on it being up, and the
feature keeps working when our site is down. What our site's uptime affects is
the platform's own servers, which is out of our hands either way.

The rules to build to, for the next increment:

- Twitch's `parent` is a **bare host name**. A scheme (`parent=https://x.com`)
  or a port (`parent=x.com:8080`) is rejected outright, and so are wildcards.
  A missing or malformed parent redirects to an error page rather than
  rendering. Non-localhost parents must be HTTPS; `localhost` and `127.0.0.1`
  are exempt from that, which is exactly why a local server works.
- Rumble has **no** `frame-ancestors` and **no** `X-Frame-Options` on its embed
  endpoint, so it embeds from anywhere. But its embed id is **not** the id in a
  video page's URL, so a page URL cannot be turned into an embed URL without
  their oEmbed endpoint at
  `https://rumble.com/api/Media/oembed.json?url=<page url>`.
- YouTube wants a referrer. Serving the page from localhost supplies one.

The synthetic click that started Rumble also proves the input half works: the
probe sends an (x, y) in page pixels to the browser, which is exactly the shape
`engine::screens` already computes when the player looks at a wall and clicks.

## Codecs

Measured directly in this build rather than taken from documentation, by asking
the browser what it can play (`canPlayType` and `MediaSource.isTypeSupported`,
the latter being what a streaming player actually asks before choosing a
variant):

| Codec | `<video>` | MediaSource |
|---|---|---|
| H.264 (avc1), baseline and high | **no** | **no** |
| AAC audio | **no** | **no** |
| HEVC | **no** | **no** |
| MPEG-TS with H.264 (the HLS container) | **no** | **no** |
| VP8, VP9 | probably | yes |
| AV1 | probably | yes |
| Opus, Vorbis, MP3 | probably | yes |
| FLAC | probably | no |

And then put to the test, because "the browser said no" and "the video did not
play" are different claims. Two twenty second clips were generated locally with
ffmpeg, one H.264 in MP4 and one VP9 in WebM, served from the local server and
played in a plain `<video>` element:

| Clip | Result | Frames delivered |
|---|---|---|
| `h264.mp4` | **failed**, `PipelineStatus::DEMUXER_ERROR_NO_SUPPORTED_STREAMS: FFmpegDemuxer: no supported streams`, then `NotSupportedError: Failed to load because no supported source was found` | **0.00 fps**, the page never repainted |
| `vp9.webm` | **played**, position advanced past 11 seconds | 30.11 fps |

So this is observed, not inferred.

**Why: the official CEF builds are compiled without the patent-encumbered
codecs, because distributing them requires a licence the CEF project does not
hold.** This has been the project's stated position since 2015 and was restated
in 2024. It is a PATENT licensing limit. It is not a law and it is not a
platform's rule, so it is the kind of limit that could in principle be lifted,
by building CEF from source with `proprietary_codecs=true` and
`ffmpeg_branding=Chrome` and taking on the licensing ourselves. That is a real
decision with real cost, not a build flag we should flip casually.

What it means per platform:

- **YouTube: fine.** It serves VP9 with Opus, both present. Proven playing.
- **Rumble: fine in practice.** It played, so it served something in the open
  set.
- **Twitch: the embed loads and the player runs, but the video will not
  decode.** Its baseline is H.264 with AAC (HEVC for 1440p, AV1 still in
  beta), and H.264 was observed failing outright above. The one thing left
  unobserved is whether Twitch would offer a browser that lacks H.264 some
  other variant. It has no reason to, since every browser it supports has
  H.264. Confirm against a live channel before promising anything to a user,
  but plan as though the answer is no.

## Widevine

The standard CEF distribution does **not** contain the Widevine content
decryption module. Since Chromium 93 CEF downloads it at runtime instead, which
sounds like the problem solves itself, and it does not.

Playing anything from Netflix, Disney+ or Prime Video additionally needs
**Verified Media Path** signatures: files sitting beside the executable, signed
with a certificate obtained from Google, or from castLabs as a lab Google
appointed for the purpose. Obtaining one involves agreements and a compliance
audit of the application. Without it the licence servers do not issue licences
and playback simply does not happen.

**So the operator should not expect paid streaming services to work, and should
not plan a feature around them.** This is a PLATFORM HOLDER'S rule. Ordinary
ad-supported video needs none of this: YouTube and Twitch playback is
unencrypted and uses no CDM at all.

Per the project's standing rule on saying what we will not build and why, this
belongs on the screen where somebody runs into it, not only in this file. The
next increment should put both limits (no H.264 so no Twitch video, no signed
Widevine so no paid services) somewhere a person meets them, and name which is
a patent and which is a platform rule.

## The version we would pin, and what that costs us

CEF tracks even-numbered Chromium milestones with a new branch about every four
weeks, and Chrome moved to a two week release cycle from Chrome 153 in
September 2026. Support windows:

| Channel | Branches | Supported for |
|---|---|---|
| Stable | every other milestone from M152 | about 2 weeks |
| Extended | every fourth milestone from M152 | about 8 weeks |
| LTS | every sixth from M138 to M150, then every twelfth from M160 | about 9 months |

Current at the time of writing: **M152 stable** (`152.0.6+g708dc14+chromium-152.0.7977.83`,
built 2026-09-07), with **M150** (`150.0.20`) and **M144** (`144.0.35`) as the
live LTS branches.

The maintenance shape this implies: **pin an LTS branch, not stable.** A two
week stable window would mean a version bump every fortnight forever, for a
component whose whole point is that it is a security boundary facing the open
web. Nine months is a maintenance item. Two weeks is a treadmill. The Rust
binding's version tracks the CEF major, so an LTS pin is expressible as an
ordinary version requirement.

## What the next increment would be, if this goes ahead

In order, each provable on its own:

1. **A `ScreenProvider` that reads the ring.** No CEF in the engine at all: the
   provider opens the shared memory, takes the newest frame and calls
   `write_pixels`. It can be built and tested against `tools/cef-probe` as the
   publisher, exactly as `scripts/live-publish.js` stands in for a live
   streamer in the existing live-screen rig. Give the surface a
   **`Bgra8UnormSrgb`** format through `ScreenProvider::surface_format` so the
   red and blue swap disappears.
2. **The host process, shipped.** `tools/cef-probe` grown into a real host: a
   command channel for navigation and input, a clean shutdown, and the
   decision about whether the CEF SDK is bundled with the download or fetched
   on first use. The engine must build and run with the host absent.
3. **The watch page served from the app's own server**, with `parent=localhost`
   for Twitch, and the site out of the path.
4. **Input and audio.** The click is proven; the wheel, keyboard and where the
   sound comes from are not.
5. **The two limits on screen**, per the standing rule: no H.264 so no Twitch
   video, no signed Widevine so no paid services, each named as patent or
   platform rule.
6. **An LTS pin and a note in the release checklist** about who bumps it.

One thing worth investigating before step 1 is settled: the `cef` crate has an
`accelerated_osr` feature that hands over a **shared GPU texture** instead of
a buffer of pixels, which would remove the copy, the pipe and the upload
entirely. Its example is built against **wgpu 30**, and this engine is on
**wgpu 24**, so it cannot be taken as it stands. Given that the measured cost
of the plain route is 0.064 ms of GPU, it is not worth a wgpu upgrade on its
own. Revisit it if the engine moves to wgpu 30 for other reasons.

## Reproducing any of this

```bash
# Once: fetch the SDK (164 MiB download, 431 MB on disk, outside the repo)
cargo install export-cef-dir --version "152.3.0+152.0.6" --root <toolingdir>
export-cef-dir --target x86_64-pc-windows-msvc <sdkdir>/cef

# Build, with CMake and Ninja from the VS Build Tools on PATH
export CEF_PATH=<sdkdir>/cef
cd tools/cef-probe  && cargo build --release
cd tools/frame-sink && cargo build --release

# Frame rate and cost of the source, worst case
cef-probe --url=file:///<repo>/tools/cef-probe/pages/anim.html \
          --width=1280 --height=720 --fps=30 --seconds=30 --out=<dir>

# The pipe and the GPU, with the producer already running and --shm set
frame-sink --shm=humanity_cef_probe --seconds=30 --gpu=1

# The GPU numbers on their own, no browser needed
frame-sink --bench-only=1 --gpu=1 --iters=1000

# The six platform scenarios, file:// and localhost
node tools/cef-probe/run-embed-tests.js

# The codec tests. The two clips are generated once, into pages/, and are
# gitignored because they are 130 KB and 730 KB of generated noise:
cd tools/cef-probe/pages
ffmpeg -f lavfi -i "testsrc=size=640x360:rate=30:duration=20" \
       -c:v libx264 -pix_fmt yuv420p h264.mp4
ffmpeg -f lavfi -i "testsrc=size=640x360:rate=30:duration=20" \
       -c:v libvpx-vp9 -b:v 500k -deadline realtime vp9.webm
cef-probe --url=http://localhost:8788/playback.html?which=h264 ...
cef-probe --url=http://localhost:8788/playback.html?which=vp9 ...
cef-probe --url=file:///<repo>/tools/cef-probe/pages/codecs.html ...
```

The probe writes `result.json` with every number, `frame.png` and
`frame_end.png` beside it. Evidence for this document is under
`%LOCALAPPDATA%\cef-sdk\runs\`.

## What was not measured, and why

- **A live Twitch stream.** The `twitch` channel was not live during the run,
  so what was observed is that the embed is accepted and the player runs, plus
  separately that H.264 does not decode here. What joins those two facts, that
  Twitch has no non-H.264 variant to offer, is reasoning rather than
  observation.
- **Audio.** Nothing in this spike played sound or measured what routing it to
  a screen's position would cost.
- **Two browser hosts at once.** The GPU upload was measured for one, two and
  three screens, but all from one publisher. Three separate Chromium instances
  would multiply the host CPU and memory, and 625 MB each is the figure to
  multiply.
- **A long run.** The longest window was 45 seconds. Memory growth over hours,
  which is the usual way an embedded browser goes wrong, is unmeasured.
- **Anything on a machine other than this one.** i7-8700K, 32 GB, RTX 4070.
