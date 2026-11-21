# MIDI keyboard input: the open Rust crates (scanned 2026-11-20)

Why: a person with a MIDI keyboard should be able to play the game's instruments live on a Music
page, on Windows, macOS and Linux. CLAUDE.md asks for the open alternatives to be compared before a
tool is adopted. The voice engine already takes notes in MIDI terms: a `NoteOn` "carries the
instrument index, a role, a MIDI note as `f32`" ([`docs/design/score-player.md`](../../../design/score-player.md)),
so a keyboard's note number and velocity map straight in. What is missing is the part that opens the
keyboard and receives its bytes.

**Nothing was added or downloaded.** Adopting any crate below is a crate download, which needs the
operator's yes first (the project asks before every download).

How this was read, on 20 November 2026 (Pacific; 02:47 on 21 November UTC): each crate's newest
version, licence field, publisher and release date from crates.io's public API
(`https://crates.io/api/v1/crates/<name>`, one request every 1.5 seconds; crates.io's robots.txt
allows everything); repository facts, licence files and manifests from GitHub's REST API
(`api.github.com`, no robots.txt). The licence quoted first is the crates.io `license` field of the
newest version; the second is the first lines of the repository's own licence file.

## The candidates

| Crate | Role | Licence, quoted | Publisher | Pure Rust or binds | Platforms | Newest release |
|---|---|---|---|---|---|---|
| **midir** 0.11.1 | Opens MIDI ports, receives and sends | crates.io `"MIT"`; `LICENSE`: "The MIT License (MIT)", "Copyright (c) 2015 Patrick Reisert and the RtMidi contributors" | Boddlnagg (Patrick Reisert) | Rust code over each system's MIDI API | Windows (WinMM; WinRT as a feature), macOS and iOS (CoreMIDI), Linux (ALSA; JACK as a feature), Web MIDI, Android API 29+ | 2026-10-10 |
| **wmidi** 4.0.11 | Decodes and encodes MIDI messages | crates.io `"MIT"`; `LICENSE` opens "Permission is hereby granted, free of charge, to any" (the MIT text) | wmedrano, RustAudio organisation | Pure Rust, no I/O | Any | 2026-04-19 |
| **midly** 0.5.3 | Decodes live MIDI events and `.mid` files | crates.io `"Unlicense"`; `LICENSE`: "This is free and unencumbered software released into the public domain." | kovaxis | Pure Rust, no I/O, optional `no_std` | Any | 2023-01-01 |
| **midi-msg** 0.9.0 | Typed MIDI 1.0 messages, SysEx, extensions | crates.io `"MIT"`; `Cargo.toml` `license = "MIT"`; **no licence file in the repository** | AlexCharlton | Pure Rust, no I/O | Any | 2026-05-08 |
| **coremidi** 0.9.2 | Apple CoreMIDI bindings | crates.io `"MIT"`; `LICENSE`: "The MIT License (MIT)", "Copyright (c) 2016 Christian Perez-Llamas" | chris-zen | Binds CoreMIDI | macOS, iOS | 2026-08-02 |
| **alsa** 0.12.1 | ALSA bindings (sound and the MIDI sequencer) | crates.io `"Apache-2.0/MIT"`; files `LICENSE-APACHE` and `LICENSE-MIT` | diwic | Binds libasound | Linux | 2026-07-31 |
| **jack** 0.14.0 | JACK audio and MIDI | crates.io `"MIT"`; `LICENSE`: "The MIT License (MIT)", "Copyright (c) 2016 Will S Medrano" | wmedrano, RustAudio organisation | Binds the JACK library; a JACK server must be running | Wherever JACK is installed | 2026-11-07 |
| **portmidi** 0.3.0 | PortMidi bindings | crates.io `"MIT OR Apache-2.0"`; file `LICENSE-APACHE` | musitdev | Binds the C PortMidi library, which must be present | Windows, macOS, Linux | 2023-07-13 |
| **midi-control** 0.2.3 | "Communicate with MIDI controllers" | crates.io `"LGPL-3.0-or-later"` (repository on GitLab, not read) | hfiguiere | not examined | not examined | 2025-07-31 |
| **oxisound-midi** 0.2.1 | "MIDI device I/O implementation for OxiSound, backed by midir" | crates.io `"Apache-2.0"` | cool-japan | A wrapper over midir | as midir | 2026-08-06 (first published 2026-06-01; 186 downloads) |

