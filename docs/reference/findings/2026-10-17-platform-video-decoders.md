# What does each operating system give an app, with nothing extra installed, to play an ordinary MP4?

**Research date: 17 October 2026.** Every source below was read on that date. Each carries its
own date beside it where it shows one (Apple's reference pages and GStreamer's module pages show
none). If you are reading this later, compare those dates with whatever has changed since.

**I am not a lawyer. This is a reading of public sources, not legal advice.** It is mostly a
technical finding. It extends [2026-10-17-windows-media-foundation-decoders.md](2026-10-17-windows-media-foundation-decoders.md)
(Windows) to the other platforms, and leaves the legal reading in
[2026-09-19-video-codec-licensing.md](2026-09-19-video-codec-licensing.md) where it was: the
project ships no H.264 or AAC decoder of its own and calls the one the person's own system already
has, because a patent licence bought by us would not pass to forks and mirrors.

## The question

The operator, after the Windows finding: "How do we incorporate access via android, iOS, Mac,
Linux automatically? Not everyone wants to install Microsoft Store."

So: on macOS, iOS and iPadOS, Android and Linux, what does the operating system itself provide,
with no extra install, for an app to decode an ordinary MP4 (H.264 video with AAC sound), and
H.265 where it matters?

## The short answer

**First, the Microsoft Store worry is smaller than it sounds.** On Windows the Store is needed only
for H.265. Ordinary MP4 (H.264 and AAC) plays through Windows' own Media Foundation with nothing
installed, on Windows 7 and later (the Windows finding).

| Platform | H.264 + AAC, nothing installed | H.265 | What an app calls |
|---|---|---|---|
| Windows | Yes, Windows 7 and later (not "N" editions) | Only with Microsoft's Store extension | Media Foundation |
| macOS | Yes | Yes, macOS 10.13 High Sierra and later | AVFoundation, with VideoToolbox and AudioToolbox beneath it |
| iOS and iPadOS | Yes | Yes, iOS 11 and later | AVFoundation |
| Android | Yes on every phone and tablet that meets Google's compatibility definition: H.264 and AAC decoding are a MUST | Yes on phones and tablets: also a MUST | MediaExtractor and MediaCodec (from native code: AMediaExtractor, AMediaCodec, Android 5.0 and later) |
| Linux | **Not guaranteed.** Depends on the distribution and on what the person chose to install | Same, and less often present | Whatever is there: the person's own ffmpeg libraries, or GStreamer |
| Web browsers | Yes in Chrome, Edge and Safari everywhere; Firefox uses the system's decoders, which on Linux means the system's ffmpeg | Varies by browser and platform | The browser itself |

**Apple and Android: the decoders are part of the system, no install, nothing to buy.** On Apple
systems H.265 arrived with iOS 11 and macOS High Sierra (2017). On Android, Google's Compatibility
Definition Document requires every compatible phone and tablet to decode H.264, H.265 and AAC and
to make those decoders available to third-party apps.

**Linux is the one platform where "automatically" cannot be promised.** There is no single system
decoder. The two distributions researched here, Fedora and Ubuntu, leave H.264 and H.265 out or
make them an opt-in, for patent reasons, and Mesa's own default build leaves them out of its
graphics drivers. Fedora brings in Cisco's OpenH264 for H.264 after its first online update, does not carry
H.265, and switched H.264 and H.265 off in its graphics drivers in 2022, with a Red Hat engineer
saying Fedora will not provide an operating-system interface that exposes patented algorithms.
Ubuntu leaves them on in its graphics drivers but puts the software decoders behind a tick-box in
the installer. What an app can count on is only what that particular person installed.

**One surprise worth flagging.** Cisco's own documents disagree about what OpenH264 can decode: its
README says Constrained Baseline only, its release notes say Main and High Profile decoding was
added in 2019 (Finding 11). The September finding relied on the README. A cheap test would settle
it, and it matters for Fedora.

**Is this the same legal posture as Windows?** In shape, yes on Apple and Android: the system
vendor or the device maker supplies the decoder, and the app ships none. No vendor read today says
in plain words that developers may call them. On Linux the shape differs: there is no vendor behind
the decoder, only whoever the person installed it from. Details under "The legal posture,
platform by platform" below. Not legal advice.

