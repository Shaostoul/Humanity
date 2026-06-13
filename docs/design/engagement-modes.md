# Engagement Modes: How Players Meet Their Needs

> **Status:** Proposed (2026-05-04). Drafted from in-session design discussion. Supersedes the earlier "tier framework" idea, which had combinatorial mod-failure problems and did not solve the accessibility goal.

## The problem this solves

HumanityOS simulates real-world systems, water, food, fertilizer, energy, at meaningful depth. But not every player wants to engage with every system. A player who loves gardening might hate electrical engineering. A player who wants to focus on social or governance work might want to ignore survival production entirely. Two failure modes to avoid:

- **Forcing every player through every system** kills accessibility.
- **Making the simulation shallow enough for everyone** kills depth.

The right answer is to keep the simulation deep AND give players multiple ways to interact with each domain.

## The principle

**One simulation depth, multiple engagement modes.**

The underlying simulation runs at consistent engineering depth always. One code path. Mods extend the simulation (add new content) rather than fork it (add tiers).

The PLAYER picks per-domain how they meet a given need. There are five engagement modes.

## The five modes

### 1. Direct production
Player tends garden, runs electrical system, manages water, refines fertilizer. Engineering UI exposed. Maximum agency, maximum hands-on time.

### 2. Trade
Player works at something else they enjoy (programming, music, art, governance), and the market provides survival needs. UI is a marketplace. NPCs or other players grew the food.

### 3. Automation
Player sets up robots, aeroponics, solar tracking, sensor networks. System runs unattended once configured. Player checks in occasionally; harvest is passive.

### 4. Cooperative
Player joins a guild or community that pools production. They contribute one thing (programming, music, security, leadership) and access the cooperative's pool of food, water, energy.

### 5. Background
Needs met automatically (late-game, post-scarcity, fully-automated homestead). Player engages with whatever they enjoy. UI exposes the survival domain only on request.

A single player typically MIXES modes, Direct for what they enjoy, Trade for what they don't. The mode is chosen per-domain (water, food, fertilizer, energy), not globally.

## Why this works

- **One simulation = one code path** = far fewer failure modes than a tier matrix.
- **Mods add content, not tiers.** A new fertilizer type or contaminant slots into existing structure rather than forking it. Massively reduces combinatorial breakage from third-party mods.
- **Accessibility is automatic.** Player picks engagement level once; UI shows only what's relevant to that mode for that domain. They don't navigate "what depth is this domain at?"
- **Five distinct onramps** for different player types. Direct serves engineers. Trade serves traders. Automation serves builders. Cooperative serves social players. Background serves players who treat HumanityOS as a hangout space.
- **Maps to the post-scarcity arc.** As the world advances, more domains drift toward Background mode for most players; Direct mode becomes a hobby/practice/cultural-memory activity rather than necessity. Same simulation, evolving relationship.
- **Maps to systems already in HumanityOS.** Market/Trade pages, Guilds, Civilization, Automation are already in the architecture for modes 2, 4, and (eventually) 5.

## Investigation pattern (Direct mode UX)

When a player chooses Direct mode for a domain, the UI must support an exploratory, comparative decision flow. The unit of design is **diagnostic + education + options + economics + idle-resource awareness**.

Worked example, player notices a problem with a tomato plant:

```
Tomato plant - Status: Underperforming

Soil sample:
  N (Nitrogen):     12 ppm   ← LOW (need 30-50)
  P (Phosphorus):   45 ppm   ← OK
  K (Potassium):   180 ppm   ← OK
  pH:                6.4     ← OK
  Moisture:          28%     ← OK

Why N matters:
  Nitrogen drives leaf growth. Without it, plants yellow, growth
  stalls, fruit yield drops by ~40%.

Options to add N (cheapest first):
  • Compost (slow, free)          - you have 12 kg ready ✓
  • Urine diversion (fast, free)  - your bathroom produces 1.5 L/day
  • Wood ash (very slow)          - N-low, mostly K - skip for this
  • Biochar charge (medium)       - needs pre-soaking in N source
  • Ammonium sulfate (instant)    - Market: 8 cr/kg | you have: 4 cr

Idle resources flagged:
  • Mining drone (idle 3 days)    - could be hauling compost
  • Compost pile #2 (ready 2d ago) - apply now for free fix
```

This is the unit of UI design for Direct mode. Each plant, each system, each domain gets its own version. The pattern is consistent; the data varies.

Players in Trade / Automation / Cooperative / Background modes for this domain do **not** see this view. They see their preferred interface, market UI, automation config, guild dashboard, or nothing. The simulation still runs underneath; they just don't touch it.

## Implementation pattern

- ECS components hold simulation state at consistent depth (one tier, roughly equivalent to "properties and ratios": NPK values, watts/kWh, calories+macros, contaminant categories).
- UI panels filter what to expose based on the player's chosen mode for that domain.
- The simulation itself doesn't know which mode the player picked, it always runs at full depth.
- Settings page exposes per-domain mode choice.
- Real/Sim toggle informs defaults (Real mode tends Direct; Sim mode tends Background).
- Mode transitions need narrative/UX care (e.g., player abandons their garden → NPCs or automation take over; what happens to their crops in transition?).

## Open questions

- How granular should mode-per-domain be? Just water/food/fertilizer/energy, or finer (rainwater vs well water, leafy vs fruiting plants, AC vs DC electrical)?
- What does the transition between modes feel like in-game? (Player abandons garden → NPC tenders take over → garden produces; player retakes Direct mode → reclaims)
- Does Background mode have a "subscription cost" (cooperative dues, automation maintenance) or is everything free in late-game?
- How do mods declare which mode they extend? (A new automation drone is a Mode 3 extension; a new fertilizer recipe applies to Modes 1 and 3.)
- How does the engagement-mode choice surface to new players during onboarding? (Probably: pick one default, change later. Don't force five choices upfront.)

## Relationship to existing design rules

- **Infinite-of-X:** modes 2, 4, 5 require lots of NPCs, lots of guilds, lots of automation, all of which must be data-driven.
- **Universal widgets:** the diagnostic+options panel becomes a reusable widget that adapts to any domain (water, food, fertilizer, energy).
- **Real/Sim toggle:** informs default mode per domain, but doesn't override player choice.
- **AI-as-citizens:** AI agents can occupy any of the five modes alongside humans. An AI agent might Direct-produce food while trading services with human players.
- **Mod-first architecture:** mods extend the simulation horizontally (new content) rather than vertically (new tiers), which keeps mod compatibility tractable.

## Related docs

- `docs/design/ui-system.md`, universal widget contract
- `docs/design/infinite-of-x.md`, data-driven content rule
- `docs/design/educational-gameplay.md`, teach real survival skills through simulation
