//! Environment regions: positioned, sized environmental effects the whole
//! renderer and simulation can agree on.
//!
//! WHY THIS EXISTS (operator, 2026-09-21): "Can we modularize/layer the params
//! thing so we don't max it out? What else should have latitude, longitude, and
//! size?" BUG-080 needed ONE new per-frame scalar for the cloud deck and there
//! was nowhere to put it: `material.params2` is full, and every camera light pad
//! is claimed (much of it through `.xyz` swizzles a grep for `.x` never sees).
//!
//! Packing one more scalar into a leftover vector lane would have bought a
//! single feature and made the next one harder. Instead this mirrors what
//! v0.782 already did for scene lights: an UNCAPPED storage buffer of fixed-size
//! records at its own binding. Adding a new kind of environmental effect now
//! costs a row in a data file, not a scavenger hunt for a spare float.
//!
//! THE RULE THIS ENFORCES, and the reason BUG-080 happened: the weight of an
//! environmental effect at a point is a function of THAT POINT, never of the
//! camera. A region sits on the world. Nothing here may read camera altitude,
//! camera distance, or how much of the planet is on screen.
//!
//! Full design, including the inventory of what else should be regional:
//! `docs/design/environment-fields.md`.

/// Bytes per record. Three `vec4<f32>`, matching the WGSL `EnvRegion` struct.
/// Mirrors the 64-byte `GpuLight` convention: fixed size, no padding surprises.
pub const ENV_REGION_BYTES: usize = 48;

/// Floats per record, for the packed upload slice.
pub const ENV_REGION_FLOATS: usize = ENV_REGION_BYTES / 4;

/// One positioned environmental effect.
///
/// `dir` is a UNIT direction from the body centre to the region's ground point,
/// in the body's own frame, so the region stays where it is when the camera
/// moves or the planet spins under it. Storing a direction rather than a
/// latitude and longitude pair keeps the shader side free of trigonometry: the
/// falloff is a dot product.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvRegion {
    /// Unit direction from body centre to the affected ground point.
    pub dir: [f32; 3],
    /// Great-circle angular radius, in radians. An Earth-sized body turns a
    /// 600 km storm into roughly 0.094 rad.
    pub angular_radius: f32,
    /// Which kind of effect this is. Kinds are DATA
    /// (`data/environment/region_kinds.ron`), never a Rust enum with a
    /// hardcoded match, per `docs/design/infinite-of-x.md`. 0 means an empty
    /// slot and contributes nothing.
    pub kind: f32,
    /// Strength at the centre, 0..1.
    pub intensity: f32,
    /// Fraction of the radius over which the edge fades, 0..1. At 0 the region
    /// has a hard rim, which no real weather system has; the default in the
    /// data file is deliberately generous.
    pub softness: f32,
    /// Which altitude band the effect occupies, for consumers that care
    /// (ground fog is not a storm top). Interpreted per kind.
    pub band: f32,
    /// Per-kind payload. Cloud storms use `.x` as the coverage floor and `.y`
    /// as the placement weight, which is what the hardcoded condition table in
    /// `frame_shells.rs` used to carry.
    pub params: [f32; 4],
}

impl EnvRegion {
    /// An empty slot. The shader loops over the whole buffer and a zero-kind
    /// row contributes nothing, which is why the upload zero-fills the tail
    /// instead of carrying a separate count uniform. Avoiding that count is the
    /// whole point: a count would have needed a free lane in the camera
    /// uniform, which is the problem this module exists to stop.
    pub const EMPTY: EnvRegion = EnvRegion {
        dir: [0.0, 1.0, 0.0],
        angular_radius: 0.0,
        kind: 0.0,
        intensity: 0.0,
        softness: 0.0,
        band: 0.0,
        params: [0.0; 4],
    };

    /// Pack into the exact float layout the WGSL struct reads.
    pub fn pack(&self) -> [f32; ENV_REGION_FLOATS] {
        [
            self.dir[0], self.dir[1], self.dir[2], self.angular_radius,
            self.kind, self.intensity, self.softness, self.band,
            self.params[0], self.params[1], self.params[2], self.params[3],
        ]
    }

