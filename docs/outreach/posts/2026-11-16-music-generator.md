# The music generator: an in-depth post

A Discord post asked for by the operator on 2026-11-16: what the music generator is, how it works, how
other people can make genres for it, and ideas not yet built. Every claim was checked against
docs/design/music.md, crates/humanity-music (genre.rs, favourite.rs), data/music/ and data/i18n/en.json
on 2026-11-16; three stale claims were corrected before posting (share codes already exist; a recorded
choir pack already exists; the planned editor is for moods and motifs). Post it as six messages, each
under 1,700 characters.

### Message 1

**The HumanityOS music generator: what it is and how it works**

HumanityOS composes its own music, live, while you play. It isn't a playlist of recorded songs. The app works like a musician improvising within rules: it picks a key, a song shape, chords, melodies and instruments, and plays them on the spot. It never plays exactly the same thing twice.

**What you hear**
- A new piece every few minutes, each with its own key, form, chords, riffs and lead instruments.
- Eight genres today: Mothership calm (the ship's own score, the default), lo-fi, electro swing, deep dubstep, house, drum and bass, synthwave and ambient. Switching genre is heard within seconds.
- You can blend two genres with a slider: tempo, swing and the sound move smoothly, and each part of the music takes one genre or the other.
- The calm score follows where you are (your home, the Commons, the garden, near the engines), the ship's time of day and what you're doing, and it rises with danger and settles back.
- Every note is tuned to A = 432 Hz by default, and A = 440 Hz is one setting away. That's offered as a matter of taste; we make no health claims.
- The ship hums: a soft, deep engine sound under everything aboard, tuned to the music's key so the two never clash, with its own switch and volume. There's also an optional "tones" layer, off unless you turn it on.

**What it never does**
Nothing is downloaded while it plays and nothing goes over the network: each player's own computer composes their music. It uses no GPU at all, so it never costs you frame rate. And it uses no AI to generate music (more on why below).

### Message 2

**How it works, part 1: moods, genres and seeds**

The music is built from three layers of plain data files:

- **A mood** says where you are and what the music is about: calm aboard, a garden morning, a night watch, tension, action, the menu.
- **A genre** says how it sounds: its tempo range, its groove (how swung the beat is), its drum patterns, its harmony rules, which instrument plays each role, its effects and its loudness. Every field is optional, so a genre only changes what it names.
- **A piece is a seed.** One number decides, within the genre's rules: the key (and, in about a third of pieces, a key change in the bridge), the tempo, the form (like AABA or ABAB), every section's chords, the riffs, and which instruments play the lead and the answer. Same seed, same genre: same piece, every time.

**Chords** come from a grammar rather than a fixed list. Each chord has a job (home, moving away, pulling back home), and a section takes a shape like "home, away, pull, home" and fills each slot with a chord that does that job. Then, by chance set in the genre, it adds the touches musicians use: a borrowed dominant leading into the next chord, a substitute chord a tritone away, a passing diminished chord.

**Riffs** come from rules too: how many notes, rhythm cells strung together, the melody's contour, steps of the scale with the odd leap. **Songs have structure**: an intro, verses, choruses, a bridge and an outro, each with its own instruments and energy, with risers and reversed cymbals swelling into a chorus and drum fills at the turns.

### Message 3

**How it works, part 2: the instruments and the mix**

Two kinds of instruments play, and every one is a data file:

- **Synthesisers built into the app:** wavetable and supersaw synths, a pulse-width synth, FM electric pianos and bells, plucked basses, and 808 and 909 style drum machines, all made from oscillators, filters and envelopes the data describes.
- **Recorded instruments from free libraries:** eleven free-to-share instrument libraries, with optional packs to download from Settings (417 instruments in seven full packs). Long recordings play straight from disk, so the full Steinway piano takes about 15 MB of memory instead of 452 MB and sounds the same.

**The mix** gets the effects producers use, all driven by the genre's data: the bass ducking under the kick (sidechain), gated reverb on an 80s snare, tape-style delay, tape wow and flutter, saturation, long reverbs with their lows cut, risers, downlifters and tape stops.

**It stays light.** The whole score runs on the audio thread in about 2% of one processor core on average (a budget of 0.05 ms per audio block), and tests check that every genre and instrument fits it. Up to 64 voices play at once; the calm moods use 12 to 24.

**Why no AI music generation:** the one openly licensed music model we read would share your graphics card with the game, and generated audio can't follow the tuning or where you are the way the live score does. Of the music services we read, one forbids commercial use on its free tier and another's terms couldn't be read. So it's a licence and performance choice, not a gap we forgot.

### Message 4

**The Music page**

In the app, the Music page lets you explore pieces by seed and genre, tune one, keep your favourites, export a piece as a WAV file, and watch the music as a piano roll while it plays. Every favourite has a share code (it starts with `HOSM1-`): send it to a friend and they hear the exact same piece. Whatever you choose there keeps playing after you leave the page.

**Make your own genre**

A genre is one text file. Drop it into `data/music/genres/` next to the app and it appears in the genre list. That's the whole install. Copy an existing genre (synthwave and electro swing are good starting points) and change what you like. A genre can set:

- `tempo_bpm`: its tempo range, like `(80.0, 118.0)`.
- `groove`: the swing (0.5 is straight, 0.667 a triplet shuffle), the grid (eighths or sixteenths), syncopation, and a few milliseconds of human timing.
- `drums`: each drum an instrument with patterns written as text over the grid: `x` a hit, `o` softer, `g` a ghost note, `.` a rest. For example, a four-on-the-floor kick is `x...x...x...x...`.
- bass and pad patterns, written the same way (`x` the root, `5` the fifth, `o` the octave).
- `progressions`: the keys it likes (each with a weight), the chord grammar, and how often the borrowed and substitute chords appear.
- `forms`, `motif_rules` and `leads`: song shapes, how riffs are built, and which instruments take the lead.
- `structure`: the sections (intro, verse, chorus...), how long each is, which parts play in it and its energy.
- the mix: sidechain depth, gated reverb, delay, tape wow, saturation and a level so it sits at the same loudness as the rest.

### Message 5

**The rules for sharing a genre**

- **Capture the genre, never an artist.** A genre's commonplace building blocks (a tempo range, a swing feel, a classic drum pattern, a typical chord move) are free for anyone. An artist's recordings, melodies or riffs are not, and genre files don't name artists. Our own genre files cite their sources in a comment at the top: write yours the same way.
- **Free to share.** Genre files are data, and HumanityOS data is public domain (CC0). Any recorded sound has to be free to share as well: the app never samples copyrighted recordings, because copyright law forbids it.
- **Honest limits:** genres built on recorded timbres (swing-era brass, vocal chops) need free recordings of them, so some styles sound better than others until free samples exist.
- **Help wanted:** free-to-share (CC0) recordings of the sounds we're missing, such as swing-era brass and clarinet played note by note, would make those genres sound much more real.

**Already planned**
- An editor in the app, so moods and motifs can be shaped with live preview and no text editor.
- The same composer on the website.

### Message 6

**Ideas we haven't built yet** (open for discussion)
- **Play cards in chat.** Paste a piece's share code in chat and it shows as a card with a play button, so friends hear it without leaving the conversation.
- **Stems export.** Export each part (drums, bass, pads, lead) as its own WAV, for musicians who want to remix.
- **"What am I hearing?"** A teaching overlay on the piano roll that names each chord's job and the key as they play, so the generator teaches music theory as it goes.
- **Ear training.** A practice game built on the generator: name the interval, the chord or the key.
- **Play along.** Plug in a MIDI keyboard and the score follows your key and tempo, or hands you the lead.
- **Instruments in the world.** Playable pianos and guitars in homes using the same instruments, and buskers or a band in the Commons playing the live score.
- **Jam together.** Players in the same place share one seed and tempo (a few bytes, never audio) and each adds a part.
- **Leitmotifs.** Each crew member gets a short theme that slips into the score when they're near; players could opt in to their own.
- **Radio stations.** A station is a genre blend and a schedule; organisations could run their own.
- **Focus and sleep.** A piece that slowly calms and fades over a set time.
- **Feel the beat.** A visual beat and controller rumble for players who are deaf or hard of hearing.
- **The world in the music.** Weather, the seasons and the homestead's health shaping the score.

Everything is free and open source.
https://united-humanity.us
https://github.com/Shaostoul/Humanity
