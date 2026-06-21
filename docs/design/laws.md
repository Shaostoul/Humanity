# Laws: location-aware rules and rights

> Shipped first increment v0.496 (2026-06-21). Native page + data model + the
> nested-jurisdiction logic. Web mirror + content expansion + location
> auto-detect are follow-ups.

## The problem

There are millions of laws in the real world. No human can remember the ones
that apply to them, and they change over the years. Meanwhile HumanityOS has its
own framework of rights and prohibitions (the [Humanity Accord](../accord/README.md))
that most people have never read. People deserve a clear, memorable answer to
"what are the rules where I live, and what are my rights?"

## The idea

A single browsable surface with two kinds of rule, layered by a nested
jurisdiction tree:

- **Real laws** - plain-language *summaries* of real-world laws, each with a
  source to verify. The goal is to **condense**, not ingest: a small, memorable
  set of the rules that actually matter to daily life, not a statute database.
- **HumanityOS base set** - the rights and prohibitions we author, distilled from
  the Humanity Accord. This is the top of the nest (it applies to all of
  Humanity) and is the same everywhere.

You are located at a leaf of the tree (for example Silverdale, Washington, USA,
Earth, Humanity), and you see the **union** of every level above you: the base
set first, then country, state, and local rules. That mirrors how rules actually
stack in real life.

## Data model (`data/laws/laws.json`)

Data-driven per the infinite-of-X rule: jurisdictions and rules are data, not
code. Anyone can add a jurisdiction or a rule and the app picks it up
(hot-reloadable).

- `jurisdictions[]`: `{ id, name, level, parent }`. A tree by `parent` (the root,
  Humanity, has `parent: null`).
- `rules[]`: `{ id, jurisdiction, kind, category, title, summary, source, tags }`.
  - `kind`: `"base"` (our framework) or `"real"` (a real-world law summary).
  - `summary`: the condensed, plain-language version (the memorable part).
  - `source`: the Accord article (base) or the statute citation / link (real).
- `disclaimer`: shown prominently. These are summaries for learning, **not legal
  advice**; real laws change and vary, so always verify with the source.

The loader (`src/gui/laws.rs`) caches the file and exposes:
`path_to_root(location)` (the chain up to Humanity), `applicable_rules(location)`
(every rule on that chain, broadest first), and `location_breadcrumb(location)`.

## UI

`src/gui/pages/laws.rs` (`GuiPage::Laws`, reached from the Humanity hub's "Laws"
section): a location picker, the breadcrumb, a kind filter (All / base / real), a
search box, and the applicable rules grouped under each jurisdiction with a
BASE/REAL badge, the summary, and the source. Rendered headlessly in the UI
snapshot tests (`tests/snapshots/laws_page.png`).

## Principles + cautions

- **Condense, do not ingest.** The value is a memorable base set, not
  completeness. A few high-impact real laws per level beat thousands of statutes.
- **Never legal advice.** Every real-law entry is a summary with a source; the
  disclaimer is always visible. Wrong legal claims are harmful, so summaries stay
  general and point to the authoritative text.
- **The base set is ours, not the law.** The HumanityOS base set is our framework
  (from the Accord); it is clearly distinct from government law.

## Roadmap

- Web mirror (`web/pages/laws.html`) for dual-UI parity.
- More content: more jurisdictions + condensed real laws, ideally community- and
  AI-maintained (with sources), since laws change.
- Location auto-detect / save (tie to the user profile or a setting).
- Cross-links: a base rule links to the Accord article; a real rule links to the
  official source; rules link to relevant [governance](../../src/gui/pages/governance.rs)
  proposals.
- "What changed" tracking, since real laws change over the years.
