# The two realities (foundational design axiom)

> **Read this before designing ANY page, widget, or data model.** Operator direction,
> 2026-07-15: "in everything we design there are two distinct realities." This is not a
> feature, it is the lens every feature passes through.

## The axiom

Every domain in HumanityOS exists twice:

1. **A video game** — for fun, education, and socialization. The 3D sim: your character,
   the homestead, the ship, crafting, farming, the solar system you explore.
2. **A real-life tool** — for actually living. Your real possessions, your real body, your
   real tasks, your real money, your real location.

**No Real/Sim toggle.** (Operator, 2026-07-16, revising the earlier draft of this doc — a
mode toggle was tried and deliberately removed before; do NOT bring it back.) A toggle only
exists because one page is trying to show two datasets. The fix is to stop making one page do
double duty: **the app IS your real life, and the game is a place you walk into.**

- Everything in the app chrome (Home, Chat, Tasks, Map, Market, Community, Library) is REAL,
  always. No mode, nothing to switch. Your real possessions live in Home; your real body
  lives in Home. It just *is* real, because that is what the app is.
- The game is a door: **Play**. Inside the 3D world you have a game character, game inventory,
  game crafting — reached from *inside* the world (a backpack / in-game menu). Step out
  (Esc / back) and you are in your life again.

So the two realities are separated by **navigation, not a button**. Entering the world IS the
boundary. Real tools and game systems use similar UI *components* (a slot grid, a stat sheet)
but are different features in different places. The reason both exist: **we teach real-life
skills by mirroring them in the game.** The game is the safe practice space; the real tool is
where it counts.

Operator's rule of thumb: *"Not everything needs to affect the player character, but
everything does affect our real bodies."* So the real side is never a toy — it is a genuine
life-management app that happens to share its interface (and its teaching) with a game.

## What the REAL side must actually do

The game side already exists (the sim). The real-life side is under-built and is the priority
(non-game first). Concretely, the app's real-life side is an all-in-one life suite:

- **Real inventory / possessions.** A list of things you own, how many, and WHERE they are
  stored ("garage, shelf 3", "storage unit A", "lent to Marcus"), with optional value. This
  is the real counterpart to the character's game inventory. Same slot/container UI, real data.
- **Real body / health / fitness / diet ("Real Me").** Height, weight, strength capacity,
  measurements, resting heart rate, etc., tracked over time (trends). Diet/nutrition and
  workouts. The real counterpart to the character sheet + in-game vitals. An all-in-one
  health/fitness/diet app lives here.
- **Real tasks** (already partly there): real to-dos vs the game's quests.
- **Real map** (see two-realities in `docs/design/streaming.md`'s sibling Map design): your
  real-world location + community, over the same real-solar-system substrate the game uses.
- **Real money**: the Market/wallet's real side.

Design mandate: when you build a game system (a crafting station, a vitals bar, a skill),
ask "what is its real-life mirror, and does the app expose it?" If the game teaches cooking,
its mirror is a diet/nutrition tracker in Home. If the game has vitals, the mirror is real
health metrics in Home. The mirror lives in the app's real-life pages, not behind a mode
switch on the game screen.

## The relay is a personal encrypted backup (and a redundancy web)

The real-life data above is precious and irreplaceable, so it must survive a lost/broken
computer. The architecture (extends the existing vault-sync + signed-object + E2EE stack,
see `docs/design/storage-architecture.md`):

- **Local-first, E2EE.** Real data lives on the user's device, encrypted with their own key.
  A backup is an encrypted blob the relay stores but CANNOT read (same zero-knowledge posture
  as DMs and the vault). Losing the computer never means losing the data or leaking it.
- **The relay is a backup host, not just a chat server.** The operator's relay backs up the
  operator's data AND, with permission, the data of family/friends he grants space to. Each
  permitted user gets an encrypted, quota-bounded backup slot. "A safe place for all of their
  stuff."
- **Redundancy web (aspirational, noted 2026-07-15).** Anyone can run a relay; relays on
  different continents mirror each other so a backup survives the loss of any one site. This
  is the federation/mirroring layer applied to personal backups, not just public content.

Permission + quota + encryption are the trust boundary. Never store a user's real data in a
form the relay operator can read; never let one user's backup consume another's quota.

## How this reshapes the nav (ties to the 8-button proposal)

- **Home = your real life.** The real-life dashboard: Real Me (body/health/fitness/diet),
  your possessions (with backup status), your real tasks and money at a glance. "Me."
- **Play = your game self.** The character, inventory, and crafting live *inside* the world,
  reached from a backpack / in-game menu — not from the app chrome.
- **No shared toggled pages.** The real-life tools are the app's pages (always real); the game
  systems live inside the sim (always game). Similar UI components, different places. You move
  between the two realities by entering or leaving the world, never by flipping a mode switch.

The split is clean: Home is who you really are, Play is who you are in the game, and the
toggle inside each shared tool flips the dataset. Both are first-class; the Real side is the
one we owe the most work.
