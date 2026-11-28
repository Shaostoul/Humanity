# Playing without the 3D world: Your home on any device

Written 2026-11-27. Status: **plan; nothing built yet.** The file keeps the plan's working name,
"the companion"; the page people see is named by question 1 of section 12 (recommended: **Your
home**, under Play). This is the whole plan, decided once so the first rung is a rung of the real
thing (CLAUDE.md, "Plan the whole feature, build it in rungs").

## Where this comes from

The operator, 2026-11-24 (the journal, `data/coordination/orchestrator_state.json`): "We also need
to think of how players could do some gameplay stuff in a non-3D game wise. Like tending to crops,
equipment, or something else. We're trying to think of ways we can connect desktop and mobile with
the same MMO server. I don't know how we could handle single-player stuff but, MMO makes sense."

The proposal put to him that day: one world on the server, with the 3D world as one window onto it
and a homestead panel on the phone or desktop as another, on the same state. Without the 3D world
you do what your home's machines can do (the Automation engagement mode); hands-on work needs the
3D world, and goods stay physically carried. The server's real time (1x) makes checking in from a
phone meaningful, with notifications that open the panel. The panel is the game side, never mixed
with the real-life pages. Single player is your own private server on your own computer (Host a
node already exists), which the phone joins like any server: one model, every world is a server.

His answer, 2026-11-27: "Let's go ahead plan it."

## 0. In short

- **Every world is a server, and one host runs each home.** The home (crops, animals, tanks,
  batteries, air, machines, storage, the drone) is simulated by one host: the server for a server
  world, your own node inside your own app for single player. The 3D game and the panel are both
  windows onto it; neither simulates the home itself.
- **The panel does what your home's machines can do.** It shows the whole home and sets its
  machines: what a machine makes, its keep target, on or off, a setpoint, the drone's standing
  order. Work that needs a body (sowing, harvesting, collecting eggs, carrying, building,
  repairing, crafting by hand) needs your character in the 3D world, or a machine of your home that
  does that job. One judge checks every action, whoever sends it, so the panel never does anything
  more cheaply than the world does.
- **While your character is out of the world, your home keeps living** under the offline
  progression rules it already has: machines run on what is on hand, animals eat stored feed,
  nothing is ruined without your say. The change is that this is computed live instead of on your
  return, so the panel shows your home as it is now. Whether Realistic keeps its full rules while
  you are away is question 2.
- **Notices come to your devices**: a crop ripe, a tank low, a machine stopped for want of an
  input, storage full, the drone back, eggs waiting. They are folded, so a thousand ripe crops are
  one notice.
- **It is cheap.** A panel left open uses about 1 KB a minute, never more than 32 KB. Nothing
  cosmetic ever crosses the network.
- **Native first.** The same page on the desktop and the phone (one column on a phone). The
  website mirrors it later, because an iPhone has no native app yet and the panel needs no game
  engine.
- **Your phone reaches your single-player home** on your home network or over Tailscale, after
  scanning a QR code your desktop shows, over an encrypted connection pinned to your own node. When
  your PC is off, your home waits and catches up when it is on again.
- **Only you act on your home** (later, the people you give a household permit). The server learns
  nothing it does not already hold, and keeps no record of when you look.
- **The first rung**: the home's read model and the Your home page on the desktop, from the world
  on this computer (section 11).

## 1. What exists to build on, and what is missing (read 2026-11-27)

| Piece | Where | What it gives this plan |
|---|---|---|
| One simulation depth, five ways to engage per domain | [engagement-modes.md](engagement-modes.md); resolved in [progression-skills-gear.md](progression-skills-gear.md); each start's defaults in `crates/humanity-ship/src/starts.rs` ("Shown in words; nothing else reads it yet") | The panel is the Automation mode's surface for the game side; Direct is the hands in 3D |
| Machines that run on their own | AutoRefine in `crates/humanity-systems/src/crafting/mod.rs`: runs one recipe from home storage only (BUG-150), rests at a keep target, "Deliberately no skill gate: owning the machine is the unlock" (docs/FEATURES.md, Economy Automation Phase 1) | What a machine job is, and that it needs no skill |
| Setting a machine in the world | The walk-up card's "Auto-build" dropdown (`src/gui/pages/hud.rs`, `draw_machine_recipe_selector`); `PowerConsumer.enabled` (a machine on or off) | The controls the panel offers, and the in-world twin of every panel control |
| The drone | `crates/humanity-systems/src/mining.rs`: trips, "Keep mining" standing orders, the Mining panel | A machine job a panel can set |
| The time away | `save_load::catch_up_world` (`src/save_load.rs`), `crafting::away` (`crates/humanity-systems/src/crafting/away.rs`), `mining::advance_away`, `livestock::timers_after_away`, `animal_care` feeding from storage; Settings > Gameplay > "Keep growing while away"; [offline-progression.md](offline-progression.md) | The rules a home lives by while you are away, already decided and built |
| The farm on its own clock | `crates/humanity-systems/src/farming/clock.rs` ([farm-clock.md](farm-clock.md) rung 2); rung 1 made every long step exact; rung 4 is "the server: each home is simulated lazily" | A server can advance a home rarely and exactly |
| The simulation without a GPU | `src/engine/sim_boot.rs`, `src/engine/world_sim.rs` (`the_homestead_sim_builds_with_no_gpu`, `a_game_day_passes_with_no_gpu`), `HumanityOS --sim` ([headless-sim-runner.md](headless-sim-runner.md)) | The home already runs headless, inside the engine |
| The relay's game world | `src/relay/handlers/game_state.rs` (entities, crew, ship stores, a 20 Hz tick), plots (`home_plots.rs`, `storage/plots.rs`), the speed check (`move_check.rs`), delivery by view (`game_interest.rs`), the fleet ledger (`fleet_ledger.rs`), a server's gameplay rules (`src/relay/storage/gameplay_rules.rs`, Forgiving on united-humanity.us) | Where homes join the server, and who owns which plot |
| A contract both sides compile | `crates/humanity-systems/src/construction/shared.rs` with `src/relay/handlers/shared_build.rs`: one rule module, judged by the relay; a change count (`seq`) per frame and a check every 5 s with `game_time_sync` that mends lost messages | The exact pattern the home's actions follow |
| One person, several sockets | `src/relay/handlers/live_conns.rs`: the game seat belongs to the socket that joined | A panel watches without taking the game seat |
| Game notices | `data/notifications/notifications.json` `game_events` (`crop_ripe`, `water_low`, `short_of_air`, `hungry`, `thirsty`), `systems::push_game_event`, `notifications::game::Folder` ([notifications.md](notifications.md) section 11: "Nothing is networked: an event is this device's") | The rows and the fold the host's notices reuse |
| The phone | [platforms.md](platforms.md) A1: the app pages run without the world's renderer, and Play's Enter button becomes a notice (`crates/humanity-core/src/platform/device.rs`); A4: Stay connected while closed (`src/notifications/stay_connected.rs`, a foreground service) | The panel already has a place on the phone, and a held connection |
| Host a node | `src/gui/pages/host_node.rs`: the relay in this app's own process on its own thread; Who can connect (this computer, or devices on my network); a Public address; Tailscale (`ON_TAILNET`); the owner's door on 127.0.0.1 for this app's key alone (`src/relay/audience.rs`) | Single player as your own server, mostly built |
| Your identity on the phone | Settings > Identity: a QR another device scans to bring this identity onto it (`src/gui/pages/settings.rs`) | The phone signs in as you |
| Your own mailbox, sealed | `src/net/self_mail.rs`, key cards ([key-cards.md](key-cards.md)) | A way for notices to reach your phone without opening your PC (question 5) |
| The Play hub | `src/gui/pages/play.rs`: a card per game page with its `hub_summary` ([top-menu-and-game-pages.md](top-menu-and-game-pages.md) section 7) | Where the panel's card goes |
| The home's widgets | `src/gui/pages/home_loops.rs`, `herd.rs` (`animal_care::HerdRow`), `household.rs` on the Homes page | Reused, not rebuilt |

