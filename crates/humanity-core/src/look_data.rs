//! THE LOOK'S DATA, read (character animation, "the look", 2026-11-09;
//! docs/design/character-animation.md section 16).
//!
//! What `scripts/gen-makehuman-body.js` and `scripts/gen-makehuman-look.js` write beside the bodies,
//! read from bytes into plain data (the files are opened by the engine; the animation crate builds
//! each person's own mesh from this, like `skin_data`). WHY IT LIVES IN THE CORE: the same reason as
//! `skin_data`: the engine fills it and the animation crate, which reads no file format, uses it.
//!
//! - `BaseShared` (`makehuman_base.bin`): the raw MakeHuman base mesh, each kept joint's vertex list
//!   (a joint is the mean of its vertices), and each base vertex's cover region.
//! - `BodyBase` (`makehuman_<gender>.base.bin`): every base vertex (body and helpers) at that body's
//!   own shape, each of its 15 shape targets over all of them, and the base vertices each glTF
//!   vertex follows.
//! - `DetailTargets` (`makehuman_details.bin`): the face and body detail targets, sparse.
//! - `GarmentData` (`garments/<id>/<id>.bin`): a garment fitted to the base mesh.
//!
//! The layouts are written out in the converters (`writeBase`, `writeShared`, `writeDetails`,
//! `writeGarment`). Every reader is bounds-checked and returns an error, never panics: a mod's file
//! is untrusted input.

/// A little-endian reader over a byte slice that says what was being read when it runs out.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    what: &'static str,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], what: &'static str, tag: &[u8; 8]) -> Result<Self, String> {
        if bytes.len() < 8 || &bytes[..8] != tag {
            return Err(format!("{what}: not a {} file", String::from_utf8_lossy(tag)));
        }
        Ok(Reader { bytes, at: 8, what })
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or_else(|| format!("{}: cut short at byte {}", self.what, self.at))?;
        let s = &self.bytes[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self) -> Result<f32, String> {
        let v = f32::from_bits(self.u32()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(format!("{}: a number that is not finite at byte {}", self.what, self.at - 4))
        }
    }

    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }

    fn vec3(&mut self) -> Result<[f32; 3], String> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    /// A count that the rest of the file could hold at `each` bytes apiece (so a corrupt count
    /// never asks for gigabytes).
    fn count(&mut self, each: usize) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n.saturating_mul(each.max(1)) > self.bytes.len() - self.at {
            return Err(format!("{}: a count of {n} at byte {} is more than the file holds", self.what, self.at - 4));
        }
        Ok(n)
    }

    fn name(&mut self) -> Result<String, String> {
        let n = self.count(1)?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| format!("{}: a name that is not UTF-8", self.what))
    }

    fn done(&self) -> Result<(), String> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(format!("{}: {} bytes left over", self.what, self.bytes.len() - self.at))
        }
    }
}

/// `makehuman_base.bin`: what both MakeHuman bodies share.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BaseShared {
    /// The raw base mesh, metres, feet on the ground: the body MakeHuman's garments were fitted on.
    pub raw: Vec<[f32; 3]>,
    /// Each kept joint's base vertices, in the glTF skin's joint order: the joint is their mean.
    pub joint_verts: Vec<Vec<u32>>,
    /// The cover regions a garment's `covers` names.
    pub region_names: Vec<String>,
    /// Each base vertex's region (an index into `region_names`; 255 for a helper).
    pub regions: Vec<u8>,
}

impl BaseShared {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(bytes, "makehuman_base.bin", b"HOSMHS01")?;
        let nb = r.count(12)?;
        let raw = (0..nb).map(|_| r.vec3()).collect::<Result<Vec<_>, _>>()?;
        let nj = r.count(4)?;
        let mut joint_verts = Vec::with_capacity(nj);
        for _ in 0..nj {
            let n = r.count(4)?;
            let list = (0..n).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
            if list.is_empty() || list.iter().any(|&v| v as usize >= nb) {
                return Err("makehuman_base.bin: a joint's vertex list is empty or names a vertex past the mesh".into());
            }
            joint_verts.push(list);
        }
        let nr = r.count(4)?;
        let region_names = (0..nr).map(|_| r.name()).collect::<Result<Vec<_>, _>>()?;
        let regions = r.take(nb)?.to_vec();
        r.done()?;
        Ok(BaseShared { raw, joint_verts, region_names, regions })
    }

    /// A region's index by name.
    pub fn region(&self, name: &str) -> Option<u8> {
        self.region_names.iter().position(|n| n == name).map(|i| i as u8)
    }
}

