# Code patterns and file facts

**Who reads this:** anyone writing web chat code, a relay handler or route, an ECS system or a
signed request, putting a symbol in native UI text, or editing one of the files in the File map.
Where code lives in general: `docs/ai/code-map.md`.

Moved from `CLAUDE.md` on 2026-10-23 with the wording unchanged.

## File map

Every source file under `src/` and `web/` has a line in **`docs/ai/code-map.md`**: grep it. This table keeps only what the map cannot tell you (a constraint, a mirror pairing, a non-obvious fact); do not re-add plain locations here.

| Path | What to know |
|------|------|
| `src/media/system.rs` | Plays an MP4 (H.264/AAC and more) through Windows Media Foundation, its functions LOADED AT RUN TIME, never linked: a Windows "N" edition must still start |
| `src/persistence.rs` + `src/engine/save_slot.rs` | `SaveSlot` + `slot_path`: your own home, or one save per server and identity (file layout: `docs/ai/storage.md`, the desktop's data folder). A server's save starts FRESH (only name, look and outfit cross); an erased account's save is set aside, never deleted |
| `src/ship/starts.rs` + `data/homes/starts.ron` | The starts (Cabin, Apartment not offered yet, Homestead the default) as data, read by relay AND game; the `game_home_request` / `game_choose_home` / `game_home` wire shapes |
| `src/relay/core/profile_copy.rs` | The profile copy other servers keep (BUG-191); web mirror `web/shared/profile-copy.js` |
| `src/gui/pages/where_you_live.rs` | "Choose where you live"; web mirror `web/chat/chat-home.js` |
| `web/shared/shell.js` | The nav-injection IIFE, loaded near the top of every page, before the page's own scripts |
| `data/star_systems/sol.json` | The 70 celestial bodies. `data/solar_system/` is only 3 planet RONs (earth, mars, sun) |
| `data/external/` | The external catalog (software + help services), one file for the Tools page on both clients; replaced `data/tools/` + `data/resources/` 2026-07-30 |
| other `data/` | `data/chemistry/` (elements, alloys, compounds, gases, toxins), `data/glossary.json` (the glossary overlay's terms), `data/i18n/` (en, es, fr, ja, zh) |

## Script load order (web/chat/)

The `<script>` order in `web/chat/index.html` is load-bearing for an IIFE shell. READ IT OFF THAT FILE, and add a new script there in dependency order. A copy of the list lived here and drifted (it named `chat-voice.js` after that was split into five files, and omitted nine that exist), so it was dropped on 2026-10-19: do not re-add it.

## REST routes

`src/relay/mod.rs` is the only complete list (129 `.route(` calls on 2026-10-19, the whole `/api/v2/*` surface included). Before concluding an endpoint does not exist, grep it: `grep -n '\.route(' src/relay/mod.rs`. The partial copy that lived here was dropped on 2026-10-19: do not re-add it. Not obvious from the router: `/api/send` and `/api/vault/sync` are authenticated by a Dilithium3 signature (shape under Key patterns), and `/api/upload` is multipart and needs an upload token.

## Key patterns

**Chat identity is DILITHIUM3** (set in web/chat/app.js `connect()`; `myKey` is the Dilithium3 public-key hex):
```js
myIdentity = { publicKeyHex, privateKey, publicKey, canSign }
```
> Ed25519's only remaining roles are the BIP39 seed source and the Solana wallet. Federation objects and chat share the Dilithium3 identity; see `docs/reference/cryptography.md`.

**Relay unicast**: `target: Option<String>` on message variant; broadcast loop `continue`s if target≠my_key

**Authenticated API requests** (vault sync, system profile, push, admin stats):
```js
pqSignChatMessage("vault_sync\n" + timestamp)   // timestamp = Date.now()
// Server: verify_dilithium_signature(&vk, "vault_sync", ...) + auth_nonce_fresh
```

> There is no key-rotation route. It and its dual-signed certificate were
> deleted in Inc5b (v0.265.0); the `key_rotations` table survives inert. This
> block documented both until 2026-09-20.

**AES-256-GCM + PBKDF2-SHA256** (vault, notes, backup):
`deriveKeyFromPassphrase(passphrase, salt)` → CryptoKey
- Web client: 600,000 iterations
- Native vault: 600,000 iterations (`PBKDF2_ITERATIONS_NEW`, v0.277.0; `_LEGACY` 100k is decrypt-only for old vaults)
- Relay-stored secrets: Argon2id via `src/relay/core/kdf.rs` (replaces PBKDF2 for server-side)

**Rate limiting**: Fibonacci backoff per public key in `src/relay/relay.rs`

**Game System trait** (src/ecs/systems.rs):
```rust
trait System: Send + Sync {
    fn name(&self) -> &str;
    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore);
}
```
Systems registered with `SystemRunner`, ticked in order each frame.

**Signed profile replication** (no home server):
Profiles are signed objects. Any server caches them. Latest timestamp wins.
ProfileGossip messages propagate profiles between federated servers.

**Data-driven game content** (Space Engineers style):
Game data in external files next to exe. CSV for items/plants/recipes,
TOML for config, RON for quests/blueprints/ships/planets. Hot-reloadable
via notify file watcher. Mods = editing files in the data directory.

## Web gotchas

- `settings.js` has `injectGearButton()` -- don't call it on pages that also load `shell.js` (already fixed: guards for `a[href="/settings"]`)
- Tasks scope filter: `activeScope = 'cosmos'` by default; task labels must match or they're filtered out
- CSP `'unsafe-inline'` retained for inline event handlers on HTML pages

## Theme and glyph lints

CLAUDE.md's "One theme source" rule in full, with its three lints:

**Enforced by `cargo test --test theme_token_lint`.** Every UI file under `src/gui/` and `src/renderer/` is scanned for hardcoded `Color32::from_rgb(...)` literals. New violations FAIL the test. Pre-existing offenders are allowlisted in `tests/theme_token_lint.rs::LEGACY_OFFENDERS` with notes; remove an entry from that list when you migrate the file. Genuinely transient/computed colors (debug overlays, HSV math, programmatic gradients) escape via a `// theme-exempt: <reason>` comment on the line. **If you add a new color and the test fails, do not "fix" it by adding the file to LEGACY_OFFENDERS: add a token to `theme.ron` + accessor in `theme.rs` instead.** The whole point is to shrink that list, not grow it.

**Also enforced by `cargo test --test theme_editor_coverage`.** Every color and numeric token defined in `src/gui/theme.rs` MUST appear as an editable row in `src/gui/pages/settings.rs` (color tokens in `draw_appearance_content`, numeric tokens in `draw_widgets_content`). Adding a token without wiring it into the editor breaks the "100% of theme tokens editable in-app" promise; the test catches this immediately. Tokens that are intentionally non-user-editable (computed, context-bound) go in the test's `intentionally_omitted` list with a reason.

**Also enforced by `cargo test --test icon_glyph_lint`.** The egui font loaded by HumanityOS has spotty Unicode coverage: `Math Operators` (`U+2200..U+22FF`) and `Dingbats` (`U+2700..U+27BF`) blocks have many glyphs that render as tofu (▢), and the `U+FE0F` variation selector ALWAYS renders as a trailing tofu next to its emoji. The lint scans `src/gui/` for known-broken codepoints in UI text (`painter.text`, `RichText::new`, `ui.button`, etc.) and fails the build on any new occurrence. **Reliable glyph families** (use these freely): `Latin / Latin-1 Supplement`, `General Punctuation U+2000..U+206F`, `↩ →` from the Arrows block, and a handful of confirmed-working symbols: `❤ ⭐ ∞ ✓ ⚠ ·`. **The Arrows block is NOT uniformly safe**: `← U+2190` and `↔ U+2194` render as tofu (snapshot-proof 2026-07-06, follow badges) and are in BROKEN_GLYPHS; use `icons::paint_arrow_left` / `paint_arrow_both` / `paint_arrow_right` instead. **Unreliable**: write plain text labels instead, or paint a shape via `widgets::icons::paint_*` SVG-like helpers. Add new tofu glyphs to `tests/icon_glyph_lint.rs::BROKEN_GLYPHS` as you discover them. Override per-line with `// glyph-exempt: <reason>` only when you have screenshot evidence the glyph works.
