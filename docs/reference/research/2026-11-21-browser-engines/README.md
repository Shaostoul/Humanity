# Which web engine could make HumanityOS a full browser

**Research date: 2026-11-21.** Every page below was read on that date unless a line says "could not read".
Each quotation carries its own date where the source gave one. Compare those dates with today's before acting.

This is a comparison scan, not a legal finding. The legal edge (Widevine) is in
[`../../findings/2026-11-21-widevine-licensing.md`](../../findings/2026-11-21-widevine-licensing.md),
and the codec edge is in
[`../../findings/2026-09-19-video-codec-licensing.md`](../../findings/2026-09-19-video-codec-licensing.md)
(with a "Re-checked 2026-11-21" addendum). I am not a lawyer; nothing here is legal advice.

It builds on, and does not redo, the 2026-09-18 CEF spike in
[`docs/design/embedded-browser.md`](../../../design/embedded-browser.md) and our own reader in
[`docs/design/readable-web.md`](../../../design/readable-web.md).

## The operator's questions, answered first

1. **What are our options, what is the most basic one where we control every part, and does Chromium come with
   things we cannot see or choose?** Seven open engines were scanned. Only three can be put inside another program
   today: Chromium (through CEF), Servo (a Rust library on crates.io since 2026-04-13), and, on Windows only,
   Microsoft's WebView2 (which is Microsoft's build of Chromium, updated by Microsoft). The "Linux, only what I
   choose" option is Servo: it is a library whose features are Cargo switches, written mostly in Rust, and it is the
   only one of the three where you could, in principle, read and remove every part. It is also the least finished.
   And yes, Chromium comes with things you did not pick: see "What Chromium's source does and does not include".
2. **Twitch video:** four routes exist, none proven end to end. See "Twitch" below.
3. **Netflix, Disney+, Prime Video (Widevine):** there is a published agreement route and a free signing service for
   one engine family, but nothing found says an open-source project distributed under AGPL can get either in a
   usable form, and the services themselves list which browsers they accept. See the finding.
4. **Can security patching be automated?** The building, testing and publishing can be. The decision to ship still
   needs a person to look at a failing test. See "Keeping the engine patched automatically".
5. **Servo, and the others:** Servo is the best fit for the project's values and the weakest on today's web. The
   recommendation is at the end.

---

## What each engine is, side by side

"Read" means I opened the page on 2026-11-21. Anything marked **not stated** was looked for and the page did not
say. No site compatibility figure below is mine.

| | **Servo** | **Ladybird** | **Chromium / CEF** | **WebView2** | **WebKit** | **Gecko** | **Blitz** |
|---|---|---|---|---|---|---|---|
| Licence | MPL 2.0 | 2-clause BSD | BSD 3-clause (Chromium and CEF) | Microsoft's runtime licence (terms not read) | BSD and LGPL mix (not read) | MPL 2.0 (not read) | Apache 2.0 or MIT |
| Who | Linux Foundation Europe, open governance | 501(c)(3) non-profit, small full-time team | Google (Chromium), one maintainer (CEF) | Microsoft | Apple and community | Mozilla | DioxusLabs |
| Languages | Rust, plus SpiderMonkey (C++) for JS | C++ moving to Rust; own JS engine mostly Rust | C++ | C++ | C++ | C++, Rust | Rust |
| JS engine | SpiderMonkey | LibJS (own) | V8 | V8 | JavaScriptCore | SpiderMonkey | none |
| Windows | yes (build docs exist) | not yet (WSL2) | yes | yes (only) | not as an embeddable port | not as an embeddable port | yes |
| Embeddable now | yes, `servo` crate | no | yes, CEF | yes, Windows only | GTK and WPE on Linux | Android only | yes, no JS |

### Servo

