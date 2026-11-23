# Platforms: HumanityOS on every device

> **Status:** plan (2026-11-14). Built today: the desktop app for Windows, Linux and macOS (CI makes
> all four builds on every release, `.github/workflows/build-desktop.yml`) and the web app (PWA,
> `web/shared/manifest.json` and `web/shared/sw.js`). Not built: Android, iOS, any store listing,
> any operating-system trust label. The facts, fees and quotations behind every line below are in
> the dated finding
> [`2026-11-14-app-distribution-costs.md`](../reference/findings/2026-11-14-app-distribution-costs.md);
> where this document and the finding disagree, the finding's sources win.

## Origin

The operator, 2026-11-14, verbatim:

> "How much work do we have to actually put in to make our software run on all devices? we got PCs.
> What about mobile OSes? Are there any costs involved? I think there was signing we still need to do
> for the app to get it labeled safe to install on Windows?"

> "It would be nice to have the native app on my one plus nine pro instead of the PWA version."

> "I guess we need to separate things between what we can do with the cheap hardware today and what
> would need millions in funding to actually work."

> "Let's aim for free."

> "I guess direct downloads are always a thing for all devices?"

> "Do I absolutely need a Mac to build for iPhone and iPad? Does Github not cover it?"

## The decision: aim for free

"Let's aim for free." So the plan is the free route on every platform, and anything that costs money
is listed beside it as a fact, never as the plan:

| Platform | The free route (the plan) | What it costs if money ever arrives (fact, not plan) |
|---|---|---|
| Windows | Free open-source code signing (SignPath Foundation, once the licence qualifies) for direct downloads; the Microsoft Store's free MSIX route for a download with no warning at all | Artifact Signing from 9.99 USD a month; certificates from 69 EUR (Certum's open-source set) to 129 USD a year plus key storage (SSL.com) |
| macOS | Direct download, unnotarized, with plain steps for System Settings > Privacy & Security | Apple Developer Program, 99 USD a year, for notarization |
| Linux | Direct download (exists today); Flathub, with its AI policy in mind | none found |
| Android | Direct APK download (sideloading) and F-Droid; a free Limited Distribution registration before 2027 | Google Play 25 USD once; full Android developer verification 25 USD |
| iPhone, iPad | Build and test in the Simulator on GitHub's free macOS runners; nothing reaches other people's devices | Apple Developer Program, 99 USD a year, for any install on another person's device |
| Web (PWA) | Live today, every device with a browser | none |
| Builds | GitHub's standard hosted runners (Windows, Linux, macOS), free for public repositories | none |

