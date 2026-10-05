# Death and your pack

Written 2026-10-04. Code: `src/systems/death_pack.rs` (the rules and the arithmetic, no
window, compiled into the relay with the save format), `src/engine/death_pack.rs` (the
frame: where you fell, the pack drawn, the marker, the E press, the clock; tests in
`death_pack_tests.rs`). Data: `data/world/death.ron`. Setting: Settings > Gameplay > Death.

## The decision

The first-hour audit (`docs/design/first-hour-audit-2026-10-04.md`) found that death cost
nothing: full health back, food, water and rest raised, nothing dropped. The operator
accepted its recommendation the same evening: "Today the death screen says 'Nothing was
lost.' I'd keep that as the Simplified mode. In Realistic mode, your carried items would stay
where you fell for a while, to go back for."

That is the project's dual-mode rule (CLAUDE.md, "Dual modes"): every deep system ships full
realism and a simplified mode, and general play defaults to the softened one. It sits beside
Body heat and Carrying weight in Settings > Gameplay, Simplified by default.

## What each mode does

Either way you wake in the respawner with full health, your food, water and rest raised to at
least 60 of 100, full breath, a normal body temperature and your status effects cleared
(unchanged since v0.745).

**Simplified** (the default): nothing is lost. The death screen says what it always said: "You
wake in the respawner. Nothing was lost, but the body remembers: keep fed, hydrated, warm, and
breathing."

**Realistic**:

- **What dies with you:** everything in your backpack. It stays behind, as one pack, where you
  fell.
- **What is kept:** what you wear (the outfit, worn clothing and equipped gear, the large
  backpack included, so its carrying capacity is still yours), your credits, skills, quests,
  home storage, and anything given to a fleet and still waiting for its answer (it had already
  left the backpack).
- **Where it lies:** on the floor under your feet aboard (the floor the walk had, a stair top
  included), on the ground under you on a planet. Where no one can stand it lies at the nearest
  place that can be walked to, and the death screen says why and how far:
  - open space (aboard, more than 2 m outside every room, out on the hull or off it; or off
    the ship near no planet or moon): the nearest room's floor, kept 0.4 m in from its walls.
    Closer than 2 m is a doorway or a wall's width, where people walk, so it counts as where you
    fell, and so do feet up to 0.5 m under a floor or over a ceiling (a stair top between two
    storeys);
  - deep water (on a planet, where the connected-ocean mask or a water world's sea floor says
    so, the same test that stops building on water): the nearest dry ground, found by searching
    out in rings and walking back to the shore, then 3 m onto the land;
  - the air (more than 3 m over the ground when you died): the ground below, and how far down.
  If a planet has no dry ground within the search (3,000 km, which reaches land from anywhere in
  Earth's oceans), or a body has no known surface at all, the pack goes to the ship's nearest
  floor, and the death screen says that too: "There was no ground you could walk to near where
  you died, so it lies on the ship's nearest floor, 12,000 km away, in the Kitchen."
- **The marker:** every pack is a tracked marker on the HUD, the direction-placed one the Home
  Station uses (BUG-148's `marker_placement`): "Your pack · 120 m", pinned to the screen's edge
  on the side to turn toward when it is behind you. Several packs are numbered, newest first.
- **Taking it back:** walk to it, face it, and press E: "[E] Take back your pack (14 items)".
  As much as your backpack holds goes back in, each stack with its wear, grade and a food's age,
  through the same volume limit every add to the backpack meets; the rest stays in the pack. An
  emptied pack is gone. The pack comes last in the E chain, so a machine, a person, a vehicle, a
  door or a built bed in front of you still gets the press.
- **How long it stays:** 60 minutes of PLAY (data/world/death.ron), counted only while the
  world is loaded and you are alive. A menu open over the running world counts, as the world's
  own clock does; the death screen, the time before you first enter the world, and the time the
  game is closed never do (the offline catch-up does not touch it), so quitting never loses it.
  A notice comes 5 minutes of play before it is gone, and another when it is: "Your pack in the
  Commons is gone: it lay there for 60 minutes of play. The 14 items in it are lost."
- **Food in it** keeps aging, at the home air's rate (the same rule and the same known gap as a
  chest built on a planet).
- **The save** keeps every pack whole: what it holds, where it lies, the play time it has
  counted, and whether it belongs to the death still on screen, so a quit on the death screen
  comes back to the same words. A pack lying in the home moves with the home when the home is
  carried to another plot, as a built chest does.

The death screen in Realistic says what stayed, where, and how to get it back: "You wake in the
respawner. Everything in your backpack, 14 items, stays behind in your pack. What you wear and
what you have equipped stay on you." / "It lies where you fell, in the Kitchen." / "It is marked
on your screen. Walk to it and press E to take back what fits. It stays for 60 minutes of play,
then it is gone." With nothing in the backpack it says so, and that nothing was left. The key
named is the one Interact is bound to, and the time is the data file's.

## Why it is built this way

- **One pack, not loose items.** A pile of thirty items is thirty things to find and thirty
  draws; one pack is one marker, one prompt and one entity, and it is what a person's carried
  goods actually become when they drop.
- **Minutes of play, not a wall-clock deadline.** A deadline on the clock would punish closing
  the game for the night; counting play means the risk is only ever running while the player can
  do something about it. The countdown is a field in the save (`played_s`), so there is no clock
  to compare against on load and nothing for the offline catch-up to advance by accident.
- **Placed when the death happens, before the save.** The pack is left in the same frame the
  systems record the death, before the periodic save, so no save can hold a death with a full
  backpack; a quit on the death screen loads the pack and the death together.
- **Every number in data.** The time, the warning, the reach, the facing cone, the air
  threshold, the wall clearance, the room slack, the open-space distance, the shore search and
  the pack's size and colours are in `data/world/death.ron`; the code holds none (the Default
  reads the same file).
- **What it is drawn as.** The pack is built from five boxes (bag, lid, front pocket, two
  shoulder straps) in proportion to the data file's size. There is no backpack model in the
  assets; a real model is the obvious next rung and drops into `push_render_objects`.

## On a shared server

For now the pack is yours alone. It lives in your own game and your own save, never on the
relay, so nobody else sees it, and nobody can take it. Whether a pack should be lootable by
others is a decision for the operator. A shared version would need:

1. The pack to live on the server: the relay owns it, its contents and its place, and every
   player in range is sent it (the shared world's interest management, `game_interest.rs`).
2. A rule for who may open it and when: anyone, your faction, only after a delay, never in a
   safe zone. That is the operator's call and should be a server setting.
3. The take to be atomic on the server, so two players pressing E at once cannot both get the
   same items, and the items to move between backpacks through the server, the way trades do.
4. The clock to run on the server's time, not each player's play time, because the pack is no
   longer one person's.
5. The death screen and the marker to say whether it can be looted, so nobody is surprised.

## Known gaps

- A pack on a planet ages its food at the home air's rate, not that planet's weather (the
  planet chest's gap).
- The rules are read at startup: an edit to the data file needs a restart.
- A death outside the world (the systems tick in the menus before it too) has nowhere to leave
  a pack, so nothing is lost and the death screen says nothing was lost.
- The pack is five boxes, not a modelled backpack.
- Each death in Realistic with something in the backpack leaves a pack of its own; several can
  lie about at once, each on its own clock.
- The mode is each player's own choice, on a server too; a server cannot yet require Realistic
  (tracked in `docs/design/in-app-ops.md`).
- Rig tooling: the showcase verb `{"die":"<cause>"}` kills the player through the same path a
  real death takes, so a rig can die, respawn (`{"respawn":"1"}`) and walk back to the pack.
