# Quaternius Universal Animation Library clips: where they come from

Made by `scripts/gen-quaternius-clips.js` (HumanityOS, character animation rung 1, 2026-11-09):
the clips the game plays, trimmed out of Quaternius's file with their keys unchanged. Do not edit
these files by hand: change the converter and run it again.

## Licence

**CC0 1.0 Universal** (https://creativecommons.org/publicdomain/zero/1.0/), by the `License.txt`
inside the download, copied beside this file unchanged. Quaternius's licence page showed a
different licence (QAL v1.0, "Last updated: 8/28/2026") when it was read on 2026-11-08; the
instrument that travels with these exact bytes is CC0. Finding:
`docs/reference/findings/2026-11-08-character-assets.md`, its 2026-11-09 addendum.

## Source

- Page: https://quaternius.itch.io/universal-animation-library (v3.0 (16/6/2026), the free Standard tier)
- File: `Universal Animation Library[Standard].zip`, 15904933 bytes, SHA-256 `cc73fc4e495b82958207316596317a3f40b9fa38065bde1027937452da537724`
- Downloaded: 2026-11-09, approved by the operator, 2026-11-09 ("Yes, let's see what it has.")
- Read from it: `Universal Animation Library[Standard]/Unreal-Godot/UAL1_Standard.glb` (SHA-256 `69591853d817488edaa8fd9bf8fc1d821eaeaf789f8627b3cd23b41c4ed67997`, root motion off) and, for the
  travel column below only, `UAL1_Standard_RM.glb`.

## The clips, and each gait's ground speed

Speed: measured from the feet (the converter's header says how), the number
`data/characters/clips.ron` uses. Travel: the root-motion file's own root movement over the clip.

| Clip | Seconds | Channels kept (left at rest) | Speed from the feet, m/s | Root-motion travel, m/s |
|---|---:|---:|---:|---:|
| Idle_Loop | 2.500 | 50 (145) |  |  |
| Walk_Loop | 1.333 | 52 (143) | 0.979 | 0.975 |
| Jog_Fwd_Loop | 0.933 | 52 (143) | 5.898 | 5.357 |
| Sprint_Loop | 0.667 | 52 (143) | 8.905 | 8.250 |
| Crouch_Idle_Loop | 2.933 | 51 (144) |  |  |
| Crouch_Fwd_Loop | 2.000 | 51 (144) | 0.717 | 0.750 |
| Jump_Start | 1.333 | 50 (145) |  |  |
| Jump_Loop | 2.500 | 48 (147) |  |  |
| Jump_Land | 1.267 | 52 (143) |  |  |
| Sitting_Enter | 1.300 | 51 (144) |  |  |
| Sitting_Idle_Loop | 1.667 | 46 (149) |  |  |
| Sitting_Exit | 1.033 | 51 (144) |  |  |

## Output hashes (SHA-256)

`node scripts/gen-quaternius-clips.js --check` compares the files with these when the download is
not on the machine.

- `quaternius_ual.gltf` `cd0c1d532fdbfac714c5313ce6282013838be98d792dc8df10867d5fddadf75d`
- `quaternius_ual.bin` `ce448a08b51a4427a9ce6ac828d95e687b0e6a0cbdf023ef29834b58cb521b28`
- `License.txt` `6d01f55c6e4c49a2c9963e147e561945ae2c83958c8ca667d90a6bffdbfac061`
