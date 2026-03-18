# Gardening Game — HumanityOS First Minigame

**Status:** Proposed
**Author:** Shaostoul + Claude
**Date:** 2026-03-17
**Affects:** `game/` directory, relay multiplayer, marketplace integration

---

## Overview

A farming/gardening minigame inspired by Stardew Valley, rendered in 2D isometric using Canvas 2D. This is the first minigame for HumanityOS. It runs in the browser and Tauri desktop today; the 3D desktop engine (Bevy) can host an upgraded version later. The 2D version is the canonical implementation.

Players tend a garden plot: planting, watering, harvesting, and selling crops. Real botanical data grounds the gameplay in actual agriculture, tying into HumanityOS's educational mission.

---

## Core Gameplay Loop

### Plant → Tend → Harvest → Sell

1. **Prepare soil** — Use the hoe to till a dirt tile into farmable soil.
2. **Plant seeds** — Select a seed from inventory, click a tilled tile.
3. **Water** — Water each planted tile daily (or let rain handle it).
4. **Wait** — Crops grow through stages over real time (configurable speed multiplier).
5. **Harvest** — Click a mature crop with the harvest basket to collect it.
6. **Sell or use** — Sell at the market for coins, or use in recipes/gifts.

### Soil System

| Property | Range | Effect |
|----------|-------|--------|
| Fertility | 0–100 | Growth speed multiplier (100 = 1x, 50 = 0.5x) |
| Moisture | 0–100 | Depletes daily; below 20 = wilting; 0 = withering |
| pH | 4.0–9.0 | Each crop has preferred range; outside = growth penalty |

- Fertility drops with each harvest; restored by composting, crop rotation, or fertilizer.
- Companion planting bonuses: tomatoes + basil = +15% yield for both.
- Monoculture penalty: same crop 3+ seasons in a row = fertility drain.

### Seasons (4 cycles, each ~7 real days at 1x speed)

| Season | Weather bias | Available crops |
|--------|-------------|-----------------|
| Spring | Rain 40%, Sun 50%, Overcast 10% | Lettuce, peas, carrots, strawberries, potatoes |
| Summer | Sun 60%, Rain 20%, Heat 15%, Storm 5% | Tomatoes, corn, melons, peppers, sunflowers |
| Autumn | Overcast 35%, Rain 30%, Sun 25%, Frost 10% | Pumpkins, squash, apples, wheat, garlic |
| Winter | Snow 40%, Frost 30%, Overcast 20%, Sun 10% | Greenhouse only: herbs, mushrooms, sprouts |

### Weather Effects

| Weather | Soil moisture | Growth | Special |
|---------|--------------|--------|---------|
| Sun | -10/day | +10% speed | Sunflowers love it |
| Rain | +30/day | Normal | No need to water |
| Storm | +50/day | -10% speed | Chance of crop damage (5%) |
| Heat | -20/day | -20% speed | Wilting risk doubles |
| Frost | 0 | -50% speed | Kills unprotected warm-season crops |
| Snow | 0 | Paused (outdoor) | Aesthetic only; greenhouses unaffected |

### Crop Varieties

Each crop definition includes:

```js
{
  id: "tomato",
  name: "Tomato",
  season: ["summer"],
  growthDays: 8,          // at 1x speed, fertility 100
  stages: 5,              // seed → sprout → vine → flowering → ripe
  waterNeed: "medium",    // low/medium/high
  preferredPH: [6.0, 6.8],
  baseValue: 15,          // sell price
  seedCost: 5,
  companions: ["basil", "carrot"],
  rivals: ["cabbage", "fennel"],
  realFact: "Tomatoes are botanically berries and originated in South America."
}
```

Starter crops (unlocked from the beginning):
- Lettuce (3 days, value 5) — spring
- Carrot (5 days, value 8) — spring/autumn
- Tomato (8 days, value 15) — summer
- Pumpkin (12 days, value 25) — autumn

Advanced crops unlock through progression.

### Tool System

| Tool | Action | Upgrade path |
|------|--------|-------------|
| **Hoe** | Till soil tiles | Bronze → Iron → Steel (wider area) |
| **Watering Can** | Add moisture to soil | Copper → Silver (larger radius, more water) |
| **Seed Bag** | Plant selected seed | Auto-selects from inventory |
| **Harvest Basket** | Collect mature crops | Upgraded = auto-harvest adjacent tiles |
| **Compost Bin** | Convert waste → fertilizer | Unlockable structure |

