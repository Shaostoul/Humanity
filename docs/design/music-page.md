# The Music page: explore, tune, keep, take anywhere, see

> Written 2026-11-13. The whole feature, planned once, built in rungs (CLAUDE.md, "Plan the whole
> feature, build it in rungs"). The engine it steers is [music.md](music.md) (the live score, genres
> as data, pieces by seed in section 7c, the budget in section 14); the crate's card is
> [humanity-music.md](../crates/humanity-music.md). Rung 1 is built; section 13 says what it is.

## 0. In plain words

The game composes its own music while you play: a genre's rules plus a number, the SEED, make a
piece, and a new seed makes a new piece. The Music page is where a person goes looking through
those pieces on purpose: pick a genre and a mood, step through seeds (next, previous, random, or
typed), hear each one, read what it is in words ("D harmonic minor, AABA, 128 beats a minute,
clarinet leading, trombone answering"), nudge it (another key, a little faster, a different lead
voice, the bass louder, the drums off), and KEEP the ones you love as favourites with your own
names. A favourite can become "my music" for a mood (calm, danger, the menu), be shared with a
friend as a short code, or be saved as an audio file for the car or the phone. While it plays you
can SEE it: a visualizer that pulses with the beat and the bar and shows each part moving, and a
piano roll with the chord names and the section letters.

It is a generative studio, not a digital audio workstation: the engine composes, the person
steers, keeps and exports (section 10).

## 1. Why, in the operator's words

The operator, 2026-11-13, verbatim: "We need to add a way in the settings menu or like a dedicated
music page to try infinite seeds and variations and genres and what not. That way we or a random
person can find the exact seeds we want using the genre we want and all that and set some as
permanent. Like maybe seed X has a particular sound I love and want to combat or just to listen to
in my real-life car. We'll want to add all the features we can for like visualizing and tuning the
music. Like it'd be cool if we could see the song in some sort of music sheet kind of thing and have
a visualizer that moves around to the beat."

"to combat" is read as "for combat": a favourite can be assigned to a mood (calm, danger and
combat, the menu) or to a place.

The same evening he asked whether this is becoming a DAW. The answer, section 10.

## 2. What exists (read 2026-11-13)

- The score (`crates/humanity-music`): a composer that improvises within a mood and a genre, the
  voices, the reverb and limiter; renders into a plain buffer, so it runs offline and in tests.
- Pieces by seed (`piece.rs`, music.md 7c): a seed draws the key, the tempo, the form, each
  section's chords, the riffs and the lead voices from the genre's data.
- Genres as data (`data/music/genres/*.ron`): mothership calm, lo-fi, electro swing; a community
  genre is a file.
- Settings > Music (`src/gui/pages/settings_music.rs`): music on, on the pages, A = 432 or 440, the
  genre and a blend, places that pick a genre, "A new piece" every N minutes.
- The Dev page's Music card (`src/gui/pages/dev_music.rs`): the danger knob, the seed, the section,
  the cost per block. `HUMANITY_MUSIC_SEED` fixes a session's seed.
- The offline renderer: `just music-render <mood> <genre> <seconds> --seed N [--stems]`.
- The runtime (`src/engine/music.rs`): builds a score on its own thread and lets a new one take
  over at a bar or a phrase, crossfading.

There was no Music page, no way to keep a seed, and nothing to look at.

## 3. The rules this follows

