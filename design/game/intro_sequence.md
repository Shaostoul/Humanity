# Intro Sequence — "The Dream"

## Overview
The first-time player experience. No menus, no tutorials, no text walls. Pure cinematic immersion that establishes the stakes, the beauty, and the mission.

## Sequence

### Act 1: Beauty (~60 seconds)
- **Fade in** from black
- Player is standing on the deck of a beautiful house on Mount Rainier (their default home)
- Night sky: **heavy meteor shower** streaking across the sky
- **Aurora borealis** — green/purple curtains dancing across the northern horizon
- Sparse clouds drift past, occasionally occluding stars
- Real constellations visible — Orion, the Big Dipper, the Milky Way band
- Ambient sounds: wind, distant owl, crackling fire from inside
- The player can look around freely (or camera slowly pans if passive viewing)
- A shooting star leaves a long, lingering trail
- **Feeling: peace, wonder, home**

### Act 2: The Calamity (~90 seconds)
- A new "star" appears — brighter than the others, not moving with the shower
- It grows. Slowly at first. The aurora dims.
- The meteor shower intensifies — fragments of something larger
- The bright point becomes a visible disc. A comet tail unfurls behind it.
- The ground trembles subtly. Animals go silent.
- **Cut to orbital view**: Earth from space. The asteroid approaches.
- The scale becomes apparent — it's massive. Extinction-class.
- **Impact.** Blinding flash. Shockwave visible from orbit.
- The atmosphere ignites. A ring of fire expands across the surface.
- **Feeling: horror, helplessness, loss**

### Act 3: Survival (~60 seconds)
- **Time skip**: decades/centuries compressed into moments
- Mars colonies expand — domes, then open sky (terraforming)
- Europa under-ice cities. Titan refineries. Asteroid mining operations.
- The solar system is colonized. Humanity survived — but barely.
- A fleet assembles in orbit around Mars — massive generation ships
- **The First Fleet** — humanity's first interstellar expedition
- Ships light their drives. A comet-tail of exhaust stretches behind them.
- They're heading for Alpha Centauri — the nearest star.
- Camera follows the fleet as Earth (scarred, but still there) shrinks to a point of light
- **Feeling: determination, hope, scale**

### Act 4: The Awakening (~30 seconds)
- **Sudden cut to black**
- Sound of breathing. A heartbeat. A low hum — the ship's reactor.
- Eyes open — player is in their quarters aboard **Humanity 1**, the mothership
- Through the viewport: stars. The gentle curve of a habitat ring. Distant nebula.
- A chime sounds — a shuttle is approaching
- Player looks out the window: a small craft glides in and docks at their **private landing port**
- The docking clamps engage. Hiss of pressurization.
- The player pulls up their wrist device / holographic Map
- **Earth Status: 🟢 Healthy**
- It was a dream. Earth is fine. They're already among the stars.
- The calamity hasn't happened. Maybe it never will — if they build well enough.
- **Feeling: relief, awe, purpose**

### Act 5: The Game Begins
- The quarters door opens — the ship's corridor stretches ahead
- Through windows: the interior of Humanity 1 — a multi-kilometer vessel with residential towers, greenspace, rail cars, market districts
- A gentle prompt from the ship's AI: *"Good morning. The fleet awaits your direction. What would you like to build today?"*
- The full game interface reveals itself
- The player steps out into their ship. The real work begins.

### Setting: Humanity 1
- A generation ship — multi-kilometer scale
- Residential towers with private quarters (player's home)
- Private landing ports for personal shuttles
- Rail car transport system between sections
- Market districts with kiosks (links to the Marketplace)
- Greenspace and agriculture sections (links to Garden Tracker)
- Command deck with the Map (Astral Projection)
- Engineering sections (crafting, Skill DNA)
- Observatory dome (Sky View / Stargazer)
- The fleet consists of multiple ships — other players' servers are other ships

## Technical Notes

### Rendering
- Acts 1-3 are pre-rendered cinematics OR real-time using the game engine
- If real-time: leverages the existing sky renderer (stars, constellations, Milky Way)
- Meteor shower: particle system (streaks with trails)
- Aurora: animated wave shader (or canvas gradient animation for web version)
- Impact: screen flash → fade to white → orbital view

### Web Version (MVP)
For the current web client, this could be:
- A full-screen canvas cinematic using existing sky rendering code
- Simplified but still impactful
- Pre-rendered video alternative for low-end devices
- Skip button (but hidden for 5 seconds — let it breathe)

### Audio
- Act 1: Ambient nature sounds, gentle music
- Act 2: Music builds tension, rumbling bass, silence before impact, then BOOM
- Act 3: Epic orchestral swell, hopeful theme
- Act 4: Sudden silence → birds → gentle morning ambience
- Original soundtrack ideal; licensed music as placeholder

### Player Home
- Default: Mount Rainier, Paradise area
- Uses real terrain data (USGS 3DEP 1m LiDAR)
- Real weather from Open-Meteo
- Real star positions from HYG database
- The house is procedurally generated based on player preferences (customizable later)

### The Subtext
The intro is Project Universe's mission statement:
1. **Earth is beautiful** — worth protecting
2. **Catastrophe is possible** — not guaranteed, but possible
3. **Humanity can survive** — if we prepare, collaborate, build
4. **It's not too late** — Earth is healthy. We have time.
5. **What you build matters** — the game is the training ground

The dream shows the worst case. The awakening shows reality. The gap between them is what the player fills.

## Variations
- **Returning players**: Skip the full intro. Instead, brief "Previously..." or just wake up at home.
- **Custom home**: After first play, intro uses the player's actual chosen home location.
- **Seasonal**: Intro sky matches real current season/weather at the player's location.
- **Multiplayer**: During the fleet sequence, other players' ships are visible (their server's fleet).

## Easter Eggs
- The constellations visible during Act 1 match the real sky for the player's time zone
- If you look carefully during Act 3, one of the generation ships is named after the player's home server
- The aurora colors match the player's chosen accent color
- The dream asteroid's trajectory matches a real NEO (Near-Earth Object) from NASA's database