/// `makehuman_<gender>.base.bin`: one body's base mesh, shape targets and glTF vertex map.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BodyBase {
    /// Every base vertex at the body's own shape, metres (unsmoothed).
    pub positions: Vec<[f32; 3]>,
    /// Each shape target's movement (base vertex, delta), in the glTF's target order.
    pub targets: Vec<Vec<(u32, [f32; 3])>>,
    /// For each glTF vertex (primitive after primitive), the base vertices it follows and how much.
    pub follow: Vec<[(u32, f32); 3]>,
}

impl BodyBase {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(bytes, "a body's base file", b"HOSMHB01")?;
        let nb = r.u32()? as usize;
        let nt = r.u32()? as usize;
        let ng = r.u32()? as usize;
        if nb.saturating_mul(12) > bytes.len() || ng.saturating_mul(24) > bytes.len() || nt > 1024 {
            return Err("a body's base file: its counts are more than it holds".into());
        }
        let positions = (0..nb).map(|_| r.vec3()).collect::<Result<Vec<_>, _>>()?;
        let mut targets = Vec::with_capacity(nt);
        for _ in 0..nt {
            let n = r.count(16)?;
            let mut list = Vec::with_capacity(n);
            for _ in 0..n {
                let v = r.u32()?;
                let d = r.vec3()?;
                if v as usize >= nb {
                    return Err("a body's base file: a target moves a vertex past the mesh".into());
                }
                list.push((v, d));
            }
            targets.push(list);
        }
        let mut follow = Vec::with_capacity(ng);
        for _ in 0..ng {
            let mut f = [(0u32, 0.0f32); 3];
            for slot in &mut f {
                let v = r.u32()?;
                let w = r.f32()?;
                if v as usize >= nb {
                    return Err("a body's base file: a glTF vertex follows a vertex past the mesh".into());
                }
                *slot = (v, w);
            }
            follow.push(f);
        }
        r.done()?;
        Ok(BodyBase { positions, targets, follow })
    }

    /// The base vertex a glTF vertex IS (it follows that one vertex alone), or None for a generated
    /// vertex (brows, eyes) that follows a blend.
    pub fn sole(&self, gltf_vertex: usize) -> Option<u32> {
        let f = self.follow.get(gltf_vertex)?;
        (f[0].1 == 1.0 && f[1].1 == 0.0 && f[2].1 == 0.0).then_some(f[0].0)
    }
}

/// The units a detail delta is stored in: a tenth of a millimetre (MakeHuman writes three decimals
/// of a decimetre, so this is exact).
pub const DETAIL_UNIT_M: f32 = 1.0e-4;

/// `makehuman_details.bin`: the face and body detail targets by name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetailTargets {
    /// The base mesh's vertex count the targets were made for.
    pub base_vertices: usize,
    /// Each target's name and movement (base vertex, delta in `DETAIL_UNIT_M`), names sorted.
    pub targets: Vec<(String, Vec<(u32, [i16; 3])>)>,
}

impl DetailTargets {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(bytes, "makehuman_details.bin", b"HOSMHD01")?;
        let base_vertices = r.u32()? as usize;
        let n = r.count(8)?;
        let mut targets = Vec::with_capacity(n);
        for _ in 0..n {
            let name = r.name()?;
            let m = r.count(12)?;
            let mut list = Vec::with_capacity(m);
            for _ in 0..m {
                let v = r.u32()?;
                let d = [r.i16()?, r.i16()?, r.i16()?];
                let _pad = r.i16()?;
                if v as usize >= base_vertices {
                    return Err(format!("makehuman_details.bin: target {name} moves a vertex past the mesh"));
                }
                list.push((v, d));
            }
            targets.push((name, list));
        }
        r.done()?;
        Ok(DetailTargets { base_vertices, targets })
    }

    /// A target's index by name.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.targets.binary_search_by(|(n, _)| n.as_str().cmp(name)).ok()
    }
}