- **Native first, the website mirrors it** (CLAUDE.md, "Rust-first canonical UI"; "Web MIRRORS
  native"). The page is built in egui; the web page mirrors what it can (section 11).
- **Every word by key, every control with a hint that says what it changes** (CLAUDE.md, "One word
  source"; "Every graphics setting says what it actually changes" holds for sound too).
- **Theme tokens only; every Area through `src/gui/layers.rs`.**
- **Infinite-of-X.** Genres, moods, lead voices, roles and keys come from the music's data; the
  page hardcodes no list of them. The moods a favourite can be assigned to are the mood files.
- **Nothing kept across runs except the person's own data** (CLAUDE.md, "No caches kept across
  runs"). The favourites file is the person's data, like a save, not a cache. In-memory reuse (the
  piano roll's transcript) is tagged by what it was made from and obeys `humanity_core::reuse::enabled()`.
- **The budget** (music.md section 14): the page adds nothing to the audio thread unless it is open,
  and then a few microseconds a block (section 12).
- **Every new tool is compared before it is adopted** (CLAUDE.md): compressed export formats wait
  for a written scan with licences quoted (section 8).
- **AI is optional**: nothing here uses a model; the engine is rules and a seed.
- **No health claim** about tuning or tones (music.md section 15's words only).

## 4. Where the page sits

`GuiPage::Music` (page id `music`), in the Apps group of the navigation (`data/gui/navigation.json`)
beside Notes, Calendar and the Calculator: it is a tool a person uses for their own life (the music
for the car) as much as for the game. Its web mirror is `/music`.

The page is one scroll with five parts, in the order a person uses them: **Now playing** (the
visualizer, what the piece is in words, Play here), **Explore**, **Tune**, **Keep** and **See**
(the piano roll). On a narrow window they stack; on a wide one the visualizer and the piano roll
take the width.

**Play here.** While the page is open and "Play here" is on (the default), the game's music plays
the page's choice: its genre, mood, seed and tuning. Leaving the page, the music goes back to what
it was doing (the person's genre, new pieces, or their "my music" favourites). This uses the one
score the game already runs (no second audio stream, no second budget): the runtime takes the
page's choice as its next piece and lets it take over at the next bar, crossfading.

## 5. Explore

- **Genre**: the genres from the data (community genres included), each with its one sentence.
- **Mood**: the moods from the data (mothership calm, garden morning, night watch, the menu,
  tension, action). A mood that an intensity band names (tension, action) is heard as danger: the
  page sets the intensity into that band, so the stinger, the riser and the danger layers play as
  they would in the game.
- **Seed**: Previous, Next, Random, and a box to type one. The seed is a plain number; Previous and
  Next step by one.
- **What it is, in words**: the key and its mode ("D harmonic minor"), the form ("AABA"), the
  tempo ("128 beats a minute"), the lead voice and the answering voice, the section playing now
  and the chord. These come from the playing score and from a dry run of the composer (section 9).

Rung 3 adds a **history** (the last 50 seeds heard, to go back to one) and **seed ranges to
sweep** (play 20 seconds of each seed in a range, star the good ones as they pass).

## 6. Tune

What a person can change about a piece without changing which piece it is. Each control says what
it changes, in words, and each is saved with a favourite.

| Control | What it changes | Where it acts | Rung |
|---|---|---|---|
| Key | the key's root note, the mode kept (a transposition: a bridge that moves key moves with it) | the composer, from the next phrase | 1 |
| Tempo | beats a minute, within the genre's range (a genre played at one tempo, electro swing's 128, may move 10% either way) | the composer, from the next phrase | 1 |
| Lead voice | which of the genre's lead voices plays the tune and which answers | the composer, from the next phrase | 1 |
| Each part's level | louder or softer, -24 to +6 dB, per role (pads, bass, drone, choir, bells, melody, drums, texture) | the score, at once | 1 |
| Mute | silences a part; the piece is still composed the same (its other parts do not change) | the score, at once | 1 |
| Solo | hear only the soloed parts | the score, at once | 2 |
| Swing | how far the off-beats lean late (straight to heavy) | the composer, from the next phrase | 2 |
| A = 432 or 440 | the tuning (Settings > Music's choice, shown here too) | the score | 2 |
| Intensity | how much danger, for the danger moods | the composer, at the next bar | 2 |
| Density, register, dynamics | how busy the lead is, how high it sits, how much it swells | the composer | 3 |

Changing the key, the tempo or a lead voice starts the same seed again with the change, at the next
bar; a level or a mute is heard at once. A part that the genre does not play shows greyed with the
reason ("lo-fi has no choir").

## 7. Keep

**Favourites.** A favourite is a name, the genre, the mood, the seed and the tuning. Saved in the
person's data folder as `music/favourites.json` beside `config.json` (`%APPDATA%\HumanityOS\` on
Windows; the `HUMANITY_DATA_DIR` and portable rules apply), written to a temporary file and renamed
so a crash never tears it. It is the person's own data: never a cache, never sent anywhere. Genres,
moods and lead voices are saved by their ids, not by list positions, so a favourite still plays the
same after a genre file gains a voice. A favourite whose genre is missing (a community genre
removed) says so and keeps its entry.

**"My music" for a mood.** Each mood (the mood files) can have a favourite assigned: when the game's
mood becomes that mood (a place's calm, the danger bands' tension and action, the menu), the score
plays that favourite instead of new pieces. In danger the favourite's genre, seed and tuning play in
the danger mood the game is in, so the stinger and the danger layers still come. Rung 1 assigns to
moods; rung 3 assigns to a place (a room, a zone) the same way, and a PLAYLIST of favourites (played
in order or shuffled, each for "A new piece" minutes) can stand where new seeds would.

**Share codes.** A favourite as a short code a friend can paste into their own Music page:
`HOSM1-` and Crockford base32 (no I, L, O or U, case and dashes ignored) of the genre id, the mood
id, the seed, the tuning and the name, with a checksum so a mistyped code is refused rather than
playing something else. The code carries no account and no address. A code from a genre the friend
does not have says which genre it needs.

## 8. Take it anywhere

**Export a favourite as an audio file** of a chosen length (1, 3, 5, 10, 30 or 60 minutes), ending
in a fade, for a car, a phone or a music player. Rung 1 writes WAV (16-bit, 48 kHz, stereo: every
player and car reads it; about 11 MB a minute). The export renders on its own thread with the same
engine and the same recordings the game plays, faster than real time, into the person's Music
folder (`Music\HumanityOS\` in their home folder), and the page shows where it went with a button
to open the folder. The export is the offline renderer's output for the same seed, byte for byte
(a test holds it).

**Compressed formats** (rung 2, after a scan): Opus, Ogg Vorbis, FLAC and MP3 are the candidates.
CLAUDE.md, "Every new tool gets compared before it is adopted": a written scan in
`docs/reference/research/<date>-audio-export-formats/` quoting each encoder crate's licence, its
publisher, whether it is pure Rust, what a car stereo plays (MP3 everywhere; FLAC and Ogg on many;
Opus on few) and any patent or licence question as a dated finding. MP3's patents expired in 2017
(that is a claim to verify from a primary source in the finding, not from memory).

**The bridge to real DAWs** (rung 2): a favourite as a **standard MIDI file** (each role a track,
the tempo and the key signature in it, the section letters as markers, the chord symbols as text
events) and as **stems** (one WAV per role, the `--stems` renderer already exists). A person who
wants to edit by hand opens them in LMMS, Ardour or Reaper (section 10).

## 9. See it

**The visualizer** (rung 1): a beat pulse (a ring that swells on each beat, the bar's first beat
the strongest), the bar and the beat counted, each role's activity as a bar that jumps when the role
plays a note and falls away, and a spectrum (ten octave bands, 31 Hz to 16 kHz). The beat and the
roles come from the score itself, not from guessing at the sound; the spectrum is measured on the
audio thread only while the page is open (section 12). Rung 3: a full-screen mode, and the
visualizer as a screen in the world (an in-world monitor showing it).

**The piano roll** (rung 1): the piece's notes as bars on a pitch grid (time across, pitch up), each
role in its colour (theme tokens), the chord symbols above each chord ("Dm6", "A7", "Bb7") and the
section letters at each section's start ("A", "A", "B", "A"), with a playhead following the music.
It is made by running the composer DRY (no sound) over the same seed, genre, mood and tuning the
score plays, which composes an hour in well under a second; the notes in the roll are the events the
score plays (a test holds it for a bar).

**A lead sheet** (rung 4): the tune on a real staff with the chord symbols over it, the way a jazz
musician reads a song: the key signature, the time signature, the notes spelled in the key (a
sharp or a flat by the key's side), rests, ties across bar lines, swung eighths written straight
with the swing marked. Built from published engraving rules (Elaine Gould, "Behind Bars", for
spacing, beaming and spelling; the SMuFL font standard for the glyphs, with a font whose licence
is quoted in a finding), not from an engraver's code. Rung 5: the full score (each role on its own
staff) and printing.

## 10. Is this a DAW?

No: it is a generative studio. A digital audio workstation records and arranges sound a person
makes, note by note and take by take. Here the engine composes, and the person steers (genre,
mood, seed, tuning), keeps (favourites, my music), and exports (audio, and in rung 2 MIDI and
stems). That keeps the page usable by someone who has never written music, which is the point.

The bridge to real DAWs is the export: a favourite as a standard MIDI file and as stems opens in
LMMS, Ardour or Reaper, where a person can do anything. We do not rebuild those tools.

**Editing what the engine made** (rung 6, DEFERRED BY THE OPERATOR'S DECISION, 2026-11-13): lock a
section or a role and regenerate the rest ("keep this bass, try other tunes over it"); edit notes
in the piano roll; write or hum your own motif for the engine to develop. These move the page
toward a DAW. Asked, the operator answered: "I'm okay with keeping it simple while we develop more
important things ... I'm okay with exporting and editing in another app until we're much further
along in development." So editing is not built: rung 2's MIDI and stems export is the way to edit,
in another app. The data shapes leave room for it (a favourite is a seed and a tuning, so a later
"edits" field can sit beside them) for when he raises it again.

## 11. The web mirror

The website cannot run the Rust engine today, so rung 1's `/music` page is the page's shell: what
the Music page is and does, said plainly, the favourites a person has pasted as share codes (kept
in the browser, `hos_music_favourites`), each with its genre, mood, seed and code to copy, and the
reason it cannot play them yet: the music is composed live by the app's own engine, which runs in
the desktop app. That reason is a choice of order, not a law, a patent or a platform rule.

Rung 5: **the music crate compiled to WebAssembly** (music.md already plans the crate to compile to
wasm; it is pure and allocation-free on its audio path), played through an AudioWorklet, so the web
page explores, plays, sees and exports like the app, with the synthesised voices; the recorded
instruments as an optional download.

## 12. Performance

| Where | Cost | Basis |
|---|---|---|
| Audio thread | the score as it is (music.md section 14's 0.05 ms a block for the calm score); with the page open, the analyser adds the spectrum's ten band filters and the role meters, measured in rung 1 (section 13) | the page is the only reader, so the analyser runs only while it is open (`MusicContext::analyse`) |
| Game thread | the dry composer run for the piano roll once per choice (tens of milliseconds for minutes of music, rebuilt only when the choice changes, every frame for the visible bars with "turn off all reuse" off); the visualizer draws a few dozen shapes | in-memory reuse tagged by the choice and the reuse switch |
| Export | its own thread, faster than real time (a 3-minute file in a few seconds) | music.md's offline renderer |
| Disk | the favourites file (kilobytes); exported audio where the person asked for it | nothing kept across runs except the person's own data |
| Network | none; a share code travels however the person sends it | music.md: each game composes its own music |

## 13. The rungs

**Rung 1 (built 2026-11-13).** The page in the Apps group: explore (genre, mood, seed with
previous, next, random and typed; the piece in words); tune (key, tempo, lead and answer voices,
each role's level and mute); keep (favourites saved to and loaded from the person's data folder,
renamed and removed, assigned to a mood, share codes in and out); export to WAV; the beat
visualizer with role meters and a spectrum; the piano roll with chord symbols and section letters;
the web shell with the favourites list. Tests: a favourite saved and loaded plays the same piece
(an event-sequence hash); a share code round-trips and a mistyped one is refused; the exported WAV of
seed S is the offline render of seed S; a favourite assigned to danger plays when the mood turns to
danger; the piano roll's notes are the piece's events for a bar.

**Rung 2.** MIDI export (a track per role, tempo, key, markers, chord text); stems export (a WAV per
role); compressed formats after the written scan; solo; swing; the intensity slider; A = 432/440 on
the page.

**Rung 3.** History and seed sweeps; playlists of favourites; my music for a place; density,
register and dynamics; a full-screen visualizer and the visualizer on an in-world screen.

**Rung 4.** The lead sheet: real staff notation of the tune with chord symbols, from published
engraving rules.

**Rung 5.** The music crate as WebAssembly: the web page plays, sees and exports; the full score and
printing.

**Rung 6 (deferred by the operator, 2026-11-13: "I'm okay with exporting and editing in another
app until we're much further along in development").** Editing what the engine made: lock a section
or a role and regenerate the rest, edit notes in the piano roll, write or hum a motif for the engine
to develop (section 10). Not built; rung 2's MIDI and stems export carries a piece to another app.

## 14. Questions for the operator, each with a recommendation

1. **Tempo for a one-tempo genre.** Electro swing is written at 128 and only 128. Recommendation: let
   the page move it 10% either way (115 to 141) and say so; a genre file may later name its own
   tuning range.
2. **Where exports go.** Recommendation: the Music folder in your home folder, `Music\HumanityOS\`,
   with an "Open folder" button, until a save dialog exists in the app.
3. **Rung 6 (editing): answered 2026-11-13, deferred.** "I'm okay with keeping it simple while we
   develop more important things ... I'm okay with exporting and editing in another app until we're
   much further along in development." Rung 2's MIDI and stems export is the way to edit.

## 15. As built: rung 1 (2026-11-13)

**The engine's half** (`crates/humanity-music`):
- `tune.rs`: the `Tune` a `MusicContext` carries (and the composer's `Steer`). The composer reads
  the transposition (every key the piece plays, its bridge's included), the tempo (`Tune::tempo_for`,
  inside `Tune::tempo_range`: the genre's range, or 10% either way of a one-tempo genre), the swing
  and the chosen lead entries when it plans a phrase; the draws the seed makes are taken whatever the
  person chose, so a tempo change leaves the rest of the piece alone. The score's sink applies each
  role's gain (level, mute, solo) as notes reach the voices: a muted role's notes never start a
  voice, and the composer composes them all the same.
- `sheet.rs`: `transcribe` runs a `Composer` through the score's own loop (128-frame blocks, chunks
  of at most 64 samples cut at each decision), so each note is stamped with the sample the score
  starts it on: notes with lengths, chords (the `Event::Chord` now carries its root) and their
  symbols (`chord_symbol`, spelled in the key: `key_uses_flats`), sections with key, mode and tempo,
  bar lines, the lead picks (`Composer::lead_picks`), and an FNV hash of every note-on and note-off
  with its sample. `Sheet::bar_notes` is the piano roll's bar.
- `favourite.rs`: `Favourite`, `SavedTune` (by ids), `Favourites` (load; save through a `.tmp` and a
  rename; an unreadable file moved aside to `favourites.unreadable.json`), `explore_intensity`, and
  share codes: the seed as a seven-bits-a-byte number, the tuning, the genre, mood and name (24 bytes
  at most), a 16-bit checksum, Crockford base32. About 90 characters for a favourite with a short
  name and no changes, about 120 with every part of the tuning changed.
- `render.rs`: `export_favourite` renders and writes as it goes (an hour would be 1.4 GB held in
  memory) through `WavWriter`; the test holds it byte for byte to the buffered `write_wav` path that
  `just music-render` takes, and `just music-render x x <seconds> --favourite <code>` renders a
  favourite through the export itself.
- `score.rs`: the analyser (ten octave band-pass filters on the output and a meter per role) runs
  only with `MusicContext::analyse`; the status carries the beat position. Measured: about 0.007 ms
  a block, under the run-to-run noise (`the_pages_analyser_is_within_the_budget`).

**The runtime** (`src/engine/music.rs`): `choice_now` is the Music page's choice while it is open
with Play here on, else the favourite that is the music for `mood_now` (the mood an intensity band
names when the danger intensity is in it, else the place's). A choice decides the genre built, the
seed, the mood and intensity (the page's) and the tuning; a change of choice (`Take::Choice`) takes
over at the next bar line, like a genre change; a chosen piece is never replaced by "A new piece".
The runtime keeps a copy of the playing score's `Resolved` (now `Clone`: the recordings are shared)
and its starting context, and reports them in `MusicReadout::playing`. The page's export runs on a
`music-export` thread that reads the data and decodes the genre's recordings itself.

**The page** (`src/gui/pages/music/`): `mod.rs` (state, the favourites file, `choice`, words),
`controls.rs` (Explore, Tune, Keep), `view.rs` (Now playing with the visualizer, See with the piano
roll; the transcription kept in memory tagged by the piece and how far it reaches, made again every
frame with "turn off all reuse" off). The part colours are theme tokens (`music_role_*`, editable in
Settings > Appearance). The web mirror is `web/pages/music.html` and `music-app.js`.

**Known limits, for rung 2:** with "turn off all reuse" off the runtime rebuilds the score every
phrase and resumes it mid-piece; the resumed composer is not the one that started, so the piano roll
(written from the start) can drift from what is heard until the piece changes. Solo, swing and the
intensity are in the `Tune` and the share code but have no control on the page yet.
