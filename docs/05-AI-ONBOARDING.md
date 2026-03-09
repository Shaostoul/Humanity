# 05-AI-ONBOARDING

This file is the minimum context an AI needs to start useful work.

## Project one-liner

Humanity is a modular platform + game ecosystem where shared domain systems (math, physics, materials, progression) power both practical learning modules and gameplay.

## First questions to answer before touching code

1. What layer am I changing? (core, module, composition, app)
2. Does this introduce a reverse dependency?
3. Can this be isolated to one crate/module?
4. What docs must change with this code?

## Non-negotiables

- Keep module boundaries explicit.
- Keep docs readable in plain text editors.
- Prefer small, composable APIs.
- If design intent changes, write an ADR.

## New contributor quick path

1. Read [`00-START-HERE.md`](./00-START-HERE.md)
2. Read [`03-MODULE-MAP.md`](./03-MODULE-MAP.md)
3. Pick one module area
4. Implement one narrow change with tests
5. Update docs and leave a concise handoff note

## Obsidian + plain Markdown

- Obsidian users can add wiki links (e.g., `[[02-ARCHITECTURE]]`)
- Always keep standard Markdown links too, so files stay usable in Notepad++ and GitHub