    /// How strongly this region affects the ground point under `sample_dir`.
    ///
    /// This is the CPU half of a twin: `env_region_influence` in
    /// `assets/shaders/pbr/00-bindings-vertex.wgsl` computes the same value the
    /// same way. They must stay in lockstep, because the simulation and the HUD
    /// read this one and the sky reads the other, and the whole point of the
    /// mechanism is that they agree. `env_influence_matches_the_shader_contract`
    /// pins the behaviour both sides owe.
    ///
    /// `sample_dir` is expected to be unit length; a caller that has a position
    /// should normalize it first rather than relying on this to do it, so the
    /// cost is paid once per sample rather than once per region.
    pub fn influence(&self, sample_dir: [f32; 3]) -> f32 {
        if self.kind < 0.5 || self.intensity <= 0.0 || self.angular_radius <= 0.0 {
            return 0.0;
        }
        let d = self.dir[0] * sample_dir[0]
            + self.dir[1] * sample_dir[1]
            + self.dir[2] * sample_dir[2];
        // Guard the domain before acos: a dot product of two unit vectors can
        // land a hair outside [-1, 1] in f32 and NaN the whole term, which would
        // then propagate silently into a coverage value.
        let angle = d.clamp(-1.0, 1.0).acos();
        let soft = self.softness.clamp(0.0, 1.0);
        let inner = self.angular_radius * (1.0 - soft);
        if angle <= inner {
            return self.intensity;
        }
        if angle >= self.angular_radius {
            return 0.0;
        }
        // smoothstep from the rim inward, so the edge has no visible seam. The
        // degenerate inner == radius case is already handled by the two early
        // returns above, so the divisor here cannot be zero.
        let t = (angle - inner) / (self.angular_radius - inner);
        let s = t * t * (3.0 - 2.0 * t);
        self.intensity * (1.0 - s)
    }
}

/// Pack a slice of regions plus a zero-filled tail out to `capacity` rows.
///
/// The tail matters: the shader walks `arrayLength()` over the whole buffer, so
/// stale rows left behind by a previous frame with more regions would keep
/// contributing. Zeroing is cheaper and far harder to get wrong than tracking a
/// count through a uniform.
pub fn pack_all(regions: &[EnvRegion], capacity: usize) -> Vec<[f32; ENV_REGION_FLOATS]> {
    let mut out = Vec::with_capacity(capacity);
    for r in regions.iter().take(capacity) {
        out.push(r.pack());
    }
    while out.len() < capacity {
        out.push(EnvRegion::EMPTY.pack());
    }
    out
}

/// Total influence of one KIND at a sample direction, summed and clamped.
///
/// Summing rather than taking a maximum is deliberate: two overlapping storm
/// systems should be worse than either alone, which is what weather does. The
/// clamp keeps a pile-up from driving a consumer past its own range.
pub fn influence_of_kind(regions: &[EnvRegion], kind: f32, sample_dir: [f32; 3]) -> f32 {
    let mut total = 0.0;
    for r in regions {
        if (r.kind - kind).abs() < 0.5 {
            total += r.influence(sample_dir);
        }
    }
    total.clamp(0.0, 1.0)
}

// ── The kind table, loaded from data ──────────────────────────────────────
//
// This replaces a hardcoded match in src/engine/frame_shells.rs that mapped
// WeatherCondition::Storm straight to the literal pair (0.95, 0.95). A list of
// domain objects in code is what docs/design/infinite-of-x.md forbids, and it
// meant retuning a storm needed a recompile.

/// One row of `data/environment/region_kinds.ron`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegionKind {
    /// Stable string name, used in saves and by the HUD.
    pub id: String,
    /// The float written into `EnvRegion.kind_shape.x`. 0 is reserved for an
    /// empty buffer slot, so real kinds start at 1.
    pub kind: u32,
    /// Typical RADIUS of one system of this kind, in km.
    pub default_radius_km: f32,
    /// Fraction of the radius over which the edge fades.
    pub softness: f32,
    /// 0 = the cloud slab, 1 = ground level, 2 = high.
    pub band: u32,
    /// The per-kind payload, copied straight into `EnvRegion.params`.
    ///
    /// Its MEANING is the kind's business, which is the whole point of a
    /// generic payload: a cloud kind spends it on a coverage floor and a
    /// placement weight, an aurora spends it on the ring it draws and the
    /// altitude band it occupies. Each kind documents its own layout in the
    /// RON beside the numbers, where whoever is tuning them can see it.
    pub params: [f32; 4],
}

