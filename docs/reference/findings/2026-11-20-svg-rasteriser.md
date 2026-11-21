# An SVG rasteriser for the desktop's emoji: which one, under what licence, and what it downloads

**Researched: 2026-11-20.** Not legal advice: this was written by a developer reading primary
sources (each project's own licence file or README, and crates.io's published metadata), not by a
lawyer.

**The question.** Which open SVG renderer should the desktop app use to draw the emoji pack's
pictures (`assets/emoji/`, OpenMoji 17.0.0; `docs/design/emoji.md` rung 2), under what licence, and
which crates would adding it download?

## The short answer

- **resvg**, with its parser **usvg** and its rasteriser **tiny-skia** (all three now under the
  Linebender organisation). resvg and usvg are "Apache-2.0 OR MIT", tiny-skia is the New BSD
  (BSD-3-Clause) licence: permissive, compatible with the project's AGPL-3.0 code, asking only that
  their copyright notices travel with the program. All Rust, no C code, actively released (resvg
  0.50.0 on 2026-11-11), and the renderer egui's own image loaders use (`egui_extras`'s `svg`
  feature is `resvg`).
- **Recommended line**, for the operator to approve (it downloads crates):
  `resvg = { version = "0.50", default-features = false, optional = true }` in the root
  `Cargo.toml`'s `[dependencies]`, and `"dep:resvg"` added to the `native` feature's list. Without
  default features it leaves out text (fonts, shaping), raster images inside SVGs and gzip-packed
  SVGs: the pack uses none of them (counted below).
- **What that downloads: 12 crates, about 909 KiB compressed** (crates.io's own sizes):
  resvg 0.50.0 (78 KiB), usvg 0.50.0 (147 KiB), tiny-skia 0.12.0 (262 KiB), tiny-skia-path 0.12.0
  (48 KiB), kurbo 0.13.1 (165 KiB), roxmltree 0.21.2 (56 KiB), svgtypes 0.16.2 (46 KiB), imagesize
  0.15.0 (29 KiB), data-url 0.3.2 (23 KiB), simplecss 0.2.2 (22 KiB), polycool 0.4.0 (22 KiB),
  pico-args 0.5.0 (11 KiB). Everything else it needs is already in `Cargo.lock` (bytemuck, log, rgb,
  siphasher, arrayref, arrayvec, cfg-if, strict-num, smallvec, memchr, png).
- **Correction, 2026-11-20 (found while adding it):** the real download is **14 crates, about
  1,005 KiB**. The walk above missed two. **float-cmp 0.9.0** (10,102 bytes, MIT) comes in because
  usvg 0.50 asks for strict-num with its default features, and strict-num's default `approx-eq`
  feature turns float-cmp on, so it is compiled. **euclid 0.22.14** (88,222 bytes, MIT OR
  Apache-2.0) comes in because kurbo 0.13's `std` feature names `euclid?/std`, a weak feature. That
  writes euclid into Cargo.lock, and Cargo then downloads it even though nothing turns kurbo's
  `euclid` feature on, so it is never compiled. `cargo update --workspace` listed both, and the
  first build fetched both. The operator approved both on 2026-11-20 (euclid: "We'll keep it for
  now. Less work for us."). Lesson: count what a new dependency adds to Cargo.lock, not only what
  its active features need.
- **A smaller alternative**: resvg 0.45.1 (2025-04-17) downloads 10 crates, about 519 KiB, because it
  uses the tiny-skia 0.11.4 already in `Cargo.lock`; but it is five releases and 19 months behind,
  and the house rule is to keep components current (memory `feedback_keep_components_current`), so
  it would be an update owed from day one.
- **Declined**, each for a stated reason below: lyon (tessellation, no anti-aliasing, no clipping),
  vello (GPU compute on a different wgpu), NanoSVG bindings (C, last release 2019), librsvg (LGPL,
  needs cairo, pango and glib), zeno (paths only, no SVG). A renderer of our own over tiny-skia 0.11.4
  and quick-xml 0.41 (both already in `Cargo.lock`, nothing to download) is possible for the pack as
  it is, and is the fallback if no download is approved.

## Findings

### 1. resvg and usvg: Apache-2.0 OR MIT

- Source: <https://github.com/linebender/resvg/blob/main/README.md>, read from the main branch on
  2026-11-20 (the release it describes, 0.50.0, was published 2026-11-11 per crates.io). The
  "License" section, verbatim (its Markdown source, so the links stay as written):

  ```text
  Licensed under either of

  - Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
  - MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/license/mit>)

  at your option.
  ```

- Source: <https://github.com/linebender/resvg/blob/main/LICENSE-MIT>, read 2026-11-20. It opens
  "Copyright 2017 the Resvg Authors" and its one condition is:

  > The above copyright notice and this permission notice
  > shall be included in all copies or substantial portions
  > of the Software.

- Source: crates.io's API, <https://crates.io/api/v1/crates/resvg> and
  <https://crates.io/api/v1/crates/usvg>, read 2026-11-20: both 0.50.0 and 0.45.1 declare
  `"license": "Apache-2.0 OR MIT"`; 0.50.0 published 2026-11-11, minimum Rust 1.85 (the project
  is on 1.99.1). The README calls it "an SVG rendering library" written completely in Rust, with "a
  vast test suite that includes around 1600 tests".

### 2. tiny-skia: the New BSD licence

- Source: <https://github.com/linebender/tiny-skia/blob/main/LICENSE>, read 2026-11-20 (tiny-skia
  0.12.0 published 2026-02-02 per crates.io). It names "Copyright (c) 2011 Google Inc." and
  "Copyright (c) 2020 Yevhenii Reizner", and among its conditions:

  > Redistribution and use in source and binary forms, with or without
  > modification, are permitted provided that the following conditions are
  > met:
  >
  > ...
  >
  >   * Redistributions in binary form must reproduce the above copyright
  >     notice, this list of conditions and the following disclaimer in
  >     the documentation and/or other materials provided with the
  >     distribution.

- Source: <https://github.com/linebender/tiny-skia/blob/main/README.md>, read 2026-11-20, its
  "License" section, in its Markdown source:

  ```text
  The same as used by [Skia]: [New BSD License](./LICENSE)
  ```

  crates.io declares `"license": "BSD-3-Clause"` for 0.12.0.

### 3. What egui uses

- Source: <https://github.com/emilk/egui/blob/0.31.1/crates/egui_extras/Cargo.toml> (the tag of the
  egui the project is on; egui_extras 0.31.1 published 2025-03-05 per crates.io), read 2026-11-20:

  > svg = ["resvg"]

  > resvg = { version = "0.37", optional = true, default-features = false }