Repository activity (GitHub `pushed_at`): midir 2026-11-14, wmidi 2026-04-19, midly 2024-07-03 (the
repository moved from negamartin/midly to kovaxis/midly), midi-msg 2026-05-08, coremidi 2026-09-13,
alsa-rs 2026-10-14, rust-jack 2026-11-07, portmidi-rs 2023-07-13. None is archived. crates.io total
downloads: midir 1,214,621; midly 648,984; wmidi 176,599; midi-msg 51,579.

## Notes on each

- **midir** is the only maintained crate that does the whole job on every desktop system. Its README:
  "Cross-platform, realtime MIDI processing in Rust", "inspired by [RtMidi]", "supports the same
  features*, including virtual ports (except on Windows) and full SysEx support", and it lists ALSA
  (Linux), WinMM (Windows), CoreMIDI (macOS, iOS), WinRT (Windows 8+, "enable the `winrt` feature"),
  Jack ("enable the `jack` feature"), "Web MIDI (Chrome, Opera, perhaps others browsers)" and "Android
  (API 29+, NDK AMidi + JNI)". It hands the program raw message bytes: "A higher-level API for parsing
  and assembling MIDI messages might be added in the future." Its `Cargo.toml` has `default = []` (no
  features on by default) and picks its backend by target: `windows` (">=0.59.0,<=0.62", features
  `Win32_Media_Multimedia` and three others) and `parking_lot` on Windows, `alsa` (">=0.9.0, <0.13")
  and `libc` on Linux, `coremidi` ("0.9.0") on macOS and iOS, `web-sys` MIDI features on WebAssembly,
  `jni`, `ndk-context` and `jni-min-helper` on Android. It is the binding layer the next three crates
  provide, assembled for you.
- **coremidi, alsa, jack** are what midir sits on (JACK only with its feature). Using them directly
  means writing three platform backends ourselves; nothing is gained.
- **portmidi** binds a C library that would have to be built or shipped beside the exe on every
  platform, and its Rust crate has not been released since 2023.