Two signings are easy to confuse. This document is about each operating system's own trust label
(Windows' Authenticode and SmartScreen, Apple's notarization, Android's developer verification). The
project's own release signature, the one the desktop updater checks, is a separate subject
([`signing-tiers.md`](signing-tiers.md)) and is not part of this plan.

## The Windows "safe to install" label, in plain words

Windows decides with two things: is the file signed by a known publisher, and has this exact file
been downloaded by many people without trouble ("reputation"). Today HumanityOS is not signed, so:

- **SmartScreen** shows "Windows protected your PC" and the person has to choose "Run anyway". Every
  new release starts again from zero reputation.
- **Smart App Control** (Windows 11, on some computers) blocks an unsigned app it cannot vouch for,
  and Microsoft says there is "currently no way to bypass" it for one app: the person would have to
  turn the whole feature off. This is the strongest reason to sign.
- **Signing does not make the warning disappear on day one.** A signed file still shows an
  "unrecognized" warning, with the publisher's name, until reputation builds ("several weeks and
  hundreds of clean installs"). What signing changes: the publisher is named, Smart App Control lets
  it run, and reputation carries over from one release to the next instead of resetting.
- **Paying more does not help.** Microsoft: EV certificates "no longer bypass SmartScreen".
- **The only route with no warning at all is the Microsoft Store**, which signs the package itself.
  Registration is free for individuals (an ID check and a selfie) and an MSIX package is signed and
  hosted by Microsoft for free. The account publishes under the account holder's own name.
- **The free certificate for open-source projects** (SignPath Foundation) requires "an OSI-approved
  Open Source license". Since 2026-11-14 HumanityOS's code is AGPL-3.0-or-later, which the OSI approved
  in 2008, so it meets that rule (plan B of
  [`2026-10-25-licence-options.md`](../reference/findings/2026-10-25-licence-options.md), adopted in
  [`2026-11-14-licence-change.md`](../reference/findings/2026-11-14-licence-change.md)); SignPath's
  other conditions still apply. The publisher name Windows shows would then be "SignPath Foundation".

## Direct downloads, platform by platform

"I guess direct downloads are always a thing for all devices?" Almost: on every platform except
iPhone and iPad.

| Platform | Where a downloaded app installs | What the person sees | How they allow it |
|---|---|---|---|
| Windows | Anywhere; the app runs from its folder | "Windows protected your PC" (SmartScreen); blocked outright where Smart App Control is on | "Run anyway"; Smart App Control has no per-app exception |
| macOS | Anywhere (normally Applications) | Gatekeeper refuses an app that is not notarized | Since macOS Sequoia, no Control-click override: System Settings > Privacy & Security (Apple's words: "to review security information for software before allowing it to run") |
| Linux | Anywhere | Nothing from a platform holder (no gatekeeper sits between a download and running it) | Mark the file executable |
| Android | Installed by the system package installer | Android blocks it until the person allows the browser or file manager it came from | "Install unknown apps" for that source (Android 8.0 and later). From 2027, an app from an unregistered developer also needs Google's one-time "advanced flow", with a 24-hour wait, or ADB |
| iPhone, iPad | No route outside Apple's program | Not possible | Only the developer's own devices, through Xcode on a Mac, for 7 days at a time; in the EU only, notarized apps from alternative marketplaces or an authorized developer's website (both paid, both EU-only) |
| Web (PWA) | The browser's own "Install" or "Add to Home screen" | Nothing extra | Nothing extra |

## "Do I absolutely need a Mac to build for iPhone and iPad? Does Github not cover it?"

GitHub covers the building. Apple keeps the installing.

- **Building and testing: no Mac needed.** GitHub's standard macOS runners (Apple M1, 3 cores, 7 GB,
  with an `xcode-27` label) are free for public repositories, and HumanityOS already builds its macOS
  desktop app on them. An iOS build and the iOS Simulator run there too, so CI can prove the app
  compiles and starts on iOS without anyone owning a Mac.
- **Putting it on a real iPhone or iPad: needs Apple.** A device install needs a signing certificate
  and a provisioning profile from Apple. Free: Xcode on a Mac, signed in to a free Apple Account,
  installs on up to 3 of your own devices, and the install expires after 7 days. Paid (99 USD a year):
  TestFlight, the App Store, and the EU's alternative routes. Whether the free 7-day profile can be
  made on a GitHub runner instead of a Mac is untested; Apple's page ties it to signing in to Xcode.
- The operator's phone is Android, so iOS is not on his path either way.

## What our stack already supports

From the crates' own documentation in the local cargo registry (versions as pinned in `Cargo.toml`
on 2026-11-14):

| Piece | What we use | Android | iOS | Notes |
|---|---|---|---|---|
| Windowing | winit 0.30 | yes, through `android-activity`; the app must be a `cdylib` with an `android_main` entry | yes | Built 2026-11-14: the engine's lib is the `cdylib` on Android with `android_main` in `src/android.rs`, and `suspended` / a later `resumed` drop and rebuild the surface (`src/engine/app_lifecycle.rs`) |
| GPU | wgpu 24 | Vulkan (wgpu's README: "First Class Support") | Metal ("First Class Support") | The renderer already picks Vulkan or Metal off Windows (`src/renderer/mod.rs`) |
| GUI | egui 0.31 through egui-winit and egui-wgpu (no eframe) | eframe lists Android; egui-winit is the same layer | through winit | Touch input needs testing on a phone. The on-screen keyboard and the system bars: BUILT 2026-11-22, the pages laid out only in the part of the window nothing covers (`src/gui/visible_area.rs`; Android's insets through `android/java/.../VisibleArea.java`, since winit 0.30 drops NativeActivity's content-rect event) |
| Audio | kira 0.12 on cpal 0.18 | AAudio, Android 8.0 (API 26) and later | CoreAudio | Microphone needs a permission prompt on both |
| Key storage | keyring 3 | **no backend** (Linux, BSD, macOS, iOS, Windows only) | Keychain | **Android: BUILT 2026-11-20** through Android's own key store (rung A1 below): keyring stays out of the phone build (its in-memory mock would forget the key) |
| Clipboard | arboard 3 | **no** (its platform code excludes Android) | **no** | Needs the platform's clipboard through JNI or UIKit. The phone build leaves arboard out; words still copy and paste inside egui |
| Tray icon | tray-icon 0.26 | **no** (Windows, macOS, Linux, FreeBSD) | **no** | Left out of the Android build (2026-11-14); the tray makes no icon there |
| TLS for the chat socket | tungstenite with `native-tls` | OpenSSL, which must be cross-compiled | Security framework | Done for Android (2026-11-14): rustls with the compiled-in roots there, native-tls kept on the desktop |
| Database | rusqlite, bundled SQLite | yes | yes | Bundled C source compiles with the NDK and Xcode |
| Starting other programs | `std::process::Command` in 23 files (Tools page, Games library, launchers) | restricted | not allowed | The Apps page says why on a phone (2026-11-14); the other screens that start a program still need their notice |
| Files | `data/` and `assets/` as loose folders beside the exe (`crates/humanity-core/src/storage.rs`), about 0.94 GB together | files inside an APK are not a normal folder | the app bundle is readable | Built 2026-11-14: the APK carries the pages' files (about 135 MiB, the release's 772 MiB less the 3D world's) and the first start of each version copies them into the app's folder. The 3D world's files are a later download (rung A5) |

## The phone's GPU against the game renderer

The operator's phone (OnePlus's spec page): Snapdragon 888, Adreno 660, 12 GB LPDDR5, shipped with
Android 11.

