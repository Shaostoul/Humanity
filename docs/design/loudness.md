# Loudness: every sound at the loudness of its kind, and the volume a video starts at

Written 2026-11-06. Rung 1 is built (the meter, the kinds as data, the shipped sounds levelled,
the Settings switch); the video start-volume choices are built in both apps; each shipped sound's
own level in Settings is built (section 3). The later rungs are planned here in full, so the first
one is a rung of the real thing.

## What was asked

The operator, 2026-11-06, about chat videos (then: one volume and Mute for every chat video, kept
only for the run):

> "Let's add the toggle option maintain last used volume setting between app resets or don't.
> Another could be all videos start at custom % volume. So I could set them at say 5% for every new
> video I start because, not everyone balances audio the same. Are we able to normalize audio
> volumes between everything? Like speaking is always at X volume. SFX are always X volume.
> Instrumentals are X. Etc."

Two features, then: a choice of the volume a new video starts at (built, section 1), and levelling
every sound the app plays to a loudness set per kind of sound (designed in full, rung 1 built,
section 2).

## 1. The volume a new video starts at (built)

**Settings > Audio > Videos**, one choice for every video the app plays (a video posted in chat,
the Media page's player, a video on a screen in the world), saved in the config:

- **My last volume** (the default): a new video starts where the last video's volume bar was left,
  kept across restarts.
- **A set volume**, with a slider (0 to 100%, default 100%): every new video starts there, 5% say,
  whatever the last one was turned to. Each video's own bar still turns that video up or down.

Why two choices and a slider, and not the three ("last", "set", "full") or the "remember" toggle
the brief floated: full volume IS the set volume at 100%, so a third choice would be a second way
to say one thing; and a toggle whose off state quietly means "use the slider" hides how the two
relate, where a choice names both. Mute is not remembered: a video that starts silent the next
day, with no reason on the screen, reads as broken.

How it is built:

- `crates/humanity-core/src/audio/start_volume.rs` holds the choice and the last volume
  process-wide (the way the chat's timestamp format is held): the config gives them at load and
  reads them at save (`video_start`, `video_start_volume`, `video_last_volume`), Settings edits
  them, and each player asks `start_volume()` when a video starts. Moving any video's volume bar
  calls `note_volume`, which marks the settings unsaved; the main loop turns that into a save.
- The players: `VideoView::start` (the Media page, and every chat video under it),
  `ChatVideos::start` (a chat video starting fresh; the chat's one shared volume follows it, as one
  video plays at a time), and the screen provider's `open_player` (`engine/screens/video.rs`). The
  Media page's player and a screen's control strip gained a volume bar for this (a 5% start with
  no bar would have no way back up but Settings); a screen's volume multiplies into its mix
  (`compose_mix`: master x sound effects x its own volume x distance).
- The chat video's volume bar has a small cog beside it, "Volume settings", which opens Settings >
  Audio (`chat::follow_volume_settings_link`).
- **The website** (`web/shared/video-volume.js`, loaded by the chat and the Media page): the same
  choice in Settings > Audio & Video > Videos, kept in the browser (`video-start`,
  `video-start-pct` in the shared preferences; the last volume under `hos_video_last_volume`). It
  gives each `<video>` its start volume on its first play and keeps the person's moves of the
  browser's own volume bar as the last volume.
- Tests (each seen red first): `start_volume::tests`, `config::tests::the_video_volume_choices_survive_a_restart`,
  `chat::inline_video::tests::a_new_chat_video_starts_at_the_chosen_start_volume`,
  `widgets::video_view::start_volume_tests::a_media_page_video_starts_at_the_chosen_start_volume`,
  `engine::screens::video::tests::a_screen_video_starts_at_the_chosen_start_volume_and_its_bar_sets_the_last`
  (a drawn press on the strip's bar), and `scripts/tests/video-volume-web.test.js`.

## 2. Levelling: every sound at the loudness of its kind

### The honest limits, first

- **A video or a song is ONE mixed track.** The speech, the music and the effects inside it were
  mixed together by whoever made it; nothing here can turn the music in a video down and the voice
  up. What can be done, and what this does, is level each SOURCE as a whole: a video, a song, a
  voice in a call, a sound file. A video with a loud music bed and quiet speech stays that way
  inside, at a sensible overall loudness. EBU R 128 measures a programme the same way: "the audio
  signal shall generally be measured in its entirety, without emphasis on specific foreground
  elements such as speech, music or sound effects" (R 128-2023, item l).
- **Loudness is a measurement of how loud a sound seems, not a guarantee.** BS.1770 matches
  listening tests well for speech and music (its Annex 1: correlation 0.977 over 336 recordings)
  and says itself that it "is not, in general, suitable for use to estimate the subjective loudness
  of pure tones". A very short click is the weakest case (next section).
- **A quiet sound is raised only so far.** Never more than 12 dB, and never so far that its highest
  point as played passes -1 dBTP (the sound card would clip it). A nearly silent recording stays
  quiet rather than having its hiss raised to full loudness.
- **The person's own volume always wins.** Levelling sets where each sound starts from; Master,
  Music, Sound effects and Interface (and later one slider per kind) apply on top. "Even out
  loudness" (on by default) turns it all off, and the app plays every file as it was recorded.

### Measuring loudness: ITU-R BS.1770

`crates/humanity-core/src/audio/loudness.rs`, written here (no crate; it is two filters, a mean
square and a gate). From ITU-R BS.1770-5 (11/2023): each channel K-weighted (a high shelf of about
+4 dB above about 1.5 kHz, the head; a high-pass under about 40 Hz, the ear's weak bass; both given
in the Recommendation as 48 kHz biquads, moved to other rates through the continuous-time filter
with pre-warping), mean square, channels summed with their weights, `-0.691 + 10 log10(sum)` in
LUFS. The integrated loudness gates 400 ms blocks (100 ms apart): under -70 LUFS is silence, more
than 10 LU under the rest is a pause. EBU Tech 3341 (V4, 11/2023) adds the live windows, Momentary
(400 ms) and Short-term (3 s). True peak (BS.1770-5 Annex 2) is 4x oversampling with the Annex's
own 48-tap interpolator.

Proof, `audio::loudness::tests`: EBU Tech 3341's minimum-requirements test signals synthesised from
its own table (cases 1 to 6, 9, 10, 12, 13 and the true-peak cases 15 to 19; 7 and 8 are recordings,
20 to 23 need a 4x-rate synthesis with its own anti-aliasing filter), each within the table's
tolerance, at 48, 44.1, 22.05 and 96 kHz for the steady tones; and BS.1770's own statements (its
filter tables come back exactly at 48 kHz; a full-scale 997 Hz sine in one channel reads -3.01; the
-0.691 cancels the filter's +0.691 dB at 997 Hz). The relation the brief asked to verify: a 1 kHz
sine at -23 dBFS reads -23.0 LUFS when it is in BOTH channels of a stereo pair (Tech 3341 case 1);
in one channel alone it reads -26.0 (a sine's mean square is 3 dB under its peak, and the second
channel adds the 3 dB back).

**Measuring a short sound** (our choice, not the standard's: BS.1770 is for programmes, and a 0.2 s
click has no complete 400 ms gating block): a sound of 3 s or less is a one-shot and its loudness is
its loudest Momentary window, the rest of the window silence; a longer sound (a loop, a song, a
voice, a video) its integrated loudness. Every sound is measured AS IT PLAYS: the mixer sends a mono
file to both speakers, so it is measured as two channels, the same way as every other sound, and
the readings compare.

### The kinds and their targets (data: `data/audio/loudness.toml`)

A target is the loudness of a sound of that kind at authored volume 1.0. A video or a voice has no
authored volume, so it plays AT its target; a shipped sound's `volume` in `data/sounds.toml` then
says how far under it the sound sits (0.25 is 12 dB under: a footstep is softer than a crash). A new
kind is a new table in the data file; a sound names its kind with `category` in `sounds.toml`, else
its bus's kind is used (`[buses]`).

| Kind | Target | Why this number |
|---|---|---|
| speech | -16 LUFS | Apple's podcast requirement: "around -16 dB LKFS, with a +/- 1 dB tolerance"; AES TD1008's speech figure is -18 (secondary reading, below). The anchor the rest are set around. |
| video | -16 LUFS | As loud as a person talking; between the streaming services' music levels (Spotify -14, AES TD1008 music -16). |
| music | -18 LUFS | 2 LU under speech, so a voice stays clear over the game's music; the Music slider moves it from there. |
| sfx | -21 LUFS | The average of the shipped sound effects (2026-11-06: -20.9), so turning levelling on keeps a typical effect where it was and brings the loud and quiet files to it. |
| ambient | -26 LUFS | A bed under everything; no shipped ambience yet. |
| interface | -20 LUFS | The average of the shipped interface sounds (2026-11-06: -19.5); their authored volumes (0.2 to 0.55) put them 5 to 14 dB under it. |
| notification | -15 LUFS | The average of the two shipped (the notification and the chat chime), heard over a video or the game. |

The limits: `ceiling_dbtp = -1.0` (EBU R 128: "the True Peak Level of a programme shall not exceed
-1 dBTP"; Apple's podcasts and Spotify's limiter use the same -1), counted at the sound's authored
volume (most shipped files peak near full scale while playing 6 to 14 dB under it; without that,
a quiet footstep at volume 0.25 could not be raised even 1 dB when as played it has 8 dB of room);
`max_boost_db = 12`.

**What changes for the shipped sounds when it is on** (the default), measured 2026-11-06: within a
kind, sounds of the same authored volume now play at one loudness (the seven footsteps, all at 0.25,
were 6.1 LU apart and are now level; the doors and blocks at 0.5, 12 LU apart). The quietest files
come up and the loudest come down: the grass footstep (the planet's) about +4.7 dB, the button click
about +8.9 dB (its file is among the quietest interface sounds, -28.9 LUFS), the success chime about
-11 dB; the item pickup (-33.9 LUFS) is held by the 12 dB raise limit, 1.9 LU short of its kind.
The authored volumes were tuned by ear against the raw files; with levelling on they mean "how far
under the kind", and any that now sounds wrong is a one-number change in `sounds.toml`, or, for the
person's own ear, a slider in Settings > Audio > Sound levels (section 3).

### Mechanism, per kind of source

1. **Shipped sound files: BUILT (rung 1).** Measured once from the repo's own files into
   `data/audio/measured_loudness.toml` by `just loudness` (data made from the repo, the way the
   Library is made from the Accord, not a cache: nothing at run time writes it). The lib test
   `the_measured_loudness_data_matches_the_shipped_sounds` (in `just validate-data`) fails when a
   sound file changes without it. `AudioManager` folds the level into each sound's volume when it
   starts (`one_shot`, `play_music`, `play_spatial`); a file the data does not list (a mod's) is
   measured when it loads.
2. **A video, a received song or sound: rung 2.** Measured as it plays, on kira's decode thread
   (a levelling wrapper around the player's `streaming::Decoder`, `media/audio.rs` and
   `media/system.rs`, so the real-time audio callback does no extra work): Short-term loudness
   (3 s) drives the gain toward the video target, moved slowly (about 1 dB per second, faster down
   than up) so speech and music inside it never "pump"; it starts at the loudness of the first 3 s,
   measured on the fetch worker before Play, so the first seconds are not wrong; never raised past
   12 dB; and a true-peak limiter at -1 dBTP (look-ahead of a few milliseconds, using the meter's
   interpolator) catches what the slow gain cannot. The same wrapper serves a screen's clip and a
   song on the Media page. The website: Web Audio (`MediaElementAudioSourceNode` into an
   `AudioWorklet` meter and a `GainNode`); honest limit, a browser outputs silence from a cross-site
   video without CORS, so only videos on the page's own server can be levelled there.
3. **Live voice: rung 3.** `net/voice.rs` has NO automatic gain today (checked 2026-11-06: the
   chain is the person's mic gain, a high-pass, the noise filter, the transmit gate, Opus). The plan
   levels each REMOTE voice on the receiving side, per person, toward the speech target (momentary
   loudness measured only while they speak, the gain slewed slowly, the same raise limit and
   ceiling), so everyone in a call is as loud as everyone else whatever their microphone, and the
   listener stays in control. An opt-in "Automatic mic level" on the sending side (rung 5) helps the
   people hearing someone on the website or an older app.
4. **Per-kind sliders: rung 4.** Settings > Audio gains a slider per kind the person meets (Speech,
   Video, Notifications beside the existing Music, Sound effects and Interface), read from the kinds
   in the data file, so a new kind brings its slider. (Each single sound already has its own level,
   section 3; a kind's slider will sit between the bus sliders and those.) **Notifications is
   built (2026-11-07, BUG-265)**: a "notifications" bus in `AudioManager` with its own slider,
   Settings > Audio > Volumes' Notifications Volume (saved as `AppConfig::notifications_volume`).
   The chimes are catalogued on it (`sfx.notification`, `sfx.chat_message` and `sfx.success`, the
   three sounds a notice plays and nothing else does; `[buses]` maps it to the notification kind),
   and a notice plays its chosen sound on it whatever that sound's own bus (the "soft" and "pop"
   choices are the menu-open and item-pickup files, which stay interface sounds elsewhere), so the
   UI Volume at 0 never silences a notice. `sfx.error` stays an interface sound: it answers the
   person's own action, as a click does, and nothing plays it as a notice.
5. **Dual modes** (CLAUDE.md, every deep system): the full mode is levelling on; the simple one is
   the switch off, every file as recorded.

### The reuse switch

The measured table is data. What is kept in memory obeys the Dev page's "turn off all reuse"
switch: `AudioManager`'s decoded sounds (it never asked before: BUG-257), its measurements of unlisted
files, and the table itself: with reuse off every sound is decoded and measured on the spot each
time it plays and the table is not trusted, so a stale table is ruled in or out with one click
(`levels::level_for`'s `trust_data`; `with_reuse_off_a_sound_is_measured_not_looked_up`). With reuse
off the mixer also reads the sound catalogue and the targets again at every play (`read_data`), and
Settings > Audio > Sound levels reads its list of shipped sounds every frame instead of every 5 s.
The live levellers of rungs 2 and 3 keep only the state of the sound playing (its meter and gain),
which is not reuse; anything they keep across sources will obey the switch.

### Budget

- **Rung 1: nothing on the audio thread.** The level is one multiply folded into the volume kira
  already takes when a sound starts. On the game thread, per sound started: a hash lookup and one
  `powf`. With reuse off: a decode and a measure per play (a few hundred microseconds for a 1 s
  one-shot in a release build), the price of the diagnostic.
- **Rung 2, per playing source, on kira's decode thread:** two biquads per channel (about 20
  operations a sample), the true-peak interpolator (48 multiply-adds per sample per channel), the
  3 s window's sum every 10 ms: about 6 million operations a second for 48 kHz stereo, roughly
  1 to 2 ms of one core per second of sound, and nothing on the real-time callback. Network: none;
  levelling is local, like every cosmetic sound.
- **Rung 3, per remote voice:** the same meter without true peak on 48 kHz mono, about 1 million
  operations a second each.

### Sources (read 2026-11-06)

- ITU-R BS.1770-5 (11/2023), "Algorithms to measure audio programme loudness and true-peak audio
  level": https://www.itu.int/rec/R-REC-BS.1770 . The filter tables, equations 1 to 7, Table 3's
  channel weights, Annex 2's true-peak filter, and "If a 0 dB FS, 1 kHz (997 Hz to be exact ...)
  sine wave is applied to the left, centre, or right channel input, the indicated loudness will
  equal -3.01 LKFS."
- EBU Tech 3341 V4 (11/2023), "Loudness Metering: 'EBU Mode' metering to supplement EBU R 128
  loudness normalisation": https://tech.ebu.ch/docs/tech/tech3341.pdf . Table 1's test signals;
  "a 1 kHz stereo sine-wave ... with its peak level at -18 dBFS ... The meter should read -18.0
  LUFS."
- EBU R 128 (2023, V5), "Loudness normalisation and permitted maximum level of audio signals":
  https://tech.ebu.ch/docs/r/r128.pdf . Target -23.0 LUFS for broadcast; "the True Peak Level of a
  programme shall not exceed -1 dBTP"; item l, quoted above.
- Spotify for Artists, "Loudness normalization" (no date on the page):
  https://support.spotify.com/us/artists/article/loudness-normalization/ . "We adjust tracks to
  -14 dB LUFS, according to the ITU 1770 standard"; Loud -11, Quiet -19; a limiter at -1 dB.
- Apple Podcasts, "Audio requirements" (no date on the page):
  https://podcasters.apple.com/support/893-audio-requirements . "around -16 dB LKFS, with a +/- 1 dB
  tolerance", true peak not above -1 dB FS, both by BS.1770-5.
- AES TD1008 (2021), "Recommendations for loudness of Internet audio streaming and on-demand
  distribution": the AES's own copy refused automated reading (HTTP 403), so its numbers here are a
  secondary reading, https://productionadvice.co.uk/td1008/ (dated 2021-10-01): speech -18 LUFS,
  music -16. Read the AES text before quoting them as the AES's.

## 3. Each sound's own level (built)

The operator, 2026-11-06, told that levelling made the button click about 9 dB louder and that it
is "a one-number change in the sound data":

> "Can we edit them in the settings menu?"

**Settings > Audio > Sound levels** lists every sound the game ships (each entry of
`data/sounds.toml` whose file exists; a catalogue entry with no file plays nothing), grouped by its
kind (the kinds above, by their names in the person's words), each with its name by key
(`sound_names.<id>` in `data/i18n/en.json`), a level slider from -24 dB to +12 dB, the level in dB,
Play and Reset; above them a search box and Reset all, and a line saying how the levels sit on top
of "Even out loudness".

- **What 0 dB is.** The sound as the app plays it on its own: with levelling on, at its kind's
  target times its authored volume; with it off, its file as recorded times that volume. A level
  moves it from there by exactly its dB (an amplitude of 10^(dB/20)), and Master and the bus slider
  apply on top. -24 is the floor (close to silent beside everything else; the bus slider silences
  outright), +12 the ceiling, the same limit levelling puts on its own raise; levels are kept on a
  half-dB grid so the number shown is the number kept. Raising past a file's headroom can clip:
  the hint says so, and the person's own choice is not second-guessed.
- **Where it is kept.** The person's config (`AppConfig::sound_levels`, by sound id: a sound that
  later points at another file keeps its level), only the sounds changed (Reset takes the entry
  out). `data/sounds.toml` is never written: it stays the shipped default for everyone.
  At run time the levels are held by `humanity_core::audio::sound_levels`, the way the video start
  volume is: the config gives them at load and reads them at save, Settings edits them.
- **Where it is applied.** `AudioManager::one_shot` (and `play_music`, `play_spatial`) multiplies
  the sound's level in as the sound starts, after levelling; the catalogue tells the mixer which id
  a file is (`SoundCatalog::ids_by_sound_key`, read with the targets). One lock and one lookup per
  sound started, nothing on the audio thread. A file no catalogue entry names (a video, a mod's
  sound) has no level.
- **Play.** The row's Play asks the engine for the sound; the engine's audio stage
  (`sound_levels::play_preview`) plays it through `AudioManager::preview`, which is the game's own
  one-shot of it (its authored volume, its bus, its loudness level, the level as it is now) and
  stops the previous preview first. So a slider just moved is heard at once, exactly as the game
  will play it. The bus is the catalogue's: the interface volume for an interface sound, the
  notifications volume for a chime, the music volume for music, the ambience volume for an ambient
  sound, the sound-effects volume otherwise. The game plays every catalogued sound on that same bus
  (`engine/sfx.rs`: the queue of sounds raised by actions, the ECS systems' channel and the click),
  so Play and the game agree; a notice plays its sound on the notifications bus, which for the chimes
  is their catalogue bus too. (Until BUG-265, 2026-11-07, the queue played every sound on the
  sound-effects bus: with the UI Volume at 0 a chime still sounded and its Play was silent.
  `engine::sfx::tests::a_chime_plays_at_its_play_level_on_the_notifications_volume` holds it.)
- **The list** is read from the data and the disk when the section is drawn, again every 5 s while
  it is open, and every frame with the Dev page's "turn off all reuse" switch off.
- **The website** plays no game sounds: its only sounds are the notification sounds of
  `data/notifications/notifications.json` (`sounds`: the chat message chime, the alert, and the
  bright, soft and pop sounds a person can pick for a kind of notice), played by the chat
  (`chat-notify.js` `playSound`) and Settings' Play buttons, always at 60% volume before this. So
  its Settings > Audio & Video > Sound levels lists those five, named as the notification sound
  picker names them, with the same slider, Play, Reset and Reset all, and says every other sound is
  set in the desktop app (`web/shared/sound-levels.js`). Kept in the browser (`hos_sound_levels`),
  separate from the desktop app's. It has no Notifications volume (BUG-265 gave the desktop one):
  every sound the website plays is a notice's, so a bus slider there would be a second control over
  the same five sounds, which the browser's and the system's own volume already scale; the website
  has no volume slider for any sound, only each sound's level. 0 dB there is the website's 60%; a browser's `<audio>` volume
  stops at 1.0 (+4.4 dB over 60%), so a level past it goes through Web Audio (the element at full
  volume into a `GainNode`; the files are on the page's own server, so Web Audio may read them),
  and plays at full volume where Web Audio is missing. The website does not level loudness (rung 2
  plans it), so its 0 dB is the file as recorded at 60%.
- Tests (each seen red first): `audio::sound_levels::tests` (a level moves a sound by exactly its
  dB, Reset returns it to its default, a sound with no level plays at its default, the config's
  levels come back clean), `audio::sound_level_tests` (through `one_shot` and `preview` on kira's
  mock backend, `AudioManager::without_device`: with levelling on and off), `config::tests::
  a_sound_level_survives_a_restart`, `gui::pages::settings::sound_levels::tests` (the list shows
  only shipped files and every one has its name; a drawn press on the click's slider then its Play
  previews at -24 dB through the engine's half; Reset and Reset all as drawn; the search), and
  `scripts/tests/sound-levels-web.test.js`.