- Source: <https://crates.io/api/v1/crates/egui_extras/0.37.0/dependencies>, read 2026-11-20: the
  newest egui_extras (0.37.0, 2026-10-15) depends on `resvg ^0.45.1`, still without its default
  features. The project would call resvg directly rather than through egui_extras: egui_extras
  0.31's loader pins the old resvg 0.37, and drawing at an exact pixel size, keeping textures only in
  memory and obeying the "turn off all reuse" switch (CLAUDE.md) are easier in code of our own.

### 4. Maintenance and what the pack needs

- crates.io's API, read 2026-11-20: resvg 0.50.0 (2026-11-11), about 17.6 million downloads in the
  last 90 days; usvg 0.50.0 the same day; tiny-skia 0.12.0 (2026-02-02), about 30.8 million.
- resvg 0.50.0's default features are `svgz`, `text`, `system-fonts`, `memmap-fonts` and
  `raster-images` (fontdb, harfrust, skrifa, unicode-bidi and more for text; gif, image-webp and
  zune-jpeg for pictures inside SVGs; flate2 for svgz). resvg depends on usvg without its defaults.
- The pack, counted 2026-11-20 across all 4,495 files of `assets/emoji/openmoji/`: 42,366 `<path>`,
  18,937 `<g>`, 4,834 `<circle>`, 4,694 `<line>`, 3,168 `<rect>`, 2,966 `<polyline>`, 2,090
  `<polygon>`, 613 `<ellipse>`, one `<clipPath>`; no `<text>`, no `<image>`, no gradient, no mask or
  filter. Nothing in it needs a default feature.

### 5. The dependency walk (what each choice downloads)

Walked 2026-11-20 on crates.io's API (`/api/v1/crates/<name>` and `/<version>/dependencies`, pages,
no crate downloaded): each non-optional dependency resolved to the newest release its requirement
allows, and checked against the repository's `Cargo.lock`.