Tools are selected from a toolbar; active tool determines what click/tap does on tiles.

---

## Rendering — 2D Isometric Canvas

### Tile Grid

- **Tile size:** 48x48 pixels (matches hosIcon viewBox, good density on mobile)
- **Isometric projection:** Standard 2:1 diamond. Screen coordinates:
  ```
  screenX = (tileX - tileY) * (TILE_WIDTH / 2) + cameraOffsetX
  screenY = (tileX + tileY) * (TILE_HEIGHT / 4) + cameraOffsetY
  ```
- **Grid size:** Starts at 6x6 (small plot), expandable to 24x24 (full farm).
- **Rendering order:** Back-to-front (top-left to bottom-right in iso) for correct overlap.

### Sprite System

All sprites live on a single sprite atlas (`garden-sprites.png`) to minimize draw calls.

**Plant growth stages (per crop):**
```
Stage 0: Seed       — small dot in soil
Stage 1: Sprout     — tiny green stem
Stage 2: Growing    — leaves/vine visible
Stage 3: Flowering  — color appears (optional per crop)
Stage 4: Ready      — full fruit/vegetable visible, subtle bounce animation
Stage 5: Withered   — brown/gray, drooped (if neglected)
```

Each stage is a 48x48 sprite frame on the atlas. Crops with fewer visual stages skip flowering.

**Terrain tiles:**
- Grass (default)
- Dirt (hoed)
- Tilled soil (ready for planting)
- Wet soil (recently watered, darker shade)
- Path (stone, wood)
- Fence, gate

**Player character:**
- 48x48, 4-direction walk (N/S/E/W in iso)
- 2-frame walk cycle per direction (8 frames total)
- Idle pose per direction (4 frames)
- Tool-use animation: 2 frames per tool (swing hoe, pour water, etc.)

### Day/Night Cycle

Implemented as a semi-transparent canvas overlay drawn last each frame:

```js
// 0.0 = midnight, 0.5 = noon, 1.0 = midnight
const alpha = Math.max(0, 0.6 - Math.abs(timeOfDay - 0.5) * 1.2);
ctx.fillStyle = `rgba(10, 10, 40, ${alpha})`;
ctx.fillRect(0, 0, canvas.width, canvas.height);
```

Dawn/dusk tint shifts from blue-gray to warm amber.

### Weather Particles

Drawn on a dedicated layer above tiles, below UI:

| Weather | Particle |
|---------|----------|
| Rain | Blue-white diagonal lines, 200–400 particles |
| Snow | White dots with slow sine-wave drift, 100–200 particles |
| Sun rays | 3–5 translucent gold triangles from top-right corner |
| Storm | Rain + periodic white flash (lightning) every 8–15s |

Particle system budget: max 500 particles, recycled from pool (no GC pressure).

### Camera

- Pan: click-drag or WASD/arrow keys
- Zoom: scroll wheel or pinch, 3 levels (0.5x, 1x, 2x)
- Snap-to-grid optional (for precise planting)
- Smooth lerp on camera movement (0.1 factor per frame)

---

## Progression

### Garden Expansion

| Level | Plot size | Unlock |
|-------|-----------|--------|
| 1 | 6x6 | Starting plot |
| 2 | 8x8 | After first 10 harvests |
| 3 | 12x12 | After earning 500 coins |
| 4 | 16x16 | After growing 10 different crops |
| 5 | 20x20 | After completing "Master Gardener" quest |
| 6 | 24x24 | After all seasonal crops grown at least once |

### Economy

- Coins earned by selling crops at the **Market** (ties into the marketplace page at `pages/marketplace.html`).
- Prices fluctuate by +-20% based on supply (community-wide if multiplayer is active).
- Rare crops and perfect-quality harvests earn bonus coins.
- Coins can buy: seeds, tools, plot expansions, decorations, greenhouse.

### Quality System

Harvest quality depends on care:

| Quality | Condition | Value multiplier |
|---------|-----------|-----------------|
| Wilted | Moisture hit 0 at any point | 0.25x |
| Normal | Basic care | 1.0x |
| Good | Watered every day, correct pH | 1.5x |
| Perfect | Good + companion bonus + peak fertility | 2.5x |

