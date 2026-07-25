# Adding a Planet to HumanityOS

This guide walks you through creating your own planet, step by step.
You do not need any experience with games or programming. If you can
open a folder and edit a text file, you can do this.

## What is a planet in HumanityOS?

A planet is a world you can see in the sky, on the Maps page, and (for
some worlds) walk on. Each planet is described by small text files that
the game reads when it starts. Change the text, and you change the world.

## What you need

1. **A text editor.** This is a program for writing plain text. Windows
   comes with one called Notepad. Any text editor works.
2. **The game folder.** This is the folder that holds the game program
   (`HumanityOS.exe`) and its `data` folder. If you have the source code,
   it is the project folder, for example `C:\Humanity`. All paths below
   start from there.

A planet lives in two layers:

- **Layer 1, the catalog:** one entry in `data/star_systems/sol.json`.
  This file lists all 69 bodies in the solar system (planets, moons, the
  Sun) and says what orbits what. Every view in the game (the Maps page,
  the sky, the orrery) reads this one file.
- **Layer 2, the surface file:** `data/planets/<name>.ron`, for example
  `data/planets/moon.ron`. This optional file turns a plain dot in the
  sky into a detailed world with terrain, colors, and gravity. RON is
  just a text format, like a form with labeled blanks [glossary: RON].

There is also a third, optional layer: `data/solar_system/<name>.ron`
holds encyclopedia facts (Earth, Mars, and the Sun have one). And
`schemas/celestial_body.toml` documents the whole pattern, including a
modding guide, if you want to read more later.

## Step by step

We will copy the Moon and turn it into your own world.

1. Open your file browser and go to the `data\planets` folder inside the
   game folder (for example `C:\Humanity\data\planets`).
2. Find the file `moon.ron`. Copy it, then paste the copy in the same
   folder. Rename the copy to something like `myworld.ron`. Keep the
   `.ron` ending.
3. Open `myworld.ron` in your text editor. Ignoring the comment lines
   (lines that start with `//`), it looks like this:

```
(
    name: "Moon",
    radius: 1737400.0,
    gravity: 1.62,
    terrain_seed: 7,
    ore_seed: 777,
    atmosphere_color: None,
    atmosphere_scale: 0.0,
    has_water: false,
    albedo: Some("planets/moon_albedo.bin"),
    sea_level: 0.45,
    land_color: (0.45, 0.45, 0.45, 1.0),
    water_color: (0.1, 0.1, 0.1, 0.0),
    orbital_radius: 384400000.0,
    orbital_period: 2360592.0,
    rotation_period: 2360592.0,
    axial_tilt: 0.0267,
    surface_relief: 0.025,
    noise_frequency: 3.0,
    noise_octaves: 6,
    ...
    polar_cap_latitude: 2.0,
)
```

   (The `...` stands for a few extra color lines in the real file.)

4. Change one field at a time. Start with `name: "Moon"` and put your
   own name between the quotes.
5. Delete the `albedo:` line. That line points at a real photo map of
   the Moon. Your new world does not have one, so the game will paint it
   from the color fields instead.
6. Try changing `terrain_seed: 7` to any other whole number. A seed is a
   starting number for the random terrain generator. Different seed,
   different mountains.
7. Make it colorful. Each color is four numbers between 0.0 and 1.0:
   red, green, blue, and opacity. For example, `land_color:
   (0.2, 0.6, 0.2, 1.0)` is green land.
8. Save the file. That is the surface layer done.

Want real-world detail instead of generated terrain? Earth uses extra
data files (`data/planets/earth_heightmap.bin`, `earth_albedo.bin`,
`earth_ocean_mask.bin`, and the `earth_tiles` folder) built from NASA
and NOAA sources by the scripts `scripts/build-earth-heightmap.js`,
`scripts/build-earth-albedo.js`, and `scripts/build-planet-albedo.js`.
That is an advanced topic, but it is the same pattern: point the
`heightmap:` and `albedo:` fields at the built files.

## Seeing it in the game

Two situations, two different rules:

- **You edited an existing planet** (like Mars or the Moon). Just
  restart the game. The `.ron` files are plain data, read at startup.
- **You added a brand new body.** It also needs an entry in the catalog,
  `data/star_systems/sol.json`, and that catalog is baked into the game
  program when the program is built (the code in `src/embedded_data.rs`
  embeds it at compile time, and `src/cosmos.rs` reads that embedded
  copy). So a new catalog entry only appears after rebuilding the game
  with `cargo build`. Editing the json file alone is not enough. (You
  may see an older path, `solar_system/bodies.json`, mentioned in places;
  that name is kept only as an alias for the same catalog.)

If you are not set up to rebuild the game, no problem: editing the
existing worlds is the fun part anyway, and everything above works
without rebuilding.

## If something goes wrong

- The game is forgiving. If a file has a mistake in it, the game skips
  that file and keeps running instead of crashing. Your broken planet
  just will not show up, or will fall back to defaults.
- If you have the source code, run `just validate-data` in a terminal.
  It checks every data file in about a fifth of a second and tells you
  exactly which line is wrong.
- Otherwise: restart the game, and if the planet looks wrong, compare
  your file against `data/planets/moon.ron` or `data/planets/mars.ron`.
  A missing comma or quote is the usual culprit.
- Worst case, delete your new file and copy `moon.ron` again. You cannot
  break anything permanently by editing these files.

## Field reference

One line per field, in plain words. Fields marked (optional) can be
left out entirely.

- `name`: the display name of the world.
- `radius`: size of the planet in meters (Earth is 6371000.0).
- `gravity`: pull at the surface in meters per second squared (Earth is 9.81).
- `terrain_seed`: starting number for the terrain generator; any whole number.
- `ore_seed`: starting number for where ores and resources appear.
- `atmosphere_color`: color of the air glow, or `None` for airless worlds.
- `atmosphere_scale`: thickness of the air layer as a fraction of the radius (Earth is about 0.015).
- `has_water`: `true` if the world has liquid water, `false` if not.
- `sea_level`: how high the water sits, from 0.0 (none) to 1.0 (highest peaks).
- `land_color`: base color of low land (red, green, blue, opacity, each 0.0 to 1.0).
- `water_color`: color of the oceans.
- `orbital_radius`: distance from the Sun (or parent body) in meters.
- `orbital_period`: how long one trip around the orbit takes, in seconds.
- `rotation_period`: how long one day lasts, in seconds.
- `axial_tilt`: how far the spin axis leans, in radians (Earth is about 0.41).
- `surface_relief`: how tall mountains get, as a fraction of the radius (Earth is about 0.02).
- `noise_frequency`: higher numbers give more, smaller continents.
- `noise_octaves`: 1 to 8; more octaves give rougher, more detailed ground.
- `shore_color`: color of the beach band just above the water.
- `highland_color`: color of mid-height land.
- `mountain_color`: color of high rocky ground.
- `cap_color`: color of polar ice caps and the very highest peaks.
- `basin_color` (optional): color of low basins, or `None` to auto-darken the land color.
- `polar_cap_latitude`: where ice caps start; a value above 1.0 turns caps off.
- `heightmap` (optional): path to a real elevation data file, or leave out for generated terrain.
- `albedo` (optional): path to a real surface color map; when present it replaces the color fields above.
- `scale_height_m` (optional): thickness of the visible atmosphere shell, in meters.
- `cloud_coverage` (optional): how cloudy the sky is, 0.0 (clear) to 1.0 (overcast).

Happy world building.