| Choice | New crates | Compressed | Already in Cargo.lock |
| --- | --- | --- | --- |
| resvg 0.50.0, no default features | 12: resvg, usvg, tiny-skia 0.12.0, tiny-skia-path 0.12.0, kurbo 0.13.1, roxmltree 0.21.2, svgtypes 0.16.2, imagesize 0.15.0, data-url 0.3.2, simplecss 0.2.2, polycool 0.4.0, pico-args 0.5.0 | about 909 KiB | bytemuck, log, rgb, siphasher, arrayref, arrayvec, cfg-if, strict-num, smallvec, memchr, png |
| resvg 0.45.1, no default features | 10: resvg, usvg, kurbo 0.11.3, roxmltree 0.20.0, svgtypes 0.15.3, imagesize 0.13.0, data-url 0.3.2, simplecss 0.2.2, xmlwriter 0.1.0, pico-args 0.5.0 | about 519 KiB | tiny-skia 0.11.4, tiny-skia-path 0.11.4, base64 0.22, flate2, and the rest above |

The licences crates.io declares for the new crates: Apache-2.0 OR MIT (resvg, usvg, kurbo, svgtypes,
simplecss), MIT OR Apache-2.0 (roxmltree, data-url, polycool), MIT (imagesize, pico-args,
xmlwriter), BSD-3-Clause (tiny-skia, tiny-skia-path). All permissive.

### 6. The alternatives, and why each is declined

- **lyon** (<https://github.com/nical/lyon/blob/main/README.md>, read 2026-11-20; lyon_tessellation
  1.0.23 published 2026-10-20). Its "License" section: "Licensed under either of" Apache-2.0, MIT or
  the Mozilla Public License 2.0, "at your option". It turns paths into triangles (egui meshes, no
  texture), but it is a tessellator, not an SVG renderer: it would still need usvg to read the files,
  its triangles have no anti-aliasing at emoji size, and it does no clipping. Declined.
- **vello** with **vello_svg** (<https://github.com/linebender/vello/blob/main/README.md>, read
  2026-11-20; vello 0.12.0 published 2026-11-11): "Licensed under either of" Apache-2.0 or MIT. A GPU
  compute renderer; crates.io lists its `wgpu` dependency as `^30.0.0`, while the project is on wgpu
  24.0.5, so it would bring a second wgpu. Too heavy for small pictures. Declined.
- **nsvg**, bindings to NanoSVG (<https://github.com/nickbrowne/nsvg/blob/master/README.md>, read
  2026-11-20): "NanoSVG is licensed under [zlib license]"; crates.io declares "Zlib/MIT". C code, and
  its last release (0.5.1) was 2019-09-03. Declined.
- **librsvg** (crates.io, read 2026-11-20: 2.63.4, 2026-10-23, `"license": "LGPL-2.1-or-later"`,
  depending on cairo-rs, pango, pangocairo, glib and gio). Its GitLab home was not read for this
  finding. A copyleft licence with C libraries to ship on Windows. Declined.
- **zeno** (crates.io, read 2026-11-20: 0.3.3, 2025-05-08, Apache-2.0 OR MIT). A path rasteriser with
  no SVG reading. It would need everything usvg does. Declined.
- **A renderer of our own** over tiny-skia 0.11.4 and quick-xml 0.41.0, both already in `Cargo.lock`
  (tiny-skia arrives on Linux through the window decorations), so nothing new to download. The pack
  uses only eight element kinds (finding 4), so it is feasible; but the volunteers' set that replaces
  OpenMoji one emoji at a time may use more of SVG, resvg already exists, open and permissive, and
  its test suite is far larger than ours would be ("build what does not exist", CLAUDE.md). The
  fallback if the operator declines every download.

## What is still unknown, and what would settle it

- **Where the copyright notices go.** MIT and BSD-3-Clause ask that the notices travel with the
  program. This search found no place in the release where the Rust crates the app already links
  (hundreds, many MIT or BSD) have their notices collected; `LICENSES.md` covers data, pictures and a
  few named programs. Adding resvg makes it two more of the same. What would settle it: a generated
  third-party notices file shipped with the exe (a tool such as cargo-about would be one more
  download to approve), or a hand-kept list for the crates the app is built from. A question for the
  operator, not specific to resvg.
- **Exact compiled size.** crates.io gives compressed source sizes only; the size added to the exe
  is known only by building it, after the download is approved.
- **resvg 0.50's behaviour on the whole pack.** Not tried, since trying needs the download. What would
  settle it: once approved, rasterise all 4,495 pictures in a test and compare a sample to the
  website's rendering.

Not legal advice: the licence readings above are a developer's, from the projects' own words.