---

## Finding 1: Apple says H.265 is built into its systems since 2017

Source: Apple Support, "Using HEIF or HEVC media on Apple devices",
<https://support.apple.com/en-us/116944>, published date shown on the page: 5 December 2025.

- "Support for HEIF and HEVC is built into iOS 11 and later". The same sentence names iPadOS,
  visionOS and macOS High Sierra 10.13 and later.
- Older devices can still view HEVC video, with limits: Apple says resolution and frame rate
  matter, and 1080p at 60 frames a second or less works more widely.
- Nothing is installed or bought. The article compares HEVC against H.264 as the older, more
  compatible format, which is what an iPhone records when set to its most compatible setting.

## Finding 2: The Apple frameworks an app would call

Source: Apple Developer Documentation. These reference pages carry no publication date; each lists
the first system version where the interface exists, which is what is recorded here.

- **Video Toolbox**, <https://developer.apple.com/documentation/videotoolbox>, listed from macOS
  10.8 and iOS 6.0. It gives apps "direct access to hardware encoders and decoders". Apple adds
  that apps which do not need that low level should not use it directly.
- **The software path exists.** The decoder setting
  `kVTVideoDecoderSpecification_EnableHardwareAcceleratedVideoDecoder`
  (<https://developer.apple.com/documentation/videotoolbox/kvtvideodecoderspecification_enablehardwareacceleratedvideodecoder>,
  macOS 10.9) lets an app say "To specifically prevent hardware-accelerated decoding", which only
  makes sense if Apple decodes in software otherwise. Its sibling setting, which demands hardware,
  lists why hardware may be missing: the machine lacks it, the format is not supported, or it is
  busy.
- **AVAssetReader**, <https://developer.apple.com/documentation/avfoundation/avassetreader>, listed
  from macOS 10.7 and iOS 4.1, reads "file-based media like QuickTime movies or MPEG-4 files" and can
  hand back decoded pictures and decoded sound. This is the piece our player would use: it opens
  the MP4 and decodes both tracks.
- **AAC**: the format identifier `kAudioFormatMPEG4AAC`,
  <https://developer.apple.com/documentation/coreaudiotypes/kaudioformatmpeg4aac>, listed from
  macOS 10.2 and iOS 2.0, names "the MPEG-4 AAC Low Complexity codec", the sound in an ordinary
  MP4.
- **Checking for hardware**: `VTIsHardwareDecodeSupported`
  (<https://developer.apple.com/documentation/videotoolbox/vtishardwaredecodesupported(_:)>, macOS
  10.13 and iOS 11) tells an app whether a codec has hardware decoding on this machine.

No Apple page read today gives a first version for H.264 decoding itself. My reading: it is older
than HEVC on every Apple system (Finding 1 treats H.264 as the older, compatible format), so any
Mac, iPhone or iPad that runs a current app has it.

## Finding 3: Apple's own licence carries the consumer notice for H.264

Source: Apple, "Software License Agreement for macOS Tahoe 26",
<https://www.apple.com/legal/sla/docs/macOSTahoe.pdf>, document EA1955, dated 07/11/2025 on its
last page.

- Section D, the H.264/AVC notice, licenses the AVC functionality "ONLY FOR THE PERSONAL AND
  NON-COMMERCIAL USE OF A CONSUMER", and points to MPEG LA for other uses.
- This is the pool's standard consumer notice. The September finding could only report it for
  Apple at second hand; this is Apple's own text. It limits the person using the Mac, and says
  nothing either way about a developer calling the decoder.
- The iOS licence was not read.

## Finding 4: Android requires H.264, H.265 and AAC on every compatible phone and tablet

