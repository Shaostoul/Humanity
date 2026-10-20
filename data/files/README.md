# data/files: the Files page's data

What the Files page (in code `src/gui/pages/files.rs`, the relay's `src/relay/files.rs`; called
the Media page until 2026-10-19) reads besides the server's own answers. Moved here from
`data/media/` the same day, which keeps the playback engine's clips and `ingest.json`.

- `labels.json`: the content labels the Files page accepts (Spoiler, Flashing lights, Graphic), read by the relay at start (`src/relay/files.rs`, design: `docs/design/files-showcase.md`). A labelled item stays hidden until it is clicked.
- `licences.json`: the licences an item on the Files page can carry (All rights reserved, the default, and four Creative Commons choices, each with its deed and one plain line), which of them let an item be offered for the game (CC0 and CC BY 4.0), the warnings shown before a Creative Commons licence is saved, and the "Your uploads stay yours." paragraph both apps show beside Add to Files. Read by the relay at start (`src/relay/files_licences.rs`); the apps take it from the server's answer. Design: `docs/design/media-round-5-plan.md` 3.7 and 10; facts: `docs/reference/findings/2026-10-12-media-item-licences.md`.
- `kinds.json`: the kinds of item on the Files page (pictures, videos, models, files), the name each goes by and the mark a tile without a thumbnail shows, read by the desktop's Files page (`src/gui/pages/files/kinds.rs`). The ids are the relay's (`src/relay/storage/files.rs` `KINDS`); a test holds the two together.