/// The whole table.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegionKinds {
    pub kinds: Vec<RegionKind>,
}

impl RegionKinds {
    /// Parse the shipped RON.
    pub fn from_ron(src: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(src)
    }

    /// Byte-oriented parse, in the shape `engine::registries` wants so the
    /// table loads through the SAME disk-first, embedded-fallback path as every
    /// other registry rather than inventing a second way to read data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let src = std::str::from_utf8(bytes)
            .map_err(|e| format!("region_kinds.ron is not utf-8: {e}"))?;
        Self::from_ron(src).map_err(|e| format!("region_kinds.ron: {e}"))
    }

    /// Look a kind up by its stable id.
    pub fn by_id(&self, id: &str) -> Option<&RegionKind> {
        self.kinds.iter().find(|k| k.id == id)
    }

    /// Look a kind up by the float the GPU record carries.
    pub fn by_kind(&self, kind: f32) -> Option<&RegionKind> {
        self.kinds.iter().find(|k| (k.kind as f32 - kind).abs() < 0.5)
    }
}

impl RegionKind {
    /// This kind's default radius as a great-circle ANGLE on a body of the
    /// given radius, which is the unit `EnvRegion` stores. Keeping the data in
    /// km and converting here means the same table works on bodies of
    /// different sizes: a 600 km storm is a much larger share of a small moon.
    pub fn angular_radius(&self, body_radius_km: f32) -> f32 {
        if body_radius_km <= 0.0 {
            return 0.0;
        }
        (self.default_radius_km / body_radius_km).clamp(0.0, std::f32::consts::PI)
    }

