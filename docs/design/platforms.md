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
  Open Source license". HumanityOS is CC0 today, which the OSI never approved, so it does not qualify
  as written. Plan B of the pending licence vote (AGPL-3.0 for code,
  [`2026-10-25-licence-options.md`](../reference/findings/2026-10-25-licence-options.md)) would. The
  publisher name Windows shows would then be "SignPath Foundation".

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
| Windowing | winit 0.30 | yes, through `android-activity`; the app must be a `cdylib` with an `android_main` entry | yes | Our app is a `bin` with `main`, and handles `resumed` but not `suspended` (`src/lib.rs`). Android takes the drawing surface away when the app goes to the background, so the renderer must drop and rebuild it |
| GPU | wgpu 24 | Vulkan (wgpu's README: "First Class Support") | Metal ("First Class Support") | The renderer already picks Vulkan or Metal off Windows (`src/renderer/mod.rs`) |
| GUI | egui 0.31 through egui-winit and egui-wgpu (no eframe) | eframe lists Android; egui-winit is the same layer | through winit | Touch input and the on-screen keyboard need testing on a phone |
| Audio | kira 0.12 on cpal 0.18 | AAudio, Android 8.0 (API 26) and later | CoreAudio | Microphone needs a permission prompt on both |
| Key storage | keyring 3 | **no backend** (Linux, BSD, macOS, iOS, Windows only) | Keychain | Android needs the Android Keystore through JNI (already noted in `docs/PRIORITIES.md`, TIER 2 item 13) |
| Clipboard | arboard 3 | **no** (its platform code excludes Android) | **no** | Needs the platform's clipboard through JNI or UIKit |
| Tray icon | tray-icon 0.26 | **no** (Windows, macOS, Linux, FreeBSD) | **no** | Gate it out of mobile builds; phones have no tray |
| TLS for the chat socket | tungstenite with `native-tls` | OpenSSL, which must be cross-compiled | Security framework | Switching it to rustls (as `tokio-tungstenite` already uses) removes the OpenSSL step |
| Database | rusqlite, bundled SQLite | yes | yes | Bundled C source compiles with the NDK and Xcode |
| Starting other programs | `std::process::Command` in 23 files (Tools page, Games library, launchers) | restricted | not allowed | Hide these features on mobile with the reason on screen |
| Files | `data/` and `assets/` as loose folders beside the exe (`crates/humanity-core/src/storage.rs`), about 0.94 GB together | files inside an APK are not a normal folder | the app bundle is readable | Unpack or download the data on first run into the app's own folder |

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
  quality), stated in words per the graphics-settings rule ([`graphics-settings.md`](graphics-settings.md)),
  and the app pages should redraw only when something changes, not every frame.
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
2. **W2, free signing for direct downloads (after the licence vote).** If the vote lands on an
   OSI-approved licence: apply to SignPath Foundation, add the signing step to the release workflow
   (it signs builds made in CI from the repository), publish the "Code signing policy" page it
   requires (team roles, the privacy line), turn on multi-factor sign-in for every member, and
   approve each release by hand. First ask SignPath whether the DXC DLLs the Windows build loads
   when present count against its "No proprietary code" rule.
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

1. **A1, the app pages on a phone.** An Android build target: a `cdylib` with `android_main`
   through `android-activity` (the version winit pins), built on GitHub's free Linux runners with the
   Android NDK. Gate out tray-icon and arboard, put the identity key in the Android Keystore through
   JNI, move the chat socket's TLS to rustls, handle `suspended` and `resumed` (drop and rebuild the
   surface), unpack or download `data/` and `assets/` into the app's own folder on first run, and
   make touch and the on-screen keyboard work in egui. Hide desktop-only features (launching programs,
   the tray, file paths outside the app's folder) with the reason on screen. Free.
2. **A2, the APK as a release asset**, signed with a key the project keeps, and a page that walks
   through "Install unknown apps". Free.
3. **A3, a free Limited Distribution registration before 2027**, so the operator's phone (and up to
   50 authorized devices, per Google's guide) keeps installing without the advanced flow when
   developer verification reaches everyone. Needs the operator's own Google account.
4. **A4, chat while the app is closed.** Android stops background apps; a live connection needs a
   foreground service with a visible notification, or a push service (Google's, or an open one such
   as UnifiedPush). Pick one that keeps messages sealed end to end. Free.
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
2. **Post the licence vote?** SignPath Foundation's free certificate needs an OSI-approved licence;
   CC0 is not one. You said you will vote for plan B (AGPL-3.0 code), which qualifies.
   *Recommendation:* post the vote proposal soon; free Windows signing for direct downloads waits on
   its result.
3. **Start Android now, app pages first?** *Recommendation:* yes. It is free, it is your phone, and
   rungs A1 and A2 need nothing from anyone else.
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
