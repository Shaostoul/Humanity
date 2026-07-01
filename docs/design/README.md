# UI Architecture

This folder contains top-level UI/navigation architecture docs:

- `knowledge_tab_architecture.md`

These are early (2026-03) nav-IA explorations. **None of the Private | H | Public dropdown
shell they describe was adopted** — the actual shipped nav is a flat top-level `GuiPage`
enum (see `docs/PAGES.md`, `src/gui/mod.rs`). `header_dropdown_navigation.md` and
`menu_submenu_matrix.md` (previously moved) and now `app_shell_information_architecture.md`
(confirmed 2026-06-30: describes a "Private | H | Public" 3-cluster header + a markdown
"Knowledge tab" explorer that was never built; the real nav uses red/green/blue color groups,
see `web/shared/shell.js`) were moved to `docs/history/` as superseded proposals. Treat the
remaining doc in this list with the same caution until re-verified against the current nav.
