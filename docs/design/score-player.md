# The score player: written music stored as MEI, played on our own instruments

Planned and rungs 1 and 2 built 2026-11-20. The crate is `crates/humanity-music` (its card:
[docs/crates/humanity-music.md](../crates/humanity-music.md)); the live composer it sits beside is
[music.md](music.md); the page it plays from is [music-page.md](music-page.md).

## 0. In plain words

Until now the game's music was only ever composed live. The score player plays music that is
WRITTEN DOWN: a piece stored as a file of notes (Beethoven's "Ode to Joy" theme first, later Grieg's
"In the Hall of the Mountain King" and the Humanity Accord set to music), played on the same
synthesised and recorded instruments the composer uses, at the same tuning (A = 432 Hz unless the
person chose 440). The file format is MEI, an open XML format for written music that the free
MuseScore editor reads and writes, so anyone can open a piece, change a note and hear it. Each piece
is a data file; adding a piece is adding a file and a line to an index, never code.

## 1. Why, in the operator's words

The goal (2026-11-16, docs/PRIORITIES.md TIER 0): "To some how save orchestral music in a file format
everyone can easily modify ... If we use a special file format then we should also convert the
humanity accord to that format so we can play the humanity accord as instrumental music." Earlier
the same day: "For the music, could we somehow play music like Beethoven? I'd love to hear In The
Hall of the Mountain King by Edward Grieg in-game." On the sing-along: "I love that sing along idea!
That'll make reading and hearing the accord instrumentally a unique experience."

The format, after the comparison in
[docs/reference/research/2026-11-16-score-formats/](../reference/research/2026-11-16-score-formats/mei-and-humdrum.md)
(its section 6, the verdict: store MEI with the musical core valid MEI Basic), approved 2026-11-20:
"MEI sounds right unless there's better".

## 2. What exists (read 2026-11-20)

- **The composer** (`composer.rs`) decides notes on the audio thread and hands each to a `Sink`
  (`note_on`, `note_off`); the score's sink (`score.rs`, `PoolSink`) starts voices in the
  `VoicePool` (`voice.rs`). A `NoteOn` carries the instrument index, a role, a MIDI note as `f32`, the
  A4 frequency, a velocity, a pan and `hold_s` (how long the note is held before it lets go).
- **The score** (`ScoreEngine`, `score.rs`) runs the composer, the voices, the reverb (the mood's
  hall), the genre's recipe, the level and the limiter into a plain stereo buffer; kira plays it
  (`kira_sound.rs`); the engine builds one on a worker thread and a new one takes over at a bar line
  or a phrase (`src/engine/music.rs`).
- **Every pitch** comes from `tuning::note_hz(midi, a4)`.
- **No written music**: nothing read notes from a file, and no XML reader was in the crate.

## 3. The rules this follows

- **Every pitch through `tuning::note_hz`** at the person's tuning (the crate's first rule): a
  written piece is in concert pitch at A = 432 Hz by default, like everything else.
- **One audio path.** A written piece plays through the same `ScoreEngine`: its conductor stands in
  for the composer and sends `NoteOn`s to the same voice pool, reverb, level and limiter. No second
  mixer, no second kira sound.
- **Nothing allocates on the audio thread.** The file is read and turned into a timeline of notes on
  the music's build thread; the audio thread only walks that timeline (`render_allocates_nothing`
  covers a written piece too).
- **Scores are data** (infinite-of-x): `data/music/scores/*.mei` and `data/music/scores/index.ron`;
  no list of pieces in code.
- **No caches across runs**: the index and the files are read each time a written piece starts.
- **Never networked**: each player's game plays its own copy.
- **Loudness by the app's one meter**: each piece carries a `level_db`, measured so it renders at
  the music target (-18 LUFS) and under -1 dBTP (a test holds the shipped pieces to it).
