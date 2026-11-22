# A server's gameplay rules

> Built 2026-11-21 (rung 1). The operator, that day: "Let's make sure the first server we have
> setup, via united-humanity is the forgiving game mode. I think realistic will be too much for
> most people."

## 1. What it is

Every realism level of Settings > Gameplay (`data/gameplay/levels.json`: body heat, illness and
injury, farm animals, carrying weight, death, the mining drone, the walking pace, hostile
wildlife, the vitals drain, the garden's and the ship's levels, skill learning, cooking, and each
level added later) is the person's own choice when they play alone. In a server's shared world
those levels are the **server's**: one ruleset, the same for everyone there. Nobody plays an
easier game beside someone playing a harder one, which is what fairness needs at MMO scale, and a
server owner can say what kind of world they run.

Leaving the shared world gives the person their own levels back, untouched: the server's rules
are held beside the person's own (`SettingsState::server_rules`), never written over them.

## 2. The ruleset

A ruleset is a **preset** or **Custom**:

| Ruleset | What it plays | Where it is defined |
|---|---|---|
| Forgiving (a new server's) | every setting at the level a new player gets (the setting's `default`, the softened one) | `data/gameplay/presets.json`, naming no level |
| Realistic | every setting at its full, real-world level | `data/gameplay/presets.json`, naming every setting |
| Custom | the admin's own level for each setting | the server's database |

Presets are data (infinite-of-x): a server owner who wants another (a "Hardcore", a "Classroom")
adds a row to `data/gameplay/presets.json`. A setting a ruleset does not name plays at its
default. A lib test fails when a setting is added to `levels.json` without a Realistic level
(`every_ruled_setting_has_a_realistic_level`), so the hard preset can never silently play a new
system softened.

**Personal settings.** A setting marked `"personal": true` in `levels.json` stays the person's own
everywhere: today only the bodies' Animation (how bodies are drawn, not a rule of the game). No
preset names a personal setting (the same test).

**Not in the ruleset (yet).** The play mode (Normal, Creative, Dev) is a separate axis
(`docs/design/game-modes.md`, axis 1); in a shared world Dev already loses whole-ship editing
(`ship_editing_allowed`). The world clock and the fleet's supply are their own server settings
(Server Settings > ADMIN > Shared world clock, Fleet supply). Whether a preset should also set
those (Realistic with a Stocked fleet) is an open question for the operator (section 6).

## 3. Where it lives

- **Stored** in the relay's `server_settings` row, two columns: `gameplay_preset` (DEFAULT
  `'forgiving'`) and `gameplay_levels` (a JSON object, kept only for Custom, DEFAULT `'{}'`). A
  database from before the columns gains them by guarded ALTERs at startup (BUG-046's shape) and
  plays Forgiving. Shape and checks: `src/relay/storage/gameplay_rules.rs`.
- **Set** by an admin or the owner from Server Settings > ADMIN > Gameplay rules
  (`src/gui/pages/gameplay_rules_admin.rs`; the web mirror is the Game Admin window,
  `web/chat/chat-game-admin.js`), as one `server_settings_update` carrying `gameplay_rules`. The
  relay refuses anyone else ("Only admins can update server settings."), holds the ruleset to its
  shape (a preset the file lists, or Custom with at most 64 levels whose ids look like ids;
  anything else changes nothing), saves it, and broadcasts the new `server_settings_state`. No CLI
  or environment variable: like the world clock and the fleet supply, the in-app panel is the way
  (GUI-first).
- **Sent** to a game in its `game_welcome` (`rules`: the preset and the levels in play) and, when
  an admin changes it, to every socket as a `game_rules` (`src/relay/handlers/gameplay_rules.rs`),
  the way the world clock reaches them. A game takes a `game_rules` only while it is in the shared
  world (`src/engine/server_rules.rs`).
- **Read** by every system through `SettingsState::level`, `level_value`, `rules_map` and
  `vitals_drain_rate` (`src/gui/server_rules.rs`): the server's levels while joined, the person's
  own otherwise. Nothing reads `SettingsState::gameplay` for play any more; it is only the
  person's own choice, which Settings edits and config.json saves.
- **Shown** in Settings > Gameplay while joined: a note naming the server's rules, every ruled
  setting greyed at the server's level and tagged "this server's rules"
  (`src/gui/pages/settings_gameplay.rs`).

## 4. united-humanity.us

The first public server plays Forgiving. That is the default: the live database gains
`gameplay_preset = 'forgiving'` from the ALTER at the first start of a relay with this code, and
nothing needs setting. To check after the deploy: Server Settings > ADMIN > Gameplay rules says
"Now: Forgiving", or on the server `sqlite3 <relay.db> "SELECT gameplay_preset, gameplay_levels
FROM server_settings"` prints `forgiving|{}`.

## 5. The whole plan, in rungs

1. **(built 2026-11-21)** The ruleset as data on the relay, presets as data, the admin panel on
   both apps, the welcome and the live change, every system reading the effective level, Settings
   greyed while joined.
2. The relay **enforces** what it can see: the move check's allowance follows the server's walking
   pace (today it trusts the fastest pace), and a death in the shared world follows the server's
   death level when the relay starts keeping death packs.
3. The **server browser** shows each server's rules before joining (the launcher's server-detail
   pane already shows its description), so a person picks a Forgiving or a Realistic world
   knowingly.
4. **Per-place rules** for very large servers: a Realistic wing of a Forgiving ship, by plot or
   zone, if the operator wants worlds with more than one kind of play.

## 6. Open questions (the operator's)

- Should the Realistic preset also run the fleet Stocked and the clock at real time (it already
  is real time by default), or stay levels only? Recommendation: levels only, so each server
  setting keeps one meaning and one control.
- Should a server be able to let each player keep their own levels for some settings (a
  "player's choice" level in Custom)? Recommendation: not before a real need; fairness is the
  reason the rules are the server's.

## Related

- `docs/design/game-modes.md`, the six axes (this is axis 2 meeting axis 3)
- `docs/design/graphics-settings.md`, "The garden's and the ship's levels" (why each level exists)
- `docs/admin/SELF-HOSTING.md`, "Your server's gameplay rules" (for a server owner)