    /// Build a region of this kind at a ground direction.
    pub fn region_at(&self, dir: [f32; 3], body_radius_km: f32, intensity: f32) -> EnvRegion {
        EnvRegion {
            dir,
            angular_radius: self.angular_radius(body_radius_km),
            kind: self.kind as f32,
            intensity: intensity.clamp(0.0, 1.0),
            softness: self.softness,
            band: self.band as f32,
            params: self.params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(x: f32, y: f32, z: f32) -> [f32; 3] {
        let l = (x * x + y * y + z * z).sqrt();
        [x / l, y / l, z / l]
    }

    fn storm_at(dir: [f32; 3], radius: f32) -> EnvRegion {
        EnvRegion {
            dir,
            angular_radius: radius,
            kind: 1.0,
            intensity: 1.0,
            softness: 0.4,
            band: 0.0,
            params: [0.95, 0.95, 0.0, 0.0],
        }
    }

    /// The record must stay exactly three vec4s, because the WGSL struct it
    /// feeds is declared that way and a mismatch reads garbage rather than
    /// failing. This is the cheapest possible guard on that contract.
    #[test]
    fn the_packed_record_is_three_vec4s() {
        assert_eq!(ENV_REGION_BYTES, 48);
        assert_eq!(ENV_REGION_FLOATS, 12);
        let packed = storm_at(unit(0.0, 1.0, 0.0), 0.1).pack();
        assert_eq!(packed.len(), ENV_REGION_FLOATS);
        // Field order is the contract with the shader: dir, radius, then kind,
        // intensity, softness, band, then the payload.
        assert_eq!(packed[3], 0.1, "angular radius rides in dir_radius.w");
        assert_eq!(packed[4], 1.0, "kind rides in kind_shape.x");
        assert_eq!(packed[8], 0.95, "payload starts at params.x");
    }

    /// The behaviour the shader twin owes, stated once so both sides can be
    /// checked against it: full strength at the centre, nothing outside the
    /// radius, and no discontinuity in between.
    #[test]
    fn env_influence_matches_the_shader_contract() {
        let centre = unit(0.0, 1.0, 0.0);
        let r = storm_at(centre, 0.2);

        assert_eq!(r.influence(centre), 1.0, "full strength at the centre");

        // A point well outside the radius is untouched. 0.5 rad against a
        // 0.2 rad region is comfortably clear.
        let far = unit(1.0, 0.0, 0.0);
        assert_eq!(r.influence(far), 0.0, "nothing outside the radius");

        // Monotonic as the sample walks outward, and strictly between 0 and 1
        // inside the soft band.
        let mut last = f32::INFINITY;
        for i in 0..=20 {
            let angle = 0.2 * (i as f32 / 20.0);
            let s = unit(angle.sin(), angle.cos(), 0.0);
            let v = r.influence(s);
            assert!(v <= last + 1e-6, "influence must not rise going outward");
            assert!((0.0..=1.0).contains(&v), "influence stays in range, got {v}");
            last = v;
        }
        assert!(last < 1e-6, "and reaches zero by the rim, got {last}");
    }

    /// A zero-kind row is an empty slot. The upload relies on this to avoid
    /// carrying a count through the camera uniform, so it is load-bearing
    /// rather than a nicety.
    #[test]
    fn an_empty_slot_contributes_nothing() {
        let centre = unit(0.0, 1.0, 0.0);
        assert_eq!(EnvRegion::EMPTY.influence(centre), 0.0);
        // And a region with a real kind but no size or strength is also inert,
        // so a half-initialised row cannot tint the world.
        let mut r = storm_at(centre, 0.0);
        assert_eq!(r.influence(centre), 0.0, "zero radius is inert");
        r.angular_radius = 0.2;
        r.intensity = 0.0;
        assert_eq!(r.influence(centre), 0.0, "zero intensity is inert");
    }

    /// The tail must be zeroed, or a frame with fewer regions than the last one
    /// keeps rendering the ones that went away.
    #[test]
    fn packing_zero_fills_the_tail_so_stale_rows_cannot_linger() {
        let regions = vec![storm_at(unit(0.0, 1.0, 0.0), 0.2)];
        let packed = pack_all(&regions, 4);
        assert_eq!(packed.len(), 4);
        assert_eq!(packed[0][4], 1.0, "the real region kept its kind");
        for row in packed.iter().skip(1) {
            assert_eq!(row[4], 0.0, "tail rows must be empty slots");
            assert_eq!(row[5], 0.0, "tail rows must carry no intensity");
        }
        // More regions than capacity must not overflow the buffer.
        let many = vec![storm_at(unit(0.0, 1.0, 0.0), 0.2); 9];
        assert_eq!(pack_all(&many, 4).len(), 4, "packing is bounded by capacity");
    }

    /// The shipped kind table parses, and the invariants the rest of the
    /// system assumes about it actually hold. Named to match the `from_ron`
    /// filter `just validate-data` runs, so editing the RON is covered.
    #[test]
    fn region_kinds_parse_from_ron_and_hold_their_invariants() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/environment/region_kinds.ron"
        ))
        .expect("the kind table ships with the repo");
        let table = RegionKinds::from_ron(&src).expect("kind table must parse");
        assert!(!table.kinds.is_empty(), "an empty table would silence weather");

        let mut seen_ids = std::collections::HashSet::new();
        let mut seen_kinds = std::collections::HashSet::new();
        for k in &table.kinds {
            // 0 is the empty-slot sentinel the packed tail relies on. A kind
            // numbered 0 would be invisible AND would make every unused row
            // look like that kind.
            assert!(k.kind >= 1, "{} uses the reserved empty-slot id 0", k.id);
            assert!(seen_ids.insert(k.id.clone()), "duplicate id {}", k.id);
            assert!(seen_kinds.insert(k.kind), "duplicate kind number on {}", k.id);
            assert!(k.default_radius_km > 0.0, "{} has no size", k.id);
            assert!(
                (0.0..=1.0).contains(&k.softness),
                "{} softness is a fraction of the radius", k.id
            );
            assert!(
                k.softness > 0.0,
                "{} has a hard rim; no real weather system does", k.id
            );
            // The payload is per-kind, so the only universal claim is that it
            // carries no NaN: one would propagate into a coverage value or a
            // ring angle and be very hard to trace back to a data file.
            for (i, v) in k.params.iter().enumerate() {
                assert!(v.is_finite(), "{} params[{i}] is not finite", k.id);
            }
        }

