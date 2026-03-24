# Constructible Objects Schema

This reference defines every field used by constructible objects in `data/construction/objects.ron` and explains how the data is consumed by `source/construction/objects.rs`. Use it when authoring or reviewing new construction content so objects remain future-proof, hot-reload friendly, and compatible with upcoming systems (utilities, atmosphere, build phases).

## Data Layout
- Objects are grouped by category inside an `objects` map. Each entry is a tuple-style record containing the fields documented below.
- Layering behaviour lives in the `layering_rules` map and must stay in sync with every `layer` identifier referenced by objects.
- The object database hot-reloads the file; edits take effect in game once `ObjectDatabase::check_for_updates` runs (triggered when the construction system ticks).

## Required Core Fields
- **id** (`String`) – Unique, lowercase identifier used for lookups, persistence, and blueprints. Must remain stable across patches.
- **name** (`String`) – Player-facing name displayed in the HUD and tooltips. Localize via string tables later.
- **category** (`String`) – Matches `ObjectCategory` (`structural`, `furniture`, `lighting`, `utilities`, `containers`, `appliances`, `safety`). Controls menu grouping and default layering behaviour.
- **size** (`(f32, f32, f32)`) – Width (X), depth (Y), height (Z) in meters. Snapping, collision, and placement volumes derive from this value.
- **mass** (`f32`) – Kilograms used by physics, structural checks, and logistics.
- **health** (`f32`) – Structural durability points; consumed by damage, repairs, and build phases.
- **construction_time** (`f32`) – Seconds required to assemble during queued build phase simulations.
- **material_requirements** (`Vec<MaterialRequirement>`) – Crafting bill: each entry contains `material` id and `quantity` (SI units: m³, m², or count depending on material type).
- **research_required** (`String`) – Technology gate id; empty string if available from start.
- **description** (`String`) – HUD tooltip body; keep concise but informative.
- **model** (`String`) – GLB filename located in `models/`. Must be hashed by `build.rs` to participate in asset hot-reload.
- **texture** (`String`) – Material/texture key resolved by the renderer and theme system.
- **shader** (`String`) – Shader pipeline identifier (`pbr`, `ghost_preview`, etc.). Use existing shader names from `shaders/`.
- **collision** (`bool`) – Enables physics collision component creation.
- **passable** (`bool`) – Whether characters can traverse the volume when collision is true (e.g., doors vs. walls).
- **layering_allowed** (`bool`) – If false, object exclusively occupies its volume; if true, layering rules decide coexistence.
- **layer** (`Option<String>`) – Logical layer id. Defaults to the category when omitted; supply explicit values for nuanced layering (e.g., `structural_floor`).
- **functionality** (`Vec<String>`) – Capability tags consumed by gameplay systems (`structural_support`, `walkable_surface`, `hvac_intake`). Keep consistent naming for future queries.
- **power_consumption** (`Option<f32>`) – Watts drawn during normal operation. Defaults to `0.0` when omitted.
- **maintenance_cost** (`Option<f32>`) – Credits per in-game day required for upkeep. Defaults to `0.0`.

## Optional & Conditional Fields

### Lighting
- **light_output** (`Option<LightOutput>`) – Attach to lighting or emissive objects.
  - **intensity** (`f32`) – Lumens.
  - **color_temperature** (`f32`) – Kelvin for white balance.
  - **color** (`Vec3`) – RGB values (0.0–1.0) for tinted lights.
  - **range** (`f32`) – Effective radius in meters.

### Power & Energy Systems
- **power_capacity** (`Option<f32>`) – Watt-hours stored (batteries, capacitors).
- **power_consumption_emergency** (`Option<f32>`) – Watts used in emergency/backup mode.
- **power_generation** (`Option<f32>`) – Continuous watt output for generators or solar panels.

### Fluid & Atmosphere
- **water_consumption** (`Option<f32>`) – Litres per minute consumed from plumbing.
- **water_storage** (`Option<f32>`) – Litres stored internally (cisterns, tanks).
- **airflow_capacity** (`Option<f32>`) – Cubic meters per hour moved through HVAC components.
- **pressure_rating** (`Option<f32>`) – Kilopascals the object can withstand; used for atmospheric sealing.
- **suppressant_capacity** (`Option<f32>`) – Fire suppressant volume/charge for safety devices.

