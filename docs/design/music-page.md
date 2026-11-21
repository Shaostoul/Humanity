# The Music page: listen, explore, written music, your music, the orchestra

> Version 1 written 2026-11-13 (explore, tune, keep, take anywhere, see; its rung 1 built that day).
> **Version 2 written 2026-11-20**, after the operator looked at the page on his 65-inch screen and
> asked for a condensed layout, the whole piece in the piano roll, written music as a searchable
> list, a plainer Explore, a Like button and an orchestra (section 1). The whole feature is planned
> once and built in rungs (CLAUDE.md, "Plan the whole feature, build it in rungs"); section 20 lists
> them. The engine it steers is [music.md](music.md) (the live score, genres as data, pieces by seed
> in section 7c, the budget in section 14), the written pieces are [score-player.md](score-player.md),
> and the crate's card is [humanity-music.md](../crates/humanity-music.md).

## 0. In plain words

The game composes its own music while you play: a genre's rules plus a number, the SEED, make a
piece, and a new seed makes a new piece. It also plays WRITTEN music: real scores (Beethoven, Grieg)
kept as files, played on the game's own instruments. The Music page is where a person chooses what
plays, everywhere in the app and the world, until they choose again.

Version 2 lays it out like a music player on a wide screen. Along the top, always in view, a strip
says what plays now and holds the buttons a person reaches for most: pause, the previous and the
next piece, and **Like**, which keeps the piece in your favourites in one click. Below it, five tabs:
**Listen** (what plays, the piano roll and the mixer), **Explore** (a card for each genre and each
mood that says plainly what it is, and the pieces you heard), **Written music** (a list of every
written piece you can search and sort, with where each came from), **Your music** (your favourites,
the music you picked for each mood, share codes and audio files) and, in a later rung, the
**Orchestra** (a stage you lay out by dragging instruments and microphones). On a wide screen the
piano roll stays docked along the bottom on every tab, and a slider says how much of the piece it
shows, from a few bars to the whole thing.

It is a generative studio, not a digital audio workstation: the engine composes, the person
steers, keeps and exports (section 13).

## 1. Why, in the operator's words

**2026-11-13**, verbatim: "We need to add a way in the settings menu or like a dedicated music page
to try infinite seeds and variations and genres and what not. That way we or a random person can
find the exact seeds we want using the genre we want and all that and set some as permanent. Like
maybe seed X has a particular sound I love and want to combat or just to listen to in my real-life
car. We'll want to add all the features we can for like visualizing and tuning the music. Like it'd
be cool if we could see the song in some sort of music sheet kind of thing and have a visualizer
that moves around to the beat." ("to combat" is read as "for combat": a favourite can be the music
for a mood.)

**2026-11-20**, looking at the page, verbatim: "I feel like we need to improve the music page GUI.
Like, I love the bars view at the bottom but, could we draw the whole song or variable? Like a slider
or something choosing visualized length. Also, we have a lot of real estate for the UI. We could
condense things. Incorporate the infinite of X rule. Like the written music we want to make like a
searchable list for infinite songs, including the little details, like the origin/source. We need to
find some way to make the explore section more intuitive. What each style means and mood is kind of
confusing. We should add a button to save a liked seed. Also, we could have like a drag and drop
section for like all the places in the orchestra. The instruments themselves AND their placement
relative to mics so we could have actual 2D representations of the pins or something. This would
essentially set us up for in-game orchestras with NPCs or players playing the instruments. It'd be
cool to broadcast real orchestras in-game in real stadiums/concert halls/locations. What else could
we do to improve this UX?"

What his screenshots showed (2560x1440 on a 65-inch television at about 4 feet): one long scroll
whose panels used the left quarter of the width; Now playing large at the top; the piano roll
("See") at the very bottom; keeping a favourite meant typing a name in a box far down the page;
Explore was rows of radio buttons with genre and mood names and long paragraphs.

His closing question ("What else could we do?") is answered by the proposals marked **(proposed)**
below, each in a rung, each for him to strike.

## 2. What exists (read 2026-11-20)

