# 02-ARCHITECTURE

> The full, always-current architecture (file map, build commands, the canonical
> cryptography table, the non-negotiable design rules) lives in
> **[../../CLAUDE.md](../../CLAUDE.md)**. This page is the orientation; CLAUDE.md is the
> source of truth.

## One crate, one binary

HumanityOS is a **single Rust crate** at `src/`. There is no Cargo workspace and no
sub-crates. Everything compiles into one binary, `HumanityOS`. **Feature flags** decide
what is included:

- `native` (default) the full desktop app: renderer, game engine, GUI, and an embedded
  relay.
- `relay` the headless server: the relay only, no GPU dependencies. The VPS runs this.
- `wasm` the browser/WebAssembly target.

```
HumanityOS                # full desktop app  (--features native)
HumanityOS --headless     # relay server only (--features relay --no-default-features)
```

> History: before v0.90 there was a Cargo workspace (`crates/humanity-core`,
> `crates/humanity-relay`, a separate `server/`, a separate `native/`). The v0.90
> unified-binary restructure folded all of it into the one `src/` crate. If you ever see
> a doc, comment, or AI agent referencing `server/src/`, `native/src/`, or `crates/`,
> that path is dead, the real tree is `src/`.

## The modules inside `src/`

| Module | Role |
|--------|------|
| `src/main.rs`, `src/lib.rs` | Entry point + engine init and main loop. |
| `src/relay/` | The axum server: WebSocket relay, REST API, SQLite storage. Runs headless on the VPS. Subfolders: `core/` (crypto), `handlers/`, `storage/` (30 domain modules). |
| `src/renderer/` | wgpu PBR rendering: camera, sky, bloom, particles, hologram. |
| `src/gui/` | egui immediate-mode UI: theme, widgets, pages. |
| `src/ecs/` | The hecs ECS: components, the `System` trait, the `SystemRunner`. |
| `src/systems/` | 15-plus game systems (farming, crafting, AI, vehicles, weather, ...). |
| `src/terrain/`, `src/ship/`, `src/physics/`, `src/audio/` | World, ship layouts, rapier3d physics, kira audio. |
| `src/assets/` | Data loading (CSV/TOML/RON/GLTF) with hot-reload. |
| `src/net/` | Multiplayer networking: WebSocket client, protocol, ECS sync. |

Outside the crate:

- `web/` the website and web chat client (plain HTML/JS/CSS, served by nginx). It
  **mirrors** the native UI, native is the parent, web follows.
- `data/` hot-reloadable game and platform data (CSV, TOML, RON, JSON). Anything that
  can exist more than once is a data file, not code (the "infinite-of-X" rule).
- `assets/` shared media (icons, shaders, models, textures, audio).
- `schemas/` TOML schema definitions for the data files.

## The design rules you must follow

These are non-negotiable and enforced by tests. Read them in CLAUDE.md before writing
code, but in brief:

1. **GUI-first.** Anything an operator/admin/user can configure must be reachable from
   inside the app, not only from a terminal.
2. **Rust-first UI.** New UI patterns are implemented in native egui first; web mirrors.
3. **One theme source.** Design tokens live in `data/gui/theme.ron`; web's CSS is
   regenerated from it. Do not hardcode colors.
4. **Infinite-of-X.** No hardcoded arrays of domain objects; they are data files.
5. **Dual-UI parity.** A new web UI pattern gets ported to native (or the reason it does
   not is documented).

## Where to go next

- **[03-MODULE-MAP.md](03-MODULE-MAP.md)** what each module is for, in plain language.
- **[06-SOURCE-OF-TRUTH-MAP.md](06-SOURCE-OF-TRUTH-MAP.md)** which file wins when docs
  disagree, and what is real versus planned.
- **[../design/](../design/)** the deeper design specs per system.