        // The storm row is the one BUG-080 is about, and its numbers are the
        // ones the hardcoded match in frame_shells.rs used to carry.
        let storm = table.by_id("storm").expect("a storm kind must exist");
        assert_eq!(storm.params[0], 0.95, "storm coverage floor");
        assert_eq!(storm.params[1], 0.95, "storm placement weight");
        assert!(table.by_kind(storm.kind as f32).is_some(), "lookup both ways");

        // A 600 km storm on Earth is a patch, not a hemisphere. This is the
        // number that stops a local condition painting the whole marble.
        let earth_km = 6371.0;
        let ang = storm.angular_radius(earth_km);
        assert!(
            ang > 0.05 && ang < 0.2,
            "an Earth storm should subtend a fraction of a radian, got {ang}"
        );
        // The same table on a small moon gives a proportionally larger system,
        // which is why the data is in km and the conversion lives in code.
        assert!(storm.angular_radius(600.0) > ang * 5.0);

        // And a built region behaves: full strength at its centre, nothing on
        // the far side of the planet.
        let centre = unit(0.0, 1.0, 0.0);
        let r = storm.region_at(centre, earth_km, 1.0);
        assert_eq!(r.influence(centre), 1.0);
        assert_eq!(r.influence(unit(0.0, -1.0, 0.0)), 0.0, "not the antipode");
    }

    /// The Rust record and the WGSL struct are one contract in two files, and
    /// a mismatch does not fail loudly: the shader just reads the wrong floats
    /// out of the buffer and renders something plausible. So pin the shape
    /// against the shipped shader text.
    #[test]
    fn the_wgsl_struct_matches_the_rust_record() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/shaders/pbr/00-bindings-vertex.wgsl"
        ))
        .expect("the bindings shader ships with the repo");

        let at = src
            .find("struct EnvRegion {")
            .expect("EnvRegion must be declared in the shader");
        let end = src[at..].find("};").expect("struct must close") + at;
        let body = &src[at..end];

        // Three vec4<f32> fields, in the order pack() writes them.
        let fields: Vec<&str> = body
            .lines()
            .filter(|l| l.contains("vec4<f32>,"))
            .collect();
        assert_eq!(
            fields.len() * 16,
            ENV_REGION_BYTES,
            "shader struct is {} vec4s, Rust record is {ENV_REGION_BYTES} bytes",
            fields.len()
        );
        assert!(fields[0].contains("dir_radius"), "field 0 is dir_radius");
        assert!(fields[1].contains("kind_shape"), "field 1 is kind_shape");
        assert!(fields[2].contains("params"), "field 2 is params");

        // And the binding the layout promises. If this number moves, the three
        // create_bind_group sites in renderer/mod.rs move with it or world
        // entry fails wgpu validation (the v0.1029 lesson).
        assert!(
            src.contains("@group(0) @binding(4) var<storage, read> env_regions: array<EnvRegion>;"),
            "env_regions must be bound at group 0 binding 4"
        );
    }

    /// Overlapping systems compound rather than one winning, because that is
    /// what weather does, and the result stays inside the consumer's range.
    #[test]
    fn overlapping_regions_of_a_kind_compound_and_stay_clamped() {
        let centre = unit(0.0, 1.0, 0.0);
        let mut a = storm_at(centre, 0.3);
        let mut b = storm_at(centre, 0.3);
        a.intensity = 0.4;
        b.intensity = 0.4;
        let both = influence_of_kind(&[a, b], 1.0, centre);
        assert!((both - 0.8).abs() < 1e-6, "two halves compound, got {both}");

        a.intensity = 0.9;
        b.intensity = 0.9;
        assert_eq!(
            influence_of_kind(&[a, b], 1.0, centre),
            1.0,
            "a pile-up clamps rather than running past the consumer's range"
        );

        // A different kind at the same place is invisible to this query, which
        // is what keeps fog out of the storm term.
        let mut fog = storm_at(centre, 0.3);
        fog.kind = 2.0;
        assert_eq!(influence_of_kind(&[fog], 1.0, centre), 0.0);
    }
}