- **wmidi, midly, midi-msg** decode the bytes midir delivers. midly's README: "Supports both `.mid`
  files and real-time MIDI packets.", "just load your data and call `Smf::parse` or
  `LiveEvent::parse`", "Optional `no_std` and no `alloc` support." Its default features are `["alloc",
  "std", "parallel"]`, where `parallel` is multithreaded file parsing. wmidi decodes live messages
  only ("Midi encoding and decoding library."). midi-msg is the most complete (SysEx and the
  specification's extensions), but its repository carries no licence text: the MIT licence's own
  condition is that its copyright and permission notice travel with the code, and there is no notice
  to carry.
- **midi-control** is copyleft (LGPL-3.0-or-later), which is compatible with the project's AGPL-3.0
  but heavier than MIT for no gain. **oxisound-midi** is a five-month-old wrapper over midir.
- **The web mirror needs no crate.** A browser page can open a keyboard through the Web MIDI API in
  plain JavaScript; midir's own WebAssembly backend uses the same API.

## What adopting midir would download

From this worktree's `Cargo.lock` (2026-11-20): `windows` 0.62.2, `parking_lot` 0.12.5, `log`
0.4.34, `alsa` 0.11.0, `libc` 0.2.191, `block2` 0.6.2, `core-foundation` 0.10.1,
`core-foundation-sys` 0.8.7, `jni` 0.22.4, `ndk-context` 0.1.1 and `cc` 1.10.0 are already there,
each inside midir's accepted range where it states one. So a Windows or Linux build would download
**midir alone**; macOS adds `coremidi` and `coremidi-sys`, Android `jni-min-helper` (cargo fetches
only what the target being built uses, but the lock file records every platform's entries). midly
would add one more crate (with `default-features = false, features = ["std"]` it needs nothing else;
`parallel` would use `rayon`, which the lock already has at 1.12.0). The exact list is what a
`cargo tree` shows after the yes.

## Recommendation

**midir for input, midly to decode.** midir is the only maintained, MIT-licensed crate that opens a
keyboard on Windows, macOS and Linux (and the web and Android, later) through each system's own MIDI
service, so nothing has to be installed beside the exe, and on Windows and Linux it is the only new
download. midly turns its bytes into note on and off, velocity, sustain pedal and pitch bend
(`LiveEvent::parse`), and the same crate reads `.mid` files, which the public-domain scores finding
names as a fallback source of notes (it lists Mutopia's `.mid` files for Für Elise and Symphony
No. 5). Its only dependency is the optional `rayon`. Its licence
is a public-domain dedication, it is pure Rust, and the MIDI 1.0 byte format it reads does not
change, so a quiet repository is not a risk. If only live input is ever wanted, wmidi (MIT, released
2026-04-19) is the equal alternative to midly; a first rung could even decode the handful of
channel messages a keyboard sends by hand and download midir alone.

Fit with the house rules (judgement):

- Keep the device code in the native app behind the `native` feature, so the relay build never
  compiles it, and hand the music crate plain note events (`NoteOn` with the key number as the MIDI
  note and the velocity mapped to `vel`), keeping the pure-Rust music crate free of system bindings.
- The Music page lists the connected keyboards (re-listed when the page opens and on a Refresh
  button), shows which one is playing, and says plainly when none is found, per the hardware-notice
  rule (for example "No MIDI keyboard found. Plug one in and press Refresh."), with the words added
  through `data/i18n/en.json` like every other.
- Notes from the keyboard go through `tuning::note_hz(midi, a4)` like every other note, so the A =
  432 Hz or 440 Hz choice applies to live playing too.

## Reading MIDI files (Standard MIDI Files)

Added later on 20 November 2026. Why: a person should be able to import a `.mid` file and hear it on
the game's own instruments, and Mutopia's `.mid` files are a fallback source of notes. What rights a
MIDI file itself can carry (a sequencer's own, even for Beethoven) is a legal question, answered in
[`../../findings/2026-11-20-orchestra-broadcast-rights.md`](../../findings/2026-11-20-orchestra-broadcast-rights.md),
section F; this section is only about the code that reads the format.

**Nothing was added or downloaded here either.** Adopting any crate below is a crate download, which
needs the operator's yes first.

How this was read: crates.io's search API for "midi file", "smf" and "standard midi" (sorted by
downloads), then each candidate's `https://crates.io/api/v1/crates/<name>` and its newest version's
dependency list, one request every 1.5 seconds (crates.io's robots.txt read again first: "User-agent:
*" with an empty "Disallow:"); repository facts, licence files and READMEs from GitHub's REST API
(`api.github.com/robots.txt` answers HTTP 404, so nothing is disallowed); midly's documentation on
docs.rs (its robots.txt disallows only `/releases` and three URL patterns). Repositories on Codeberg
and on a personal server were not read.

| Crate | What it does | Licence, quoted | Publisher | Pure Rust; normal dependencies | Newest release | Repository (GitHub `pushed_at`) | Downloads |
|---|---|---|---|---|---|---|---|
| **midly** 0.5.3 | Parses and writes `.mid` files and live events | crates.io `"Unlicense"`; `LICENSE`: "This is free and unencumbered software released into the public domain." | kovaxis (Martín Andrighetti) | Yes; only `rayon` ^1, optional | 2023-01-01 | 2024-07-03; 202 stars, 5 open issues | 648,984 |
| **nodi** 1.0.3 | "playback and abstraction of MIDI files", on top of midly | crates.io `"MIT"`; `LICENSE`: "Copyright (c) 2022 Taylan Gökkaya", then the MIT text | insomnimus | Yes; `midly` ^0.5.3, `midir` ^0.10.1 optional | 2025-01-01 | 2025-01-01; 30 stars | 65,753 |
| **ghakuf** 0.5.6 | "parsing/building SMF (Standard MIDI File)", by handler callbacks | crates.io `"MIT/Apache-2.0"`; `LICENSE-APACHE`: "Apache License", "Version 2.0, January 2004" | acknak | Yes; `byteorder` ^1, `log` ^0.4 | 2020-10-04 | 2021-01-09; 36 stars | 38,869 |
| **augmented-midi** 1.8.0 | "Experimental MIDI file/event parser using nom combinators" | crates.io `"MIT"`; repository `LICENSE`: "Copyright (c) 2022 Pedro Tacla Yamada", "The MIT License (MIT)" (GitHub reports the monorepo's licence as NOASSERTION) | yamadapc (Pedro Tacla Yamada) | Yes; `nom` ^7.1.0, `cookie-factory` ^0.3, `audio-processor-traits` ^4.3.0 | 2024-08-26 | 2026-06-24 (a monorepo of audio crates); 38 open issues | 25,534 |
| **apres** 0.3.4 | "MIDI Library" | crates.io `"GPL-2.0-or-later"` (repository on a personal server, not read) | quintinfsmith (Quintin Smith) | Yes; none | 2022-08-04 | not read | 24,432 |
| **rimd** 0.0.1 | "reading and writing Standard Midi Files" | crates.io `"MIT"`; `LICENSE`: "The MIT License (MIT)", "Copyright (c) 2015 Nick Lanham" | RustAudio organisation (crates.io records no publisher) | Yes; `byteorder` ^0.4.2, `enum_primitive` ^0.1.0, `num` ^0.1.27 | 2016-02-09, its only release | 2021-04-14; 7 open issues | 17,324 |
| **midi_file** 0.2.0 | "For reading and writing MIDI files." | crates.io `"MIT"`; `LICENSE`: "Copyright 2021 Matthew James Briggs", then the MIT text | webern (crates.io records no publisher for 0.2.0) | Yes; none | 2026-07-19 | 2026-07-19; 16 stars | 12,982 |
| **midi-reader-writer** 0.1.3 | "Facilitate reading and writing midi files." (adapters over midly) | crates.io `"MIT OR Apache-2.0"` (Codeberg, not read) | PieterPenninckx | Yes; `midly` ^0.5.2, `itertools`, `timestamp-stretcher`, all optional | 2025-05-12 | not read | 8,905 |
| **bmidi** 0.0.3 | "Midi parsing and processing library" | crates.io `"LGPL-3.0"`; `LICENSE`: "GNU LESSER GENERAL PUBLIC LICENSE", "Version 3, 29 June 2007" | filmor (Benedikt Reinartz) | Yes; `pitch_calc` ^0.12.0 | 2022-02-07 | 2025-03-10; 3 stars | 6,974 |
| **lyrica** 0.2.1 | "Phantasmically simple MIDI file handling" | crates.io `"MIT OR Apache-2.0"` (Codeberg, not read) | PatchMixolydic | Yes; `midir` ^0.9.1, `midly` ^0.5.3, `thiserror` ^1.0.31 | 2023-07-10 | not read | 5,196 |

(`midi-file` on crates.io is the same crate as `midi_file`: crates.io treats `-` and `_` alike.) None
of the GitHub repositories is archived. Every crate here is pure Rust; none binds a C library.

Notes on each:

- **midly** is the only complete, widely used reader. README: "Very complete, supports reading and
  writing while handling all edge cases in the MIDI spec." Its documentation: by default the parser
  will "attempt to plow through non-standard and even obviously corrupted files," discarding what it
  cannot read, and "By enabling the `strict` feature the parser will reject uncompliant data and do
  additional checking." `parallel` (on by default) "enables the use of multiple threads when parsing
  large midi files"; without `alloc` the `Smf` type is unavailable. Its README's benchmark (the
  author's own, which it calls "only referential") shows rimd 0.0.1 failing on a `.rmi` file ("Error")
  and freezing on a damaged one ("Freeze"), and nom-midi 0.5.1 and augmented-midi 1.3.0 failing on
  both ("Error"), where midly reads all five test files. The last release is from 2023, but the
  Standard MIDI File format it reads is a settled part of MIDI 1.0 (general knowledge), so a quiet
  repository is no risk.
- **nodi** is a playback layer over midly: "This crate works alongside the [midly][] and [midir][]
  crates." Its features: "Time-map MIDI events.", "Join or merge multiple MIDI tracks.", "Play MIDI
  files.", "Transpose a track.", with a default `hybrid-sleep` timer. The game does not need its
  player: the voice engine schedules notes itself. The one useful part, merging tracks and converting
  ticks to seconds through the tempo map, is a small function we would write anyway.
- **ghakuf** works but has not been released since 2020; its callback-style reader suits streaming
  more than loading a whole piece.
- **rimd** has had one release (2016) on dependencies of 2015 vintage and fails on difficult files
  (midly's benchmark). Not a candidate.
- **augmented-midi** calls itself "Experimental" and pulls in a parser-combinator library and an
  audio-processing trait crate for a job that needs neither.
- **midi_file** is MIT, dependency-free and released this year, but its README says its purpose "is
  primarily to be able to author and write MIDI files in Rust. The library also parses MIDI files and
  thus can "round trip" files." It has 16 stars and a small user base; reading other people's
  imperfect files is not its focus.
- **midi-reader-writer** and **lyrica** are wrappers over midly; **apres** (GPL-2.0-or-later) and
  **bmidi** (LGPL-3.0) are copyleft, heavier than midly's public-domain dedication for no gain.

**The specification.** The MIDI Association's page (https://midi.org/standard-midi-files-specification;
robots.txt read first, it disallows `/midi/` and account pages, not this one) lists
"RP-001_v1-0_Standard_MIDI_Files_Specification_96-1-4.pdf" behind a "Join Us to Download" button that
leads to a sign-up page; the page does not say whether the account is free. Creating an account needs
the operator's yes, so the specification text was not read. What follows about the format is general
knowledge, not a reading of it.

**Writing our own reader instead.** The project wrote its own MEI reader
([`crates/humanity-music/src/mei.rs`](../../../../crates/humanity-music/src/mei.rs), 1,741 lines), and
a Standard MIDI File reader is smaller: a header chunk (`MThd`: format 0, 1 or 2, the track count, and
the time division, ticks per quarter note or SMPTE frames), track chunks (`MTrk`), delta times as
variable-length quantities, running status (a channel message may leave out a repeated status byte),
SysEx events, and meta events for tempo, time signature, key signature, track name and end of track.
For formats 0 and 1 that is a few hundred lines with tests (judgement). The work is in the edge
cases real files contain: wrong chunk lengths, a missing end-of-track, running status around meta and
SysEx events, a note-on with velocity 0 used as a note-off, the same key struck again before it was
released, RIFF-wrapped `.rmi` files, extra unknown chunks. Those are exactly the files on which the
smaller crates fail in midly's benchmark.

**Recommendation: adopt midly for files as well as for keyboard bytes**, with `default-features =
false, features = ["alloc", "std"]` (dropping `parallel` and its threads; the game reads small files
and keeps its own thread budget). It is public domain, pure Rust, has no required dependency, reads
the damaged and non-standard files that every alternative above fails on in its benchmark, and it is
the same crate this note already recommends for decoding a keyboard's bytes, so one approved download
covers both jobs. Writing our own makes sense only if the operator declines that download; it would
then be a lenient format 0 and 1 reader, tested against a set of awkward files. The part no crate
does for us, and which we write either way, is the step from MIDI events to the score player's notes:
merge the tracks and convert ticks to seconds through the tempo map, map program changes to the game's
instruments through the General MIDI table the score player already plans as data
([`docs/design/score-player.md`](../../../design/score-player.md), the `instrDef` hint), send channel
10 to the drum kit, carry velocity, the sustain pedal and pitch bend, and play every pitch through
`tuning::note_hz(midi, a4)` so the 432 or 440 choice applies to imported files too.

## Still unknown

1. **Whether Windows lets two programs open the same keyboard at once** through WinMM (a person's DAW
   and the game together), and whether midir's WinRT feature behaves differently. Not checked; settled
   by trying it with a real keyboard.
2. **Latency from key to sound** through midir into the voice engine. Settled by a measurement once
   wired up.
3. **midi-control's repository and whether it builds on midir.** Not read (GitLab); not needed.
4. **How midly handles the particular files we would import** (Mutopia's LilyPond-made `.mid`, a
   person's own files from a keyboard or a DAW). Settled by a test over a small set of files once the
   crate is approved.
5. **The Standard MIDI File specification's own text.** Behind the MIDI Association's sign-up (not
   created, it needs the operator's yes); needed only if we write our own reader.
6. **midi-reader-writer's, lyrica's and apres's repositories** (Codeberg and a personal server). Not
   read; not needed while midly is the choice.

All licences above were read on 2026-11-20 from crates.io's API and the repositories' own files, as
quoted; licences change, so re-read the newest version's licence field at the moment of adoption.