/// One garment vertex's place: three base vertices with weights, plus an offset scaled per axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Binding {
    pub verts: [u32; 3],
    pub weights: [f32; 3],
    /// Metres at the scale the garment was fitted at (each axis scaled by `GarmentData::scale`).
    pub offset: [f32; 3],
}

/// `garments/<id>/<id>.bin`: a garment, hair, brows or lashes fitted to the base mesh.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GarmentData {
    /// x, y and z: two base vertices and the distance between them (metres) along that axis when the
    /// garment was fitted; an offset axis is scaled by how far apart they are now over it.
    pub scale: [(u32, u32, f32); 3],
    pub bindings: Vec<Binding>,
    /// MakeHuman's own fitted positions of the bound vertices (its .obj), metres, raw base frame.
    pub reference: Vec<[f32; 3]>,
    /// Each bound vertex's skin: four joints and weights summing to 65535.
    pub skin: Vec<([u8; 4], [u16; 4])>,
    /// The drawn vertices: a bound vertex and a texture coordinate.
    pub drawn: Vec<(u32, [f32; 2])>,
    /// Triangles over the drawn vertices.
    pub indices: Vec<u32>,
    /// The base vertices the garment hides (MakeHuman's delete_verts).
    pub hidden: Vec<u32>,
}

impl GarmentData {
    pub fn parse(bytes: &[u8], base_vertices: usize) -> Result<Self, String> {
        let mut r = Reader::new(bytes, "a garment file", b"HOSMHG01")?;
        let nv = r.count(36)?;
        let nd = r.count(12)?;
        let ni = r.count(4)?;
        let nh = r.count(4)?;
        let mut scale = [(0u32, 0u32, 0.0f32); 3];
        for s in &mut scale {
            *s = (r.u32()?, r.u32()?, r.f32()?);
        }
        let in_mesh = |v: u32| (v as usize) < base_vertices;
        if !scale.iter().all(|&(a, b, d)| in_mesh(a) && in_mesh(b) && d > 0.0) {
            return Err("a garment file: a scale reference is past the mesh or not a distance".into());
        }
        let mut bindings = Vec::with_capacity(nv);
        for _ in 0..nv {
            let verts = [r.u32()?, r.u32()?, r.u32()?];
            let weights = [r.f32()?, r.f32()?, r.f32()?];
            let offset = r.vec3()?;
            if !verts.iter().all(|&v| in_mesh(v)) {
                return Err("a garment file: a binding names a vertex past the mesh".into());
            }
            bindings.push(Binding { verts, weights, offset });
        }
        let reference = (0..nv).map(|_| r.vec3()).collect::<Result<Vec<_>, _>>()?;
        let mut skin = Vec::with_capacity(nv);
        for _ in 0..nv {
            let j = r.take(4)?;
            let joints = [j[0], j[1], j[2], j[3]];
            let weights = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
            skin.push((joints, weights));
        }
        let mut drawn = Vec::with_capacity(nd);
        for _ in 0..nd {
            let v = r.u32()?;
            let uv = [r.f32()?, r.f32()?];
            if v as usize >= nv {
                return Err("a garment file: a drawn vertex names a bound vertex past the list".into());
            }
            drawn.push((v, uv));
        }
        let indices = (0..ni).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
        if indices.len() % 3 != 0 || indices.iter().any(|&i| i as usize >= nd) {
            return Err("a garment file: its triangles are not whole or name a vertex past the list".into());
        }
        let hidden = (0..nh).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
        if hidden.iter().any(|&v| !in_mesh(v)) {
            return Err("a garment file: it hides a vertex past the mesh".into());
        }
        r.done()?;
        Ok(GarmentData { scale, bindings, reference, skin, drawn, indices, hidden })
    }