- **Licence (quoted):** the licence file of servo/servo is "Mozilla Public License Version 2.0"
  ([LICENSE](https://github.com/servo/servo/blob/main/LICENSE), read 2026-11-21).
- **Governance:** "managed with open governance under Linux Foundation Europe through our Technical Steering
  Committee" ([servo.org](https://servo.org/), 2026-11-21).
- **Platforms:** "Servo has multi-platform support, including Windows, macOS, Linux, Android, and OpenHarmony."
  The Windows build needs Visual Studio 2022 components and `.\mach bootstrap`
  ([Servo Book, Windows](https://book.servo.org/building/windows.html)).
- **Embedding API and its maturity:** the `servo` crate first reached crates.io as v0.1.0 on 2026-04-13
  ([announcement](https://servo.org/blog/2026/04/13/servo-0.1.0-release/)). The post says it is "not a 1.0 release"
  and that breaking changes in the monthly releases are expected. The API centres on `ServoBuilder`, `WebView` and
  pixel readback (per a search summary of that post; the post itself only says "allows Servo to be used as a
  library"), which is what an in-world monitor needs. Releases since: 0.3.0 (May), 0.5.0 (July), 0.6.0 (August),
  0.7.0 (September, blog dated 2026-11-05).
- **Long-term support (quoted from [the Servo Book](https://book.servo.org/embedding/lts-release.html)):** "A new LTS
  release / branch is introduced every 6 months, based on the current regular release at the time." "The expected
  support duration is 9 months, giving embedders time to migrate to the next LTS release." "The LTS release will
  receive security fixes only."
- **How real sites fare, from Servo's own posts:** 0.5.0: "The DuckDuckGo 'duck' landing page 'now renders in
  v0.5.0.' Gumroad pages 'render almost perfectly.'" 0.7.0 (September): "a playable Candy Crush game, and a TinyMCE
  editor example" now work. **Gmail, YouTube, Twitch, Discord and banking are not mentioned in any post I read**, so
  there is no published statement either way. We would have to measure.
- **Media path:** only through GStreamer, and only if the optional `media-gstreamer` Cargo feature is on
  (`components/servo/Cargo.toml`: the default features are `bundled`, `clipboard`, `default_tooling`, `js_jit`,
  `multiprocess`, `harfrust`; `media-gstreamer` is not among them). The README of the media crate says "For now, the
  only functional backend is GStreamer." That repository, servo/media, shows "archived by the owner on Apr 23, 2026",
  yet the main crate still depends on `servo-media-gstreamer`, so the code moved somewhere I did not find. Media fixes
  in the posts: 0.6.0 "fixed crashes related to media playback" and "canPlayType() ... can report that MP4 content
  is playable"; 0.7.0 fixed an audio/video memory leak and a `<video>` panic. Whether adaptive streaming (MediaSource)
  works well enough for Twitch is **unknown**.
- **DRM:** none. Searching servo/servo issues for "widevine" returned 0 results and for "EME DRM" returned 0
  results (2026-11-21). A negative: no plan is on its tracker.
- **Security response:** a security report path through GitHub ("Please submit security related issues as GitHub
  security reports"), no bug bounty stated. Fixes arrive in the monthly release: 0.3.0 fixed SpiderMonkey memory
  safety bugs ("CVE-2026-7322", "CVE-2026-7323", "MFSA 2026-36"), 0.7.0 took SpiderMonkey 153.3 with "fixes for
  security bugs reported upstream". The sandbox is a build option ("Multiprocess mode is enabled at build time via a
  new Cargo feature", in the 0.7.0 post; it is on by default per Cargo.toml).
- **Size and build cost:** not measured here (no download rule). It compiles SpiderMonkey (C++) and ANGLE, so
  expect a long first build and a Visual Studio dependency.
- **Fit:** the engine you can read, trim and patch, in the language the project already uses. The cost is
  maturity and the monthly API breakage.

### The GStreamer question: could Windows' own decoders give H.264 and AAC?

Servo's only media backend is GStreamer, so the question is what GStreamer's Windows elements are. From GStreamer's
own plugin pages (read 2026-11-21, undated pages):

- Media Foundation plugin
  ([mediafoundation](https://gstreamer.freedesktop.org/documentation/mediafoundation/index.html)): "mfaacdec:
  Microsoft Media Foundation AAC Decoder" is in the list, next to MP3 decoder and H.264/H.265/VP9 **encoders**. **There
  is no mfh264dec** on that page. The page's own description: "hardware and software video encoders, software audio
  encoders, and webcam capture elements".
- Direct3D 11 plugin ([d3d11](https://gstreamer.freedesktop.org/documentation/d3d11/index.html)): "d3d11h264dec:
  H.264/AVC video decoder", plus H.265, AV1, VP8, VP9 and MPEG-2, using Direct3D11 / DXVA hardware acceleration,
  Windows 8 or later.

So on paper both halves exist as operating-system routes: **H.264 video through the d3d11 plugin (hardware, via the
graphics driver) and AAC audio through the Media Foundation plugin (Windows' own decoder).** That is exactly the
combination the CEF issue could not get ("Chromium does not (directly) support AAC audio decoding"). It would close the
audio gap named in the codec finding, **if** (a) Servo's GStreamer player picks these elements instead of
`gst-libav` (FFmpeg, whose own licence position is the one the finding was trying to avoid), (b) the machine has a
GPU able to decode H.264 (the same limit the CEF maintainer named for old or blocklisted GPUs), and (c) Twitch's
player works on Servo's MediaSource at all. None of the three is tested. The GStreamer FAQ puts codec licensing on the
distributor ("The GPL operates on a per-action, not per-program basis" and the question is about whoever
distributes), which matches the finding's reasoning: we would ship none of the decoders. We would, however, have to
ship or ask people to install GStreamer's Windows runtime, and its own advisory list is long (see the patching section).

### Ladybird

- **Licence (quoted):** "licensed under a 2-clause BSD license" ([README](https://github.com/LadybirdBrowser/ladybird/blob/master/README.md)).
  "Ladybird is in a pre-alpha state, and only suitable for use by developers."
- **Governance:** 501(c)(3) non-profit funded by donations and sponsorship; "board seats are not for sale"
  ([ladybird.org](https://ladybird.org/)).
- **Platforms:** "Our first alpha is planned for later this year" for macOS and Linux
  ([October 2026 newsletter](https://ladybird.org/newsletter/2026-10-31/)). Windows: "On Windows, video now plays and YouTube
  works" appears in that newsletter, but the FAQ says "There are very few maintainers working on Windows support"
  and "we may consider a Windows port, but it's not a priority" ([FAQ](https://github.com/LadybirdBrowser/ladybird/blob/master/Documentation/FAQ.md)).
- **Embedding API:** none documented. Ladybird is a browser, not a library; nothing I read offers a `WebView` to embed.
- **Sites:** newsletter: "Google Slides presentations open, GitHub renders Mermaid diagrams, and nytimes.com and
  reuters.com get past bot checks" and "Gmail no longer keeps the old page in memory after a reload". No banking or
  Twitch statement.
- **Media:** "Ladybird reads MP4 files directly instead of using FFmpeg" and hardware video decoding "runs in its own
  process and works on Linux through Vulkan" (same newsletter).
- **DRM:** the work has only just started. Issue 11390, "Spotify cannot play songs due to missing EME implementation"
  (opened 2026-08-27, open). Pull request 15167, "LibWeb: Implement Encrypted Media Extensions in Rust", merged
  2026-11-20; its walkthrough mentions only ClearKey and "does not establish full EME playback support". Widevine is not
  mentioned.
- **Security:** [SECURITY.md](https://github.com/LadybirdBrowser/ladybird/blob/master/SECURITY.md): "Security fixes go
  into the master branch, and are backported to the latest release's branch." "Ladybird doesn't offer bug bounties for
  security issues at this time." "Ladybird's process isolation is still being hardened."
- **Fit:** the most interesting independent engine, and not usable by us yet: no library, no Windows, pre-alpha.
  Re-check after its alpha.

### Chromium and CEF

- **Licence:** Chromium's LICENSE and CEF's LICENSE.txt are BSD 3-clause (first lines read: "Copyright 2015 The
  Chromium Authors" and "Copyright (c) 2008-2020 Marshall A. Greenblatt").
- **Embedding:** CEF (what the spike measured), plus Rust bindings. A new branch about every four weeks; pin an LTS
  branch (embedded-browser.md has the table).
- **Sites:** the spike measured YouTube and Rumble playing; Twitch loaded but video would not decode.
- **What Chromium's source does not include that Chrome has.** Chromium's own page
  ([chromium_browser_vs_google_chrome.md](https://chromium.googlesource.com/chromium/src/+/main/docs/chromium_browser_vs_google_chrome.md))
  says Chrome has "H.264, AV1, VP8, and VP9" video and "AAC, MP3, Opus, Theora, Vorbis, FLAC, and WAV" audio, and
  that Chromium omits H.264 and AAC; "Sandbox is always on" in Chrome but "Sandbox depends on the distribution" in
  Chromium; "Google API keys are added by Google" in Chrome but "depend on the distribution" in Chromium; Chromium
  "Does not ever report crashes". That page does not mention Widevine or auto-update. From other primary pages:
  Widevine is a separate component (see the finding) and CEF downloads it from Google at runtime, "Widevine CDM
  binaries will be downloaded on supported platforms shortly after application startup" (CEF issue 3149, 2021), which
  can be turned off with `--disable-component-update`.
- **Google services and their terms:** the API key page
  ([chromium.org](https://www.chromium.org/developers/how-tos/api-keys/), undated) lists Chrome Sync, Safe Browsing,
  Google Drive and others, and says "Many of the Google APIs used by Chrome are specific to Google and not intended for
  use in derived products." On keys: "Software distribution with keys acquired for yourself is allowed, but the keys
  themselves cannot be shared with parties outside the legal entity that accepted the API ToS." It also says keys "are
  not for distribution purposes and must not be shared with other users", which reads as conflicting with the earlier
  sentence; I show both. "Signing in to Chromium is restricted." Practical meaning: a build we ship has no Google
  sync or Safe Browsing unless we register our own keys, and a hidden service call is a thing to audit for.
- **ungoogled-chromium** ([README](https://github.com/ungoogled-software/ungoogled-chromium/blob/master/README.md)):
  "Google Chromium, sans dependency on Google web services", removes Google-specific code, disables Safe Browsing and
  "Blocks remaining runtime requests to Google domains through domain substitution". BSD-3-clause. Supports "many Linux
  distributions, macOS, and Windows". It is a desktop browser patch set, not an embedding framework: using it under
  CEF would mean carrying its patches into our own CEF build.
- **Security response:** Chrome moved to a two-week stable cycle with Chrome 153 on 2026-09-08
  ([Chrome blog](https://developer.chrome.com/blog/chrome-two-week-start), via search summary); security fixes ship on
  the weekly schedule even for Extended Stable. Bug-bounty details were not fetched.
- **Size and cost:** the spike measured 164 MiB of SDK and 431 MB on disk for the build; a custom CEF build with
  other codec flags is a multi-hour Chromium build.
- **Fit:** the only engine proven to play YouTube on our in-world monitor today, and the one that comes with the most
  things you did not choose. Keep it as the agreed opt-in video host, not the foundation.

### Microsoft WebView2

- **What it is:** "The WebView2 control uses Microsoft Edge as the rendering engine to display the web content in
  native apps" ([Microsoft Learn](https://learn.microsoft.com/en-us/microsoft-edge/webview2/), updated 2026-10-02).
  Windows 10 and 11 and Windows Server only; Xbox and HoloLens 2.
- **Updates:** "The WebView2 Runtime updates automatically without requiring any action from you" and receives the
  Edge updates ([distribution page](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution),
  updated 2026-10-02). It "will be included as part of the Windows 11 operating system". The Fixed Version mode
  ("over 250 MB") leaves updating to us, which is the trap for patching.
- **A caution in the same page:** a running app keeps the old runtime until restarted, which "has security
  implications"; the app should listen for `NewBrowserVersionAvailable` and ask the person to restart.
- **Codecs:** it is Microsoft's own build, so Edge's decoders come with it; I did not find a WebView2 page listing them
  (not verified).
- **DRM:** PlayReady works on the Evergreen runtime and fails on Fixed Version
  ([issue 4632](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4632), 2024, open). A feature request for
  Widevine ([issue 4828](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4828), opened 2024-09-21) is open; Microsoft's one reply (2024-11-22) asks for
  the requester's workaround and says nothing about plans.
- **Fit:** the lowest-effort full browser on Windows, but Windows only, a closed Microsoft component we cannot trim, and
  the opposite of "only what I choose". Useful as a clearly labelled fallback on Windows, not as the base.

### WebKit

WebKit's download page ([webkit.org](https://webkit.org/downloads/)) lists two embeddable ports, "GTK (WebKitGTK)" and
"WPE", both Linux; a Windows port is not listed there. The Windows build (WinCairo) exists in the tree and is used
by Playwright, but I found no documented embedding API or maintenance statement for it (search results only; not a
primary page). Security advisories: WSA-2026-0007 on 2026-11-18, WSA-2026-0006 on 2026-09-29, 0005 on 2026-08-20
([webkitgtk.org](https://webkitgtk.org/security.html)), roughly every one to two months; current stable WebKitGTK
2.54.3. **Not a fit for a Windows-first game.** If Linux becomes the first platform, WPE is the embedded-friendly
choice.

### Gecko (Firefox's engine)

GeckoView is Android only. A Mozilla developer wrote in 2019: "GeckoView is pretty directly tied to the Android
platform, so turning it into an Electron-like Desktop runtime" is not directly feasible
([Mozilla Hacks comment](https://hacks.mozilla.org/2019/06/geckoview-in-2019/), 2019-07-12). I found nothing newer
saying otherwise (a community idea "Ability to embed Gecko" is "In review" on Mozilla Connect). **Not available.**
Firefox itself does use Widevine; see the finding.

### Blitz (Rust, no JavaScript)

[DioxusLabs/blitz](https://github.com/DioxusLabs/blitz): "currently in a beta state", dual licence "Apache 2.0 and
MIT". It builds on Stylo (the CSS engine from Servo and Firefox), Taffy, Parley and Vello/WGPU. "We don't yet have
Blitz bindings for other languages (JavaScript, Python, etc)". It renders HTML and CSS well; it will not run a site
that needs JavaScript. **Compared with our reader (`docs/design/readable-web.md`):** Blitz is a better renderer of
static HTML and CSS than hand-rolled egui drawing, but we already chose to render readable text ourselves. Worth a
look as the rendering back end of the reader (for rich static pages), not as a browser.

### Other open engines found

- **Verso**, a Servo-based browser: a July 2026 third-party blog says it stalled and its repository was archived; I could
  not confirm that against a primary page. Servo's own embedding replaced the need for it.
- **NetSurf, Lightpanda, Tauri/Wry:** named in my search but not read. Wry is a wrapper around the operating system's
  own web view (WebView2 on Windows, WebKitGTK on Linux), so it is the WebView2 and WebKit rows again, not a new engine.
  Lightpanda is a headless engine for automation. NetSurf is a small C browser with little JavaScript.
- **Excluded (proprietary, so out under the open-source-only rule):** Ekioh Flow, Ultralight and Sciter (closed
  licences), and Safari's WebKit on Apple platforms (a platform component, not ours to ship).

---

## Twitch: what options exist

Facts first. Twitch's own post of 2026-06-17 says "2k streaming uses HEVC, a next-generation video codec that provides
higher quality at lower bitrates" and "AVC (used for 1080p streams and lower)". So everything up to 1080p is
H.264/AVC; 1440p needs HEVC (and the post says nothing about AV1). That is the codec finding's case, with HEVC added for
the top quality. (Source: [Twitch blog](https://blog.twitch.tv/en/2026/06/17/introducing-dual-format-and-2k-streaming-on-twitch/).)

| Route | What it gives | What stops it |
|---|---|---|
| **CEF with the OS decoder only** (CEF issue 3559) | H.264 video through Windows | AAC audio: "Chromium does not (directly) support AAC audio decoding"; issue still open, nothing new since 2026-06-01. Picture without sound. |
| **Servo + GStreamer with d3d11h264dec and mfaacdec** | On paper, both video and audio from Windows' own decoders | Untested: Servo's MediaSource support, element selection, GPU requirement, GStreamer packaging. |
| **WebView2 (Windows only)** | Edge's media stack, Microsoft licensed | Windows only; closed; not "only what I choose". Codec list not documented on a page I could read. |
| **Open codecs only** (VP9, AV1) | Works today (YouTube) | Twitch does not send them to viewers. |
| **Wait for the patents** | Free, no work | Last US H.264 patent now listed 2031-05-22 (see the addendum), AAC unclear. |

Plain answer: there is no proven route today. The Servo-with-GStreamer route is the only one that could cover both
picture and sound with Windows' own decoders, and it is the one to test first.

---

## Android: what each engine means for the Android app

Read 2026-11-21. The short version: **on Android the practical engine is the one the phone already has, Android System
WebView, and Twitch's two codecs come from the phone's own decoders.** Nothing in the others is ready.

- **Android System WebView.** Chromium's engine inside an Android view. Google's WebView FAQ
  ([Chromium docs](https://chromium.googlesource.com/chromium/src/+/lkgr/android_webview/docs/faq.md), undated):
  "WebView needs regular security updates just like your browser." and "We release a new version every 6 weeks". The
  Android developer pages say it updates through Google Play on Chrome's cadence and describe stable releases as
  every 4 weeks (the two figures differ; treat the interval as roughly a month). The same FAQ: "Chrome-specific features
  like Sync and Data-Saver aren't available inside of WebView" and WebView and Chrome "don't share data". On Android 7
  and later the Chrome package itself provides WebView. Licence: Chromium's is BSD 3-clause; the terms of Google's
  packaged WebView were not read. **Patching is Google's job through Play, not ours.**
- **Its codecs.** The Android platform's own list
  ([supported media formats](https://developer.android.com/media/platform/supported-formats), undated) says AAC LC has a
  decoder on all versions ("YES"), and for H.264 Main Profile "The decoder is required"; Baseline has a decoder too;
  HEVC decoder from Android 5.0, AV1 decoder from Android 10. So H.264 and AAC come from the phone through MediaCodec
  without us shipping a decoder, which is the codec finding's licensing posture, here as the platform's documented
  contract. That WebView hands Twitch's video to those decoders is **likely (it is Chrome's media stack) but not stated
  on a page I read.**
- **Its Widevine.** The WebView FAQ does not mention DRM. Secondhand reports (search results only, not primary):
  Widevine EME works in a WebView if the host app accepts the DRM permission prompt, persistent licences do not.
  Google's Chrome 62 post says Widevine L1 is supported in Chrome for Android where the device has it; whether the same
  holds in WebView was not found. Netflix, Disney+ and Prime Video still need the finding's agreement on Android as on
  Windows, and Prime/Disney pages were unreadable. **No promise.**
- **How an app embeds it.** As an Android `WebView` view (Jetpack Webkit adds newer features through a library,
  per Android's overview page). It draws as a normal view, so an Android app needs no offscreen texture trick.
- **Servo on Android.** Real, early. April 2026: servoshell for Android "now has a revamped browser UI" and a 30%
  smaller APK; July 2026: "servoshell now runs on Android 10+ rather than only Android 13+". **Nothing says
  video plays on Android** or that the embedding crate has an Android integration guide. Not ready for Twitch.
- **GStreamer's Android elements.** The plugin is `androidmedia` (source folder
  `subprojects/gst-plugins-bad/sys/androidmedia`: `gstamcvideodec`, `gstamcaudiodec`, `gstamcvideoenc`,
  `gstamcsurfacetexture`, camera and sensor sources). The audio decoder's code maps AAC (`audio/mpeg`, mpegversion 2 or 4)
  to `"audio/mp4a-latm"`, and the video decoder is a "MediaCodec video decoder"; element names are made at run time from
  the device's codec list (a bug report shows `amcviddec-omxqcomvideodecoderavc` for H.264). So **yes, in code, H.264 and
  AAC go through Android's MediaCodec, i.e. the phone's licensed decoders**, but there is no fixed element list, GStreamer's
  published plugin docs had no page for it (404), and I found no statement of Servo on Android using it.
- **Ladybird on Android.** None. Pull request 15240, "Everywhere: Remove the Android port for now", merged 2026-11-21.
- **GeckoView.** Firefox's engine for Android only: "GeckoView powers all currently active Mozilla browsers on Android"
  (Mozilla docs, via search summary). The mature non-Chromium option on Android. Its licence (MPL 2.0 is Mozilla's) and its
  DRM and codec pages were not read, so no claim here. It adds a second engine's patch stream and a large download; it
  is a fallback idea, not a first choice.

**What this means for the Android app:** use Android System WebView for any in-app web view. It costs us no patch stream
(Google updates it through Play, and the app should still check that its version is not ancient), H.264 and AAC come
from the phone, and it is the same Chromium family that YouTube, Twitch and Discord test against. Do not bundle an engine
on Android. Paid streaming stays the finding's answer: no promise, and "Open in your own browser" is the honest button.

---

## Several engines, chosen per service

The operator's idea: let people pick, and make a service that needs engine X use X, "prioritize whichever is best for the
desired service". **It is workable, and it is how the Windows and Android sets above already fall out.** The condition is
that the choice is data, not code, and that we are honest about the cost of each extra engine.

### Which engine each kind of page needs (my assessment from the sources above; "unmeasured" means no source)

| Kind of page | Windows | Android | Why |
|---|---|---|---|
| Reading (articles, wikis, docs) | our reader, no engine | our reader | runs no scripts, so nothing to patch |
| Ordinary sites with scripts | Servo (to be measured), else WebView2 | System WebView | Servo's posts name only demo-class sites |
| Sign-in sites (Gmail, banks) | WebView2 | System WebView | the engines Microsoft and Google patch fastest; sign-ins are the most valuable thing to lose to an old engine. Servo until it has a published or measured record: unmeasured |
| YouTube | CEF measured working (VP9, Opus); Servo unmeasured; WebView2 unmeasured | System WebView, unmeasured | open codecs, no DRM |
| Twitch | Servo plus GStreamer's d3d11 and Media Foundation elements (untested), WebView2 (unmeasured), CEF with OS decoder (picture only) | System WebView (H.264 and AAC from the phone) | needs H.264 and AAC; HEVC beyond 1080p |
| Rumble | CEF played it in the spike | System WebView, unmeasured | it served open codecs |
| Discord | unmeasured on every engine | System WebView, unmeasured | heavy scripts; a Chromium family is the likelier fit |
| Paid streaming (Netflix, Disney+, Prime) | none of ours: "Open in your own browser" | none of ours: the service's own app | the finding: a platform holder's rule |

### What each extra engine costs

1. **Its own security stream, forever.** Every engine we ship or depend on adds a feed to watch (Servo monthly, Chromium
   every two weeks, GStreamer batches). WebView2 and Android System WebView are the exception: Microsoft and Google carry
   that stream, our job is only to notice a stale one. That is the biggest argument for preferring them over CEF.
2. **Integration work per engine**: a host process, a frame pipe to the in-world screen, input, and tests. WebView2 on
   Windows has no offscreen frame path measured by us (the CEF spike measured CEF's); that is an unknown that decides
   whether WebView2 can sit on an in-world monitor at all, or only in a native window.
3. **Profiles and sign-ins are not shared.** Cookies, logins and saved settings live inside each engine. Switching from
   Servo to WebView2 for a site means signing in again. The app must say so in words where the person chooses, and the
   chosen engine must stay stable per site.
4. **Download size.** CEF about 164 MiB of SDK for the spike's build; WebView2 Fixed Version "over 250 MB" (Evergreen is
   shared with Windows and costs nothing); Servo and GStreamer not measured here. Each is a separate opt-in download.
5. **Words and testing.** Each engine adds failure messages to translate and a column to the test list.

### How `data/web/sites.json` could carry the choice

Each site record already has `id`, `url`, `category`, `embed` (a legal review of whether its pages may be shown inside
HumanityOS: status, basis, reviewed_on) and `affiliate`. Add one more sub-record, so the choice lives with the site:

```
"engine": {
  "needs": ["h264", "aac"],              what the site requires of an engine (data, not a guess)
  "require": ["servo-gst", "webview2"],  engines allowed, in the order we recommend (empty = reader is enough)
  "reason": "plays H.264 and AAC video"  plain words shown where the person chooses
}
```

and an `engines` file (for example `data/web/engines.json`) listing each engine's id, platforms, what it provides
(`h264`, `aac`, `vp9`, `widevine`...), its version and its known-fixed version. The chooser then follows one rule: the
person's own preference per site wins if the engine provides what the site needs; otherwise the first engine in `require`
that is installed and up to date; otherwise "Open in your own browser". The in-world monitor and the Browser page read the
same record (web mirrors native). This sits beside, and does not replace, `embed.status`: a site must still be allowed to
be shown inside HumanityOS before any engine draws it, and Twitch, YouTube and Rumble need that review of their terms
before they are `allowed`. The record is a schema change to `schemas/web_sites.toml`; since nobody uses it yet,
change it outright with no migration.

### Recommended starting set: three

1. **Our reader** (always on, every platform; no scripts, no patch stream).
2. **The platform's own Chromium web view**: WebView2 on Windows, Android System WebView on Android. It covers sign-in
   sites, Discord, YouTube, and probably Twitch, at no patching cost to us.
3. **Servo** as the engine we control, as a separate opt-in download, measured on the same list and promoted per site as
   it earns it; **CEF only if WebView2 cannot be put on an in-world screen** (it is the one we would have to patch ourselves).

Plus the always-present **"Open in your own browser"** for paid streaming. That is two engines to build and one to keep in
reserve. A fourth (GeckoView, Ladybird, WebKit) is not worth its patch stream today.

---

## Keeping the engine patched automatically

The operator's question: an embedded engine needs security fixes every few weeks, could we automate protecting people?

**What each engine's fix stream looks like (read 2026-11-21):**

| Engine | Stream | Where to watch |
|---|---|---|
| Chromium / CEF | Stable every two weeks (since Chrome 153); security fixes weekly; CEF follows with its own branches; LTS about 9 months | Chrome release blog and CEF issue tracker |
| Servo | Monthly release (0.7.0 for September); LTS branch every 6 months, 9-month support, security fixes only | servo.org blog, the crate on crates.io, GitHub security advisories |
| GStreamer (Servo's media) | batches of advisories: nine dated 2026-11-06 (SA-2026-0093 to 0101), a batch on 2026-10-08 | gstreamer.freedesktop.org/security |
| SpiderMonkey (inside Servo) | upstream Mozilla advisories, taken by Servo at its next release | MFSA numbers cited in Servo's posts |
| Ladybird | fixes on master, backported to the latest release branch; CVEs for accepted reports | its Security tab |
| WebView2 | Microsoft updates the Evergreen runtime itself | Microsoft's Edge security notes |
| WebKitGTK | advisories WSA-2026-NNNN every 1 to 2 months | webkitgtk.org/security |

**What an automatic pipeline would need, in order:**

1. A **watcher** that polls those feeds each day and records "engine X has a fixed release Y newer than ours". Plain
   HTTP and JSON; nothing exotic. Our version table is a data file (infinite-of-x), not code.
2. A **builder** that rebuilds the engine as its own separate download (never inside the main app), on CI, from the
   pinned upstream tag.
3. A **test gate**: the existing browser rig loads a fixed site list and checks the page rendered, video decodes, and
   the engine boots in its sandbox. A red result stops the publish and tells a person. This is where automation ends:
   someone must read a failing test.
4. A **publisher** that puts the engine in the release channel the in-app downloader reads, with its version and the
   advisory ids it fixes.
5. The **app's side**, the part that protects people when the pipeline is late: on every start the app compares the
   installed engine's version with the newest known fixed version. If it is behind a flaw, the app turns the engine off
   and says so in plain words ("The web engine on this computer is out of date and has a known security problem, so web
   pages are switched off until it is updated. [Update now]"), and the reader (our own, which runs no scripts) keeps
   working. WebView2's own guidance is the model: "your app would need to use the latest version" for third-party content.
6. A **deadline rule**: for the engine's own stable stream, a fix older than 14 days with no update turns the engine off;
   for Servo's LTS, the 9-month window ends on a date the app can read.

**Honest limits.** CEF's two-week cadence is a treadmill, which is why the spike pinned LTS; Servo's monthly API breakage
means "just bump the version" will sometimes fail to compile, and a person fixes the call sites. Servo's LTS is
security-only, which suits an automatic bump. The Fixed Version of WebView2 is the same job and is the reason to prefer
Evergreen. Everything here is a design, not something I built.

---

## Recommendation

**Make Servo the engine we control for the "full browser" rung, run it as a separate opt-in download in its own process,
use the platform's own Chromium web view (WebView2, Android System WebView) where Servo cannot yet do a job (see "Several
engines, chosen per service"), keep CEF only as the reserve for video if WebView2 cannot reach an in-world screen, keep
our own reader as the default for reading, and do not build anything toward paid-streaming DRM.**

Why: it matches the operator's wish to control every part (a Rust library with Cargo switches and open governance),
it has the one route that might finally cover Twitch's sound, and its LTS stream is shaped for an embedder with
few people. Why not yet the whole answer: it has no published statement about Gmail, YouTube, Twitch, Discord or banking,
its API breaks monthly, and its media path is behind a non-default feature. So the first rung is a measurement, not a
feature.

**First rung (one increment, no product change):**

1. A host process (`web-host`) that links the `servo` crate pinned to the current LTS or latest 0.x, renders offscreen
   through `ServoBuilder` and `WebView`, and returns pixel readbacks over the same pipe the CEF spike used. Build it in
   a scratch target outside the repo tree's main build.
2. Run the same measurements as the CEF spike on a fixed list: a news site, Gmail sign-in page (no sign-in), Discord
   login page, a bank's public login page, YouTube watch page, Twitch channel page. Record what draws, what scripts
   fail, frame cost, memory, and whether `media-gstreamer` with the d3d11 and Media Foundation elements plays an
   H.264/AAC sample.
3. Publish the result as a dated finding. If Twitch plays with sound, Twitch is solved without a codec licence; if it
   does not, we know exactly which piece failed.
4. In parallel, nothing for DRM: write on the screen where someone meets it that paid streaming needs a platform
   holder's permission (a PLATFORM HOLDER'S rule), per the project's say-what-we-will-not-build rule.

This reads, but does not reverse, CLAUDE.md's 2026-09-18 amendment: an embedded Chromium is the agreed route for
WATCHING video platforms specifically. Servo would be tried first for that, and CEF stays as the fallback if it fails.

## What I could not reach or confirm

- **primevideo.com robots.txt** bars Claude-User, and **www.amazon.com robots.txt** bars Claude-User and ClaudeBot
  entirely, so Prime Video's help pages were not read.
- **help.disneyplus.com** pages loaded empty to the reader (they need scripts). The Disney+ browser requirement page
  could not be read.
- **support.mozilla.org** and **support.brave.app** DRM pages: the first showed a robot check, the second returned 403.
  I did not work around either.
- Servo's stance on DRM: no tracker entry or statement exists; I found nothing, which is itself the finding.
- Not fetched: Chromium's bug-bounty rules, Servo and Ladybird build sizes, WebView2 licence terms, and any WebKit
  Windows embedding statement. Gmail, YouTube, Discord and banking results for every engine are not published by the
  projects themselves.