### Unlockables

- **Greenhouse** (500 coins) — grow any-season crops year-round, immune to weather.
- **Sprinkler** (200 coins) — auto-waters a 3x3 area each day.
- **Scarecrow** (50 coins) — prevents random crop damage in 5x5 radius.
- **Beehive** (150 coins) — pollination bonus (+10% yield) in 4x4 radius; produces honey.
- **Decorations** — flowers, paths, fences, benches, lanterns (cosmetic + garden beauty score).
- **Recipes** — combine crops into prepared foods (higher sell value).

---

## Multiplayer (via Relay WebSocket)

### Garden Visits

Players can visit friends' gardens (read-only by default, owners can grant edit permission).

```js
// Request garden visit via existing relay
ws.send(JSON.stringify({
  type: "GardenVisit",
  target: friendPublicKeyHex,
  action: "request"
}));
```

The visited garden renders as a read-only isometric view. The visitor's character appears in the garden.

### Trading

Crops and seeds are tradeable items. Trade uses direct messages through the existing DM relay system:

```js
{
  type: "GardenTrade",
  target: recipientKey,
  offer: [{ item: "tomato", qty: 5 }],
  request: [{ item: "pumpkin_seed", qty: 2 }]
}
```

Both parties confirm before the trade executes. Items transfer atomically.

### Community Gardens

Shared plots on the relay server. Any authenticated user can tend a tile. Good for cooperative events (e.g., "grow 1000 sunflowers as a community").

State stored server-side in a new `garden_plots` SQLite table:

```sql
CREATE TABLE garden_plots (
  plot_id    TEXT PRIMARY KEY,
  owner_key  TEXT NOT NULL,
  grid_data  TEXT NOT NULL,        -- JSON: tile states, planted crops
  visitors   TEXT DEFAULT '[]',    -- JSON: allowed visitor keys
  updated_at INTEGER NOT NULL
);
```

---

## Educational Tie-In

### Real Botanical Data

Every crop entry includes:
- **Real growing season** (USDA zones mapped to in-game seasons).
- **Companion planting chart** based on actual agricultural research.
- **Fun fact** shown on hover/inspection (one-liner from real botany).
- **Soil science tip** shown when soil conditions are suboptimal.

### Learning Moments (non-intrusive)

- First time planting: tooltip explains seed depth and spacing.
- First drought: tooltip about water conservation and mulching.
- First companion bonus: explains why certain plants help each other.
- Seasonal transition: brief note on crop rotation benefits.

These are dismissable and stored in a `gardenTutorialSeen` localStorage key so they only show once.

### Sustainability Concepts

- Composting mechanic: food waste → fertilizer (models real nutrient cycling).
- Monoculture penalty teaches crop diversity.
- Water conservation: rain barrels collect free water during storms.
- Pollinators: beehive mechanic mirrors real pollination dependency.

---

## Technical Architecture

### State Management

All game state serializes to a single JSON blob:

```js
{
  version: 1,
  garden: {
    size: [8, 8],
    tiles: [
      { x: 0, y: 0, type: "tilled", moisture: 65, fertility: 80, pH: 6.2,
        crop: { id: "tomato", stage: 3, plantedAt: 1710000000000, wateredToday: true }
      },
      // ...
    ]
  },
  inventory: {
    seeds: { tomato: 5, carrot: 10 },
    crops: { tomato: 3 },
    tools: { hoe: 1, wateringCan: 2 },   // tier level
    coins: 150
  },
  progression: {
    level: 2,
    totalHarvests: 14,
    uniqueCrops: ["lettuce", "carrot", "tomato"],
    tutorialSeen: ["planting", "watering"]
  },
  settings: {
    timeScale: 1,        // 1x real-time, up to 10x
    autoSaveInterval: 60  // seconds
  },
  timestamp: 1710000000000
}
```

- **Primary storage:** `localStorage` key `gardenGameState`.
- **Auto-save:** every 60 seconds and on page unload (`beforeunload`).
- **Offline-capable:** full game works without server connection.
- **Cloud sync (optional):** push/pull state via `PUT /api/vault/sync` (already exists for vault blobs, reuse the same endpoint with a garden namespace).

