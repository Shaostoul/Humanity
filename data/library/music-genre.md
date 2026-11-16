# The Music Generator, and Making a Genre

HumanityOS composes its own music, live, while you use the app and play the game. It is not a
playlist of recorded songs: the app works like a musician improvising within rules. It picks a key, a
song shape, chords, melodies and instruments, and plays them on the spot, so it never plays exactly
the same thing twice. This guide explains what you hear, how it works, and how to make and share a
genre of your own.

## What you hear

- **A new piece every few minutes**, each with its own key, form, chords, riffs and lead instruments.
- **Eight genres**: Mothership calm (the ship's own score, the default), lo-fi, electro swing, deep
  dubstep, house, drum and bass, synthwave and ambient. Switching genre is heard within seconds.
- **Blending**: Settings > Music lets you mix in a second genre with a slider. Tempo, swing and the
  sound move smoothly, and each part of the music takes one genre or the other.
- **Music that follows you**: the calm score follows where you are (your home, the Commons, the
  garden, near the engines), the ship's time of day and what you are doing, and it rises with danger
  and settles back.
- **Tuning**: every note is tuned to A = 432 Hz by default; A = 440 Hz is one setting away. This is a
  matter of taste, and no health effect is claimed.
- **The ship's hum**: a soft, deep engine sound under everything aboard, tuned to the music's key so
  the two never clash, with its own switch and volume. An optional "tones" layer is off unless you
  turn it on; some people find these tones calming, and there is no good evidence that they heal.

**What it never does.** Nothing is downloaded while it plays and nothing goes over the network: each
person's own computer composes their music. It uses no graphics card time at all. And it uses no AI
to generate music: the one openly licensed music model we read would share your graphics card with
the game, and generated audio cannot follow the tuning or where you are the way the live score does.

## How it works

The music is built from three layers of plain data files in `data/music/`:

- **A mood** (`moods/`) says where you are and what the music is about: calm aboard, a garden morning,
  a night watch, tension, action, the menu.
- **A genre** (`genres/`) says how it sounds: its tempo range, its groove, its drum patterns, its
  harmony rules, which instrument plays each role, its effects and its loudness. Every field is
  optional, so a genre only changes what it names.
- **A piece is a seed.** One number decides, within the genre's rules: the key (and, in about a third
  of pieces, a key change in the bridge), the tempo, the form (such as AABA or ABAB), every section's
  chords, the riffs, and which instruments play the lead and the answer. The same seed in the same
  genre is the same piece, every time.

**Chords** come from a grammar rather than a fixed list. Each chord has a job (home, moving away,
pulling back home), and a section takes a shape such as "home, away, pull, home" and fills each slot
with a chord that does that job. Then, by chances the genre sets, it adds the touches musicians use: a
borrowed dominant leading into the next chord, a substitute chord a tritone away, a passing diminished
chord.

**Riffs** come from rules too: how many notes, rhythm cells strung together, the melody's contour, and
steps of the scale with the odd leap. **Songs have structure**: an intro, verses, choruses, a bridge
and an outro, each with its own instruments and energy, with risers and reversed cymbals swelling
into a chorus and drum fills at the turns.

**The instruments** are data files too (`instruments/`). Some are synthesisers built into the app:
wavetable and supersaw synths, a pulse-width synth, FM electric pianos and bells, plucked basses, and
808 and 909 style drum machines, all made from oscillators, filters and envelopes the data describes.
Others are recordings from eleven free-to-share instrument libraries, with optional packs to download
from Settings (417 instruments in seven full packs). Long recordings play straight from disk, so the
full Steinway piano takes about 15 MB of memory instead of 452 MB and sounds the same.

**The mix** gets the effects producers use, driven by the genre's data: the bass ducking under the
kick (sidechain), gated reverb on an 80s snare, tape-style delay, tape wow and flutter, saturation,
long reverbs with their lows cut, risers, downlifters and tape stops.

**It stays light.** The whole score runs on the audio thread in about 2% of one processor core on
average (a budget of 0.05 ms per audio block), and the project's tests check that every genre and
instrument fits it. Up to 64 voices play at once; the calm moods use 12 to 24.

## The Music page

The Music page lets you explore pieces by seed and genre, tune one, keep favourites, export a piece as
a WAV file, and watch the music as a piano roll while it plays. Every favourite has a share code (it
starts with `HOSM1-`): send it to a friend and they hear the exact same piece. Whatever you choose
there keeps playing after you leave the page.

## Make your own genre

A genre is one text file in RON format. Put it in `data/music/genres/` next to the app and it appears
in the genre list; that is the whole install. The easiest start is to **copy an existing genre**
(synthwave and electro swing show nearly every field) and change what you like. Keep its palette
(which instrument plays each role): a genre that names no instrument for any role is not accepted.

Give your genre its own `id` (lower case, with underscores), and its name and one-line description in
plain words with `name:` and `about:` (the built-in genres use word keys instead, so they can be
translated).

What a genre can set:

- `tempo_bpm`: its tempo range, for example `Some((80.0, 118.0))`.
- `groove`: the swing (0.5 is straight, 0.667 a triplet shuffle), the grid (`subdivision`: 2 for
  eighths, 4 for sixteenths), `syncopation`, and `humanize_ms`, a few milliseconds of human timing.
- `drums`: each drum an instrument, a note, a loudness and patterns written as text over the grid:
  `x` a hit, `o` a softer hit, `g` a ghost note, `.` a rest. A four-on-the-floor kick in sixteenths:

  ```
  (instrument: "kick_909", note: "B1", velocity: 0.9, patterns: ["x...x...x...x..."]),
  ```

- bass and pad patterns, written the same way (`x` the root, `5` the fifth, `o` the octave, `3` the
  third).
- `progressions`: the keys it likes, each with a weight, the chord grammar, and how often the borrowed
  and substitute chords appear.
- `forms`, `motif_rules` and `leads`: song shapes, how riffs are built, and which instruments take the
  lead and the answer.
- `structure`: the sections (intro, verse, chorus and so on), how long each is, which parts play in
  it and its energy.
- the mix: sidechain depth, gated reverb, delay, tape wow, saturation, and a level so it sits at the
  same loudness as the other genres.

## The rules for sharing a genre

- **Capture the genre, never an artist.** A genre's commonplace building blocks (a tempo range, a
  swing feel, a classic drum pattern, a typical chord move) are free for anyone. An artist's
  recordings, melodies or riffs are not, and genre files do not name artists. The built-in genre files
  cite their sources in a comment at the top: write yours the same way.
- **Free to share.** Genre files are data, and HumanityOS data is public domain (CC0). Any recorded
  sound has to be free to share as well: the app never samples copyrighted recordings, because
  copyright law forbids it.
- **Honest limits.** Genres built on recorded sounds (swing-era brass, vocal chops) need free
  recordings of them, so some styles sound better than others until free samples exist. Free-to-share
  (CC0) recordings of missing sounds, such as swing-era brass and clarinet played note by note, are
  very welcome.

## What is planned

- An editor in the app, so moods and motifs can be shaped with live preview and no text editor.
- The same composer on the website.
- Ideas being discussed: play cards for share codes in chat, exporting each part as its own file,
  a "what am I hearing?" overlay that names the chords as they play, ear-training practice, playing
  along on a MIDI keyboard, playable instruments in homes, jamming together with a shared seed,
  themes for crew members, radio stations, focus and sleep pieces, a visual beat for people who are
  deaf or hard of hearing, playing written classical pieces from public-domain scores, and singing in
  vowels drawn from the Humanity Accord.
