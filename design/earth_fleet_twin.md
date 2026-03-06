# Earth + Fleet Twin Architecture (Local-First)

## Goal
Unify two worlds under one system:
- **Fleet Twin**: in-universe megaship / colonization gameplay.
- **Earth Twin**: real-world planning, utilities, and production context.

Core principle: same interaction language and module taxonomy, different datasets and fidelity.

---

## Twin Modes

## 1) Fleet Twin
- Fictional but systems-grounded.
- Modules: habitat, utilities, logistics, public spaces, industrial zones.
- Event model: outages, damage, cooperative restoration.

## 2) Earth Twin
- Real-world anchored planning and learning.
- Starts at personal/property scale, then scales up to region/global abstractions.
- Supports practical use cases (garden planning, structure layout, utility awareness).

---

## Local-First Fidelity Tiers

## Tier A — Personal Twin (MVP)
**Runs fully on personal PC.**
- House + lot geometry
- Zones (home, garden, structures)
- Utility overlays (power, water, network)
- Seasonal/sun/shadow planning
- Crop and structure planner

**Storage target:** 100 MB - 2 GB per user project (depending on media/assets).

## Tier B — Regional Twin (Optional packs)
- City/county context tiles (terrain/roads/base map)
- Optional utility corridor overlays where data exists
- Medium-detail infrastructure layers

**Storage target:** 2 GB - 40 GB depending on selected area and LOD.

## Tier C — Global Twin (Abstract)
- Country/region/global production and utility indicators
- Mining/refining/energy/transport flows at aggregated resolution
- No full-fidelity geometry required

**Storage target:** mostly tabular/time-series datasets (MB to low-GB range).

---

## Data Strategy

## Personal data
- User-authored local files (private by default)
- Export/import presets for sharing

## Public geospatial data (where licensing permits)
- Base map / roads / footprints / terrain (regional packs)
- Cache in chunked tiles with versioned manifests

## Macro datasets
- Curated indicators for energy, extraction, refining, transport, etc.
- Stored as compressed timeseries + regional keys

---

## Compute Strategy

- Keep heavy simulation scoped to loaded area only.
- Use LOD for geometry and system detail.
- Use async chunk loading and simulation throttling.
- Mobile companion reads summarized state, not full simulation.

---

## Earth <-> Fleet Parity Model

Shared concepts across both twins:
- utilities (power/water/network)
- logistics paths
- module dependencies
- outage/repair event loops
- access gating based on infrastructure state

This allows a player to learn systems in Earth mode and transfer mastery to Fleet mode (and vice versa).

---

## Privacy + Sharing

- Default: Earth personal twin remains local/private.
- Optional publish layers:
  - snapshots,
  - anonymized metrics,
  - collaborative planning sessions.
- Sensitive geometry and personal metadata should be selectively shareable.

---

## MVP Scope (Build Order)

1. **Earth Personal Planner (Tier A)**
   - lot layout editor
   - garden planner
   - utility overlays
   - scenario compare (realistic vs fantasy override)

2. **Twin Switcher UX**
   - Fleet / Earth mode toggle
   - shared module naming and dependency visuals

3. **Regional Overlay (Tier B-lite)**
   - optional base map and roads around user parcel

4. **Macro Dashboard (Tier C-lite)**
   - global indicators with simple flow maps

---

## Feasibility Notes

- Full real-Earth high-fidelity simulation is not practical as one monolithic local model.
- A **tiered local-first + abstracted macro** approach is practical and scalable.
- Personal-scale realism + educational systems can ship early and provide immediate value.

---

## Open Questions

1. Preferred baseline map source(s) and licensing constraints?
2. How much geospatial precision is needed for home planning MVP?
3. Which utility datasets are priority for first Earth dashboard?
4. Should Earth fantasy overrides be separate layer or editable branch timeline?
5. What is the first target hardware profile (RAM/GPU/storage budget)?