### Storage & Logistics
- **storage_capacity** (`Option<f32>`) – Cubic meters or standardized container slots depending on item class; document any conversion factor in comments.
- **storage_category** (`Option<String>`) – Type of inventory accepted (e.g., `dry_goods`, `tools`).

### Data & Control
- **data_processing** (`Option<f32>`) – Computational throughput in FLOPS for networked systems.
- **data_bandwidth** (`Option<f32>`) – Mbps handled by terminals or routers.

### Thermal & Environmental
- **thermal_output** (`Option<f32>`) – Watts of heat produced; drives HVAC load calculations.
- **insulation_rating** (`Option<f32>`) – R-value for walls, floors, or panels.
- **noise_level** (`Option<f32>`) – dB at one meter for comfort modelling.

### Gameplay & Progression Hooks
- **build_phase** (`Option<String>`) – Overrides automatic phase assignment (`preview`, `queued`, `structural`, `finish`).
- **snap_priority** (`Option<String>`) – Adjusts snapping heuristics (`top_face`, `edge_center`, `corner`).
- **atmosphere_role** (`Option<String>`) – Tag for sealing graph (`seal`, `vent`, `door`).
- **utility_ports** (`Option<Vec<String>>`) – Declares compatible connection port ids (power_in, water_out, data_bus, hvac_duct).
- **custom_properties** (`Option<HashMap<String, ron::Value>>`) – Catch-all for prototype features; prefer structured fields once stabilized.

## Validation & Hot Reload Notes
- `ObjectDatabase::convert_ron_object` validates category strings and applies defaults; invalid categories return descriptive errors.
- Optional numeric fields default to `0.0` when omitted. Prefer explicit `Some(...)` when the value conveys gameplay meaning so future audits can differentiate “unused” vs “zero”.
- Keep asset filenames synchronized with the `models/` directory; missing files trigger warnings during renderer initialization.
- After editing `objects.ron`, run `cargo run` to let the construction system reload the file and surface schema issues in the terminal logs.

## Example Entry (Fully Populated)

```ron
(
    id: "floor_sheet_3x3",
    name: "Floor Sheet 3x3",
    category: "structural",
    size: (3.0, 3.0, 0.1),
    mass: 180.0,
    health: 500.0,
    construction_time: 120.0,
    material_requirements: [
        (material: "steel_beam", quantity: 0.30),
        (material: "composite_panel", quantity: 9.00),
    ],
    research_required: "construction_basics",
    description: "Reinforced floor panel with integrated utility channels.",
    model: "sheet_3x3x0.1m.glb",
    texture: "polished_concrete",
    shader: "pbr",
    collision: true,
    passable: false,
    layering_allowed: true,
    layer: Some("structural_floor"),
    functionality: ["structural_support", "walkable_surface"],
    power_consumption: Some(0.0),
    maintenance_cost: Some(0.02),
    light_output: None,
    power_capacity: None,
    power_consumption_emergency: None,
    power_generation: None,
    water_consumption: None,
    water_storage: None,
    airflow_capacity: Some(0.0),
    pressure_rating: Some(101.3),
    suppressant_capacity: None,
    storage_capacity: None,
    storage_category: None,
    data_processing: None,
    data_bandwidth: None,
    thermal_output: Some(0.0),
    insulation_rating: Some(2.5),
    noise_level: Some(0.0),
    build_phase: Some("structural"),
    snap_priority: Some("top_face"),
    atmosphere_role: Some("seal"),
    utility_ports: Some(["power_passthrough", "data_bus", "hvac_channel"]),
    custom_properties: Some({
        "notes": "Supports edge-to-edge snapping; culled top/bottom faces when stacked.",
        "ui_icon": "floor_sheet_3x3",
    }),
)
```

> When an optional field is unnecessary, prefer `None` rather than omitting it so future tooling can distinguish intentional zeros from defaults.