**What a server holds today, and how often it speaks.** The relay is the authority for: who owns
each plot (`game_plots`); the shared-building pieces (`world_pieces`, each frame with its change
count); the ship's food stores and each player's fleet ledger; the crew, whose chore positions it
broadcasts every 0.5 s (`NPC_POSITION_BROADCAST_INTERVAL`); the world clock (`game_time`, ticked at
20 Hz and sent with `game_time_sync` every 5 s, beside the pieces check); and the server's
gameplay rules. Players send their own positions (up to about 15 updates a second, ship-homes
section 5.10), judged by the speed check and delivered only to those in view (another player within 250 m,
out of view again past 300 m: `game_interest.rs`, `data/ship/shared_world.ron` `delivery`). It holds no inventories ("the relay holds no inventories yet",
shared_build.rs), no save and no home simulation.

**What is missing, which this plan closes:**

1. **On a server, the home is not the server's.** [ship-homes-and-logistics.md](ship-homes-and-logistics.md)
   section 5.10 lists "the private save" and "a home's full layout" as never held by the server;
   increment 6 keeps each server's save for you on your own computer
   (`saves/servers/<server>-<identity>.json`, `src/engine/save_slot.rs`). So a phone has nothing
   to read; one server home played from two computers becomes two homes; and the fresh-start rule
   (the operator, 2026-10-04: "I don't want people cheating in MMO servers by loading homes fully
   kitted out and cheated inventory") rests on the player's own copy.
2. **Only the engine can set up a home.** The systems are all in `humanity-systems`, which the
   relay already compiles, but their setup is in the engine (`sim_boot`, `world_sim`,
   `save_load`), so the relay build cannot run a home.
3. **Game notices never leave the device.**
4. **No machine does a hand job.** There is no harvester, seeder, nest box, milking machine or
   carry drone in the data: the Automation mode's robots are not there yet.
5. **No page shows the whole home.** Its parts are spread over Inventory (Garden, Mining, the
   fleet), the Homes page (Household, Animals, the loops), the walk-up cards and the HUD.

## 2. The model: every world is a server, and one host per home

**The host.** One piece of code, `humanity_systems::home_host::HomeHost` (rung 3), holds one
home: its machines, grow areas, animals, tanks, batteries, rooms' air and storage as ECS entities
and DataStore slots; the systems that move them (farming on its own clock, plumbing, electrical and
solar, atmosphere and life support, crafting's AutoRefine, livestock and animal care, the drone);
and its clock. It has three verbs: `advance_to(t)`, `apply(action)` and `view()`. It compiles in
every build, so the relay runs it as easily as the desktop does.

**Where it runs.**

| World | The host | Your 3D game | Your panel |
|---|---|---|---|
| A server world (united-humanity.us, a friend's server) | That server, one host per claimed plot | A client: mirrors the home, sends hands-on actions (rung 5) | A client: watches, sends machine actions (rung 6) |
| Your single-player world | Your own node, inside your own app (rung 8) | A client of your own node, through the owner's door | On the desktop, the same page; on your phone, over your home network or Tailscale (rung 9) |

Movement and the body stay where they are today (the player's own game, checked by the relay's
speed check), because they run at frame rate and the plan does not need them moved.

**The backpack.** What you carry and your body stay in your game until ship-homes increment 8
(holdings: `carried:<did>`) gives the server what you carry. Until then, a hands-on action that
moves goods between the home and the backpack (a harvest into the pack, taking from storage,
eating) is judged by the host on the home's side (the crop is ripe and is picked once; storage
really holds the thing), and the backpack's side is trusted as it is today. The home itself can no
longer be filled from a modified client.

**When your character is out of the world.** The host keeps the home going under the rules
offline progression already has (offline-progression.md, "What must NOT advance offline"):

- crops grow, and wait when ripe;
- machines run on the inputs really on hand and the power the home can spare, and rest at their
  keep targets;
- the drone flies its standing order;
- animals eat stored feed, and their eggs, milk and wool wait;
- pests do not advance; nothing dies and nothing is destroyed without your say.

The only change is WHEN this is computed: live, on the host's clock, instead of in one step on
your return. farm-clock.md rung 4 already says it: "The offline catch-up and the live farm become
one code path." While your character is in the world, the full rules run, as today. Whether
Realistic should run its full rules while you are away is question 2.

Settings > Gameplay > "Keep growing while away" keeps its meaning: off, the host holds your home
still while you are away. In a server's world the server's rules decide, as with every realism
level.

**The fresh start, enforced.** On a server, the host is made from the start you chose, from the
server's own data, at the moment your plot is claimed (rung 4). Nothing comes from your computer
but your name and look (the operator, 2026-10-04: "only their appearance is kept"). The per-server
save on your computer goes away (rung 5) with no migration (CLAUDE.md: no backwards compatibility
before v1.0), so a home already played on a server starts fresh there once.

## 3. What the panel does, and what needs your hands

**The rule.** One judge checks every action, whoever sends it. An action is done by either:

- **your hands**: your character, in this world, doing it. Only the 3D game sends these, because
  only it has your character in the world; or
- **a machine of your home** that does that job, reached through your home's own network (rung 11;
  Simplified treats that network as always up).

The judge (rung 2) asks the panel and the 3D game's pages and walk-up cards the same questions:
is it yours, does the machine exist and take this control, does the target make sense. So nothing
the panel does skips anything the world makes you pay:

- **Skills.** Setting what a machine makes needs no skill, as in the world today ("owning the
  machine is the unlock"). Crafting by hand, which is skill gated, is a hands job. If machine
  recipes are ever gated by skill, the one judge gates both surfaces.
- **Costs.** A machine draws its power and spends its inputs whoever started it.
- **Time.** An action takes effect at the host's now. The panel cannot speed time; the drone's trip
  takes its real time.
- **Goods.** The panel never moves goods. They stay where they are until a body or a carrier moves
  them (ship-homes section 5.1: goods always sit in exactly one place).
- **XP.** An action earns what it earns in the world. Setting a machine earns nothing; harvesting
  by hand earns farming XP, in the world.
- **Sight.** The panel shows nothing the world would not: the numbers the walk-up cards and pages
  show. In Realistic, only what the home's sensors report (rung 11).

**What the panel shows of a hands job**: a list, "Needs you in the world" (ripe crops by grow
area, eggs and milk waiting, a pest to treat by hand, a generator to refuel by hand). Each is a
line, never a button.

| Domain | The panel shows | The panel sets (a machine does it) | Needs your hands in the world | Why it sits there |
|---|---|---|---|---|
| Crops | Each grow area: crop, plants, stages, when the next is ripe, how many are ripe now, water and nutrients, health, pests that are noticeable | Grow lights and fans on or off; humidifier and heater setpoints where the machine type has one; the feeder and irrigation on or off | Sowing, harvesting, picking, hoeing, mulching, hand-picking pests, treatments, hand pollination, building beds | No machine in the data sows or harvests yet. When one exists (rung 14), its row gives the panel the action with no new code |
| Animals | Each animal: how it is, fed and watered, what waits (eggs, milk, wool), the feed and water left in days | Nothing yet (feeding from storage is already the character's upkeep); automatic feeders and waterers when they exist | Collecting eggs, milking, shearing, moving, treating, butchering | Nest boxes and milking machines are rung 14 machines |
| Water | Tanks and cisterns (litres of capacity), pumps running or stopped and why, the purifier, the water heater | Pump, purifier and water heater on or off; the heater's setpoint | Fixing a leak, changing a filter, hauling water, any plumbing change | A pump has a switch; a filter is a part a hand fits |
| Power | Generation and demand now, the battery's charge, generators and their fuel, the station's share | A generator started or stopped; which machines may draw (a machine on or off) | Refuelling a generator by hand, cleaning panels, wiring | Starting a generator is a switch; fuel is carried |
| Air | Each room's oxygen, carbon dioxide, humidity and temperature; the scrubber, air handler, heaters and heat pump | Those machines on or off, and their setpoints | Loading the wood stove, replacing a scrubber's media | The same split: switches are machine jobs, parts and fuel are hands jobs |
| Storage | What home storage holds by item and by container, how full it is, the wrong-class warnings; on a server, your fleet ledger (used and given) | Nothing | Moving, sorting, taking, carrying, selling | Goods are physical. Offering goods from home storage for trade waits for holdings and escrow (ship-homes increment 8) |
| Machines and queues | Each automated machine: what it makes, its progress, its status line ("waiting for iron ore in home storage"), its output; the drone's phase, trip and arrival; the vehicle assembler | What an automated machine makes (the walk-up card's Auto-build); its keep target; on or off; the drone's trip to a known asteroid and "Keep mining" | Crafting by hand at a station, repairing, placing, upgrading, building | The Automation mode is machines doing the work; a hand at a bench is Direct |

**One set of controls in the world and on the panel.** The walk-up card and the in-world pages get
every control the panel has, from the same contract (rung 2): a keep target set on the phone can
also be set standing at the machine. A panel-only control would be a second, cheaper path, which
the rule forbids.

**Never in the panel:** the character's body (paused while you are away), walking, the 3D map,
building, chat (chat is the app's real side and has its own pages).

## 4. The data

### 4.1 What the host keeps

- **A server world:** one row per claimed plot in a new table,
  `game_homes (world_id, plot_id, owner_did, save, as_of_ms, version)`, primary key
  `(world_id, plot_id)`. `save` is the home part of the `WorldSave` shape (`persistence`, the same
  shape the desktop saves); `as_of_ms` is the server's wall clock the home is current to; `version`
  goes up by one for every change a watcher would see. A new table in its own batch, so the BUG-046
  ALTER rule is met trivially. The row is written before anyone is told (shared_build.rs's rule:
  nobody is shown a state the relay could not bring back after a restart).
- **In memory** only while it advances, while someone watches it, or while its owner is in the
  world. Otherwise it is just the row, so memory grows with the homes in use, not the homes that
  exist.
- **Single player:** your own node's database, the same table, one row.
- **Deleted with the plot**: an admin's release, a home that does not fit, an erased account
  (`home_plots.rs` already releases the plot; the row goes with it). **Included** in the account
  export (`src/relay/storage/account.rs`).

Size: a whole single-player save is about 560 KB of JSON today (one homestead, read 2026-11-27),
and the home part, without the character, is less. That is over the relay's 128 KB message cap
(`src/relay/mod.rs`, `max_message_size(131_072)`), which is one reason the panel never receives
the save, only the view.

### 4.2 What the panel reads: the home view

`humanity_systems::home_view::HomeView` (rung 1) is a summary built from the host's state by one
pure function, `home_view::build`. It lives in `humanity-systems` with no feature gate, beside
`construction::shared`, the precedent for a contract the relay and the game both compile.

- **Header:** the plot, the start's kind, `as_of`, the game time and time speed (so the device
  ticks countdowns itself), the version, and which rules run now (full, away, or held).
- **Sections:** crops by grow area; animals (the `HerdRow` the Homes page already uses); water;
  power; air by room; storage totals; automated machines and the drone; "Needs you in the world".
- **Controls:** for each machine, the controls it takes (from the contract), so the page draws
  only what the judge would accept.
- **Times are game-time instants** ("ripe at"), never countdowns. The device turns them into "in 3
  h 20 min" and ticks them locally, so nothing is sent each second.
- **Like machines fold:** ten solar panels are one row with a count.
- **Cap:** 16 KB of JSON for the shipped family homestead (a gate in rung 1).

### 4.3 What the panel sends: actions

Actions are **target states, never toggles**: "the pump on", "the mill makes flour and keeps 20",
"the drone keeps mining asteroid A-3". A target state sent twice is the same state, so a resend
after a dropped connection is harmless. Each action carries an `id` (random; the host answers a
resend with its first answer, kept in memory for ten minutes), `seen_v` (the view version the
person pressed on) and the action itself.

The messages (rung 6), in the `game_*` family and distinct from today's `game_home_request` (which
claims a plot):

| Message | From, to | Carries |
|---|---|---|
| `game_home_watch` | panel to host | the version it holds, if any |
| `game_home_view` | host to panel | the version and the whole view |
| `game_home_delta` | host to panel | the from and to versions, the changed sections |
| `game_home_unwatch` | panel to host | nothing |
| `game_home_act` | panel or game to host | `id`, `seen_v`, the action |
| `game_home_act_result` | host to the sender | `id`, and either done (the new version) or refused (the reason's words key, and what changed) |

The 5 s check that already goes with `game_time_sync` (shared_build's `game_pieces_check`) gains
`home_v`. A panel holding another version asks for the view again, so a lost delta is mended within
5 s. (A slow socket loses messages without a word: `relay.rs` `recv_skipping_lag` skips what it
missed in the 256-message queue.)

**How they are signed.** They travel on the socket you signed in on. The Dilithium3 sign-in
([cryptography.md](../reference/cryptography.md), "Sign-in") proves your key once, bound to the
address you opened, which is how building's `game_build` travels today. There is no signature on
each action: the host is the authority acting on its own records, nobody needs to check an action
later, and an ML-DSA-65 signature is about 3.3 KB, which every press would carry while proving
nothing the sign-in has not. The one signed thing an action may carry is a household permit, when a
permit holder acts (rung 13), checked exactly as building checks one
(`pq_crypto::verify_plot_permit`, given the relay's own facts, never what the permit claims).

### 4.4 How the host checks an action

The judge, in this order, the same for the panel and the game:

1. **Who.** The signed-in key owns this plot (`plot_owner_id`), or holds a live household permit
   for it that grants tending (rung 13). A server's admins and owner do not act on homes; they can
   release a plot, which ends it, as today.
2. **Fresh.** No other device changed the same target since `seen_v` (section 4.6).
3. **What.** The machine exists in this home, its type takes this control (data), and the target
   makes sense: a recipe of the machine's own station, a setpoint within the type's range, a known
   asteroid.
4. **Who does it.** Hands (the sender is the 3D game and its character is in this world) or a
   machine (in Realistic, one reachable through the home's network, rung 11).
5. **Rate.** Within the sender's bucket (section 5).

Costs are not checked at the press, because a setting costs nothing: the machine pays its power and
inputs as it runs, as today. A refusal names its reason in words by key, the same words in the
game and the panel ("The pump has no power: the battery is empty").

### 4.5 Offline and reconnect

- The panel keeps the last view **in memory only** (CLAUDE.md: no caches kept across runs) and
  shows "Not connected since 14:02" over it, with every control greyed and its hint saying why.
- **No action is queued for later.** A press applied an hour after it was made, to a home that has
  changed since, is a surprise.
- On reconnect the panel watches again and is sent the whole view (16 KB at most): simple, and
  cheaper than keeping a history of deltas for it.
- An app restarted while offline shows "Not connected" and no view.

### 4.6 The desktop and the phone at once

- The host takes one home's actions one at a time, in the order they arrive.
- A target state is refused once if another device changed the same target since `seen_v`, saying
  what changed and when ("Changed on another device at 14:02: the mill now makes bread"). Pressing
  again, on the new view, applies it. A target already true is a no-op, never a refusal.
- Hands-on actions (in the game) and machine actions (on the phone) almost never touch the same
  thing. When they meet (a harvest in the world while the phone shows the crop ripe), the next
  delta updates the phone; a stale harvest never comes from the phone, which has no harvest button.

### 4.7 Restarts and the clock

- A server world's host runs on the server's wall clock (offline-progression.md, "Cheating":
  multiplayer uses "the SERVER's clock, always"); a single-player world's runs on your own node's.
  Time is counted at the world's time speed: 1x on a new server (the operator, 2026-10-04: "let's
  have everything be real time").
- After a restart, each home resumes from its row and is brought to now from `as_of_ms`, exactly
  (farm-clock rung 1).
- A clock set backwards advances nothing (`catch_up_world`'s rule).

## 5. The budget

**The network, for one panel left open:**

| | Typical | Hard cap |
|---|---|---|
| On opening | one view, about 4 to 16 KB | one whole view per 30 s |
| While open | 1 to 3 deltas a minute, about 300 bytes each | 12 deltas a minute, each at most 2 KB |
| Down, per minute | about 1 KB | 32 KB: past it the host stops sending deltas and marks the view stale, and the next 5 s check fetches it whole |
| Up, while idle | nothing: no polling | |
| Actions | as pressed | 10 at once, then one a second, per identity |
| Watches | | 6 a minute, per identity |

An hour left open costs typically under 100 KB, at worst about 1.9 MB. For scale, positions in the
shared world cost about 45 KB a second per socket at 12 players (shared-building.md section 5,
quoted in ship-homes section 5.10). The panel watches only while it is on the screen: it unwatches
when the page closes or the app leaves the screen (on a phone, when Android suspends it).

**The server, for one home:**

- **Watched:** brought to now when watched, then advanced every 5 s (the delta interval) while
  watched. farm-clock rung 1 makes each advance one exact step, whatever time it covers.
- **Unwatched, owner away:** advanced at a sweep interval (a Server Settings row, 10 game minutes
  by default) to raise notices at their transitions. When farm-clock rung 3 gives each home the
  time of its next event, a queue of those times replaces the sweep, and a quiet home costs
  nothing.
- **The figure to start from:** FarmingSystem took 3.51 ms of a desktop frame for the showcase's
  1,981 crops (headless-sim-runner.md, v0.1539.0). Rung 4 measures one home's advance, in time
  and in memory, on the relay build, and writes the figures here. The target: a server of 1,000
  homes spends under 2% of one core on its sweeps.
- **Reuse:** a home kept loaded while watched, and a view kept until its version changes, are
  versioned reuse under CLAUDE.md's refined no-caches rule, so each obeys the Dev page's "Turn off
  all reuse" switch and `HUMANITY_NO_REUSE=1`.

**Notices:** nothing when nothing happens. A notice is computed during an advance, at a transition
only, and folded per kind by the existing Folder (each event's `every_secs` in the data), so at
most one per kind per interval, about 300 bytes on the socket. A push to a device with no socket
open carries no content. On a phone the socket is the one Stay connected already holds for chat:
no second connection for a server world.

**Never on the network:** cosmetic motion, meaning leaves, water, animals wandering, a machine's
moving parts and particles (CLAUDE.md's compute budget). The 3D game draws motion from states; the
panel draws numbers and words.

## 6. Notices

Raised by the host at transitions (rung 7): the existing `crop_ripe` and `water_low`, plus new
rows in `game_events`:

- `machine_waiting`: an automated machine stopped for want of an input, once per stop;
- `storage_full`;
- `drone_back`: a trip delivered;
- `yield_waiting`: eggs, milk or wool;
- `battery_low` and `generator_out`: only for a home with its own battery or generator, never for
  station-supplied power (notifications.md: with the fleet's supply unlimited in development,
  nothing "could honestly say" it was running out);
- `home_offline`, in Realistic (rung 11).

Each is a row plus a line in the system that raises it (infinite of X). On the device each goes
through the one rule (`notifications::decide`): its switch, quiet hours, and the locked-screen rule
(a phone with a screen lock shows nothing of it, platforms.md A4). A tap opens Your home at that
grow area or machine.

Today a game event's switch is kept on the device only ([notifications.md](notifications.md)
section 5 lists game events under "this device"). A server that pushes to a device with no socket
cannot see that switch, so rung 7 adds a synced switch for the game kind beside `dm`, `mentions`
and `tasks`, which the relay's push reads as it reads the others.

## 7. The screens

Native first (CLAUDE.md, Rust-first canonical UI). One page, `src/gui/pages/your_home.rs`
(`GuiPage::YourHome`), drawn by the desktop, the phone and an in-world wall screen alike.

- **Where it lives: under Play**, because it is the game. [two-realities.md](two-realities.md):
  the app's pages are your real life, always, and the game is reached through Play. It never goes
  under Life.
- **On a phone:** one column under 600 points wide (the operator's phone is 412 points wide). Each
  domain is a tint section in theme tokens; its header is its state in words ("3 grow areas ripe, 1
  short of water"), opened for detail. The top line says how the home is and when it was last
  heard from, with the connection. "Needs you in the world" sits right under it. Controls are the
  universal widgets (`src/gui/widgets/`), each with its hint; every word comes by key (`node
  scripts/i18n-add.js`), with jargon marked `[[term]]` for its glossary card.
- **Play on a phone:** the world card keeps its notice that the 3D world needs a desktop for now,
  and the Your home card comes first, so tending is one tap from Play.
- **On the desktop:** the same page, a card on the Play hub (with its `hub_summary`), two columns
  when wide.
- **In the world:** the same page can be a wall screen's content (`page:your_home`,
  [in-world-screens.md](in-world-screens.md)), for instance at the bedroom command center, if the
  operator wants it there.
- **On the website: yes, it mirrors the page (rung 12).** Under the dual-UI rule the question is
  whether the web needs it, and it does. The panel needs no game engine, so the reason the
  website's Play opens /download ("no game runs in a browser") does not apply to it. An iPhone has
  no native app yet (platforms.md, iPhone and iPad), so the website is how an iPhone reaches its
  home. And the website already signs in on its own socket. It is built after the native page is
  proven, from the same words keys and theme tokens. Apple delivers a website's notices only to a
  site added to the home screen: a PLATFORM HOLDER'S RULE, and the page says so where notices are
  turned on.

## 8. Single player as your own server

### What Host a node does today (read 2026-11-27)

Servers > Host a node on this PC (`src/gui/pages/host_node.rs`) runs the relay inside this app, on
its own thread with its own runtime: the same `run_relay()` that `HumanityOS --headless` calls. It
reports Running only once `/health` answers. "Who can connect" chooses "Only this computer"
(127.0.0.1) or "Devices on my network" (every interface). A Public address field names the
addresses people reach it by, and a Tailscale switch sets `ON_TAILNET`. The owner's door, on
127.0.0.1 at the next port, takes this app's own key alone and grants it the node's admin role.
Today the node carries chat and the relay's other features; the solo game does not use it.

### What changes (rung 8)

The solo game is hosted by the app's own node. The node starts with the app, bound to loopback
for this, with your world in its database and your Time settings as its world clock. Dev mode is
allowed because the server's rules are yours on your own server, as a shared world's rules are its
server's. The 3D game joins it through the owner's door exactly as it joins a server world (rung
5's client). The node runs while the app runs, with its window open or closed into the tray;
quitting the app stops it.

Nothing a player sees changes for the worse. Today the world is held until you enter it and caught
up on entry (BUG-253, offline progression); the node runs it live under the same away rules, so
what you find on entering is the same, and the panel can watch it meanwhile.

### Your phone joins it (rung 9)

1. **Turn it on:** "Let my phone see my home", in Host a node and in Your home's empty state on the
   desktop.
2. **Only your own devices sign in.** The node listens on your network but signs in only your own
   key: a new sign-in rule, "Only my own devices", refuses any other key before a member row is
   written. (Today any key that connects is joined as a member: `sign_ups.rs`, `join_as_member`.)
3. **A QR code.** The desktop shows the node's addresses on this network (and its Tailscale address
   when it is on one) and the fingerprint of the node's own TLS certificate. The phone, which
   already has your identity from Settings > Identity's device-link QR, scans it with "Add my own
   server".
4. **Encrypted and pinned.** The node serves TLS with a certificate it makes for itself, and the
   phone pins the fingerprint from the QR. No domain and no certificate authority are needed; it
   works on a home network and through a router's port forward. Plain http to a node on a network
   is refused for Your home (only loopback is plain, for development), because anyone on a shared
   Wi-Fi can read and write an unencrypted socket. Serving TLS needs a server-side TLS library and
   a certificate maker; rustls is already in the tree on the client side. The rung compares the
   options before adopting one (CLAUDE.md: every new tool gets compared).
5. **Away from home:** Tailscale on both devices (already supported), or a port forward with the
   pinned certificate ([SELF-HOSTING.md](../admin/SELF-HOSTING.md), "Reaching the wider internet").

**What is exposed:** the node's port on your network. Anyone on it can reach the node and is
refused at sign-in; nothing of your home is readable without your key.

### When your PC is off

Your home waits. The phone says: "Your home runs on your computer, which is off or out of reach.
Last heard from at 22:14. It catches up when your computer is on again." When the node starts, it
catches the home up under the away rules. There are no notices while it is off, because nothing is
running to raise them.

Later choices, not in the rungs below: move the single-player world to another computer of yours
that stays on (`HumanityOS --headless` can hold it), or have notices reach you through your usual
server, sealed to your own key (question 5).

## 9. Security and privacy

- **What the server learns from a panel:** that your key watched its own home, when, and what you
  set. It already holds the home. It keeps no log of watches or views (no table, no admin-log
  entry), counts a panel's socket in nothing public (no presence, no "online"), and keeps no
  history of actions beyond the home's current state. Watch times would show when you are awake;
  not keeping them is the protection.
- **Who learns of your home:** you. No endpoint lists homes or who lives where (ship-homes section
  5.10's privacy rules). A view goes only to the owner's sockets (later, a permit holder's).
  Visitors in the 3D world see what is in view of them, as today.
- **Who can act:** the plot's owner; later, a household permit holder (rung 13). A server's admins
  cannot act on your home (question 6).
- **Rate limits:** section 5's buckets, per identity, on the host. A refused action is answered,
  never silently dropped.
- **Transport:** TLS to a public server (through nginx); pinned TLS or Tailscale to your own node;
  never plain over a network.
- **On the phone:** a notice on a secure lock screen shows nothing; the last view lives in memory
  only.
- **Erase and export:** the home's row is deleted with the account and included in its export.
- **Cheating:** a modified client can no longer put crops, ingots or stored goods into a server
  home, because the server holds it. The backpack stays trusted until holdings (section 2).

## 10. The two modes, and the hardware note

| | Simplified (the Forgiving ruleset) | Full realism (the Realistic ruleset) |
|---|---|---|
| While your character is away | The away rules: your home lives on, nothing is ruined | Recommended (question 2): the full rules, with the panel and its notices as your way to respond |
| The home's network | Always up: every machine reports and takes commands | The panel reaches only machines on a powered home network (the home server and router, and the uplink when you are away from home), and shows only what the home's sensors report; a home that loses power to its network goes quiet, with a `home_offline` notice |
| The panel's controls | Every machine control | The same controls, through the network |
| Hands jobs | Need your character in the world | The same |

The home network is gear, so a plain toggle, "Reach every machine from Your home" (a row in
`data/gameplay/levels.json`), lets anyone play Realistic without owning that gear, development and
accessibility first (CLAUDE.md's dual-modes rule). In a server's world the server's rules decide
(`gameplay_rules.rs`), as with every realism level; united-humanity.us plays Forgiving.

**Hardware.** The page needs no GPU and no 3D world: any device that runs the app's pages runs it
(a phone starts without the world's renderer, platforms.md A1). Hosting your single-player home
needs your computer on. A relay is light (about 20 MB of memory and half a percent of one core,
measured on the live server, SELF-HOSTING.md), and the home's simulation costs what it costs in
your game today, because it is the same code. The page says so where a person meets a limit. On a
phone: "The 3D world needs a desktop for now. Your home is here." On an iPhone, until a native app
exists, the website.

## 11. The rungs

Each is sized for one agent and proven on its own. The order: 1 and 2 put the page on the desktop
at once; 3 to 5 move the home's authority to the host (the heavy part); 6 and 7 put it on the
phone for server worlds, because the MMO comes first ("MMO makes sense"); 8 and 9 bring single
player; 10 to 14 add depth.

| # | Rung | Proof |
|---|---|---|
| 1 | **The home view and the Your home page**, on the desktop, from the world on this computer. Read only. (Below, in full.) | The view's numbers equal each system's own, seen red; the view at most 16 KB, seen red; it fits a phone; a snapshot |
| 2 | **The actions contract.** `humanity_systems::home_actions` (no feature gate, beside `home_view`): the target-state actions, the one judge, the refusal words. The walk-up card's Auto-build, the Mining panel's trip and Keep mining, and the page all send through it. New controls (a keep target, on or off, a setpoint where the type has one) go on the walk-up card and the page together | The same action from the card and from the page leaves the same state and the same refusal; a stale `seen_v` is refused once; a target already true is a no-op |
| 3a | **The home host.** `humanity_systems::home_host`: the home's setup and its slow systems' tick, moved down from `sim_boot` and `world_sim`; the engine runs its home through it (no twin) | A game week, headless: the engine's home and a `HomeHost` run alone end byte-identical; the relay build compiles it; `just verify-runtime` |
| 3b | **Away as a mode of the host.** `catch_up_world`, the held world (BUG-253) and a live away become one path: `advance_to` under the away rules | One long catch-up equals the host advancing in steps under the away rules (within farm-clock rung 1's tolerance); the held-world tests stay green |
| 4 | **The server holds and runs homes** (server worlds): the `game_homes` table; a home made fresh at the claim from the start's data; lazy advances; the sweep; deleted with the plot; in the export | Relay tests: a claim makes a fresh home (only the look crossed); a game day matches rung 3a's host; a restart resumes exactly; an erase removes it; the per-home cost measured and written here |
| 5a | **The game mirrors its server home.** In a server world the game's own home systems are off; it takes the home from the host (a snapshot in parts under the 128 KB cap, then deltas at the gauges' resolution) | A rig on a throwaway loopback relay: a crop the server ripened is ripe in the world; the bytes per second measured |
| 5b | **Hands-on actions judged by the host** in a server world (a harvest, a planting, taking from storage, eating); the per-server save on the computer retired | A harvest picks once on the server; one identity on two computers sees one home |
| 6 | **The panel on the wire**: watch, view, delta, act, result; the version in the 5 s check; the caps enforced; the phone's Play shows Your home for its server worlds | A scripted panel against a throwaway loopback relay: each message, a stale press refused, a lost delta mended by the check; the byte caps hold, seen red by lowering one |
| 7 | **Notices from the host**: the new `game_events` rows, the synced game switch, a push without content to a device with no socket | A crop ripening on the server reaches a watching panel as one folded notice; no socket, one empty push; the switch off, nothing |
| 8 | **Single player on your own node**: the solo world hosted by the app's own node through the owner's door, with your Time settings and Dev mode | The boot rigs green through the node (`just verify-runtime`, the storeys and falls rigs); a day away matches today's catch-up |
| 9 | **The phone joins your own node**: "Only my own devices", the QR, the node's pinned TLS, the home network and Tailscale, "your computer is off" | A stranger's key refused at sign-in; plain http refused off loopback; a pinned pairing on loopback with a test certificate |
| 10 | **The game parts of the Life page move to Your home** (Household permits, Animals, the loops), after question 4 | `navigation_tests.rs` still reaches every page; the Life page holds only real-life parts |
| 11 | **The home's network** (Realistic): each machine type declares what it reports and which controls work remotely (data); the home server and router as the gate; the toggle; `home_offline` | With the home server unpowered, a Realistic panel cannot set a pump and says why; Simplified can |
| 12 | **The website mirror** (`web/pages/`), the same view and actions on the website's socket | The web test suite's words, hints and accessibility ratchets; the same scripted relay as rung 6 |
| 13 | **Joint homes**: a household permit grants tending; the host checks it as building does | A permit holder's action applies; a withdrawn permit's is refused |
| 14 | **Machines that do hand jobs**: a harvest and carry drone first (the proposal of 2026-11-24 named "harvest and carry orders for drones"), then nest boxes, milking and seeding machines; each a data row with its capability and real figures | A new machine row gives the panel its action with no code change |

### Rung 1, in full (the first rung)

**What it delivers.** A Your home page on the desktop showing the whole home, read only, from the
world running on this computer. It works wherever you play today, solo or in a server's world,
because today the home runs on your computer in both. The view type and its builder are the real
ones the server will send in rung 6; only where the view comes from changes later.

**The pieces:**

- `crates/humanity-systems/src/home_view/` (no feature gate, so the relay compiles it):
  `HomeView` (serde) and `build(world: &hecs::World, data: &DataStore, now: f64) -> HomeView`. It
  reads only what the systems already publish, the same numbers the walk-up cards and the
  Inventory and Homes pages show: grow areas (from `CropInstance`, the farm clock and
  `farming/outlook.rs` for ripe times), the herd (`animal_care::HerdRow`), tanks (plumbing),
  generation, demand and batteries (solar, electrical, `day_energy`), the rooms' air (atmosphere
  and life support), home storage totals, AutoRefine machines with their status lines, the drone,
  and "Needs you in the world". It brings each grow area up to now before reading it, as every
  reader of a crop does (farm-clock rung 2). Times are game-time instants; like machines fold.
- The engine fills `GuiState::home_view` when the home's version changes, at most once a second,
  and only while the page or the Play hub is showing: versioned reuse that obeys the reuse switch.
- `src/gui/pages/your_home.rs` and `GuiPage::YourHome`: one column under 600 points, two above;
  a tint section per domain with its state in words; reusing `home_loops.rs` and `herd.rs` rather
  than redrawing them; a Play hub card with its `hub_summary`; words under one key group by `node
  scripts/i18n-add.js`, each with its hint. No controls yet: rung 2 adds them.

**The gates (each seen red first, the red recorded in the test's comment):**

- A lib test enters the shipped family homestead with no GPU (`world_sim_tests.rs`'s
  `enter_homestead`) and checks the view against each system's own number: the cistern's litres
  against plumbing's, the ripe count against the farm's, a battery's charge against electrical's,
  a machine's status line against crafting's. Seen red by breaking one mapping.
- The family homestead's view serializes to at most 16 KB of JSON. Seen red with the cap set
  below the real size.
- The page fits a phone: a test in the manner of `the_recovery_phrase_card_fits_a_phone`, at 412
  by 915 points.
- `just snapshot your_home` wide and at phone width (needs a GPU; without one it is skipped with a
  note, so a green line is not proof of a render).
- The words, hints and glossary lints (`tests/i18n_lint.rs` and the rest of `just lints`);
  `cargo check --features relay --no-default-features` compiles `home_view`.

**Traps:**

- `src/gui/mod.rs` sits at 4,786 of its 4,800-line budget (`tests/file_size_ratchet.rs`): the new
  `GuiPage` variant is one line; everything else goes in the page's own file.
- `data/gui/navigation.json` and the `GuiPage` enum are being changed by another agent this week
  (the tidy pass: Real to Profile, the Market merge); add the Play row's item after that merges.
- The page's name is question 1: if the operator chooses another, rename the page, the file, the
  enum variant and the keys together before shipping (CLAUDE.md: names match what people see).

## 12. Questions only the operator can answer

Each has a recommendation; the plan proceeds on the recommendation unless he says otherwise.

1. **What is the page called?** Recommend **"Your home"**, under Play: the words the game's notices
   already use for the game home (`kind_game_event`, "Your home"), and right for a Cabin, a flat or
   a homestead alike. The real-life category keeps its working name, "Life", so the two do not
   clash. ("Homestead" is wrong for a Cabin; "Quarters" fits the ship but is less plain.)
2. **In Realistic, does your home keep its full rules while your character is away?** That is,
   tanks really run dry and crops can die, with the panel and its notices as your way to respond.
   Recommend **yes for Realistic, the away rules for Forgiving**. The panel is what makes it fair:
   offline progression chose "nothing is ruined while you are away" because a player away could
   not respond, and now they can. united-humanity.us plays Forgiving, so nobody there comes back to
   a ruin.
3. **The in-world Garden panel plants and harvests without walking to the plant** (today a group's
   "Harvest N ready" picks every ripe crop from anywhere; `farming/mod.rs`, `harvest_many_request`,
   checks no distance). Keep it? Recommend **keep it for now**: this plan's line is "your character
   in the world", not "within reach", so nothing here depends on it. Whether a hand job should need
   you at the plant belongs to the activities work ([activities.md](activities.md)).
4. **Move the game parts of the Life category's page into Your home?** The Life category's page
   (`homes.rs`) holds the Household permits, the Animals and the home's loops, which are the game's
   (two-realities.md: the app's pages are real life). Recommend **yes, as rung 10**, once the
   navigation redesign settles the Life and Home names.
5. **Your single-player home from anywhere, without opening your PC to the internet?** Your
   desktop app could seal its home's notices to your own key card and post them to your own mailbox
   on your usual server, so your phone hears of them anywhere, with no port opened on your PC; the
   usual server learns only that sealed mail for you arrived. Later, a sealed summary and sealed
   actions could ride the same way. Recommend **notices yes** (cheap: the mailbox and key cards
   exist) and **the summary and actions only if you want them**, since they leave more sealed mail
   stored on the server.
6. **May a server's admins act on players' homes?** Recommend **no**: they keep what they have
   today (releasing a plot, which ends the home), and never run your machines or read your home's
   view. A server's policy is the operator's to set.

## 13. Seams found while writing

- **A server home played from two computers becomes two homes** (increment 6 keeps the save on each
  computer). Closed by rungs 4 and 5.
- **Game notices never leave the device.** Closed by rung 7.
- **The in-world Garden panel acts at any distance.** Question 3.
- **The Life category's page shows game parts.** Question 4 and rung 10.
- **The per-start engagement defaults are read by nothing** (`starts.rs`: "nothing else reads it
  yet"). When a per-domain mode choice exists, the panel shows a domain's controls by it
  (engagement-modes.md: "UI panels filter what to expose based on the player's chosen mode");
  until then the panel shows every domain the home has.
- **`crates/humanity-systems/src/farming/automation.rs` is an empty stub** ("Off-screen farming
  autonomy", a `FarmAutomation` with only a TODO, reading a `config/farming_automation.ron` that
  does not exist), declared in `farming/mod.rs` and used by nothing. The time-away ledger and the
  farm clock do its job; it is dead code to remove.

## Related

- [engagement-modes.md](engagement-modes.md), [progression-skills-gear.md](progression-skills-gear.md),
  [gameplay-loop-map.md](gameplay-loop-map.md): the modes and the loops the panel serves.
- [two-realities.md](two-realities.md): why the panel is under Play.
- [offline-progression.md](offline-progression.md), [farm-clock.md](farm-clock.md),
  [headless-sim-runner.md](headless-sim-runner.md): the away rules, the exact long steps, the
  headless home.
- [ship-homes-and-logistics.md](ship-homes-and-logistics.md): plots, goods, holdings, what the
  server holds.
- [notifications.md](notifications.md), [platforms.md](platforms.md): notices, and the phone.
- [player-home.md](player-home.md): the home the panel shows.
- [decision-briefs.md](decision-briefs.md) Brief 5 recommended client-run crew for the home "for
  now"; this plan moves the home's authority to the host, so crew chores later run on the host
  against the layout it holds (the brief's option A, "Relay learns the client's home layout",
  becomes the host holding that layout itself).