- **Device limits.** The renderer asks for wgpu's `Limits::default()` (`src/renderer/mod.rs`), which
  wgpu documents as "a reasonable set of limits if they are targeting only desktop and modern mobile
  devices". Buffer size follows the adapter, and the star catalogue already trims itself to the
  device's limit. So the device should be creatable on a recent phone; only running it proves it.
- **Optional features are already optional.** Indirect drawing, timestamp queries and BC block
  compression are requested only when the adapter offers them, so a phone without them does not
  fail to start.
- **Textures.** BC (the desktop block compression) is a desktop format; phone GPUs use ASTC and ETC2.
  Without BC the garment textures fall back to uncompressed, which costs video memory a phone shares
  with everything else. A mobile tier needs an ASTC path.
- **Heat and battery.** A phone holds its top speed for minutes, not hours. The 3D world on a phone
  needs its own lighter settings (resolution scale, shadow distance, vegetation density, cloud
  quality), stated in words per the graphics-settings rule ([`graphics-settings.md`](graphics-settings.md)).
  The app pages already redraw only when something changes, and a phone starts without the world's
  renderer at all (both 2026-11-20, rung A1 below).
- **Say it plainly.** Per the hardware-notice rule, a phone that cannot run the 3D world (or a
  setting) says so on the screen where the person meets it: what it needs, what this phone has, and
  what works instead (the app pages always work).

## The work in rungs

Each rung is a real step of the finished thing, not a throwaway. App pages first everywhere; the 3D
world follows with its own lighter settings.

### Windows

1. **W1, the Microsoft Store (free, no warning).** An MSIX package of the same build, submitted under
   a free individual account. A Store install updates through the Store, so the Store build turns
   off the desktop updater and says so in Settings. Before listing: the California age-signal law
   (below) and the Store's policies for the wallet and the relay, which the finding did not read.
2. **W2, free signing for direct downloads.** The licence condition is met since 2026-11-14 (the code
   is AGPL-3.0-or-later, OSI-approved; plan B, chosen that day): apply to SignPath Foundation, add the signing step to the release workflow
   (it signs builds made in CI from the repository), publish the "Code signing policy" page it
   requires (team roles, the privacy line), turn on multi-factor sign-in for every member, and
   approve each release by hand. First ask SignPath whether the DXC DLLs the Windows build loads
   when present count against its "No proprietary code" rule, and whether the CC0 data and the CC BY-SA
   docs in the download count as "components" under its "OSI-approved ... for all components" rule
   ([`2026-11-14-licence-change.md`](../reference/findings/2026-11-14-licence-change.md)).
3. **W3, the download page says what each state means.** `web/pages/download.html` already tells
   people about "Run anyway"; it should also say why the warning appears, that Smart App Control may
   block an unsigned build, and what changes once signing lands.

### macOS

1. **M1, a proper app bundle.** CI ships a bare executable today; a `.app` bundle with an icon and an
   Info.plist is what macOS users expect. Free.
2. **M2, the Privacy & Security steps on the download page**, in plain words, with the reason (an
   Apple rule, the notarization fee). Free.
3. **Needs money: notarization** (99 USD a year).

### Linux

