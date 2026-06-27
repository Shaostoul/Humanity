# Door / wall LOCKS

> Stage 1 shipped v0.570. A data-driven lock system generalizing a door's single `locked: bool` into
> a list of typed locks, building on the v0.567 control panel. This doc is the staged plan + the open
> design forks so later stages build the right thing.

## Model (shipped, Stage 1)

- **Registry:** `data/blueprints/lock_types.ron` (the catalog) + `src/ship/lock_types.rs` (`LockType`,
  `LockInteraction`, `DefeatMethod`, `LockState`). Same `include_str! + OnceLock + lookup-by-id`
  pattern as `wall_materials`. Pure serde/data, no native gate (the relay parses it).
- **Instances:** `Opening.locks: Vec<LockInstance>` (`type_id`, `state`, `secret`, `offset`), serde
  default empty. `Opening::is_locked()` = a door is passable only when every lock is `Unlocked`/`Broken`;
  an EMPTY list falls back to the legacy `locked` bool (so all existing homes/tests are unchanged).
- **Runtime state:** `EngineState.door_locks: Vec<Vec<LockState>>`, parallel to `door_panels`, reset to
  the authored states on every structural rebuild (mirrors `door_manual_open`). The free fn
  `door_locked_now(panel, live)` is the single source of "is this door locked right now"; every
  consumer (open target, collision, control-panel box, energy-door colour, prompt, walk-up) uses it.
- **Interaction:** a locked door shows red lock indicators on its face; walk up + E unlocks (Stage 1
  unlocks all), then E opens. A locked AUTO door unlocks then auto-opens; a manual door's locks double
  as its open surface (no separate control panel required), so a locked door is never a dead-end.

## Staged plan

- **Stage 1 (SHIPPED v0.570):** registry + lock list + `is_locked` generalization + control-panel/
  lock-indicator interaction (stubbed unlock, no possession check) + per-door lock editor. Key/code
  enforcement is stubbed: E unlocks regardless of what you carry.
- **Stage 2:** wire `Knob` (E turns) + `Crank` (hold E to wind `turns`; the no-power emergency override
  so a `power_dependent` keypad/biometric door still opens) + key-item gating (`KeyItem` needs the
  matching item in inventory).
- **Stage 3:** `DefeatMethod::Lockpick` + `HackPanel` as a held-E progress action (requires the tool
  item + a minimum skill, reuse the skills system; progress bar over `secs`; sets `Unlocked`).
- **Stage 4:** wall-mounted locks (a lock on `InteriorWall` that controls an opening, or a free-standing
  crank/panel on the wall/floor); richer editor (per-lock code/offset fields, defeat-method display).
- **Stage 5 (DEFERRED -- needs the destructibility system):** `ShootOut(hp)` + `BlowOpen` -- a
  projectile/explosion hit on a lock's mount subtracts `hp`; at <=0 the lock -> `Broken` (door passable).

## Open design forks (operator's call -- resolve before the stage that needs them)

1. **Fail-secure vs fail-safe on power loss.** When a `power_dependent` keypad/biometric door loses
   power, default LOCKED (fail-secure, needs the crank override) or UNLOCKED (fail-safe, opens for
   egress)? Probably a per-lock-type RON flag either way -- which default? (Needed in Stage 2 when power
   is modelled.)
2. **Lock visuals.** Stage 1 uses an abstract red/green/grey indicator box per lock. Richer: a distinct
   little fixture mesh per type (a keypad box, a knob, a crank wheel). Worth the per-type meshes? (Stage 4.)
3. **Keypad code entry UX in first person.** An on-screen numeric keypad the mouse clicks, vs typing
   digits while the prompt shows? Both are GUI-first. (Stage 2/3.)
4. **Wall-mounted locks shape.** A lock on `InteriorWall` that "controls" an opening index, vs a
   free-standing placed object referencing a door by id (generalizes to a crank on the floor far from
   the door)? (Stage 4.)

## Reuse / overlap notes

- The lock indicators reuse the energy-door `energy_locked_mat` (red) / `energy_open_mat` (green)
  materials. When the lighting system's emissive-as-light pass lands, these glowing indicators should
  be harvested as light sources from the SAME emissive param, not a parallel flag (see
  `docs/design/line-overlays.md` is unrelated; the lighting plan lives in the grounding workflow output).
- Locks extend the v0.567 control-panel walk-up + E-handler; they did NOT fork a second interaction loop.