### Growth Simulation

Growth advances based on elapsed real time (not frame ticks), so closing the browser and returning later catches up:

```js
function simulateGrowth(tile, now) {
  const elapsed = now - tile.crop.lastUpdate;
  const daysElapsed = elapsed / (MS_PER_DAY / gameState.settings.timeScale);
  const growthRate = tile.fertility / 100;
  tile.crop.growthProgress += daysElapsed * growthRate;
  // Advance stages based on growthProgress vs crop.growthDays
}
```

Moisture depletes per simulated day. If moisture hits 0 and stays there for 2+ days, crop withers.

### Rendering Pipeline (per frame)

1. Clear canvas.
2. Calculate visible tile range from camera position + viewport.
3. Draw terrain tiles (back-to-front iso order).
4. Draw crops/structures on each tile (same order, offset Y for height).
5. Draw player character at correct iso depth.
6. Draw weather particle layer.
7. Draw day/night overlay.
8. Draw UI layer (toolbar, info panel, minimap) — these can be HTML overlaid on canvas or drawn on a separate canvas.

Target: 60 FPS on mid-range hardware. The tile grid is small enough that brute-force redraw is fine (no need for dirty-rect optimization at 24x24 max).

### Input Handling

| Input | Action |
|-------|--------|
| Click tile | Use active tool on tile |
| Right-click tile | Inspect tile (show soil stats, crop info) |
| 1–5 keys | Select tool from toolbar |
| WASD / Arrow keys | Pan camera |
| Scroll wheel | Zoom in/out |
| Space | Pause/resume time |
| E | Open inventory |
| M | Open market |
| Esc | Deselect / close panel |

Touch support: tap = click, two-finger drag = pan, pinch = zoom, long-press = inspect.

---

## File Structure

```
game/
├── garden.html                 # Page: canvas + UI shell, loads shell.js
├── js/
│   ├── garden-engine.js        # Isometric renderer, camera, input, sprite atlas
│   ├── garden-game.js          # Game logic: growth sim, tools, economy, save/load
│   ├── garden-data.js          # Crop definitions, recipes, progression tables
│   └── garden-multiplayer.js   # WebSocket integration for visits/trading
├── assets/
│   └── garden/
│       ├── sprites.png         # Combined sprite atlas (tiles + crops + player + UI)
│       ├── sprites.json        # Atlas metadata: frame rects, animation sequences
│       └── weather/            # Optional: separate particle sprites if needed
```

`garden.html` follows the same pattern as other pages: loads `shared/shell.js` for nav, then game scripts. No build step.

### Script Load Order

```html
<script src="/shared/shell.js"></script>
<script src="/game/js/garden-data.js"></script>
<script src="/game/js/garden-engine.js"></script>
<script src="/game/js/garden-game.js"></script>
<script src="/game/js/garden-multiplayer.js"></script>
```

---