    /// Each bound vertex placed on a mesh whose base vertices stand at `base` (MakeHuman's rule: the
    /// weighted sum of its three base vertices, plus its offset with each axis scaled by how far
    /// apart that axis's two reference vertices are against the distance the garment was fitted at).
    pub fn place(&self, base: &[[f32; 3]]) -> Vec<[f32; 3]> {
        let s: [f32; 3] = std::array::from_fn(|axis| {
            let (a, b, d) = self.scale[axis];
            match (base.get(a as usize), base.get(b as usize)) {
                (Some(pa), Some(pb)) => (pa[axis] - pb[axis]).abs() / d,
                _ => 1.0,
            }
        });
        self.bindings
            .iter()
            .map(|b| {
                let mut p = [0.0f32; 3];
                for k in 0..3 {
                    if b.weights[k] == 0.0 {
                        continue;
                    }
                    let q = base.get(b.verts[k] as usize).copied().unwrap_or([0.0; 3]);
                    for (a, pa) in p.iter_mut().enumerate() {
                        *pa += b.weights[k] * q[a];
                    }
                }
                for (a, pa) in p.iter_mut().enumerate() {
                    *pa += b.offset[a] * s[a];
                }
                p
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(rel: &str) -> Vec<u8> {
        let p = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("assets/models/characters/makehuman").join(rel);
        std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    /// THE GARMENT PLACEMENT IS MAKEHUMAN'S (2026-11-09, "the look"): a bound vertex placed by
    /// `GarmentData::place` on the raw base mesh lands where MakeHuman's own fitted .obj put it, for
    /// the shipped garments whose .obj was fitted on that same raw base mesh (the fedora, the short
    /// hair 04, lashes 02 to 04; the others were fitted on a shaped body, so their .obj is not a
    /// reference for this mesh, which the scale references say: theirs are not 1). Within 0.1 mm.
    /// Seen red 2026-11-09 with the first two weights swapped in `place` (the converter's scratch
    /// check, the same arithmetic): the afro 197 mm out, the bob 203 mm; and with the offset left
    /// unscaled the same garments still pass (their scale is 1), so the scaled ones are checked
    /// against the scale rule below.
    #[test]
    fn a_bound_garment_vertex_lands_where_makehuman_fitted_it() {
        let shared = BaseShared::parse(&shipped("makehuman_base.bin")).expect("the shared base file reads");
        let mut checked = 0;
        for id in ["fedora01", "short04", "eyelashes02", "eyelashes03", "eyelashes04"] {
            let g = GarmentData::parse(&shipped(&format!("garments/{id}/{id}.bin")), shared.raw.len()).expect("a garment reads");
            let placed = g.place(&shared.raw);
            let worst = placed.iter().zip(&g.reference).map(|(p, q)| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()).fold(0.0_f32, f32::max);
            assert!(worst < 1.0e-4, "{id}: a placed vertex is {:.2} mm from MakeHuman's fitted one", worst * 1000.0);
            checked += placed.len();
        }
        assert!(checked > 1000, "checked {checked} vertices");
    }

    /// A garment's offset is scaled per axis by its reference vertices' spread (a body twice as wide
    /// in x carries its offsets twice as far out in x, and no further in y or z).
    #[test]
    fn a_garment_offset_scales_with_its_reference_vertices() {
        let g = GarmentData {
            scale: [(0, 1, 1.0), (0, 2, 1.0), (0, 3, 1.0)],
            bindings: vec![Binding { verts: [0, 0, 0], weights: [1.0, 0.0, 0.0], offset: [0.1, 0.2, 0.3] }],
            ..Default::default()
        };
        let base = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(g.place(&base), vec![[0.2, 0.2, 0.3]]);
    }

    /// The shipped files read whole, and a cut or corrupt one is refused rather than panicking.
    #[test]
    fn the_look_files_read_and_a_broken_one_is_refused() {
        let shared = BaseShared::parse(&shipped("makehuman_base.bin")).unwrap();
        assert_eq!(shared.joint_verts.len(), 56);
        assert!(shared.region("feet").is_some() && shared.region("torso").is_some());
        let female = BodyBase::parse(&shipped("makehuman_female.base.bin")).unwrap();
        assert_eq!(female.positions.len(), shared.raw.len());
        assert_eq!(female.targets.len(), 15);
        let details = DetailTargets::parse(&shipped("makehuman_details.bin")).unwrap();
        assert!(details.targets.len() > 500 && details.index("nose-hump-incr").is_some());
        let bytes = shipped("garments/shoes01/shoes01.bin");
        assert!(GarmentData::parse(&bytes, shared.raw.len()).is_ok());
        assert!(GarmentData::parse(&bytes[..bytes.len() - 3], shared.raw.len()).is_err(), "cut short");
        let mut bad = bytes.clone();
        bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(GarmentData::parse(&bad, shared.raw.len()).is_err(), "a count past the file");
        assert!(GarmentData::parse(&bytes, 10).is_err(), "a vertex past the mesh");
    }
}
