# Adding a Sound to HumanityOS

This guide shows you how to add your own sound to the game, step by step. You do not need any programming experience. If you can open a folder and edit a text file, you can do this.

## What is a sound in HumanityOS?

A sound in HumanityOS is two things working together: an audio file (the actual noise, saved in a format called `.ogg`) and one small entry in a text file that tells the game about it. The text file gives the sound a name, says how loud it should be, and says whether it should get quieter as you walk away from it. The game reads that text file when it starts up and learns every sound it can play.

## What you need

1. **A text editor.** This is a program for editing plain text files. Windows comes with one called Notepad. Any text editor works.
2. **The game folder.** This is the folder where HumanityOS is installed on your computer. Inside it you will find two folders that matter for this guide:
   - `assets/audio/` : where the actual audio files live, sorted into subfolders like `ui/` (interface clicks) and `sfx/` (sound effects). For example, `assets/audio/ui/button_click.ogg` is the click you hear on buttons.
   - `data/sounds.toml` : the text file that lists every sound. TOML is just a simple, human-readable text format. You edit it like any other text file.
3. **An audio file in `.ogg` format.** OGG (also called Vorbis) is a free, open audio format, like MP3 but with no licensing strings attached. Many free tools (for example, the free program Audacity) can save or convert audio to `.ogg`. If you use someone else's sound, check that you are allowed to. The file `assets/audio/LICENSE.md` shows how the game's own sounds are credited.

## Step by step

We will add a new sound by copying an existing entry and changing it. Copying something that already works is the safest way to start.

1. **Put your audio file in place.** Copy your `.ogg` file into a subfolder of `assets/audio/`. For a sound effect, use `assets/audio/sfx/`. For an interface sound, use `assets/audio/ui/`. Give it a simple lowercase name with underscores, like `my_sound.ogg`. (The catalog also mentions `ambient/`, `music/`, and `voice/` folders. Those are planned but do not exist on disk yet, so stick with `ui/` or `sfx/` for now.)
2. **Open the catalog.** In your text editor, open `data/sounds.toml`.
3. **Find an entry to copy.** Scroll until you find a real entry that is similar to what you want. Here is one that already exists in the file, exactly as written:

   ```toml
   [sfx.footstep_grass]
   path = "audio/sfx/footstep_grass.ogg"
   volume = 0.4
   loop = false
   spatial = true
   falloff_min = 1.0
   falloff_max = 15.0
   bus = "sfx"
   variations = ["sfx/footstep_grass_01.ogg", "sfx/footstep_grass_02.ogg", "sfx/footstep_grass_03.ogg"]
   tags = ["movement", "surface"]
   ```

4. **Copy and paste it.** Select the whole block, copy it, and paste it at the bottom of the file, leaving one blank line above it.
5. **Rename it.** Change the first line, the part in square brackets. `[sfx.footstep_grass]` becomes `[sfx.my_sound]` (keep `sfx.` at the front for a sound effect, or use `ui.` for an interface sound). This name is the sound's ID. The game and other data files refer to your sound by this dotted name, for example `sfx.my_sound`.
6. **Point it at your file.** Change the `path` line to your file, for example `path = "audio/sfx/my_sound.ogg"`. The path always starts from inside the `assets/` folder, so you write `audio/sfx/...`, not `assets/audio/sfx/...`.
7. **Adjust the other fields one at a time.** Set `volume` between 0.0 (silent) and 1.0 (full). Set `loop = true` only if the sound should repeat forever (like rain). Set `spatial = true` if the sound comes from a place in the 3D world, or `false` if it should sound the same everywhere (like a menu click). If you do not need `variations` or `tags`, you can delete those two lines entirely. The "Field reference" section below explains every field.
8. **Save the file.** That is it for editing.

## Seeing it in the game

The game reads the sound catalog **once, when it starts**. In technical terms: `src/lib.rs` (line 1563) calls `SoundCatalog::load("data")` at engine startup, which parses `data/sounds.toml` into a lookup table of dotted IDs (the code for this lives in `src/audio/sounds.rs`).

So after you add or change an entry, **fully close and restart the game** to hear the change. The comment at the top of `data/sounds.toml` says "Hot-reloadable via FileWatcher", meaning changes would apply while the game runs. That is aspirational: nothing in the code reloads the catalog yet, so a restart is required.

When something in the game wants to play your sound, it looks the dotted ID up in the catalog, which supplies the file path and settings you wrote.

## If something goes wrong

- **Check your work automatically.** If you have the developer tools set up, open a terminal in the game folder and run `just validate-data`. It reads all the data files and reports mistakes (like a typo in the TOML) in a fraction of a second.
- **No tools? Just restart the game.** The game is built to skip broken entries rather than crash. If your sound does not play, the usual causes are: a typo in the file name, a `path` that does not match where you put the file, or a missing quote mark or bracket in your entry.
- **Compare against a working entry.** Put your entry side by side with `[sfx.footstep_grass]` and look for differences in punctuation. TOML cares about quotes, brackets, and the equals signs.

## Field reference

Every field you can put in a sound entry, in plain words:

- `path` : where the audio file is, starting from inside `assets/`. Example: `"audio/sfx/my_sound.ogg"`. Required.
- `volume` : how loud, from 0.0 (silent) to 1.0 (full). If you leave it out, the game uses 0.5.
- `loop` : `true` means the sound repeats forever until stopped; `false` means it plays once.
- `spatial` : `true` means the sound comes from a spot in the 3D world and gets quieter with distance; `false` means it plays at the same level no matter where you are.
- `falloff_min` : for spatial sounds only. Within this many meters, the sound is at full volume.
- `falloff_max` : for spatial sounds only. Beyond this many meters, the sound is silent. If you leave it out, the game uses 20.0.
- `bus` : which volume slider in the settings controls this sound. One of `"ambient"`, `"music"`, `"sfx"`, `"voice"`, or `"ui"`. If you leave it out, the game uses `"sfx"`.
- `variations` : optional list of alternate audio files. Each time the sound plays, the game picks one at random, so repeated sounds (like footsteps) do not sound robotic.
- `tags` : optional list of descriptive words. They help organize and find sounds; they do not change how the sound plays.

One note for the curious: there is a file called `schemas/sound.toml` in the repository, but it is out of date. It describes an older CSV-based sound format with fields (like `pitch` and `priority`) and folder paths that no longer exist. The real, authoritative list of fields is the `SoundEntry` structure in `src/audio/sounds.rs`, which is exactly what this guide documents.