## UI Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│  ☰ HumanityOS    Garden    🌤 Summer Day 12    💰 150    ⏸ 1x     │
├──────────────────────────────────────────────────┬───────────────────┤
│                                                  │ ┌───────────────┐│
│                                                  │ │  🍅 Tomato    ││
│            ◇ ◇ ◇ ◇ ◇ ◇                         │ │  Stage: Vine  ││
│           ◇ 🌱◇ 🥕◇ ◇ ◇                        │ │  Day 5 of 8   ││
│          ◇ 🌱◇ 🍅◇ 🌻◇ ◇                       │ │               ││
│         ◇ ◇ ◇ 🍅◇ 🌻◇ ◇                        │ │  Moisture: 72 ││
│        ◇ ◇ ◇ ◇ ◇ 🎃◇ ◇                         │ │  Fertility:85 ││
│       ◇ ◇ ◇ ◇ ◇ ◇ ◇ ◇                          │ │  pH: 6.4      ││
│          ◇ ◇ ◇ ◇ ◇ ◇                            │ │               ││
│                                                  │ │  Quality:Good ││
│          (isometric garden grid)                 │ │  Companions:  ││
│                                                  │ │   🌿 Basil    ││
│                          👤                      │ │               ││
│                      (player)                    │ │  💡 Tomatoes  ││
│                                                  │ │  are berries! ││
│                                                  │ └───────────────┘│
│                                                  │                  │
│                                                  │ ┌───────────────┐│
│                                                  │ │  Inventory    ││
│                                                  │ │  🌱 x5  🥕 x3 ││
│                                                  │ │  🍅 x2  🎃 x1 ││
│                                                  │ └───────────────┘│
├──────────────────────────────────────────────────┴───────────────────┤
│  [ 🔨 Hoe ] [ 💧 Water ] [ 🌱 Seeds ] [ 🧺 Harvest ] [ 📦 Inv ]  │
│                        ^^^  active tool                             │
└──────────────────────────────────────────────────────────────────────┘
```

**Layout breakdown:**
- **Top bar:** Navigation (shell.js), season/day indicator, coin balance, time controls.
- **Center:** Canvas element, fills available space. The isometric grid renders here.
- **Right panel:** Context-sensitive info. Shows selected tile details, crop status, educational facts. Collapses on mobile (tap to toggle).
- **Bottom toolbar:** Tool selection. Active tool highlighted. Keyboard shortcuts 1–5 mapped.

On mobile (< 768px), the right panel becomes a slide-up drawer triggered by tapping a tile. The toolbar shrinks to icons only.

---

## Art Direction

- **Style:** Pixel art, 48x48 base tile. Clean outlines, limited palette (32–48 colors).
- **Palette:** Warm earth tones for soil/paths. Vibrant greens for healthy plants. Muted browns/grays for withered crops. Seasonal tint shifts (warm spring, hot summer, amber autumn, cool winter).
- **Inspiration:** Stardew Valley's clarity and charm, not photorealism.
- **Sprites needed (MVP):**
  - 6 terrain tiles (grass, dirt, tilled, wet, path, fence)
  - 4 starter crops x 5 growth stages = 20 crop frames
  - Player character: 12 frames (4 directions x idle + 2 walk frames)
  - 5 tool icons for toolbar
  - UI elements: selection highlight, grid overlay, cursor indicators
  - Weather particles: raindrop, snowflake, sun ray (3–4 px each)

Total MVP atlas: roughly 60–80 frames on a 512x512 sprite sheet.

---

## Implementation Phases

### Phase 1 — Core (MVP)
- Isometric renderer with camera pan/zoom
- 6x6 tilled grid, 4 starter crops
- Plant/water/harvest loop
- Growth simulation with time passage
- Basic toolbar and tile inspection
- Save/load to localStorage
- Day/night cycle overlay

### Phase 2 — Depth
- Seasons and weather system
- Soil quality (fertility, moisture, pH)
- Companion planting bonuses
- Quality system (wilted → perfect)
- Market selling with coin economy
- Plot expansion (6x6 → 12x12)
- Inventory screen

### Phase 3 — Multiplayer
- Garden state sync via relay WebSocket
- Visit friends' gardens (read-only view)
- Crop/seed trading
- Community garden plots (shared server-side state)

### Phase 4 — Polish
- Full crop catalog (20+ crops across all seasons)
- Structures (greenhouse, sprinkler, scarecrow, beehive)
- Decorations and garden beauty score
- Recipes (combine crops into food items)
- Achievements / quest system
- Sound effects and ambient audio
- Tutorial flow for new players

### Phase 5 — 3D Upgrade
- Port game logic to Bevy ECS (reuse `garden-data.js` as JSON data files)
- 3D voxel or low-poly models replace 2D sprites
- Same save format, import existing gardens
- First-person or third-person camera in the 3D engine

---

## Open Questions

1. **Time scale default** — Should 1 in-game day = 1 real day (slow, meditative) or 1 real hour (faster feedback)? Configurable either way, but the default sets expectations.
2. **Monetization** — Are decorations/cosmetics purchasable with real currency, or purely earned in-game? (Aligns with HumanityOS anti-exploitation values: no pay-to-win.)
3. **Cross-game items** — Can harvested crops appear in other HumanityOS systems (marketplace listings, gifts in chat, crafting in future games)?
4. **Sprite creation** — Commission pixel artist, use AI generation as base + manual cleanup, or community-contributed?
5. **Mobile-first or desktop-first** — Canvas 2D runs everywhere, but touch input design differs from mouse. Pick the primary target for Phase 1 and adapt the other.