1. **L1, the direct download.** Exists (`HumanityOS-linux-x64`, built on Ubuntu 22.04). Free.
2. **L2, Flathub.** Possible but uncertain: everything built from source with no network (Rust
   crates vendored into the manifest), a development history it judges meaningful, and its Generative
   AI policy. HumanityOS is largely written with AI, so the submission must disclose that honestly,
   the manifest must be written by a person, and the pull request must be opened by a person ("AI
   tools or agents must not open or automate Flathub submission pull requests"). Reviewers decide.
3. Distribution packages, Snap and AppImage were not researched; each needs its own dated read first.

### Android (the operator's phone first)

1. **A1, the app pages on a phone. BUILT 2026-11-14, waiting on its first CI run and the phone.**
   What exists ([android/README.md](../../android/README.md) has the detail and the install steps):
   - **The build** ([.github/workflows/build-android.yml](../../.github/workflows/build-android.yml),
     [android/build-apk.sh](../../android/build-apk.sh)): the engine's own library compiled for
     `aarch64-linux-android` as a `cdylib` (`cargo ndk ... rustc --lib --crate-type cdylib`, so
     desktop builds never link one), packaged by Google's SDK tools on GitHub's free runner with a
     hand-written NativeActivity manifest: no Java code, no Gradle. The tool comparison (cargo-ndk,
     cargo-apk, xbuild, licences quoted):
     [research/2026-11-14-android-build-tools](../reference/research/2026-11-14-android-build-tools/README.md).
   - **The entry** ([src/android.rs](../../src/android.rs)): `android_main` points HOME and TMPDIR at
     the app's private folder before anything runs (so the storage root, the logs and the encrypted
     identity live there; storage is always the per-user mode on a phone), writes the data compiled
     into the app, copies the page files the APK carries (about 135 MiB: the release's files less
     the 3D world's models, textures, planets and sky, `release-files.js --phone-list`) once per
     version, and pipes stdout and stderr to logcat.
   - **Leaving and coming back** ([src/engine/app_lifecycle.rs](../../src/engine/app_lifecycle.rs)):
     `suspended` saves and drops the wgpu surface (now an `Option` in `renderer/surface.rs`), a later
     `resumed` makes a new one on the same window; while away the app runs the tray's hidden tick.
   - **What a phone lacks, said on screen**
     ([crates/humanity-core/src/platform/device.rs](../../crates/humanity-core/src/platform/device.rs),
     words under `device.*`): the 3D world (Play's Enter button becomes the notice; every way in lands
     on Play), launching programs (the Apps page). tray-icon, arboard and keyring are left out of the
     phone build in `Cargo.toml`; the tray makes no icon there and a clipboard picture paste returns
     nothing.
   - **Remembering the key (2026-11-20; operator: "We should add that.")**: Settings, Unlock on App
     Launch works on a phone through Android's key store. It holds keys, not secrets, so each of the
     desktop's two modes keeps the same 32 bytes (the seed for Remember on this device, the device key
     for Quick PIN) SEALED with AES-256-GCM by a key made inside the key store that cannot be copied
     out (StrongBox first, else the phone's secure hardware, else software, and Settings says which:
     a phone without secure hardware is told its passphrase is the safer choice). The sealed 68 bytes
     live in the app's folder (`crates/humanity-core/src/platform/key_wrap.rs`, host-tested); the key
     store's work is Java (`android/java/.../KeyStoreBridge.java`, checked by `javac`), called through
     JNI (`platform/android_jni.rs`, the `jni` crate already in the tree). The row in
     [cryptography.md](../reference/cryptography.md). Not proven on a phone yet. **Fixed after the
     review (2026-11-20):** every switch between the modes goes through one function that forgets
     the other mode's secret (BUG-335: Remember, then Quick PIN, left the seed remembered, on a
     desktop too); the key is usable only while the phone is unlocked (Android 9+, on a phone with a screen
     lock; Settings says whether this phone's key carries the rule) and the Quick
     PIN's words say the PIN is checked by HumanityOS, not by Android; data extraction rules keep
     the app's folder out of device-to-device transfers as well as backups, and a remembered file
     this phone's key store has no key for reads as not remembered (Settings offers Remember again);
     the plaintext copies are wiped on both sides of JNI; a Java stack trace goes to the log, never
     the screen.
   - **The Java in the APK (2026-11-20)**: three small classes under `android/java/` (the key store,
     the stay-connected service, the notices), compiled by `build-apk.sh` with the runner's `javac`
     and the SDK's `d8` into `classes.dex`; still no Gradle and no libraries. Every call Rust makes
     into them is checked against their source by `platform::android_java`'s test, and the manifest
     by `tests/android_manifest_lint.rs`.
   - **TLS**: the chat socket uses rustls with the compiled-in Mozilla roots on Android (ring is
     already rustls's provider there through reqwest); the desktop keeps native-tls.
   - **Touch**: egui lays the pages out at the phone's own scale (about 412 points wide on the
     operator's phone), and on a phone every control is at least the theme's new `touch_target` high
     (44 points, Settings > Widgets). `HUMANITY_PHONE_PREVIEW=1` shows a desktop the phone's view.
   - **A pages-only start** (2026-11-20, [src/engine/renderer_slot.rs](../../src/engine/renderer_slot.rs),
     [src/renderer/pages_gpu.rs](../../src/renderer/pages_gpu.rs)): a phone makes only the device and
     the window's surface, all egui needs; the 3D world's renderer (every shader module and pipeline,
     the cloud-noise and ground-texture bakes), the star sky and the planet surfaces are built the first
     time the world is entered, which on a phone is never. Measured that day (`just verify-phone-start`,
     release build, the PC busy with other builds): the first frame came 18,588 ms after the window
     opened on the desktop start of then (adapter 1.3 s, device 4.2 s, shaders and pipelines 9.5 s) and
     947 ms on a pages-only one, and wgpu held 6 render pipelines and no compute pipeline on the pages
     against 53 and 12 with the world built. A stray reach for the world in a pages-only run is logged by
     its source line (`[PagesOnly] world touched from ...`) and fails the rig.
   - **The desktop starts pages-first too** (2026-11-20, [src/engine/world_prep.rs](../../src/engine/world_prep.rs),
     [src/renderer/world_build.rs](../../src/renderer/world_build.rs)). The operator, verbatim: "Can we
     skip building the 3D world's graphics immediately? Could we draw it out over several seconds to get
     people in chat faster?", and the same day, on the default: "I figure the all at start could be an
     option, but I'd like to default to having to press play from now on. That way we can test non-game
     stuff faster." So a desktop draws its pages first, as a phone does, and prepares the world as
     Settings > Graphics > Prepare the 3D world says: **only when I press Play** (the default), **in the
     background after start**, or **at start, before the pages** (the old order). The world's renderer is
     built OFF THE FRAME: `Renderer::build_world_detached` needs the device and the surface's format and
     size, never the surface, so a worker thread builds it while the pages keep drawing, and the frame
     installs it (`adopt_surface` moves the surface over and follows a resize made meanwhile). The
     frame's share (looking whether it is done, installing it) has a 2 ms budget, timed every frame and
     judged by the rig. Beside the pages the build is POLITE (his standing rule: gradual, never a burst):
     every thread it starts runs one step below normal priority, the pipeline compiles' included, and the
     320 MiB of cloud volumes go to the graphics card a 64 MiB slab at a time with a pause between. Play
     before it is ready keeps the page and shows a card (the step under way in words, "Step 4 of 10: the
     drawing programs for every kind of surface", a bar, "Stay on the pages"), and the world opens by
     itself when ready; the window never freezes on it. The dev autopilot's entry answers once the world
     has loaded, so every rig waits for the world itself. Measured on this PC
     (`just verify-phone-start`, release build, other agents compiling, 2026-11-20): window to first
     page frame 914 ms on the default start against 8,814 ms in the old order (773 ms with the
     background choice, 782 ms in the phone preview); Play to the world ready 8,765 ms and to the world
     loaded 10,403 ms on the default start; with the background choice the world was ready 10,503 ms
     after the window opened, and Play then loaded it in 1,147 ms (10,214 ms when pressed during the
     preparation). The frame's share of the preparation: worst 0.04 ms against its 2 ms budget. The page
     frames (a hidden rig window, about 30 a second): with nothing being built the worst gap was 80 ms;
     beside the background build 262 ms; under Play's progress card 529 ms. Each long gap is logged with
     its parts and, since 2026-11-21, each frame's longest stretches (`[WorldPrep] a page frame N ms
     after the last ...; the frame before's longest: ...`, laps kept by `humanity_core::frame_costs`
     for page frames too), and the judge classifies it: idle (a phone that drew nothing because
     nothing asked) or slow, with the stretch named. **The causes, pinned 2026-11-21** (BUG-338): the
     3,051 ms gap before Play on v0.1567.0 was the boot frame's file work after its present (the
     instruments' first read, the settings' save with its flush to disk) on a busy disk, now off the
     frame (the instruments read on their own thread, the frame's saves on a writer thread,
     `config.rs` `save_order`); the gaps under Play's card (worst 606 ms, 10% of its frames) were the
     frame's thread losing its core to the build's forty-odd threads on every core, now kept off the
     first core (`world_build::build_thread`, no priority lowered). After: before Play no long gap
     (worst 34 ms); beside the background build 0 to 2 long gaps in about 360 frames; under the card
     1 to 5% of frames. The one card frame of every build that still froze for 440 to 940 ms was the
     star sky's thread making the Milky Way glow's GPU memory beside the renderer's own build: those
     creations crawled, and wgpu makes all GPU memory, every upload's staging included, under one
     lock, so the frame's own small uploads waited behind them. Now the star sky makes its memory only
     once the renderer's pipelines are compiled (`world_build::compiles_running`, closed before
     either thread starts): the worst card frame 127 ms, the world ready after Play 9.0 s (BUG-338).
     `just verify-phone-start` fails a page frame before Play whose longest stretch passes 100 ms,
     long gaps past 5% of the frames while the world is built, and any one of them past 300 ms. Begun on the first frame itself, the
     background build had held that frame
     970 ms, so it begins a second later; and the world's frame stages wait for the world to be entered
     (they had cost the first page frame after the install 189 ms). A build someone waits for (Play)
     runs at normal priority: with its threads below normal it took 121 s instead of 9 s while other
     compiles kept the processors busy.
   - **Drawing only on change** (2026-11-20, [src/engine/redraw.rs](../../src/engine/redraw.rs)): on
     a phone a frame is drawn for input, for egui's own requests (animations, spinners, a page's
     timer, from any thread), when the network brings something the pages show (the sockets read
     four times a second without drawing, as the tray's hidden window does), and for a dev request;
     otherwise the event loop sleeps. An idle ten seconds drew 300 frames before (the background frame cap)
     and 0 after in the phone preview (45 wakes for the network tick, none drawn). Pages that move by themselves ask for their frames: the chat's typing
     marks (already did), the Music page's visualizer and roll while music plays, pictures the image
     cache finishes in the background (its workers wake the frame), chat videos (already did).
     The Dev, Maps and Watch pages draw on purpose while open (live numbers, the orrery, a video).
     The menu's RGB colour cycle holds still on a phone (an attack pulse still moves): the menu is on
     every page, and it had asked for every frame even with animations off; the [Redraw] line names
     what asked egui for each window's frames, which is how it was found.
     `HUMANITY_REDRAW_ALWAYS=1` keeps a preview drawing every frame, for the comparison.

   **Still to do in A1:** the
   on-screen keyboard is NativeActivity's (basic key events: GameActivity's full text input needs
   Java code and Gradle, a later step); the network wakes a waiting phone by a four-times-a-second
   tick rather than by the socket itself (a socket thread waking the loop would let it sleep longer);
   the file browser still starts outside the app's folder; launching programs is
   noticed on the Apps page only, not yet on the other screens whose buttons start a program
   (`std::process::Command` sits in 19 source files: the relay control page, Settings, the dev pages,
   the music controls, the media player's ffmpeg among them).
2. **A2, the APK as a release asset**, signed with a key the project keeps, and a page that walks
   through "Install unknown apps". PARTLY BUILT 2026-11-14: the workflow publishes the APK on a
   release tag once the operator has made the key (`just android-signing-key`, which sets two
   repository secrets and keeps an encrypted backup); until then each build is test-signed and stays
   a workflow artifact. The walk-through is in [android/README.md](../../android/README.md), and
   since 2026-11-20 the website's download page shows it, with the APK's download, only when this
   server's newest release carries `HumanityOS-android-arm64.apk` (the page learns of a release's
   files only from the mirror's `/releases/manifest.json`, never GitHub's API, BUG-281); otherwise
   its Android card says plainly that the phone app is not published yet. The VPS mirror takes the
   APK as an optional file for that (`PHONE_RE` in scripts/vps/humanity-mirror-release: never
   required, so a release without one mirrors as before; an APK attached after the mirror ran is not
   added to that release's folder, and the Android workflow's ten minutes against the desktop
   builds' thirty-five keep it ahead). **No native mirror, by choice:** the download page is web-only
   in [PAGES.md](../PAGES.md) because the desktop app is itself the download, and the phone app is
   installed from the phone's own browser, which reaches the website, not the desktop app. A "send
   it to my phone" card in the desktop app (a QR code to the download page) is a possible later step.
   Free.
3. **A3, a free Limited Distribution registration before 2027**, so the operator's phone (and up to
   50 authorized devices, per Google's guide) keeps installing without the advanced flow when
   developer verification reaches everyone. Needs the operator's own Google account.
4. **A4, chat while the app is closed. BUILT 2026-11-20, waiting on the phone.** Android stops an app
   that is not on the screen, and its connection with it. Settings, Notifications, **Stay connected
   while HumanityOS is closed** (phone only, OFF by default; its hint says a notification stays in the
   bar and it uses some battery) starts a foreground service, `ChatService`
   (`android/java/`, `src/notifications/stay_connected.rs`), that keeps the app's process and its own
   connection alive; turning it on asks Android to allow notices (Android 13 and later), and Settings
   says when Android blocks them or refused the service. It is started while the app is on the
   screen (at launch, or the moment it is turned on), since Android 12 and later refuse a foreground
   service started from the background. The service's type is `specialUse` with its reason in the
   manifest: `dataSync` stops after 6 hours a day on Android 15, and `remoteMessaging` is described
   as moving messages between a person's own devices. `stopWithTask`: swiping the app away in the
   recent-apps list closes it, notification and all. **Notices on a phone** go through the same rule
   as a desktop's (`notifications::decide`): who a message is from by default, the message itself
   only when the person chose "Who it is from and the message". On the locked screen of a phone
   with a screen lock (a PIN, pattern or password) a notice shows NOTHING (`VISIBILITY_SECRET`;
   Android hides notices only on a secure lock screen, so a phone locked only by a swipe shows
   every notice whole, which the setting's hint says since the second review; corrected
   2026-11-20 after a review: the first build made each
   `VISIBILITY_PRIVATE` with a public version and said a locked screen showed only "Something new
   arrived", but Android shows the private version there unless the phone's own "hide sensitive
   content" setting is on, and by default it is off). Settings, Notifications, **Show a line on the
   locked screen** (off by default) makes it `VISIBILITY_PRIVATE` with that line as the public
   version, and its hint says the line hides the message only where the phone's setting is on.
   Stopping the service right after starting it cannot crash the app (Android kills an app whose
   foreground service ends before `startForeground`): `stop` waits for the foreground, and the
   service stops itself when told before it got there; since the second review every start owes
   a new `startForeground` and a stop marks the service gone as it stops it, both under one lock,
   so off, on, off quickly never stops the new instance early. A tap opens the app (not yet
   the conversation). Not yet: a wake lock (with the screen off Android may pause the app between
   packets; an arriving message wakes it), opening the conversation from a tap. Free.

   **Staying connected: the foreground service and UnifiedPush (compared 2026-11-20, from
   developer.android.com's foreground service types, its Android 15 behaviour changes and its
   notification permission pages, read that day).** The operator, 2026-11-20: "Whichever one you
   believe is best." A FOREGROUND SERVICE first: the app keeps its own connection to its own server,
   so it needs no other app, no third party learns when a message arrives, and messages stay sealed
   end to end; the price is the notification Android requires for as long as it runs, and the battery
   a held connection costs. UNIFIEDPUSH can come later as a battery-saving choice: a distributor app
   the person installs (ntfy, for example) holds one connection for every app that uses it and wakes
   ours when the relay posts to it, so ours can stop between messages; but it needs that second app
   and a push server (the distributor's or the person's own) that sees when messages arrive, and the
   relay would need to post to it. Google's push service (FCM) is not used: it needs Google's code in
   the app and Google's servers in the path.
5. **A5, the 3D world on a phone**: Vulkan, ASTC textures, the lighter settings above, measured on the
   OnePlus 9 Pro. Free.
6. **A6, F-Droid**, after a person reads its Inclusion Policy (Claude may not read f-droid.org).
7. **Google Play only if chosen later**: 25 USD once, an ID check, a 14-day closed test with 12
   testers for a personal account, no self-updating, and no downloaded native code, so a Play build
   could not download the separate AI runtime ("AI is optional" puts that runtime in its own download).

### iPhone and iPad

1. **I1, an iOS build and a Simulator run in CI** on GitHub's free macOS runners: proves the code
   compiles and starts on iOS. Free.
2. **Needs money: everything that reaches a device beyond the developer's own for 7 days** (99 USD a
   year). An individual's iOS build must leave out the virtual currency wallet (guideline 3.1.5(i):
   "developers enrolled as an organization" only), may not download code that changes what the app
   does (2.5.2, so no downloaded AI runtime), and push notifications need the paid membership. A
   nonprofit legal entity that sells nothing could get the fee waived; an individual cannot.

