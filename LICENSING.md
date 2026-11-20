# Licensing: which licence covers which part of HumanityOS

HumanityOS is free and open source. Since 14 November 2026 the project's own work is under three
licences, one for each kind of thing it makes. Why it changed, and the sources behind every line
here: [`docs/reference/findings/2026-11-14-licence-change.md`](docs/reference/findings/2026-11-14-licence-change.md).

This is a plain summary, not legal advice. Where it and a licence text differ, the licence text is
what counts.

## Who holds the copyright

Copyright 2026 Shaostoul and the HumanityOS contributors. Each contributor keeps the copyright in
what they wrote: there is no copyright assignment and no contributor licence agreement, and a
contribution is licensed under its folder's licence when it is accepted (Contributing, below). So no
one, the maintainer included, can relicense the whole project alone. Versions released before
14 November 2026 were dedicated to the public domain under CC0 and stay there.

## The map

| What | Where | Licence | Full text |
|---|---|---|---|
| **Code** | Everything not named in a row below: `src/`, `crates/`, `web/`, `app/web/` (a generated copy of `web/`), `scripts/`, `tests/`, `schemas/`, `assets/shaders/`, `tools/`, `examples/`, `ops/`, `.github/`, `.claude/`, `build.rs`, `Cargo.toml`, `Justfile`, `package.json` | **GNU Affero General Public License, version 3 or any later version** (SPDX `AGPL-3.0-or-later`) | [`LICENSE`](LICENSE) |
| **The Library and the documentation** | `docs/` (except `docs/accord/`), `data/library/` (except the copies of `docs/accord/` files), and the Markdown files at the top of the repository (`README.md`, `LICENSING.md`, `LICENSES.md`, `CREDITS.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `CLAUDE.md`) | **Creative Commons Attribution-ShareAlike 4.0 International** (SPDX `CC-BY-SA-4.0`) | [`docs/LICENSE`](docs/LICENSE), and [`data/library/LICENSE`](data/library/LICENSE), which ships with the app |
| **Plain data** | `data/` (except `data/library/`), and the project's own media in `assets/` (icons, textures, models, sounds and the brand art) | **CC0 1.0 Universal**, public domain (SPDX `CC0-1.0`) | [`data/LICENSE`](data/LICENSE) |
| **The Humanity Accord** | `docs/accord/`, and the copies of its documents in `data/library/` (`data/library/catalog.json` names the source of each copy) | **CC0 1.0 Universal**, public domain | [`docs/accord/LICENSE`](docs/accord/LICENSE) |
| **Other people's work** | Files that carry their own licence: map data, star catalogues, 3D models, recordings, fonts, the emoji pictures (`assets/emoji/openmoji/`: "All emojis designed by [OpenMoji](https://openmoji.org/) – the open-source emoji and icon project. License: [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/#)") and vendored code (`vendor/`, `web/shared/vendor/`) | **Their own licence**, never ours | [`LICENSES.md`](LICENSES.md), [`CREDITS.md`](CREDITS.md) and the licence file beside each one |

## What each licence asks of you

- **AGPL-3.0 (the code).** Use it, study it, change it, share it, sell it. If you share a changed
  version, share its source under the same licence. If you run a changed version as a server that
  other people use over a network, offer those people its source too (section 13 of the licence).
  The program comes with no warranty (sections 15 and 16).
- **CC BY-SA 4.0 (the Library and the docs).** Share and adapt them for any purpose, money
  included, as long as you credit HumanityOS, link the licence and say what you changed; anything
  you adapt from them is shared under CC BY-SA 4.0 too (or a licence Creative Commons lists as
  compatible). A suggested credit line: *"From the HumanityOS Library, by Shaostoul and the
  HumanityOS contributors, https://github.com/Shaostoul/Humanity, CC BY-SA 4.0."*
- **CC0 (the data and the Accord).** No conditions at all: nothing to credit, nothing to share back.

The share-alike licences are what let the Library take in other share-alike knowledge (Wikipedia,
Open Source Ecology), and what keep a changed HumanityOS server open to the people who use it.

## Things these licences do not do

- **They do not cover the name.** No copyright licence grants rights to a name or a logo. The
  name "HumanityOS" is covered by [`docs/legal/trademark-policy.md`](docs/legal/trademark-policy.md):
  a fork is welcome, and it must not present itself as HumanityOS.
- **They do not change anything already released.** Every version published before this change
  reached the repository on 14 November 2026 was dedicated under CC0 1.0, and that dedication cannot
  be taken back: those versions stay public domain for good.
- **They reach only what copyright reaches.** Much of HumanityOS is written with AI help, under
  the operator's direction and review. Copyright, and so any licence, covers the parts that carry
  human authorship; where a part carries none, no licence can bind it. The finding above sets out
  what the US Copyright Office says about that.

## Contributing

A contribution is accepted under the licence of the folder it lands in: code under AGPL-3.0 or
later, Library and documentation text under CC BY-SA 4.0, data under CC0. By opening a pull
request you agree to that. Bring in other people's work only when its licence allows it, with its
licence file beside it and a line in `LICENSES.md` or `data/credits.ron` (see
[`CONTRIBUTING.md`](CONTRIBUTING.md)).
