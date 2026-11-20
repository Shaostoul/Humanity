# An XML reader for the MEI score player (scanned 2026-11-20)

Why: the score player (docs/design/score-player.md) reads written music stored as MEI, an XML format,
in the pure-Rust music crate. CLAUDE.md asks for the open alternatives to be compared before a tool
is adopted, and the work's brief asked to prefer a parser already in the workspace's `Cargo.lock`.

Licences read 2026-11-20 from each crate's published `Cargo.toml`, the copy cargo keeps in its
registry folder (the manifest the crate's publisher uploaded to crates.io), quoted:

| Option | In `Cargo.lock` (2026-11-20) | Licence, quoted | Fit |
|---|---|---|---|
| quick-xml 0.41.0 | Yes, through wayland-scanner | `license = "MIT"` | A pull reader over a string; `default = []` (no features on by default); its one required dependency, `memchr`, is already in the tree; compiles to WebAssembly |
| xml-rs 0.8.29 | Yes, through gl_generator | `license = "MIT"` | A pull reader too, slower and allocating an event at a time |
| roxmltree | No | not examined | A read-only tree reader; it would add a crate the lock does not have |
| Our own tokenizer | n/a | ours | About 200 lines, but entity references, CDATA and encodings are a known source of bugs, and nothing is gained over a maintained MIT reader already in the tree |

**Chosen: quick-xml 0.41**, with no features (`default-features = false`). MIT fits the project's
AGPL-3.0 code licence. Its API was moving at this version (`Attribute::unescape_value` is deprecated
for `normalized_value`; entity references arrive as their own `Event::GeneralRef`), so the reader
(`crates/humanity-music/src/mei.rs`) uses the 0.41 names; a later quick-xml may rename them again.