- **Version 1, rung 1 (built 2026-11-13, section 22):** the page in the Apps group; What plays (let
  the game choose, or the person's choice); Explore (genre, mood and seed as radios and buttons);
  Tune (key, tempo, lead voices, each part's level and mute); Keep (a named favourite, "my music"
  for a mood, share codes, export to WAV); the visualizer; the piano roll of four bars around the
  playhead with chord symbols and section letters.
- **One place chooses (2026-11-14):** the choice lives in the person's settings
  (`MusicSettings`), read by the score, this page and Settings > Music (section 4).
- **Written pieces (score-player.md, 2026-11-20):** `data/music/scores/index.ron` lists four MEI
  files (Ode to Joy, Für Elise, In the Hall of the Mountain King, the first movement of Beethoven's
  Fifth), each with its edition, its source, its licence and a credit; the page lists them with Play
  and Stop, and the piano roll draws the one playing from its own timeline.
- **The data the page reads:** genres (`data/music/genres/*.ron`, eight files, each with a name and
  an "about" sentence by key), moods (`data/music/moods/*.ron`, six files, a name by key),
  "A new piece" choices (`data/music/pieces.ron`).

## 3. The rules this follows

- **Native first, the website mirrors it** (CLAUDE.md, "Rust-first canonical UI", "Web MIRRORS
  native"), in the same rung (section 19).
- **Every word by key, every control with a hint that says what it changes** (CLAUDE.md, "One word
  source"); theme tokens only; every Area and Window through `src/gui/layers.rs`.
- **Infinite-of-X** ([infinite-of-x.md](infinite-of-x.md)). Genres, moods, written pieces,
  ensembles, seatings, rooms and microphone arrays are data files; the page hardcodes no list of
  any of them. A new genre file gets its card, a new score its row, a new ensemble its stage,
  with no code change (each held by a test).
- **Nothing kept across runs except the person's own data** (CLAUDE.md, "No caches kept across
  runs"). Favourites, saved stage layouts and (rung 2) the history are the person's data, like a
  save. In-memory reuse (the piano roll's transcript, the catalogue's sorted order) is tagged by what
  it was made from and obeys `humanity_core::reuse::enabled()`.
- **The budget** (music.md section 14): the page adds nothing to the audio thread unless it is open;
  cosmetic audio (the orchestra's placement, reverb) is computed on each client, never networked.
- **Every new tool is compared before it is adopted** (CLAUDE.md): the MIDI keyboard crate has its
  scan ([research/2026-11-20-midi-input](../reference/research/2026-11-20-midi-input/README.md)).
- **Legal questions are researched and dated first** (CLAUDE.md): broadcasting real orchestras has
  its finding ([2026-11-20-orchestra-broadcast-rights.md](../reference/findings/2026-11-20-orchestra-broadcast-rights.md)).
- **Say what we will not build, and why** (CLAUDE.md, 2026-09-18): where a recording or a stream is
  out of reach because of a law or a platform holder's rule, the page says which, on the screen
  where a person meets it.
- **AI is optional**: nothing here uses a model; the engine is rules and a seed.

## 4. Where the page sits, and one place chooses

`GuiPage::Music` (page id `music`), in the Apps group of the navigation; its web mirror is `/music`.

**The Music page is THE place that chooses what plays (2026-11-14).** The operator: "When I go to the
settings it plays one music and when I go to the music page it goes to another. We should have one
place ideally or at least have the synced so users don't get different music on different pages."
The choice is one state in the person's settings (config.json's `music` object,
`humanity_music::MusicSettings`: `genre`, `game_chooses`, `mood`, `seed`, `tune`, `written`), read
by the score, by this page and by Settings > Music, and it keeps playing after the page is left.
**Let the game choose** (the default) plays new pieces of the person's genre in the mood of the
place and the moment, a mood's "my music" where one is picked, a new piece every "A new piece"
minutes; **Play my choice** plays the genre, mood, seed and tuning chosen here. Choosing a mood, a
seed, any change of tuning, opening a favourite or pasting a code picks the person's choice by
itself. Settings > Music keeps only how the music sounds and says, in the same words as this page,
what plays now (`music::now_playing_words`, one function for both). There is one genre control in
the app, on this page (`there_is_one_genre_control_and_no_settings_page_draws_one`).

## 5. The layout (version 2)

```
+--------------------------------------------------------------------------------------------+
| Music                                                                                      |
| [Pause] [<< Previous] [Next >>]  Now playing: Electro swing, seed 4242, your choice ...    |
|                                  [Electro swing] [Mothership calm] [seed 4242]   [ Like ]  |
| [Listen] [Explore] [Written music] [Your music] [Orchestra (later)]                        |
+------------------------------------------+-------------------------------------------------+
| What plays           | the piece in words, the visualizer                                  |
| Tune: key, tempo,    |                                                                     |
| lead voices, parts   |                                                                     |
+------------------------------------------+-------------------------------------------------+
| Piano roll   Show: [-----o---------] 16 bars   [x] Follow the music                         |
| overview: |||||||[====window====]||||||||||||||||||||||||||||||||||||||||||||||||||||||     |
| the bars in the window, chord symbols, section letters, bar numbers, the playhead          |
+--------------------------------------------------------------------------------------------+
```

- **The Now playing strip** (always visible, section 6) sits under the title.
- **The tabs** sit under the strip, each a page of the page. The tab chosen is kept in memory for
  the session (a tab is not a choice worth a file).
- **Wide** (the page at least 1100 points wide, the operator's 2560 at 100% scaling): the tab's
  content in columns (Listen: the controls in a column about a third of the width, the piece in
  words and the visualizer beside them; Written music: the list and its detail panel side by side),
  and the piano roll docked along the bottom on every tab, about a quarter of the height (a person
  exploring sees the notes of what they just chose).
- **Narrow** (a small window, the phone with `HUMANITY_PHONE_PREVIEW=1`, about 412 points): one
  column; the tabs wrap; the strip wraps its buttons onto two lines; the piano roll moves into the
  Listen tab, below the visualizer.
- Sizes are for a reader at 4 feet from a 65-inch screen at 2560x1440: the theme's body size, no
  smaller text than the theme's small size, buttons at the theme's control height.

## 6. The Now playing strip and Like

From left to right: **Pause / Play** (the music on or off everywhere: `MusicSettings::music_on`,
the same switch as Settings > Music's), **Previous** and **Next** (for a composed piece the seed
before and after; for a written piece the one before and after it in the Written music list as
sorted and filtered), the sentence that says what plays and who chose it (`now_playing_words`), then
chips for the genre, the mood and the seed (each opens its tab), and **Like**.

**Like** keeps the piece shown, with every change made to it (key, tempo, lead voices, each part's
level, mute and solo), as a favourite, in one click. Its name is made for you ("Electro swing,
seed 4242", words by key) and can be changed later on Your music, where the list shows a Rename
button. Nothing leaves the computer: it is the same `music/favourites.json` the Keep button wrote.
When the piece shown is already a favourite (same genre, mood, seed and tuning), the button says
**Liked**; clicking it opens Your music with that favourite marked, never removing it (an unlike
by accident would lose a name the person gave it). For a written piece the strip shows Like greyed,
with the hint that written pieces join favourites with playlists (rung 4).

## 7. The piano roll (version 2)

- **How much it shows:** a slider from 2 bars to the whole piece (2, 4, 8, 16, 32, 64 ... all), the
  number of bars written beside it. The bars shown carry their numbers (bar 1 is the first), so a
  person can say "listen at bar 40".
- **The overview:** a thin strip above the bars drawing the whole piece at once (each part's notes as
  marks in its colour, the section letters along it), the bars shown marked as a window, the playhead
  as a line. Clicking or dragging on the overview moves the window (and turns following off).
- **Follow the music:** on (the default), the window keeps the playhead in its first quarter, as
  version 1 did; off, it stays where the person put it.
- **Section letters and chord symbols** stay above the bars, as in version 1.
- **The whole piece:** a written piece is its file, drawn whole from its timeline. A composed piece
  is computed from its seed (the composer run dry, deterministic, music.md section 7c): a piece with
  a set length ("A new piece" every 4 minutes: the length is the time until the next piece, as the
  score reports it) is transcribed to its end and drawn whole; an endless piece (**Never**, or the
  person's own choice, which "A new piece" never replaces) shows a window: the transcription reaches
  40 seconds past the playhead, made again further on as the playhead nears its end, up to 20
  minutes, and the overview says it is the part written so far.
- **Clicking a bar seeks there** for a written piece (rung 2: the written player gains a seek to a
  bar line, sent through `MusicChoice` like a choice; a composed piece cannot be sought into the
  middle without composing the bars before it, which the score does in under a second for minutes of
  music, so rung 3 offers it for composed pieces too).
- **Cost:** the dry run composes minutes of music in tens of milliseconds, once per piece (made again
  every frame with "turn off all reuse" off); drawing is one rectangle per note in view, and the
  overview at most one mark per pixel column per part (a 20-minute piece at 2000 pixels is 16,000
  marks at most, merged per column).

## 8. The mixer (rung 2)

The Tune card's part levels become **strips**, one per part (pads, bass, drone, choir, bells, melody,
drums, texture; the roles from `tune::ROLE_IDS`): the part's colour and name, a level fader (-24 to
+6 dB), **Mute**, **Solo** (the `Tune::solo` bits the engine already obeys: with any part soloed only
the soloed parts sound), and a level meter fed by the score's role meters (`MusicStatus::roles`,
measured on the audio thread only while the page is open). A part the genre does not play is
greyed with the reason. Strips are compact (a column 64 points wide each on a wide screen; a row
each on a narrow one).

## 9. Explore made plain

- **A card for each genre** (`data/music/genres/*.ron`): its name, ONE plain sentence of what it
  is for a person who knows no music words ("Relaxed beats for studying or unwinding, like a rainy
  afternoon"; the file's `plain_words`, by key, or `plain` for a community genre), its tempo range
  in beats a minute, and the instruments it plays (from the file's leads, palette and added parts,
  by their words). The chosen genre's card is marked; clicking a card chooses the genre. The longer
  "about" sentence shows only on the chosen card.
- **A card for each mood** (`data/music/moods/*.ron`): its name and WHERE it plays, in a plain
  sentence ("Aboard the ship on an ordinary day"; the file's `where_words`, by key, or `where_plain`),
  and its tempo. Danger moods (an intensity band names them) say so.
- **The seed**: Previous, Next, Random and a box to type one, as in version 1.
- **The pieces you heard**: a list of the last 50 pieces that played this session (genre, mood,
  seed, a written piece's title, how long ago), newest first, each with Play (make it the choice
  again) and Like. Rung 1 keeps it in memory (gone when the app closes); rung 2 keeps it in the
  person's data folder beside the favourites, with a Clear button.
- **An audition** (rung 2, proposed): a card's Listen plays 20 seconds of that genre or mood, then
  what played before comes back, without changing the person's choice.
- **Seed sweeps** (rung 4): play 20 seconds of each seed in a range, Like the good ones as they pass.

## 10. Written music as a catalogue

Every written piece is a row in a list a person can **search** (title, composer, edition, source,
instrumentation: any word), **sort** (title, composer, era, year, length) and **filter** (by
composer, by era). Choosing a row opens its **detail panel** (beside the list on a wide screen,
under it on a narrow one): the title and the composer with their years, the era, the year written,
the key, the length, the instruments (from each staff's part), the edition it follows, its source
with a link, the typesetter, the licence, the credit, and the findings behind its tempo; with Play
or Stop.

Everything comes from data: `data/music/scores/index.ron` (one entry per piece, the fields above,
words by key where a person reads them) beside each MEI file. Adding a score is adding its file and
its entry; a test holds every shipped entry's `length_s` to the length of the piece its file plays.
Rung 4 lists an MEI file dropped in without an entry from its own header (`titleStmt`), played on
the piano part until an entry names its instruments, and keeps the list quick at thousands of pieces
(rows drawn only where they are on screen; the search over words held in memory, rebuilt when the
data is read again).

## 11. Your music

The favourites (each with its name, genre, mood and seed; **Open**, **Rename**, **Copy code**,
**Export**, **Remove**), "my music" for each mood, a pasted share code, and where exports go:
version 1's Keep, with the name box gone (Like names a favourite for you; Rename changes it).
Rung 4 adds playlists of favourites and written pieces (in order or shuffled, each for "A new
piece" minutes) and my music for a place (a room, a zone).

## 12. Take it anywhere

- **Audio** (built): a favourite as a WAV file of 1 to 60 minutes, rendered on its own thread
  (version 1, section 22). Compressed formats wait for a written scan of the encoders with their
  licences quoted.
- **MEI and MIDI** (rung 2): a composed piece written out as MEI 5.1 (the open format the written
  pieces already use: our own writer beside the crate's reader, so MuseScore, Verovio and other
  open tools open it) and as a standard MIDI file (each part a track, the tempo, the key signature,
  the section letters as markers and the chord symbols as text events; the file format is a short
  published specification, so our own writer, no crate). Both are written from the same sheet the
  piano roll draws, so the file holds the notes the person saw.

## 13. Is this a DAW?

No: a generative studio. A digital audio workstation records and arranges sound a person makes;
here the engine composes and the person steers, keeps and exports. The bridge to real DAWs is the
export (MEI, MIDI, stems), opened in LMMS, Ardour, MuseScore or Reaper. **Editing what the engine
made** (lock a section, edit notes, hum a motif) stays deferred by the operator's decision of
2026-11-13: "I'm okay with exporting and editing in another app until we're much further along in
development."

## 14. The orchestra (rung 3, then the world in rung 5)

A 2D **stage plot**, seen from above: the stage, each instrument as a **pin** (a seat: an
instrument part and a position in metres), the **microphones**, and the **listener**. A person drags
pins, microphones and the listener; what they hear follows.

- **As data** (infinite-of-X): `data/music/ensembles/*.ron` (string quartet, piano trio, chamber
  orchestra, symphony orchestra: each a list of seats, each seat a part id from `parts.ron` and a
  default position), `data/music/seatings/*.ron` (the usual plans: the American seating with the
  cellos on the right, the German or antiphonal seating with the first and second violins facing
  each other, the quartet's two layouts), `data/music/rooms/*.ron` (studio, chamber hall, concert
  hall, cathedral, stadium: size, reverberation time, early reflections, air absorption) and
  `data/music/mics/*.ron` (a spaced pair, an ORTF pair, a Decca tree, a close mic per section).
- **What a position changes, per part:** pan from the angle between the part and the listener (or
  the microphone pair's axis), level by distance (inverse distance, held near the critical distance
  of the room so a far part is quieter but never vanishes), high frequencies softened by distance
  (air absorption), and the share sent to the room's reverb rising with distance (the
  direct-to-reverberant ratio). A written piece's staves already name a part and a pan; on the
  stage, each staff takes the seat whose part matches, and the seat's position replaces the fixed
  pan.
- **Layouts saved** as the person's own files (`music/stages/<name>.ron` beside the favourites),
  shared as a code the way favourites are.
- **Seats for players and NPCs** (rung 5): each pin is a seat that an NPC or a player can take in an
  in-world venue (a concert hall, a stadium, a ship's mess). A venue's data maps the plot's metres to
  its stage. A networked performance syncs only what cannot be computed: the score's id, its
  position (a bar and a beat with a time anchor), the stage layout's code and who holds which seat;
  each client renders the sound itself. About 60 bytes when anything changes and a 16-byte heartbeat
  every two seconds: under 10 bytes a second per venue, against the budget's "gameplay state at a
  low rate".
- **Cost:** per part a pan, a gain, a one-pole low-pass and a reverb send, applied where the score
  already mixes each voice: a few operations a sample, under a microsecond a block for twelve parts
  (to measure in rung 3 against the 0.05 ms block budget). The plot itself draws a few dozen shapes.
- **Two modes** (CLAUDE.md, "Every deep system ships two modes"): **Simple** (pick an ensemble, a
  room and where you sit: front row, middle, balcony) and **Full** (every pin, microphone and room
  value).

## 15. Broadcasting real orchestras in the game (rung 6)

The finding, [2026-11-20-orchestra-broadcast-rights.md](../reference/findings/2026-11-20-orchestra-broadcast-rights.md),
settles the ground. A free composition does not make a free recording: the performers and the
recording's producer hold rights of their own, with their own terms, by LAW (in the US 17 U.S.C.
1401 for recordings made before 1972; in the EU Directive 2011/77/EU; in the UK section 13A). So:

1. **Our own renderings of public-domain scores** need no one's permission (the score player
   already does this; US law limits a recording's rights so that an independent recording of the
   same music is not a copy, 17 U.S.C. 114(b)). This is rung 1 of broadcasting, already built.
2. **Openly licensed recordings**, checked file by file (the finding names Musopen's uploads to the
   Internet Archive: some CC0, some the Public Domain Mark, some CC BY 3.0 with credit), as optional
   downloads the person chooses, each with its licence and credit on the screen. The Great 78
   Project is for research only, so it is not offered.
3. **A live stream of a real orchestra** needs, by LAW, the performers' consent (US 17 U.S.C. 1101,
   the WPPT treaty, national performers' rights), a performance licence only for a composition still
   in copyright, and the broadcaster's permission when it is someone else's broadcast. So it is a
   partnership with an orchestra that agrees, never a capture of a stream. A concert on YouTube can
   be shown only as YouTube's own player on an in-world screen (CLAUDE.md's embedded-browser
   amendment): YouTube's developer policies forbid separating a video's sound, which is a PLATFORM
   HOLDER'S RULE, not a law. No patent is involved.

Where a person meets one of these limits on the page (a venue offering a stream, a recording not
offered), the page names which kind of limit it is, with a link to the finding's plain-words page.

## 15a. Any piece in a sound style, MIDI files, and scene cues

The operator, 2026-11-20, verbatim: "Is there a genre like mode for playing any music piece in midi
like the original Elite Frontier 2 game plays music". That game is his reference only, for the
feel: classical pieces played through an early-90s sound card's synthesiser. Nothing of it is named
in the product or copied (its sounds, its pairings of pieces and moments, its words).

**(a) Sound styles: any written piece, played in a chosen style.** A STYLE is data
(`data/music/styles/*.ron`, infinite-of-X): for each part a score names (its staff's part id, or a
MIDI program number for a MIDI file), the instrument of ours that plays it, with its level, pan and
effects (reverb, a low-pass, the drive), and the room. The person picks a style on a written piece's
details or for all written music; the piece keeps its notes, only its sound changes. Styles to ship
first:

- **Retro FM synth**: the early-90s sound-card sound, made from our own synthesis (the engine's
  two-operator FM voice, music.md section 6a, with short envelopes and little reverb), with no
  recorded sample of any old card copied.
- **General-MIDI-like palette**: one of our instruments for each of the 128 General MIDI program
  numbers' families (pianos, organs, strings, brass, reeds, synth leads, percussion on channel 10),
  so any MIDI file plays sensibly; the table is data, each row naming the instrument it uses.
- **Orchestral**: the recorded parts (strings, winds, brass, timpani) where installed, their
  synthesised voices otherwise, on the orchestra's stage when rung 3 lands (section 14).
- **Piano solo**: every part on the piano part (a reduction played as written, not re-voiced).
- **Chiptune**: square and triangle waves and a noise channel from our oscillators, the parts
  limited to four voices with the rest dropped by a stated rule (the lowest and highest kept).

A style is chosen like a genre (a card each on Explore with its one plain sentence, words by key),
saved with a favourite of a written piece (rung 4's playlists), and costs what the instruments it
names cost (no new audio path: it is the score player's staff-to-instrument map, made data).

**(b) MIDI files beside MEI.** Standard MIDI Files (format 0 and 1: tempo, time and key signatures,
track names, program changes, notes with velocities) import as written pieces: a `.mid` file and an
index line, played through a style's General-MIDI table. The scan,
[research/2026-11-20-midi-input](../reference/research/2026-11-20-midi-input/README.md) (section
"Reading MIDI files"), compares ten reader crates with their licences quoted and recommends
**midly** (released into the public domain under the Unlicense; pure Rust, no other crate, reads
damaged files the others fail on), the same crate it recommends for a MIDI keyboard, so one
approved download covers both; writing our own small reader is the fallback if the operator
declines. **Either is the operator's call: adopting midly is a crate download, which needs his yes;
nothing is downloaded until he says so.** Export of a composed piece as MIDI (section 12) is our
own writer either way.

**(c) Where free MIDI comes from.** The finding's section F
([2026-11-20-orchestra-broadcast-rights.md](../reference/findings/2026-11-20-orchestra-broadcast-rights.md))
settles it: a MIDI file of a public-domain piece is free only when its own licence says so. The
Mutopia Project's MIDI files are generated by LilyPond from the same source as each score and carry
that piece's licence (Für Elise: public domain), so they are the first source, read file by file. A
person's hand-sequenced MIDI file can carry rights of its own (in the US the Copyright Office may
register it as a musical work, never as a sound recording; in the UK it may count as a sound
recording), so such files come only under a stated open licence (Wikimedia Commons lists some, each
with its own). Classical Archives' terms forbid redistribution, and IMSLP keeps no MIDI of standard
repertoire: neither is a source. Each limit here is a LAW (copyright in the sequence) or the site's
own terms (a site's rule, which binds whoever downloads from it), and the page says which beside a
file the person cannot have.

**(d) Scene cues (proposed).** A CUE ties a written piece (in a style) to a moment of the game, as
data (`data/music/cues/*.ron`: the moment's id, the piece, the style, whether it plays once or
loops, how it fades in and out): a public-domain waltz while the ship docks, a march at a launch.
The moments are the game's own events (docking, launch, arrival at a station, a jump), the pairings
our own choice; no game's specific pairing or wording is copied. A person can change or turn off
any cue on the Music page (Your music), and "my music" for a place (rung 4) can name a cue. Cues are
cosmetic, local and never networked.

## 16. Keyboard shortcuts (rung 2)

Only while no text box has the keyboard: **Space** pause or play, **Left / Right** the previous or
next piece, **L** like, **1 to 5** the tabs, **[ / ]** fewer or more bars in the piano roll, **F**
follow the music, **/** the Written music search. Listed in a hint beside the strip, and in the app's
keyboard settings once those take page shortcuts.

## 17. A MIDI keyboard (rung 7)

A person plugs in a MIDI keyboard and plays the genre's instruments live over the music, and (later)
records what they play into a sheet. It needs a crate to read the keyboard; the scan,
[research/2026-11-20-midi-input](../reference/research/2026-11-20-midi-input/README.md), recommends
**midir** (MIT licence; it uses each system's own MIDI service: WinMM on Windows, CoreMIDI on macOS,
ALSA on Linux) to read it and **midly** (the Unlicense) to decode the notes. **Adopting it is a crate
download, which needs the operator's yes; nothing is downloaded until he says so.**

## 18. Performance

| Where | Cost | Basis |
|---|---|---|
| Audio thread | the score as it is; with the page open, the analyser (about 0.007 ms a block, measured 2026-11-13); the orchestra's per-part placement in rung 3 (to measure) | the analyser runs only while the page is open |
| Game thread | the dry composer run once per piece (tens of milliseconds for minutes of music); the roll draws one rectangle per note in view and the overview one mark per pixel column per part; the catalogue filters and sorts its rows each frame (four rows today, thousands in rung 4: a sorted order kept in memory tagged by the query and the data) | in-memory reuse tagged and behind the reuse switch |
| Export | its own thread, faster than real time | music.md's offline renderer |
| Disk | favourites and stage layouts (kilobytes); exports where the person asked | the person's own data only |
| Network | none, until rung 5's venues: under 10 bytes a second per venue (section 14) | cosmetic audio is local |

## 19. The web mirror

The website cannot run the Rust engine yet, so it mirrors what it can, rung by rung, from the same
data and the same words:

- **Rung 1:** the same tabs (Explore, Written music, Your music; Listen says plainly that the desktop
  app plays the music and why); the genre and mood cards with the same sentences; the Written music
  list with the same search, sort, filters and detail; the favourites kept as share codes in the
  browser. The data reaches the website as `data/music/catalogue.json`, written from the RON files
  by `node scripts/gen-music-catalogue.js` (a test refuses a stale copy, like the Library's).
- **Rung 2 on:** each native rung's web half where it needs no engine (the orchestra's plot and
  layouts as share codes in rung 3).
- **The music crate as WebAssembly** (version 1's rung 5): the web page plays, sees and exports like
  the app. That it cannot today is the order we build in, not a law, a patent or a platform rule.

## 20. The rungs

**Rung 1 (version 2; built 2026-11-20, section 23).** The condensed wide layout: the Now playing
strip (pause, previous, next, the chips, Like in one click with a name made for you), the tabs
(Listen, Explore, Written music, Your music), the docked piano roll on a wide screen, usable at
phone width. The piano roll's length slider (2 bars to the whole piece), the overview with the
window marked, follow the music, bar numbers, the whole piece for written music and for composed
pieces of a set length. Explore's cards with one plain sentence each (genre `plain_words`, mood
`where_words`, by key) and the pieces heard this session. Written music as a searchable, sortable,
filterable list with a detail panel, from the index's new fields. Rename on Your music. The web
mirror: tabs, cards, the catalogue, the favourites. Tests: Like saves the playing piece and its
changes in one click; the slider changes how many bars are drawn (counted from what is drawn); the
overview's window matches the bars drawn; the written list filters and sorts by composer, era and
search text from data; every genre file gets a card with its sentence, and a new genre file a card
with no code change; the website's tabs, cards and catalogue. Budget: no audio-thread change; the
roll's whole-piece transcription once per piece.

**Rung 2.** The mixer strips with Solo and meters (section 8); the audition (proposed); the history
kept in the person's data folder; clicking a bar seeks in a written piece; MEI and MIDI export
(section 12); keyboard shortcuts (section 16); swing and intensity on the page; A = 432/440 shown on
the page. Web: the history kept in the browser; the shortcuts.

**Rung 2 also, after the operator's yes for the midly download (section 15a):** MIDI files read as
written pieces, played through the General-MIDI-like style. Without that yes, our own small reader
instead.

**Rung 3.** Sound styles as data (section 15a: retro FM, General-MIDI-like, orchestral, piano solo,
chiptune), chosen per written piece or for all of them. The orchestra's stage plot (section 14): ensembles, seatings, rooms and microphone arrays
as data; drag the pins, the microphones and the listener; pan, distance, air and reverb per part;
layouts saved and shared; Simple and Full modes. Measured against the block budget. Web: the plot
and layout codes, without sound.

**Rung 4.** Playlists of favourites and written pieces; my music for a place; scene cues (section
15a); seed sweeps; the catalogue at thousands (drop-in MEI files listed from their headers, rows drawn only on screen).

**Rung 5.** In-world venues: a hall or stadium whose stage is a saved layout, its seats taken by
NPCs or players, a networked performance syncing only the score and its position (section 14).

**Rung 6.** Broadcasting (section 15): openly licensed recordings as optional downloads with their
licences and credits; a live stream from an orchestra that consents, as a partnership; each limit
named on the screen by kind.

**Rung 7.** A MIDI keyboard (section 17), after the operator's yes for the crate download.

**Carried from version 1:** the lead sheet on a real staff (published engraving rules); the full
score and printing; the music crate as WebAssembly; editing what the engine made (deferred by the
operator, 2026-11-13).

## 21. Questions for the operator, each with a recommendation

1. **Like on a piece already liked.** Recommendation: it says Liked and opens Your music with that
   favourite marked, never removing it (an accidental unlike would lose a name you gave it).
2. **The pieces-heard history across runs.** Recommendation: kept in your data folder from rung 2,
   with a Clear button, since it is your own listening; this session only until then.
3. **The audition (proposed).** Recommendation: a card's Listen plays 20 seconds then returns to
   what played, so exploring never changes your choice until you choose.
4. **The orchestra's first ensemble.** Recommendation: the string quartet first (four pins, every
   part already in `parts.ron`), then the symphony orchestra with the Fifth's twelve staves.
5. **The midly download** (section 15a): one crate, public domain, pure Rust, for MIDI files and a
   MIDI keyboard both. Recommendation: yes; without it we write our own small file reader and the
   keyboard waits.
6. **The first sound styles.** Recommendation: retro FM synth and piano solo first (they need no
   recordings), then the General-MIDI-like table with MIDI files.
7. **Version 1's questions** (the tempo range of a one-tempo genre, where exports go) stand as
   answered by default: 10% either way, and `Music\HumanityOS\` in your home folder.

## 22. As built: version 1, rung 1 (2026-11-13)

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

**The runtime** (`src/engine/music.rs`): `choice_now` is the person's choice from the settings
(`music::chosen`, while `game_chooses` is off; since 2026-11-14 it plays whether the page is open or
not, section 4), else the favourite that is the music for `mood_now`. A change of choice
(`Take::Choice`) takes over at the next bar line; a chosen piece is never replaced by "A new piece".
The runtime keeps a copy of the playing score's `Resolved` and its starting context and reports them
in `MusicReadout::playing`. The page's export runs on a `music-export` thread.

**Known limits:** with "turn off all reuse" off the runtime rebuilds the score every phrase and
resumes it mid-piece; the resumed composer is not the one that started, so the piano roll (written
from the start) can drift from what is heard until the piece changes. Solo, swing and the intensity
are in the `Tune` and the share code but have no control on the page yet (rung 2).

## 23. As built: version 2, rung 1 (2026-11-20)

**The data** (infinite-of-X): each genre file names `plain_words` (one plain sentence, by key
`music.genre.<id>_plain`; `plain` for a community genre), each mood file `where_words`
(`music.mood.<id>_where`; `where_plain`), and each entry of `data/music/scores/index.ron` its
catalogue details (`era_words`, `year`, `composer_years`, `key_root`, `key_mode`, `length_s`,
`source_url`, `typesetter`, `tempo_words`, `tempo_finding`). The crate's tests hold every genre and
mood to its sentence and every piece's `length_s` to the length its file plays, within a second
(`every_written_piece_carries_its_catalogue_details`).

**The page** (`src/gui/pages/music/`): `mod.rs` lays it out (the `Tab`s, `WIDE_POINTS` = 1100: wide,
the Listen columns and the roll docked along the bottom; narrow, one column with the roll in
Listen); `strip.rs` is the Now playing strip (`like`, `liked_favourite`, `already_liked`); `roll.rs`
the piano roll (`length_steps`, `bars_shown`, `first_bar`, the overview, the bars with their numbers;
notes merged per two-pixel column past 3,000 in view); `explore.rs` the cards (`genre_cards`,
`mood_where`) and the pieces heard (`Heard`, `note_heard`, at most 50, noted every frame by
`engine/music.rs`, page open or not); `written.rs` the catalogue (`catalogue`: every word typed
against title, composer, era, year, edition, source, typesetter, licence and instruments; composers
by surname; eras in the order they came; the details panel with the source link). `view.rs` keeps
the piece in words, the visualizer and `ensure_sheet`, which now writes a composed piece of a set
length down to its end (the score's `next_piece_s` says how long is left). Your music lost the name
box (Like names a favourite) and gained Rename.

**The website** (`web/pages/music.html`, `music-app.js`): the same four tabs by the same keys, the
cards and the Written music list from `data/music/catalogue.json` (`scripts/gen-music-catalogue.js`
writes it from the RON through `scripts/lib/ron-read.js`), the favourites as before.

**Tests:** `tests_v2.rs` (`like_keeps_the_playing_piece_and_its_changes_in_one_click`,
`the_length_slider_changes_how_many_bars_are_drawn`, `the_overview_window_matches_the_visible_bars`,
`the_written_list_filters_and_sorts_from_data`, `every_genre_and_mood_file_gets_a_card_with_its_sentence`,
`the_pieces_heard_list_each_piece_once_newest_first`), each counting what the page drew; the version
1 tests moved to the tabs; `scripts/tests/music-web.test.js` for the website (the catalogue is
current, the same search, sort and cards, the tabs by the app's keys).

**Known limits, for rung 2:** a composed piece's whole length is the time until the next piece as
the score reports it, so its last bar can fall a beat short of where the next piece takes over; the
history lasts the session; Like on a written piece waits for playlists.