Source: "Android 17 Compatibility Definition",
<https://source.android.com/docs/compatibility/17/android-17-cdd>, page last updated 31 August
2026. The CDD index (<https://source.android.com/docs/compatibility/cdd>, updated 17 June 2026)
marks Android 17 as current. The document states it lists what devices must meet to be compatible
with Android 17.

- Section 2.2.2 (Handheld, Multimedia): "Handheld device implementations MUST support the
  following video decoding formats", and must make them available to third-party apps. The list:
  H.264 AVC [5.3/H-0-1], H.265 HEVC [5.3/H-0-2], MPEG-4 SP, VP8, VP9 and AV1.
- The same section requires AAC LC [5.1/H-0-3], HE-AAC [5.1/H-0-4] and AAC ELD [5.1/H-0-5], plus
  Extended HE-AAC [5.1/H-0-6], new in Android 17.
- Section 2.6 says tablets follow the handheld requirements except where marked; these items are
  not marked, so tablets carry them too.
- Section 5.1.2: any device that declares audio output must decode AAC LC [C-1-1], HE-AAC,
  HE-AACv2 and AAC ELD among others.
- Section 5.3.4: a device with an H.264 decoder must support Main Profile Level 3.1 and Baseline
  Profile, and 720p and 1080p when its screen is that tall. **High Profile is not in the floor**,
  though phones and cameras often record it; the app should ask the device what it can decode.
- Section 5.3.5: a device with H.265 must support Main Profile Level 3 (standard definition), and
  at least one of H.265 or VP9 at 720p, 1080p and 4K when its screen is that tall.
- Cars are different: section 2.5 lists H.264 for automotive devices but not H.265.
- **The handheld list has not changed for years.** The same H.264 and H.265 decoding MUSTs appear
  in the Android 9, 10, 12, 13, 14, 15 and 16 CDDs, read the same day (the Android 10 page, for
  example, last updated 21 July 2026).

The CDD binds devices that want to be called Android-compatible. A device built outside that
programme is not bound by it.

## Finding 5: Android's interfaces for an app

- **Android Developers, "Supported media formats"**,
  <https://developer.android.com/media/platform/supported-formats>, last updated 7 October 2026.
  The tables describe "media format support built into the Android platform". AAC LC decoding in
  MP4; H.264 Baseline (Android 3.0 and later) and Main Profile (Android 6.0 and later, where the
  page notes the decoder is required); H.265 from Android 5.0, Main Profile Level 3 for mobile
  devices. The page adds
  that a device may support more.
- **MediaCodec**, <https://developer.android.com/reference/android/media/MediaCodec>, last updated
  3 August 2026, added in API level 16 (Android 4.1). It is used to "access low-level media codecs,
  i.e. encoder/decoder components". Its partner **MediaExtractor**
  (<https://developer.android.com/reference/android/media/MediaExtractor>, same date, API level 16)
  pulls the encoded tracks out of a file such as an MP4.
- **The same from native code**, <https://developer.android.com/ndk/reference/group/media>, last
  updated 27 August 2026: `AMediaExtractor_new` and `AMediaCodec_createDecoderByType` are available
  since API level 21 (Android 5.0). The decoder is created "specifying a mime type obtained from
  media extractor". This matters because HumanityOS is written in Rust: native code can reach the
  phone's decoders without going through Java.

## Finding 6: Linux has no system decoder; GStreamer sorts its plugins by risk

Sources: GStreamer module pages, <https://gstreamer.freedesktop.org/modules/> (no dates shown), and
the GStreamer plugin documentation, <https://gstreamer.freedesktop.org/documentation/>.

- GStreamer is a framework many Linux desktops use for media. Its decoders come in separately
  packaged sets. The "ugly" set is good code where "distributing them might pose problems", naming
  licences and known patent problems. The "bad" set is code not yet at "good" quality. "gst-libav"
  wraps ffmpeg's decoders.
- Where the H.264 and AAC decoders live: `openh264dec` (Cisco's decoder) is in the bad set;
  `avdec_h264` is in the libav package; `vah264dec`, which decodes on the graphics card through
  VA-API using whichever VA-API driver is installed, is in the bad set; `fdkaacdec` is in the bad
  set.
- So "GStreamer is installed" says nothing about whether H.264 plays. It depends on which of those
  packages the distribution or the person put on.

## Finding 7: Fedora

- **Fedora wiki, "OpenH264"**, <https://fedoraproject.org/wiki/OpenH264>, last edited 6 December
  2023. Cisco's OpenH264 binary, built by Fedora and handed out by Cisco so Cisco's licence covers
  it, gives H.264 to ffmpeg (Fedora's `ffmpeg-free` with `openh264`), GStreamer and Firefox. The
  repository is "enabled by default since Fedora 33".
- **Fedora Packages, "ffmpeg-free"**, <https://packages.fedoraproject.org/pkgs/ffmpeg/ffmpeg-free/>,
  read 17 October 2026 (Fedora 45 stable carries 9.0.2). Fedora's own ffmpeg is "limited in the
  number of codecs supported". Which codecs could not be read: the list sits in Fedora's package
  sources, behind a bot check (see "Sources I could not reach").
- **A community answer, not an official one**, on Fedora Discussion,
  <https://discussion.fedoraproject.org/t/video-codec-h-264/202444>, 19 to 21 September 2026: a fresh
  Fedora 44 install carries a placeholder package that the first online update swaps for Cisco's
  openh264; H.265 is not in Fedora at all, and other answers point people to RPM Fusion, a separate
  repository. So a Fedora machine that has never updated has no H.264 decoder.
- **Graphics-card decoding switched off.** Phoronix, "Fedora Linux Disabling Mesa's H.264 / H.265 /
  VC1 VA-API Support Over Legal Concerns", <https://www.phoronix.com/news/Fedora-Disable-Bad-VA-API>,
  28 September 2022 (secondary: the Fedora commit and mailing-list thread it quotes are behind a bot
  check). Fedora removed H.264, H.265 and VC-1 from its Mesa graphics drivers' video decoding,
  citing the lack of legal approval. It quotes Red Hat's David Airlie: Fedora will not ship
  anything that gives the operating system "an API which exposes patent algorithms". Video still
  plays on the processor where a software decoder exists.

## Finding 8: Mesa's default, and Ubuntu's opposite choice

- **Mesa 24.0.0 Release Notes**, <https://docs.mesa3d.org/relnotes/24.0.0.html>, dated 1 February
  2024: "Add all, all_free (default) options for video-codecs option". Mesa is the open-source
  graphics driver for AMD and Intel cards on Linux. The option to leave codecs out was added in Mesa
  22.2 by Red Hat's David Airlie for distributions worried about patent pools (Phoronix, "Mesa Can
  Now Be Built With Select Video Codecs Disabled For Software Patent Concerns",
  <https://www.phoronix.com/news/Mesa-Optional-Video-Codecs>, 29 April 2022). My reading of
  "all_free" is "only the codecs nobody charges for", so H.264 and H.265 are off unless a
  distribution turns them on. The upstream option text that would confirm this is behind a bot
  check.
- **Ubuntu turns them on.** Ubuntu's Mesa build rules,
  <https://git.launchpad.net/ubuntu/+source/mesa/plain/debian/rules?h=ubuntu/noble> (24.04) and the
  `ubuntu/devel` branch (Mesa 26.2.4), both pass `-Dvideo-codecs="all"`. So on Ubuntu, an AMD or
  Intel card driven by Mesa can decode H.264 and H.265 in hardware where the card supports it.

## Finding 9: Ubuntu's software decoders are an opt-in

- **Ubuntu tutorial, "Install Ubuntu desktop"**, <https://ubuntu.com/tutorials/install-ubuntu-desktop>,
  no page date, written for Ubuntu 24.04 LTS. The installer asks whether to add third-party
  software and "support for additional media formats", and the tutorial recommends ticking it.
- **The package behind it**, <https://packages.ubuntu.com/noble/ubuntu-restricted-extras>
  (version 67, in Ubuntu's "multiverse" section): "Commonly used media codecs and fonts for Ubuntu".
  It depends on `ubuntu-restricted-addons` (version 29), which recommends GStreamer's libav and
  ugly sets and its VA-API plugins, and it recommends `libavcodec-extra`.
- **Ubuntu's ffmpeg library itself**, <https://packages.ubuntu.com/noble/libavcodec60>
  (7:6.1.1-3ubuntu5, in "universe"), is described as ffmpeg's library of decoders and encoders.
  Ubuntu's build rules for it
  (<https://git.launchpad.net/ubuntu/+source/ffmpeg/plain/debian/rules?h=ubuntu/noble>) turn no
  decoder off, so where it is installed it carries ffmpeg's own H.264, H.265 and AAC decoders. That
  is my reading of the build file. Whether a default desktop install already has it, without the
  tick-box, was not checked.

## Finding 10: Flatpak's codec extension

Sources: the Freedesktop SDK repository, <https://gitlab.com/freedesktop-sdk/freedesktop-sdk>,
read 17 October 2026 (main branch, and the `release/25.08` branch), and the Flatpak command
reference, <https://docs.flatpak.org/en/latest/flatpak-command-reference.html> (no page date).

- Flatpak is a way of shipping Linux apps that brings its own shared runtime. The Freedesktop
  runtime, from its 25.08 release, has an extension called `codecs-extra`, described in its own
  metadata as holding "patent-restricted codecs not shipped in Platform". Its ffmpeg is built with
  every decoder turned on, plus x264 and x265. (The 24.08 runtime had an "ffmpeg-full" extension
  instead, which apps had to ask for.)
- The runtime's declaration of that extension does not set Flatpak's `no-autodownload` key, which
  Flatpak documents as "Whether to automatically download extensions matching this extension point".
  My reading: an app on the 25.08 runtime gets these codecs with the runtime, without asking. A news
  report agrees (Linuxiac, "Freedesktop SDK Retires Cisco's OpenH264 Codec Library",
  <https://linuxiac.com/freedesktop-sdk-retires-cisco-openh264-codec-library/>, 25 March 2025).
- This only helps an app shipped as a Flatpak, and the
  [AB 1043 finding](2026-10-10-california-ab-1043-age-signals.md) found that a Flathub listing
  would very likely bring the app inside California's age-signal law. Who distributes these codecs,
  and on what legal footing, was not researched.

## Finding 11: Cisco's two documents disagree about OpenH264's decoder

- **README**, <https://github.com/cisco/openh264> (file last changed 21 August 2024), under
  its decoder features: "Constrained Baseline Profile up to Level 5.2". This is the line the
  September finding relied on.
- **RELEASES**, <https://github.com/cisco/openh264/blob/master/RELEASES> (file last changed 12
  October 2026), under v2.0.0 (published 17 June 2019): "B-frame decoding support for Main and High
  Profile". An earlier entry (v1.5.0) had already added decoding of Constrained High Profile.
- Both are Cisco's own files and they cannot both describe today's decoder. The release notes are
  newer and more specific; the README's feature list may simply never have been updated. That is a
  judgement, and a test settles it (see below). It matters because Fedora's built-in H.264 decoding
  goes through OpenH264, and because the September finding called the Cisco route useless on the
  README's word. It does not change the September finding's point that OpenH264 has no AAC.

## Finding 12: Web browsers

- **MDN, "Web video codec guide"**,
  <https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Video_codecs>, last modified 7
  September 2026. H.264 plays in all versions of Chrome, Edge, Firefox, Opera and Safari, but
  Firefox's support is "dependent upon the operating system's built-in or preinstalled codecs". For
  H.265: Safari on macOS High Sierra and later; Chrome on every device on macOS 11 and Android 5.0,
  and only with hardware support on Windows, Linux and ChromeOS; Edge on Windows only with
  Microsoft's Store extension; Firefox on Linux from version 137 through the system ffmpeg.
- **MDN, "Web audio codec guide"**,
  <https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Audio_codecs>, last modified 13
  October 2026: "Firefox relies upon a platform's native support for AAC", and AAC is not available
  in Chromium builds.
- **The Chromium Projects, "Audio/Video"**, <https://www.chromium.org/audio-video/>, no page date.
  H.264 and AAC are listed as "Proprietary Video Codecs (Limited to Google Chrome)" and the matching
  audio heading. So Google's Chrome carries its own decoders, while plain Chromium, which many Linux
  distributions package, does not by default.
- So the website plays an ordinary MP4 in Chrome, Edge and Safari on every platform. Firefox plays
  it using the system's decoders, which on Windows, macOS and Android are always there; on Linux it
  needs the system's ffmpeg. A distribution's own Chromium may not play it.

---

## The legal posture, platform by platform

This is what the sources say and do not say. It is not legal advice.

- **Windows** (the September and Windows findings): Microsoft ships the decoders as part of
  Windows. Microsoft has not said in plain words that developers may call them; a developer who
  asked on Microsoft's own forum got no answer from Microsoft.
- **macOS, iOS and iPadOS: the same shape as Windows.** Apple ships the decoders as part of the
  system, and Apple's own licence passes the pool's consumer notice to the person using it
  (Finding 3). Nothing read today from Apple speaks to developers either way.
- **Android: the same shape, with the duty placed on the device maker.** The CDD requires the maker
  to include the decoders and to make them available to third-party apps (Finding 4). That phrase
  is the closest any vendor text read today comes to "apps may use these", but it is a
  compatibility requirement, not a statement about licences. Who holds the patent licence for a
  given phone (the maker, the chip vendor, Google) was not researched.
- **Linux: a different shape.** There is no vendor that ships a licensed decoder to everyone. On
  Fedora, H.264 comes under Cisco's licence, by download, and Red Hat has said Fedora will not give
  the system an interface exposing patented algorithms (Finding 7). Ubuntu carries ffmpeg's
  decoders and Mesa's codecs in its archive and leaves the software ones as an opt-in (Findings 8,
  9); Canonical's reasoning was not read. Whatever decoder a Linux machine has, the person or their
  distribution chose to install it. Calling it is still "use what the machine already has, ship
  nothing", the posture of [`media-stance.md`](../media-stance.md) toward the person's own ffmpeg,
  but there is no Microsoft or Apple standing behind the decoder.

## What is still unknown, and what would settle it

1. **Whether calling a system decoder needs no licence from the developer.** Unchanged from the
   September finding, now for four vendors instead of two: none of Microsoft, Apple or Google says
   it in plain words. *Settled by:* a patent lawyer's opinion, or a written statement from a
   vendor.
2. **Whether OpenH264 decodes ordinary Main and High Profile MP4s** (Finding 11). *Settled by:*
   playing a phone-recorded MP4 through OpenH264 on a Fedora machine or virtual machine (ffmpeg with
   `-c:v libopenh264`, or GStreamer's `openh264dec`). Free and quick.
3. **Which decoders Fedora's ffmpeg-free carries, AAC especially.** *Settled by:* reading the
   `enable_decoders` file in Fedora's package sources through a browser (it is behind a bot check),
   or running `ffmpeg -decoders` on a Fedora 45 machine.
4. **Whether a default Ubuntu desktop install already has ffmpeg's library or GStreamer's libav set
   without the installer's tick-box.** *Settled by:* a fresh Ubuntu 24.04 install in a virtual
   machine, then listing the installed packages.
5. **Whether Flatpak's codecs-extra really arrives with the 25.08 runtime for every app, who hosts
   it, and on what legal footing** (Finding 10). *Settled by:* installing a 25.08-runtime app on a
   clean system, and reading Flathub's and the Freedesktop SDK's own statements.
6. **Android devices outside Google's compatibility programme, and H.264 High Profile on certified
   ones** (Finding 4). *Settled by:* asking each device at run time (MediaCodecList reports what a
   decoder supports), which the app should do anyway.
7. **Apple's iOS licence** was not read; it would show whether the consumer notice in Finding 3 is
   on iPhone and iPad too. *Settled by:* reading Apple's iOS 26 licence, free.
8. **HEVC on older Apple devices**: Apple names limits (resolution, frame rate) but not which
   devices. *Settled by:* testing on the oldest Mac the app supports.

## Sources I could not reach

- `https://src.fedoraproject.org/rpms/mesa/c/94ef544b3f2125912dfbff4c6ef373fe49806b52?branch=rawhide`,
  a bot check page. Wanted: the Fedora commit that removed the codecs,
  in its own words and with its own date. Quoted here only through Phoronix.
- `https://lists.fedoraproject.org/archives/list/devel@lists.fedoraproject.org/thread/PYUYUCM3RGTTN4Q3QZIB4VUQFI77GE5X/`,
  the same bot check. Wanted: David Airlie's message in full.
- `https://docs.fedoraproject.org/en-US/quick-docs/installing-plugins-for-playing-movies-and-music/`,
  the same bot check. Wanted: Fedora's own statement of which codecs it ships by default.
- Fedora's ffmpeg package sources (`enable_decoders`), on `src.fedoraproject.org` (bot check) and on
  a staging mirror (no file served). Wanted: the decoder list for ffmpeg-free.
- `https://gitlab.freedesktop.org/mesa/mesa/-/raw/main/meson.options`, the same bot check. Wanted:
  the upstream text defining what "all_free" includes.
- `https://sources.debian.org/`, a proof-of-work challenge. Wanted: Debian's Mesa and ffmpeg build
  files, for comparison with Ubuntu's.
- `https://developer.apple.com/videos/play/wwdc2017/503/`, Apple's 2017 talk introducing HEVC; the
  page as served has no transcript. Wanted: Apple's description of software HEVC decoding on older
  Macs.
- `https://help.ubuntu.com/community/RestrictedFormats`, now an archive notice. Wanted: Ubuntu's own
  explanation of why these formats are restricted.
- MDN's audio codec guide promises a list of when Firefox gained AAC on each platform; the list is
  empty in the page as served.

## What this means for HumanityOS

### Certain

- **The desktop app is built for Windows, Linux (x86_64) and macOS (Apple silicon and Intel)**,
  per `.github/workflows/build-desktop.yml`. There is no Android or iOS app; phones use the website.
- **The Microsoft Store is needed only for H.265 on Windows.** An ordinary MP4 needs nothing
  installed on Windows, macOS, iOS, iPadOS or a compatible Android device.
- **Linux is the one platform where the app cannot promise playback.** Whether a decoder exists
  depends on the distribution and on the person's own choices.
- **The website already covers almost everyone**: Chrome, Edge and Safari play MP4 everywhere.
  The gaps are Firefox on a Linux machine without a capable ffmpeg, and some distributions' own
  Chromium.
- Per the standing rule on saying what we will not build and why, every "cannot play" sentence
  names a **PATENT** limit, not a fault in the app.

### Judgement calls, mine, and arguable

Which decoder the app's player would call on each platform, and the sentence it shows when none is
available:

| Platform | The player calls | When there is no decoder, the app says |
|---|---|---|
| Windows | Media Foundation's Source Reader (the Windows finding) | "This copy of Windows has no video decoders. Microsoft's free Media Feature Pack adds them." For H.265: "This video is H.265. Windows plays it only with Microsoft's HEVC extension, sold separately because of patent licensing." |
| macOS | AVFoundation's AVAssetReader (decoded pictures and sound; VideoToolbox and AudioToolbox do the work beneath it) | For H.265 on an older Mac: "This video is H.265, which needs macOS 10.13 or later." |
| iOS and iPadOS (if an app is ever built) | AVFoundation, the same way | Same as macOS, for iOS 11. |
| Android (if an app is ever built) | AMediaExtractor and AMediaCodec from Rust, after asking the device what it can decode | "This device has no decoder for this video's format. Its maker decides which formats it carries." |
| Linux | The person's own ffmpeg libraries, loaded at run time if present (Firefox's approach). Until that is built, the existing route: convert once with the person's own ffmpeg program | "This video uses H.264, and this computer has no decoder for it. Many Linux distributions leave H.264 out because of patents; your distribution's multimedia codecs add it." |
| Website | The browser | The chat card's existing note, plus on Linux: "Firefox on Linux plays this using your system's ffmpeg; Google Chrome plays it as is." |

- **Prefer ffmpeg's libraries over GStreamer on Linux.** The app already relies on the person's
  ffmpeg for the convert-once route, the decoders are the same, and one Linux path is less to
  maintain than two. GStreamer would be a second route worth adding only if testing shows many
  machines have its decoders but no ffmpeg.
- **Do not ship as a Flatpak just to get codecs.** It would bring H.264, H.265 and AAC
  automatically, but it adds a distribution channel, likely brings California's age-signal law in,
  and leaves the legal footing of those codecs unresearched.
- **Test OpenH264 before repeating the September finding's verdict on it.** If it decodes ordinary
  phone video, Fedora machines decode H.264 out of the box after their first update, and the
  Linux sentence above can be softer for them.

*Written 17 October 2026. I am not a lawyer; this is a reading of public sources, not legal advice.
If this document is more than a year old when you read it, re-check the Android CDD's handheld
codec list, Fedora's and Ubuntu's codec defaults, and the OpenH264 profile question.*