- **Words by key, a hint on every control** (the Music page's list).
- **Legal first**: a piece is encoded only from a source the dated finding allows (another agent's
  finding on the Grieg and Beethoven editions, step 2 of the goal). Rung 1's one score was written
  by the project from the melody as it is commonly sung, not copied from any edition or file.

## 4. The format: MEI, the musical core kept to MEI Basic 5.1

What MEI Basic 5.1 keeps and drops, read 2026-11-20 from the MEI Basic customisation at the `v5.1` tag
of music-encoding/music-encoding (`customizations/mei-basic.xml`; the attribute classes from
`source/modules/MEI.shared.xml`, `MEI.cmn.xml`, `MEI.gestural.xml`, `MEI.lyrics.xml` and
`MEI.midi.xml` at the same tag; the schema's licence is ECL-2.0 per the research note):

- **Kept**: `note`, `rest`, `chord`, `space`, `layer`, `staff`, `measure` (with `left` and `right`
  bar lines: repeats), `mRest`, `beam`, `tuplet` (`num`, `numbase`), `tie` (the element, with
  `startid` and `endid`) and `@tie` (`i`, `m`, `t`), `tempo` (`mm`, `mm.unit`, `mm.dots`,
  `midi.bpm`), `dynam`, `hairpin`, `slur`, `fermata`, `verse` and `syl` (`wordpos` `i` `m` `s` `t`;
  `con` `s` `d` `u` `t` `c` `v` `i` `b`), `staffDef` (with `label` and `instrDef`), the key and the
  meter as ATTRIBUTES (`keysig`, `meter.count`, `meter.unit`, `meter.sym` on `scoreDef` and
  `staffDef`), `trans.semi` (transposing instruments), `vel` (MIDI velocity) on notes, `oct.ges` and
  `pname.ges`.
- **Dropped**: `accid.ges` (the sounding accidental: so the player works the key signature and the
  bar's accidentals out itself, as a musician reads them), the `keySig` and `meterSig` elements,
  `tupletSpan`, `beamSpan`, `mSpace`, the critical apparatus (`app`, `rdg`, `lem`), the text module
  (`lg`, `l`), and the header beyond `fileDesc`, `titleStmt`, `pubStmt` and `availability`.

So the richer layers the goal wants (the Accord's line groups, the simple and full readings, where
each passage came from) sit on top of the Basic core, in the same file, and a MuseScore save may drop
them (the research's open question 1). The reader accepts the full-MEI spellings too (`accid.ges`,
`keySig sig`, `meterSig count unit`, the older `key.sig`), so a file from Verovio or an MEI 4 edition
reads as well.

## 5. The subset read, now and later

| What | Rung 1 (built) | Later |
|---|---|---|
| Header | `title`, `composer` (or a `persName` with `role="composer"`), `availability` text | `respStmt` credits shown on the page |
| Structure | `music`, `body`, `mdiv`, `score`, `section`, `measure`, `staff`, `layer` (layers of a staff play together), `beam` (read through); rung 2: `ending` and the repeat bar lines played in order | |
| Notes | `note` (`pname`, `oct`, `dur` from `long` to `2048`, `dots`, `accid` as attribute or child `accid`, `oct.ges`, `pname.ges`, `vel`, `xml:id`), `rest`, `mRest`, `space`, `chord` (its notes take its duration); rung 2: grace notes (`@grace`, `graceGrp`, `grace.time`), `bTrem` with `unitdur` (its strokes) | `fTrem`, `arpeg` |
| Time | `tuplet` (nested, exact), the key and the meter (attributes and elements), a mid-piece `scoreDef` or `staffDef` (from the next bar), `tempo` at its `tstamp`; rung 2: `fermata` (element or attribute: its moment held twice its length) | `tempo@func="continuous"` (a gradual change), `rit.`/`accel.` words |
| Pitch | the key signature, a written accidental holding to the end of its bar on its staff, a tie carrying its pitch over the bar line, `trans.semi`, quarter-tone accidentals (`1qs`, `3qf` ...); rung 2: `octave` lines (`dis` 8, 15, 22, above or below, to `endid` or `tstamp2`; a note with `oct.ges` left alone) | |
| Ties | `@tie` and the `tie` element: the tied notes sound as one | |
| Dynamics | `vel` on a note; `dynam` letters (`ppp` to `fff`) at their `tstamp`, per staff; rung 2: `sf`, `sfz`, `fz` (one note louder), `fp`, `hairpin` and "cresc." or "dim." words (the loudness moves to the dynamic at its end, else one step), articulations (`stacc` held half, `stacciss` a quarter, `ten` all, `acc` and `marc` struck harder) | |
| Lyrics | `verse` (`n`) and `syl` (text, `wordpos`, `con`) under a note, or `syl` directly in it: each kept with its note's `xml:id`, its verse and its time | the Accord's `lg`/`l` line groups beside the music (rung 3) |
| Parts | `staffDef` `n` and `label`; rung 2: the index's `part` per staff (section 9) | `instrDef` (`midi.instrnum`) as a hint for the instrument mapping |
| Readings | | `app`/`rdg`/`lem`, `choice`: the simple and the full arrangement (rung 4), `@source` named on the page |

What the reader does with anything else: an element it does not know is skipped, its subtree with
it, and COUNTED by name (`MeiScore::skipped`); one it knows but does not play yet (a slur, a clef, a
fermata, a hairpin, a grace note) is counted apart (`MeiScore::not_played`); a core element it cannot
make sense of (a note with no pitch, a tie whose end is missing, an 8va line) is a PROBLEM, listed in
words (`MeiScore::problems`). Never a crash: a broken file gives an error that says where, and the
Music page says so.

## 6. The XML reader: compared before adopted

CLAUDE.md asks for a scan of the alternatives before a tool is adopted, and the brief asked to prefer
a parser already in `Cargo.lock`. The scan is
[docs/reference/research/2026-11-20-xml-reader/](../reference/research/2026-11-20-xml-reader/README.md)
(licences read 2026-11-20 from each crate's published `Cargo.toml`); in short:

| Option | In `Cargo.lock`? | Licence | Fit |
|---|---|---|---|
| **quick-xml 0.41.0** | Yes (wayland-scanner's) | `license = "MIT"` | A pull reader over a string; no default features; one dependency (`memchr`, already in the tree); compiles to WebAssembly. **Chosen.** |
| xml-rs 0.8.29 | Yes (gl_generator's) | `license = "MIT"` | A pull reader too, slower and allocating per event; no advantage here |
| roxmltree | No | not examined | A read-only tree; would add a crate the lock does not have |
| Our own tokenizer | n/a | ours | About 200 lines, but entity references, CDATA and encodings are a known source of bugs; nothing gained over a maintained MIT reader already in the tree |

The reader builds a small tree of the `<music>` part (elements, attributes, text) and walks it, so
ties (by id, forwards) and, later, the alternative readings (`app`) are easy; a piece of a few
megabytes reads in milliseconds on the build thread.

## 7. From notes to time

- **Exact fractions.** Every duration is a fraction of a whole note (`Frac`): a dotted quarter is
  3/8, a quarter in a triplet 1/6; tuplets multiply (`numbase`/`num`), nested ones too. Nothing is
  rounded until a time is turned into seconds.
- **Bars.** Each layer of each staff starts at the bar's start; the bar lasts as long as its longest
  layer (so a pickup bar is as short as it is written), or its meter's length when it holds only
  `mRest`s or nothing.
- **Ties** join notes of the same pitch into one sounding note (the first's start, the durations
  added); a syllable stays on the first.
- **Tempo** is a map of points in score time, each in quarter notes per minute (`midi.bpm`, else
  `mm` in `mm.unit` notes, dots included); a `tempo` takes effect at its `tstamp` (beats of the
  meter's unit, 1 = the bar's start). Before the first one, the index's `bpm` for the piece, else 120.
  Seconds are the integral of that map, exactly per segment.
- **The timeline** (`written::Timeline`) is what the audio thread reads: notes sorted by start (seconds,
  duration, MIDI note, velocity, staff), syllables with their note ids and times, bar lines, and the
  piece's length.

## 8. Pitch, and the tuning

Sounding pitch, as a reader of the score works it out: `pname.ges`/`oct.ges` when given; else the
written `pname` and `oct` with the written accidental (attribute or child `accid`), else the
accidental written earlier in the same bar on the same staff for the same letter and octave, else the
key signature (`keysig` `"3f"`, `"2s"`, `"0"`); then `trans.semi` for a transposing staff. The result
is a MIDI note as `f32` (a quarter tone is half a semitone), and the voice plays
`tuning::note_hz(midi, a4)`: A = 432 Hz unless the person chose 440, for written music exactly as for
composed.

## 9. How a score maps to our instruments

- **Rung 1: the index names each staff's instrument** (an id in `data/music/instruments`, its role for
  the Music page's meters and colours, its level and pan). A sampler there falls back to its
  synthesised voice when its pack is not installed, as everywhere.
- **Rung 2: parts.** A staff names a part (`data/music/parts.ron`: `piano`, `solo_violin`,
  `strings_high`, ...), and the instrument library attaches the best installed recording of it,
  as the moods' roles do (`src/instrument_packs/score.rs`, `sounding_parts` gains the written piece's
  parts while it plays).
- **Optional genre palette (rung 5).** "Arranged into a genre's palette": each staff's role is played
  by the genre's palette entry for that role (lo-fi's piano and tape, electro swing's clarinet and
  brass), with the genre's recipe on the mix. The notes stay the composer's; only the colour moves.
- `instrDef` (`midi.instrnum`, General MIDI numbers) is a hint for files that come without an index
  line: a table from General MIDI families to parts, as data.

## 10. Playing it

- **In place of the composer.** `ScoreEngine::written(res, rate, seed, ctx, player)` makes the
  score with a `WrittenPlayer` standing in for the composer: each block it sends the notes due, sample
  accurate, as `NoteOn`s whose `hold_s` is the written duration (a small gap before the next note of
  the same staff keeps repeated notes distinct). The mood's hall and the limiter are as for composed
  music; the level is the piece's own `level_db`.
- **When it ends** it rests a few seconds (the index's `rest_s`, 4 by default) and plays again, until
  the person chooses something else. "Play once, then the game's music" is a later choice on the page.
- **Choosing it.** The Music page's written-pieces list sets the person's settings (`written`: the
  piece's id), which the score reads every frame like every other choice (music-page.md section 4:
  one place chooses the music); a new score with the piece takes over at the next bar line. Stop
  clears it, and what played before plays again.
- **The readout.** The status reports the piece's bar, beat and tempo, so the Music page's beat pulse
  and the Dev page follow it.

## 11. The sing-along hooks (rung 3 builds on these)

- Every syllable is kept with its note's `xml:id`, its verse, its text, its word position and its
  connector, and its TIME in seconds (`Timeline::syllables`), so "which syllable is sung now" is a
  search by time (`Timeline::syllable_at(t)`), and the line being sung is the run of syllables from
  the last word start.
- The Accord rung: the Accord's text set to a melody in MEI, its sentences as `lg`/`l` line groups
  beside the music; the five-vowel choir (choir_ah.ron extended to a, e, i, o, u) sings each
  syllable's vowel on its note; the page scrolls the line like a karaoke line, the syllable sung now
  lit. Verovio's timemap does the same for the website (note ids per millisecond).

## 12. The native score view, later (the research's open question 4)

Options: Verovio's SVG drawn through a Rust SVG renderer (LGPL C++ linked in, a long build), our own
egui engraver, or no notation at first. **Recommendation:** first the Music page's piano roll drawing
the written notes (the roll exists for composed pieces), then our own small engraver for one staff
with lyrics (all the sing-along needs), and Verovio only on the website, where its WebAssembly build
draws the same file.

## 13. The web mirror

The website mirrors the page (music-page.md section 11). Playing a written piece in the browser needs
the music crate built for WebAssembly (music.md's website rung), the same reader and player, so the
two cannot drift. Until then the web page can list the pieces from the same index (if the site
serves `data/music/scores/`) and show each file's notation through Verovio's WebAssembly build.
Not built in rung 1: the list alone, with nothing to play, would be a page that only promises.

## 14. Scores as data

`data/music/scores/index.ron` lists the pieces (id, file, title and composer by words key or plain
text, bpm when the file has no tempo, each staff's instrument, level, rest between plays); each piece
is one `.mei` file beside it. A file encoded by the project is CC0, like the rest of `data/`, and
says so in its header; a file made from an edition names that edition in its header and is added only
after the dated finding allows it.

## 15. The budget

Reading and timing a piece happens on the music's build thread (a few milliseconds for a short piece;
the Diabelli Variations' 4.6 MB would take tens). On the audio thread the player only compares the
next note's start with the clock and sends what is due: cheaper than the composer, whose phrase
planning it replaces. The voices cost what they always cost; a dense orchestral piece is bounded by
the voice pool like any genre.

## 16. The rungs

1. **The reader, the timeline, the player, the index, one piece, the page's list** (built 2026-11-20,
   section 18).
2. Recorded parts, grace notes, repeats and endings, `octave` lines, fermatas, hairpins and
   articulations, the written piece in the piano roll (built 2026-11-20, section 19), with three
   public-domain pieces converted from Mutopia's LilyPond by our own converter.
3. The Accord sing-along: the five-vowel choir, the Accord set in MEI with its line groups, the
   scrolling line.
4. The two modes as readings (`app`/`rdg`: the simple and the full arrangement), `@source` named on
   the page.
5. A genre's palette for a written piece.
6. The native score view (section 12).
7. The website: the crate in WebAssembly, the same list and player.
8. Imports: MusicXML through MuseScore or Verovio into MEI (a person's own file opened from the page).

## 17. Questions for the operator, each with a recommendation

1. **When a written piece ends: play it again, or go back to the game's music?** Recommendation:
   again after a short rest (built), with "once" as a later choice on the page.
2. **Should the written pieces have favourites and share codes like generated pieces?**
   Recommendation: yes in rung 2 (a favourite then names a piece id instead of a seed).

## 18. As built: rung 1 (2026-11-20)

- **The reader** (`crates/humanity-music/src/mei.rs`): the subset in section 5's rung-1 column, over
  quick-xml (section 6) into a small tree and a walk; times as exact fractions (`Frac`); the sounding
  pitch worked out per bar, staff by staff in time order (an accidental holds to the bar's end, a tie
  carries its note's pitch over the bar line, the key signature otherwise); ties joined after the last
  bar; `skipped`, `not_played` and `problems` as section 5 says. Errors name the line.
- **The timeline and the player** (`written.rs`): `TempoMap` (exact per segment), `Timeline::from_mei`,
  `Timeline::syllable_at` (the sing-along's hook), `WrittenPlayer` (sample-accurate onsets, each note
  held a tenth short of its written length, at most 50 ms, so repeated notes stay two; a rest after
  each time through; `seek_bar` for a rebuild mid-piece).
- **The score** (`score.rs`): `ScoreEngine::written`, the player in place of the composer; the
  status reports the piece's bar, beat, tempo, a "phrase" every four bars (so the runtime's takeovers
  at a phrase still come) and the bar as `form_pos` (where a rebuild carries on from).
- **The index** (`scores.rs`, `data/music/scores/index.ron`), read with the rest of the music's data
  (`MusicLibrary::scores`; the library's check refuses a staff naming an instrument it lacks), and
  `Resolved::instrument_index` (an instrument and its fallbacks, as a role's are followed).
- **The engine** (`src/engine/music.rs`): `MusicChoice::written`; the piece read and timed on the
  build thread (`load_written`) with the score's build; a piece that cannot be read is logged, said on
  the Music page, and not asked for again until the person chooses another (so a broken file never
  rebuilds every frame); the score takes over at the next bar line like any choice.
- **The page** (`src/gui/pages/music/written.rs`): the Written music list under What plays, each piece
  by its words with its composer, Play and Stop with hints, "Starting at the next bar" and "Playing
  now"; `now_playing_words` names the piece, so Settings > Music says the same; the piano roll says
  plainly that a written piece is drawn there in rung 2 instead of showing a composed piece's roll.
- **The piece**: `data/music/scores/ode_to_joy.mei`, the theme in D major, 16 bars, 62 notes, with the
  first verse of Schiller's words as Beethoven set them (60 syllables), written by the project by hand
  from the melody as it is commonly sung (a simple version: Beethoven's own setting differs in places);
  played on the synthesised clarinet at 120 quarter notes a minute.
- **Not built in rung 1**, by the plan: recorded parts, grace notes (counted), repeats and endings,
  `octave` lines (problems), hairpins, the web mirror (section 13), validation of the shipped file
  against the MEI Basic RNG schema (a download of the schema, to be asked for first).

## 19. As built: rung 2 (2026-11-20)

- **Repeats and endings** (`mei.rs`, `play_order`): the bars and definitions are gathered in the order
  written, each with the ending it sits in, then played: a repeat's bars again from its start sign
  (or the last repeat's end, or the beginning), each ending only on the times through its `n` names
  ("1", "1, 2", "1-3"; an ending with none takes its place in its run), the run of endings ending the
  repeat (as many times through as the highest ending number, else two). `MeiScore::bars` is the bars
  as played and `written_bars` the count the file writes; a note's `bar` counts as played.
- **Grace notes** take their time from the note (or rest) after them: an appoggiatura (`grace="acc"`)
  half of it, two thirds of a dotted one, a group its written values up to half; a slashed or plain
  one (`unacc`) at most a 32nd each, the group up to half; `grace.time` (a percentage) when written.
  The note after them starts when they end. `graceGrp` gives its notes its kind.
- **Octave lines** (`octave`, `dis` 8, 15 or 22, `dis.place`) move every note of their staff from
  their start to their end (`endid`, or `tstamp2` as bars and a beat), both ends included, unless a
  note gives `oct.ges`. MEI's `oct` is the WRITTEN octave: under an 8va a note is written an octave
  below where it sounds (the converter writes it so; LilyPond writes the sounding pitch).
- **Fermatas** (the element by `startid` or `tstamp`, or `@fermata`) hold what they stand on (the
  note or rest named, else the longest starting there on their staff, else a beat): the timeline's
  tempo map (`TempoMap::with_holds`) plays that moment at half speed (`FERMATA_HOLD` = 2), so every
  note sounding is held and what follows comes later; fermatas on every staff at one moment hold it
  once.
- **Dynamics**: a hairpin (or "cresc." or "dim." written as words) moves the level of the notes under
  it from where it starts to the dynamic at its end, else one step (`HAIRPIN_STEP`, 0.12) on, which
  then holds; with no end written it runs to the next dynamic on its staff. `sf`, `sfz` and `fz`
  strike their one note `SFORZANDO_ADD` (0.25) harder; `fp` strikes forte and leaves piano. A note's
  own `vel` is never moved.
- **Articulations** (`@artic` on a note or its chord, or `artic` elements): `stacc` held half its
  length, `stacciss` a quarter, `spicc` 0.4, `ten` all but 5 ms, `ten-stacc` three quarters; `acc`
  0.12 harder, `marc` 0.18. `TimedNote::hold_s` carries the held length to the player.
- **Measured tremolos** (`bTrem` with `unitdur`) play their strokes (a half note with eighth
  strokes is four eighths); the first keeps the note's id and ties. `fTrem` still plays its notes
  plainly (counted as not played).
- **Recorded parts** (section 9): a staff's `part` in the index; the build attaches the piece's parts
  beside the genres' (`scores::written_parts`, with the middle of each staff's notes;
  `instrument_packs::score::attach_with`; `engine::music::build_with`), and the staves are pointed at
  the recordings found (`scores::point_staves` with `Attached::recorded`); without one, the index's
  synthesised instrument plays. `named_parts` counts the written pieces' parts, so Settings > Audio >
  Instruments says truthfully which parts the music plays (the flute, oboe, bassoon, horn, timpani and
  middle strings parts lost their "not used yet" marks).
- **The piano roll** draws a written piece: `Sheet::from_timeline` (each note in its staff's role, a
  bar line at each bar, the first bar's beats), made again when the piece changes and every frame with
  "turn off all reuse" off; the score's status reports the seconds into the piece for the playhead;
  the engine hands the page the playing piece (`MusicReadout::written_piece`).
- **The pieces**, converted with our own converter (`scripts/ly-to-mei.js`: a reader of the LilyPond
  subset these files use, written for this; it never runs LilyPond or the Scheme inside a file; each
  source file's SHA-256 in `scripts/ly-to-mei.pieces.json` is checked first). A converter rather than
  hand-encoding: the symphony alone is twelve staves and about 10,000 notes. Für Elise (106 bars, both
  repeats, 156.25 s), Grieg's piano Mountain King (88 bars, 147.7 s, its two right-hand voices as two
  layers) and the symphony's first movement (502 bars, the exposition repeated, the oboe's Adagio
  cadenza in bar 268 as the edition lays it out). Each MEI file names its source, edition and licence
  in its header and comment; the index carries them and a credit by words key, which the Music page
  shows under the piece. All three are public domain; Mutopia and its typesetters are credited as a
  courtesy (data/credits.ron, LICENSES.md), as the finding recommends. The symphony's tempo (a half
  note = 108, Beethoven's own metronome mark of 1817; the Adagio at 60 quarter notes a minute) is
  written as playback values beside the edition's words; the double basses carry `trans.semi="-12"`.
- **Section 17's questions** stay the operator's: a piece plays again after a short rest, as built; no
  favourites or share codes for written pieces yet.
- **Not built in rung 2**: fingered tremolos (`fTrem`), arpeggios, `rit.` and `accel.` words, the
  MEI Basic RNG validation (a download, to be asked for first), and the web mirror.