### The web app

Live on every device with a browser, free. It keeps mirroring the native app. A store listing of the
PWA is possible in the Microsoft Store and in Google Play (as a Trusted Web Activity) but not in
Apple's App Store, which rejects a "repackaged website".

## Free with today's cheap hardware, and what needs money

The operator's split: what we can do now, free, on the hardware we have, and what needs money.

| Work | Free, with today's cheap hardware | Needs money |
|---|---|---|
| Windows builds | Yes (GitHub runners, his PC) | |
| Windows: no warning at all | Microsoft Store, MSIX, free individual account | |
| Windows: signed direct downloads | SignPath Foundation, once the licence is OSI-approved | Artifact Signing 9.99 USD a month (US/Canada individuals); Certum 69 EUR set; SSL.com 129 USD a year plus a 379 USD YubiKey or a cloud subscription; renewed at least every 460 days |
| macOS builds | Yes (GitHub's free macOS runners) | |
| macOS: no Gatekeeper block | | Apple Developer Program, 99 USD a year |
| Linux builds and downloads | Yes | |
| Linux: Flathub | Free, if its reviewers accept the AI disclosure | |
| Android build, APK downloads | Yes (GitHub's Linux runners, his OnePlus 9 Pro to test) | |
| Android: past 2027 without the advanced flow | Limited Distribution registration, free, up to 50 devices | Full distribution 25 USD |
| Android: Google Play | | 25 USD once, plus 12 testers for 14 days |
| iOS: compile and Simulator | Yes (GitHub's free macOS runners) | |
| iOS: any real device beyond the developer's own for 7 days | | 99 USD a year, and a Mac or the 7-day free route for his own devices |
| Web app | Yes, live | |

**What would need millions?** Nothing on this page. Every cost here is between 25 USD once and about
130 USD a year. The things that genuinely need serious money sit outside app distribution (servers
at MMO scale, console development under each console maker's agreements, which were not researched
and may not be public), and belong in their own documents.

## Blockers, named

Per the rule on saying what we will not build and why ("Saying what we will not build, and why" in
`CLAUDE.md`): each limit says whether it is a law, a patent or a platform holder's rule, in the docs
and on the screen where a person meets it (the download page, and the app's own Settings on each build).

| Limit | Kind | Whose |
|---|---|---|
| SmartScreen warns on new or unsigned downloads | Platform holder's rule | Microsoft |
| Smart App Control blocks unsigned apps, no per-app exception | Platform holder's rule | Microsoft |
| Notarization costs 99 USD a year; unnotarized apps need Privacy & Security | Platform holder's rule | Apple |
| No install on other people's iPhones without the paid program | Platform holder's rule | Apple |
| The EU-only alternative routes exist | Law (the EU's Digital Markets Act), as Apple implements it | EU, Apple |
| iOS: a currency wallet only from an organization account | Platform holder's rule | Apple |
| iOS: no downloaded code that changes the app | Platform holder's rule | Apple |
| iOS: push notifications need the paid program | Platform holder's rule | Apple |
| Android: from 2027, unregistered developers' apps need the advanced flow | Platform holder's rule | Google |
| Google Play: no self-updates, no downloaded native code | Platform holder's rule | Google |
| Flathub: AI-generated material may be rejected | Platform holder's rule (a distributor's policy) | Flathub |
| A store listing would very likely trigger age signals from 1 January 2027 | Law (California AB 1043 as amended by AB 1856) | California ([finding](../reference/findings/2026-10-10-california-ab-1043-age-signals.md)) |
| Which video codecs each platform decodes | Patent, and each platform's own decoders | ([finding](../reference/findings/2026-10-17-platform-video-decoders.md)) |

## Questions only the operator can answer

Each with a recommendation, the free routes first.

1. **The Microsoft Store under your own name?** It is free and the only route with no warning at all,
   but an individual account publishes "under your own name", so your legal name would show as the
   publisher, and it needs an ID check with a selfie. *Recommendation:* yes, if you are comfortable
   with your name on it; if not, skip it and rely on W2.
2. **The licence (answered 2026-11-14).** You chose plan B ("Let's do it."): the code is
   AGPL-3.0-or-later, which SignPath's licence rule accepts, so W2 no longer waits on the licence.
3. **Start Android now, app pages first?** *Recommendation:* yes. It is free, it is your phone, and
   rungs A1 and A2 need nothing from anyone else. *Answered 2026-11-14:* "Yeah, I'd like to try it
   on my phone." A1 is built (above). **Make the APK signing key?** Once, with
   `just android-signing-key`: every APK signed with it installs over the last and keeps the app's
   files; a test-signed APK cannot be updated in place. *Recommendation:* yes, before the phone holds
   an identity you care about.
4. **Register a free Limited Distribution account before 2027?** It needs your own Google account
   (creating it is your step). *Recommendation:* yes, once A2 produces an APK, so your phone keeps
   updating without the advanced flow.
5. **iOS: park it, with only the Simulator build in CI?** *Recommendation:* yes. Every device route
   costs 99 USD a year (or needs a nonprofit entity), and you do not have an iPhone.
6. **macOS: an unnotarized download with plain Privacy & Security steps?** *Recommendation:* yes;
   notarization waits for money.
7. **Flathub: try it with an honest AI disclosure, with you writing the manifest and opening the pull
   request yourself?** Flathub forbids an AI agent from opening it. *Recommendation:* yes, once the
   Linux build is solid, accepting that its reviewers may say no.
8. **Store listings wait for the age-signal decision?** *Recommendation:* yes, decide California AB
   1043 first (it starts 1 January 2027), as `docs/PRIORITIES.md` already says, before any listing,
   the Microsoft Store included.
9. **Google Play?** *Recommendation:* not now. The direct APK, F-Droid and the free Limited
   Distribution account cover it without the fee, the ID check or the 12 testers.
