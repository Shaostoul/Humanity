# A steel pan and deep drums for the free instrument library: which recordings could ship

**Research date: 6 November 2026** (Pacific time). Every page below was read that day, by an AI
research pass working for the instrument-library build; nothing was downloaded. Each source carries
its own date where it shows one.

**I am not a lawyer; this is a reading of public pages, not legal advice.**

Builds on [`2026-11-06-music-sources.md`](2026-11-06-music-sources.md) (the four libraries the
operator approved) and serves [`docs/design/music.md`](../../design/music.md) rung 6 and
[`docs/reference/instrument-sources.md`](../instrument-sources.md).

## The question

The operator asked, on 2026-11-06, "Could we deep drums and steel drums": which recordings of a steel
pan and of deep drums (taiko, surdo, large bass and frame drums) could ship inside HumanityOS, a CC0
app anyone may redistribute commercially?

## Short answer

- **Deep drums: already in hand.** The approved libraries hold concert bass drums, timpani, a frame
  drum, toms (VCSL) and a large hand drum (VSCO 2 CE's VSCO 1 percussion); the percussion pack is
  built from them. The deepest further CC0 drums found are **Karoryfer's Big Rusty Drums** (a 22"
  bass drum set up as a floor tom and played with mallets, an 18" tom, a 24" kick), close in feel to
  a taiko. **Not downloaded: it needs the operator's yes.**
- **No CC0 or public-domain recordings of a real taiko were found anywhere.** The only real taiko
  hits found are single Freesound files under CC BY (a credit line required).
- **Steel pan: yes, one.** Jeff Learman's chromatic **C tenor steel pan**, two octaves (C4 to B5),
  in two versions: version 2 under the Unlicense, version 1 under an explicit CC0 statement. Also
  FreePats' CC0 **Hang in D minor**, a nine-note handpan (a relative, not a steel pan). **Neither
  downloaded: each needs the operator's yes.**

## Findings

### F1. jSteelDrum version 2 (Jeff Learman): the Unlicense

- Source: https://github.com/sfzinstruments/jlearman.SteelDrum (last commit 2026-10-01; no
  releases; the "Download ZIP" link is
  https://github.com/sfzinstruments/jlearman.SteelDrum/archive/refs/heads/main.zip).
- Licence file (GitHub identifies it as the Unlicense), opening: "This is free and unencumbered
  software released into the public domain."
- README: "24 notes sampled, C4 (middle C) to B5", "5 velocity layers, with velocity crossfades",
  "Stereo 16 bit, 44.1 KHz samples", "44 MB, in FLAC format". Recorded "in September 2021", of a pan
  "hand-made in Trinidad".
- Files: 368 FLAC files, 38,469,730 bytes, in `flac/`; programs `jSteelDrum.sfz` and
  `jSteelDrum-noxfade.sfz`. The upper octave reuses version 1's recordings (same names and sizes).

### F2. jSteelDrum version 1 (Jeff Learman): CC0

- Source: https://github.com/jlearman/jSteelDrum1 (last commit 2023-09-26; ZIP link
  https://github.com/jlearman/jSteelDrum1/archive/refs/heads/main.zip).
- Licence file, opening: "This is free and unencumbered software released into the public domain
  according to Creative Commons CC0".
- README: "27 notes sampled, C4 (middle C) to B5, plus C#6, D#6, and F6", "5 velocity layers".
  443 FLAC files, 44,140,751 bytes, in `jSteelDrum-flac-sfz/`.

### F3. FreePats Hang in D minor: CC0

- Source: https://freepats.zenvoid.org/ChromaticPercussion/hang.html ("Version 2022-03-30");
  repository https://github.com/freepats/hang-D-minor. The SFZ+FLAC download as shown, 13 MiB:
  https://github.com/freepats/hang-D-minor/releases/download/2022-03-30/Hang-D-minor-SFZ+FLAC-20220330.7z
- Licence: "Published under the terms of the Creative Commons CC0 1.0 public domain dedication."
- Nine notes (A3, D4, E4, F4, G4, A4, Bb4, C5, D5), 51 FLAC files (13.7 MB). The page warns the bank
  "only responds to notes produced by the original instrument".

### F4. Karoryfer Big Rusty Drums: CC0

- Sources: https://shop.karoryfer.com/pages/free-big-rusty-drums (footer "© 2026"; no date on the
  entry) and https://github.com/sfzinstruments/karoryfer.big-rusty-drums (release v1.100,
  2023-04-29). Download as shown:
  https://github.com/sfzinstruments/karoryfer.big-rusty-drums/releases/download/v1.100/Big_Rusty_Drums_1100.zip
  (619,600,370 bytes per GitHub).
- Licence: the free-samples page, "All our free sample libraries are under a Creative Commons Zero
  license."; the repository's LICENSE is the CC0 1.0 legal code.
- What matters here: the 22"x16" tom (a bass drum set up horizontally) with mallet hits, 292 FLAC
  files in `Samples/tom_22`; the 18"x16" tom with mallets (400 FLAC); a 24"x18" kick (280 FLAC). The
  page: "Over 4400 samples adding up to 2.3 GB."

### F5. Freesound "Taikodrum (41 hz).wav" by tarane468: CC BY 4.0

- Source: https://freesound.org/people/tarane468/sounds/584131/ (August 20th, 2021). Licence shown:
  "Attribution 4.0". One WAV, 4.4 s, 44.1 kHz; "This one is a big one with a very low sound."
- One sample only (no layers, no round robins), and a credit line would be required.

### F6. Rejected

- Kastendieck "Steel Drum" (https://github.com/sfzinstruments/Kastendieck.SteelDrum): no licence
  file and none stated in the README. The only lower-range (double-second) pan found; worth asking
  its author.
- SASS-E steelpan dataset (https://zenodo.org/records/7803316, "Published April 5, 2023"):
  "Creative Commons Attribution Non Commercial 4.0 International". NonCommercial cannot enter a CC0
  app.
- Pianobook "LOW Explosive Taiko Drums": its terms forbid "resale or other distribution of the
  Products or their derivatives".
- Several Freesound "taiko" files marked CC0 are not taiko recordings (layered synths, slowed toms).

## What is still unknown, and what would settle it

1. Whether the Unlicense wording ("software") covers jSteelDrum version 2's recordings the way CC0
   would: one message to Jeff Learman asking him to add version 1's CC0 line to version 2. Costs one
   message; until then, version 1 (explicit CC0) is the safe choice.
2. Whether Big Rusty Drums' mallet tom reads as a taiko: listen to it (needs the download).
3. Kastendieck's steel drum licence: ask its author or S. Christian Collins, who converted it.

## Sources not reached

- S. Christian Collins' "SCC Taiko Drums" page: a script-only site the reader could not read
  (wanted: its licence).

## What it means for this project

### Certain

- Deep drums ship today in the percussion pack, from the approved libraries.
- jSteelDrum version 1, the FreePats Hang and Big Rusty Drums are CC0 by their own words and could
  ship inside the app with no credit required.
- No public-domain recording of a real taiko was found.

### Judgement calls (an AI's reading; reject freely)

- **Recommended next download: jSteelDrum version 1** (CC0, 44 MB) for a "steel pan" part, then the
  FreePats Hang as a second steel voice, then Big Rusty Drums' 22" and 18" mallet toms for a deeper,
  taiko-like drum. Each is one recipe in `scripts/instrument-recipes.json` once downloaded.
- A taiko from tarane468's single CC BY hit is possible with a credit line in `CREDITS.md`, but one
  sample makes a thin instrument; the mallet toms are the better route.
