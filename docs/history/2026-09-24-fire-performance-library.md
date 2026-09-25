# 2026-09-24: fire performance research, the Library guides, and the fire-props design

A content-and-design session run beside other sessions in the same checkout. No `src/` changes.

## What started it

The operator opened a Facebook group, **United Humanity Fire Disciples**
(https://www.facebook.com/groups/1535831991917653), for flow arts, prop building
and crafts, partly for family and friends who spin fire and LED staffs and
partly for HumanityOS. The session made its cover art, then a run of safety
posts for the group, then moved the posts into the Library, then designed the
in-game version of the same objects.

## What was made

- **Cover art.** Six procedural long-exposure concepts (canvas pages rendered to
  1640x856 PNGs with headless Edge), judged by three lenses, top ones refined and
  checked for Facebook's crop zones. The operator picked concept C (the
  fire-to-LED helix over a planet horizon), which the judges had ranked last;
  it was refined without changing its look.
- **Facebook posts**, each researched, drafted, adversarially fact-checked and
  cold-read: clothing for fire performance (natural fibres, the plastic-fabric
  names, toxic smoke), a three-part contact staff series (tube materials and
  walls, length and sizing, wicks grips and hardware) and a dedicated fuels
  post. The cold reads mattered: they caught a dangerous duvetyne instruction
  (wetting it strips its water-soluble flame-retardant finish) after two full
  fact-check rounds had missed it.
- **Library**, merged at the Library's sourcing standard rather than pasted:
  `fire_staff_materials.md` updated, new `staff_tubes.md`,
  `fire_performance_fuels.md` and `fire_performance_clothing.md`, and
  corrections to `metals_and_alloys.md`. The raw research is kept in
  `docs/reference/research/2026-09-24-fire-performance/`.
- **Design**: `docs/design/fire-props-and-combustion.md`, and arc G in
  PRIORITIES.

## Things worth knowing next time

- **A merge beats a second guide.** The Library already had a sourced fire
  staff guide. Merging exposed real disagreements that a duplicate would have
  hidden: the old guide called titanium staffs "largely a myth" (they are sold),
  grouped lamp oil with kerosene (it is a different product), and gave burn
  times that contradicted each other.
- **Registries drift toward "everything cited".** The integrator registered 144
  sources, about 60 of them retailers, and put all 162 ids on the lethal topic's
  source list. Retailer entries are allowed in the registry (labelled "Not an
  authority; community figure only"), but the topic's `sources` field means
  authorities, so it was filtered to 108.
- **Check provenance claims at the record.** Two NASA materials handbooks sat
  under "US government (public domain)". The NTRS record says they are
  contractor reports (Western Applied Research and Development, contract
  NAS8-26644), cleared for public use but not government works. A guess from
  memory about who wrote them was also wrong until the record was read.
- **EmberGen-like effects were never built.** The operator remembered them. Six
  scouts found particle recipes for fire, smoke and explosions that no code
  spawns, and the cloud ray-march as the only volumetric technique that shipped.
- **The operator's answers became standing rules** (CLAUDE.md, "Dual modes,
  open source, and a compute budget"): full-realism and simplified modes
  everywhere, open source only with our own solvers, and cosmetic simulation
  never networked.

## Left for later

- The four fire guides stay at `sourced` until the operator reads them.
- Rung F1 (research into data, after resolving 54 data/chemistry conflicts),
  then the character body arc.
- A 240 fps calibration film of a lit staff, when someone in the group has the
  camera.
- The journal is over 150 KB and due a `just rotate-journal`, skipped here
  because other sessions were writing it concurrently.
