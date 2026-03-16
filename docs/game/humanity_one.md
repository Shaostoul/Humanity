# Humanity 1 — The Mothership

## Scale
- **Population capacity**: 10 billion (entire humanity if needed)
- **Length**: ~500 km (comparable to a small moon)
- **Diameter**: ~100 km at widest habitat section
- **Structure**: Cylindrical core with rotating habitat rings

## Timeline
- Humanity begins building orbital infrastructure: ~2050s
- First space elevator: ~2060s
- Lunar and asteroid mining at scale: ~2070s
- Construction of Humanity 1 begins: ~2080s (50+ year project)
- Fleet departure: ~2140s
- The game takes place well after departure — the ship is established, lived-in, home

## Ship Sections (Hub Tab Mapping)

| Ship Section | Hub Tab | Purpose |
|---|---|---|
| Bridge / Command | Map | Navigation, Astral Projection |
| Comms Array | Chat | Communication between ships/stations |
| Engineering Deck | Board | Task management, projects |
| Personal Quarters | Reality | Notes, skills, garden (hydro bay), inventory |
| Holodeck / Sim Bay | Fantasy | Game worlds, training simulations |
| Market District | Market | Commerce, kiosks, marketplace |
| Broadcast Center | Streams | Streaming, entertainment |
| Data Terminal | Browse | Web directory, information access |
| Observatory Dome | Sky View | Stargazing, constellation identification |
| Science Wing | Dashboard | Widgets, monitoring, data |
| Archives | Lore | Ship history, mission logs, universe lore |
| Library | Info | Help, guides, stats |
| Ship Yard | Source | Open source, contributing |
| Download Bay | Download | Get the app |

## Interior Navigation

### Challenge
A 500km ship with billions of residents = impossible to render entirely. Need LOD (Level of Detail) and spatial partitioning.

### Solution: Deck/Section/Block/Room Addressing
Similar to icosphere but for interior spaces:
```
Ship > Ring > Sector > Deck > Block > Room
Humanity1 > Ring3 > Sector7 > Deck42 > Block15 > Room2847
```

### Navigation Methods
1. **Rail car system** — fast travel between major sections (like a subway map)
2. **Elevator shafts** — move between decks vertically
3. **Walking** — local navigation within blocks
4. **Teleport (dev tools)** — instant travel for testing/admin
5. **2D map overlay** — top-down deck view with room labels, searchable

### Performance Strategy
- Only render the current section + adjacent sections
- Far sections shown as simplified geometry / flat textures
- Windows show pre-rendered or skybox-based exterior views
- Interior uses modular tileset (corridors, rooms, commons reused)
- Ship exterior visible through windows = skybox with accurate star positions

## Windows
- Windows throughout the ship show real exterior views
- Habitat ring windows: rotating star field (ring rotates for gravity)
- Command deck: forward-facing view toward destination star
- Observatory: full dome transparent view
- Implementation: skybox cube map updated based on ship heading and star catalog
- Sun/nearby stars cast dynamic light through windows

## Exterior
- Ship exterior visible from shuttles, spacewalks, other ships
- Modular hull design: habitat rings, radiator fins, drive section, docking bays
- Other fleet ships visible in formation at realistic distances
- Each federated server's ship has a unique but procedurally consistent design

## Districts (within the ship)
- **Residential towers** — apartments, houses, penthouses (player homes)
- **Agricultural rings** — massive hydroponic/aeroponic farms (garden tracker)
- **Industrial sector** — manufacturing, 3D printing, repair (Skill DNA)
- **Commercial district** — shops, kiosks, marketplace stalls
- **Recreation** — parks, theaters, sports, holodeck access
- **Medical** — hospitals, research labs
- **Education** — schools, libraries, training facilities
- **Docking bay** — shuttle arrivals/departures, cargo loading

## Population Density Math
- 500 km × 100 km diameter = enormous habitable volume
- If habitat rings are 50 km diameter, circumference ~157 km
- Multiple rings stacked = thousands of km² of living space
- 10 billion at suburban density (~2,000/km²) = 5 million km² needed
- This is achievable with multiple habitat rings and multi-level decks

## Lore Tab Concept
A dedicated "Lore" or "Archives" tab in the hub:
- **Ship's Log** — timeline of key events (construction, launch, milestones)
- **Mission Brief** — why humanity left, where we're going
- **Universe Encyclopedia** — star systems, species (if any), physics
- **Personal Journal** — player's own story entries
- **Crew Manifest** — all registered users on this ship/server
- **Dream Archive** — collected "nightmares" (the intro) with community interpretations
- Could replace or supplement the existing Info tab
