//! An OpenStreetMap PBF reader, our own (whole-Earth maps rung 2; no crate): the file format of
//! <https://wiki.openstreetmap.org/wiki/PBF_Format>, read straight from its protobuf wire format.
//!
//! A PBF file is a run of blobs: a 4-byte big-endian length, a `BlobHeader` (its type,
//! "OSMHeader" or "OSMData", and the blob's size), then the `Blob` (zlib-compressed, here, the only
//! compression Geofabrik writes). An "OSMData" blob inflates to a `PrimitiveBlock`: a string table
//! and groups of dense nodes, ways and relations whose tags index that table. Coordinates are
//! `lat_offset + granularity * value` nanodegrees; with the default granularity of 100 and no
//! offset that IS OSM's 10^7-degree integer, kept exactly (whole-earth-maps.md 3.3).
//!
//! Only what the map builder reads is decoded: node ids and positions, way ids, tags and node
//! lists, relation ids, tags and members. Untrusted input: every length is checked and a
//! malformed blob is an error, never a panic or an out-of-bounds read.

/// One step of a protobuf message: its field number and its value.
#[derive(Debug, Clone, Copy)]
pub enum Field<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

/// Walks one protobuf message's fields.
pub struct Fields<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Fields<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Fields { b, pos: 0 }
    }
}

fn varint(b: &[u8], pos: &mut usize) -> Result<u64, String> {
    crate::map_pack::get_varint(b, pos).ok_or_else(|| "truncated varint".to_string())
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u32, Field<'a>), String>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.b.len() {
            return None;
        }
        let r = (|| {
            let key = varint(self.b, &mut self.pos)?;
            let (num, wire) = ((key >> 3) as u32, key & 7);
            let f = match wire {
                0 => Field::Varint(varint(self.b, &mut self.pos)?),
                1 => {
                    let v = self.b.get(self.pos..self.pos + 8).ok_or("truncated fixed64")?;
                    self.pos += 8;
                    Field::Fixed64(u64::from_le_bytes(v.try_into().map_err(|_| "fixed64")?))
                }
                2 => {
                    let n = varint(self.b, &mut self.pos)? as usize;
                    let v = self.b.get(self.pos..self.pos + n).ok_or("truncated bytes field")?;
                    self.pos += n;
                    Field::Bytes(v)
                }
                5 => {
                    let v = self.b.get(self.pos..self.pos + 4).ok_or("truncated fixed32")?;
                    self.pos += 4;
                    Field::Fixed32(u32::from_le_bytes(v.try_into().map_err(|_| "fixed32")?))
                }
                w => return Err(format!("protobuf wire type {w} (groups are not used by PBF)")),
            };
            Ok((num, f))
        })();
        if r.is_err() {
            self.pos = self.b.len();
        }
        Some(r)
    }
}

/// A packed repeated varint field's values.
pub fn packed_varints(b: &[u8]) -> Result<Vec<u64>, String> {
    let mut pos = 0;
    let mut out = Vec::with_capacity(b.len());
    while pos < b.len() {
        out.push(varint(b, &mut pos)?);
    }
    Ok(out)
}

/// A packed repeated sint64 field, zigzag-decoded and delta-summed (PBF's DenseNodes and refs).
pub fn packed_delta_sint(b: &[u8]) -> Result<Vec<i64>, String> {
    let mut sum = 0i64;
    Ok(packed_varints(b)?
        .into_iter()
        .map(|v| {
            sum = sum.wrapping_add(((v >> 1) as i64) ^ -((v & 1) as i64));
            sum
        })
        .collect())
}

/// One blob: its type and its inflated bytes.
pub struct Blob {
    pub kind: String,
    pub data: Vec<u8>,
}

/// The blobs of a whole PBF file held in memory, in order.
pub struct BlobIter<'a> {
    file: &'a [u8],
    pos: usize,
}

/// Iterate a PBF file's blobs.
pub fn blobs(file: &[u8]) -> BlobIter<'_> {
    BlobIter { file, pos: 0 }
}

/// A blob's raw byte range in the file, not yet inflated: (type, the Blob message's bytes).
pub struct RawBlob<'a> {
    pub kind: String,
    pub blob: &'a [u8],
}

impl<'a> BlobIter<'a> {
    /// The next blob's bytes without inflating them (so a caller can inflate on worker threads).
    pub fn next_raw(&mut self) -> Option<Result<RawBlob<'a>, String>> {
        if self.pos >= self.file.len() {
            return None;
        }
        let r = (|| {
            let hl = u32::from_be_bytes(self.file.get(self.pos..self.pos + 4).ok_or("truncated blob length")?.try_into().map_err(|_| "len")?) as usize;
            self.pos += 4;
            if hl > 64 * 1024 {
                return Err(format!("BlobHeader of {hl} bytes (the format caps it at 64 KiB)"));
            }
            let header = self.file.get(self.pos..self.pos + hl).ok_or("truncated BlobHeader")?;
            self.pos += hl;
            let (mut kind, mut size) = (String::new(), 0usize);
            for f in Fields::new(header) {
                match f? {
                    (1, Field::Bytes(t)) => kind = String::from_utf8_lossy(t).into_owned(),
                    (3, Field::Varint(n)) => size = n as usize,
                    _ => {}
                }
            }
            if size > 32 * 1024 * 1024 {
                return Err(format!("Blob of {size} bytes (the format caps it at 32 MiB)"));
            }
            let blob = self.file.get(self.pos..self.pos + size).ok_or("truncated Blob")?;
            self.pos += size;
            Ok(RawBlob { kind, blob })
        })();
        if r.is_err() {
            self.pos = self.file.len();
        }
        Some(r)
    }
}

/// Inflate one Blob message (raw or zlib).
pub fn inflate_blob(blob: &[u8]) -> Result<Vec<u8>, String> {
    let (mut raw, mut raw_size, mut zlib) = (None, 0usize, None);
    for f in Fields::new(blob) {
        match f? {
            (1, Field::Bytes(b)) => raw = Some(b),
            (2, Field::Varint(n)) => raw_size = n as usize,
            (3, Field::Bytes(b)) => zlib = Some(b),
            (n @ 4..=7, _) => return Err(format!("Blob compression field {n} (only zlib and raw are read)")),
            _ => {}
        }
    }
    if let Some(r) = raw {
        return Ok(r.to_vec());
    }
    let z = zlib.ok_or("a Blob with no data")?;
    if raw_size > 32 * 1024 * 1024 {
        return Err(format!("raw_size {raw_size} over the format's 32 MiB"));
    }
    let out = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(z, raw_size.max(1)).map_err(|e| format!("zlib: {e:?}"))?;
    if out.len() != raw_size {
        return Err(format!("inflated {} bytes, the header said {raw_size}", out.len()));
    }
    Ok(out)
}

impl<'a> Iterator for BlobIter<'a> {
    type Item = Result<Blob, String>;
    fn next(&mut self) -> Option<Self::Item> {
        let raw = self.next_raw()?;
        Some(raw.and_then(|r| Ok(Blob { kind: r.kind, data: inflate_blob(r.blob)? })))
    }
}

/// The header block's replication timestamp (seconds since 1970), the snapshot's date.
pub fn header_timestamp(header_block: &[u8]) -> Result<Option<i64>, String> {
    for f in Fields::new(header_block) {
        if let (32, Field::Varint(t)) = f? {
            return Ok(Some(t as i64));
        }
    }
    Ok(None)
}

/// A node: id and OSM integer position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    pub id: i64,
    pub lat_e7: i32,
    pub lon_e7: i32,
}

/// A way: id, tags (as string-table indices resolved to strings) and node ids.
#[derive(Debug, Clone, PartialEq)]
pub struct Way {
    pub id: i64,
    pub tags: Vec<(String, String)>,
    pub refs: Vec<i64>,
}

/// A relation member: its type (0 node, 1 way, 2 relation), id and role.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub kind: u8,
    pub id: i64,
    pub role: String,
}

/// A relation: id, tags and members.
#[derive(Debug, Clone, PartialEq)]
pub struct Relation {
    pub id: i64,
    pub tags: Vec<(String, String)>,
    pub members: Vec<Member>,
}

/// What a primitive block holds, decoded as asked.
#[derive(Default)]
pub struct Block {
    pub nodes: Vec<Node>,
    pub ways: Vec<Way>,
    pub relations: Vec<Relation>,
}

/// Which element kinds to decode (the rest are skipped without decoding).
#[derive(Clone, Copy)]
pub struct Want {
    pub nodes: bool,
    pub ways: bool,
    pub relations: bool,
}

/// Decode one PrimitiveBlock.
pub fn decode_block(b: &[u8], want: Want) -> Result<Block, String> {
    let mut strings: Vec<&[u8]> = Vec::new();
    let mut groups: Vec<&[u8]> = Vec::new();
    let (mut gran, mut lat_off, mut lon_off) = (100i64, 0i64, 0i64);
    for f in Fields::new(b) {
        match f? {
            (1, Field::Bytes(st)) => {
                for s in Fields::new(st) {
                    if let (1, Field::Bytes(v)) = s? {
                        strings.push(v);
                    }
                }
            }
            (2, Field::Bytes(g)) => groups.push(g),
            (17, Field::Varint(v)) => gran = v as i64,
            (19, Field::Varint(v)) => lat_off = v as i64,
            (20, Field::Varint(v)) => lon_off = v as i64,
            _ => {}
        }
    }
    let s = |i: u64| -> Result<String, String> { Ok(String::from_utf8_lossy(strings.get(i as usize).ok_or("string index past the table")?).into_owned()) };
    // Nanodegrees to OSM's 10^7 integer: (offset + granularity * v) / 100.
    let to_e7 = |off: i64, v: i64| -> Result<i32, String> { i32::try_from((off + gran * v).div_euclid(100)).map_err(|_| "coordinate out of range".to_string()) };
    let tags = |keys: &[u64], vals: &[u64]| -> Result<Vec<(String, String)>, String> { keys.iter().zip(vals).map(|(k, v)| Ok((s(*k)?, s(*v)?))).collect() };
    let mut out = Block::default();
    for g in groups {
        for f in Fields::new(g) {
            match f? {
                (1, Field::Bytes(n)) if want.nodes => {
                    let (mut id, mut lat, mut lon) = (0i64, 0i64, 0i64);
                    for nf in Fields::new(n) {
                        match nf? {
                            (1, Field::Varint(v)) => id = ((v >> 1) as i64) ^ -((v & 1) as i64),
                            (8, Field::Varint(v)) => lat = ((v >> 1) as i64) ^ -((v & 1) as i64),
                            (9, Field::Varint(v)) => lon = ((v >> 1) as i64) ^ -((v & 1) as i64),
                            _ => {}
                        }
                    }
                    out.nodes.push(Node { id, lat_e7: to_e7(lat_off, lat)?, lon_e7: to_e7(lon_off, lon)? });
                }
                (2, Field::Bytes(d)) if want.nodes => {
                    let (mut ids, mut lats, mut lons) = (Vec::new(), Vec::new(), Vec::new());
                    for df in Fields::new(d) {
                        match df? {
                            (1, Field::Bytes(v)) => ids = packed_delta_sint(v)?,
                            (8, Field::Bytes(v)) => lats = packed_delta_sint(v)?,
                            (9, Field::Bytes(v)) => lons = packed_delta_sint(v)?,
                            _ => {}
                        }
                    }
                    if ids.len() != lats.len() || ids.len() != lons.len() {
                        return Err(format!("DenseNodes with {} ids, {} lats, {} lons", ids.len(), lats.len(), lons.len()));
                    }
                    for i in 0..ids.len() {
                        out.nodes.push(Node { id: ids[i], lat_e7: to_e7(lat_off, lats[i])?, lon_e7: to_e7(lon_off, lons[i])? });
                    }
                }
                (3, Field::Bytes(w)) if want.ways => {
                    let (mut id, mut keys, mut vals, mut refs) = (0i64, Vec::new(), Vec::new(), Vec::new());
                    for wf in Fields::new(w) {
                        match wf? {
                            (1, Field::Varint(v)) => id = v as i64,
                            (2, Field::Bytes(v)) => keys = packed_varints(v)?,
                            (3, Field::Bytes(v)) => vals = packed_varints(v)?,
                            (8, Field::Bytes(v)) => refs = packed_delta_sint(v)?,
                            _ => {}
                        }
                    }
                    out.ways.push(Way { id, tags: tags(&keys, &vals)?, refs });
                }
                (4, Field::Bytes(r)) if want.relations => {
                    let (mut id, mut keys, mut vals, mut roles, mut mems, mut types) = (0i64, Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                    for rf in Fields::new(r) {
                        match rf? {
                            (1, Field::Varint(v)) => id = v as i64,
                            (2, Field::Bytes(v)) => keys = packed_varints(v)?,
                            (3, Field::Bytes(v)) => vals = packed_varints(v)?,
                            (8, Field::Bytes(v)) => roles = packed_varints(v)?,
                            (9, Field::Bytes(v)) => mems = packed_delta_sint(v)?,
                            (10, Field::Bytes(v)) => types = packed_varints(v)?,
                            _ => {}
                        }
                    }
                    if roles.len() != mems.len() || mems.len() != types.len() {
                        return Err(format!("relation {id}: {} roles, {} members, {} types", roles.len(), mems.len(), types.len()));
                    }
                    let members = (0..mems.len()).map(|i| Ok(Member { kind: types[i] as u8, id: mems[i], role: s(roles[i])? })).collect::<Result<_, String>>()?;
                    out.relations.push(Relation { id, tags: tags(&keys, &vals)?, members });
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

/// The value of tag `k` in a tag list.
pub fn tag<'t>(tags: &'t [(String, String)], k: &str) -> Option<&'t str> {
    tags.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_pack::{put_svarint, put_varint};

    fn key(out: &mut Vec<u8>, num: u32, wire: u8) {
        put_varint(out, ((num as u64) << 3) | wire as u64);
    }
    fn bytes_field(out: &mut Vec<u8>, num: u32, b: &[u8]) {
        key(out, num, 2);
        put_varint(out, b.len() as u64);
        out.extend_from_slice(b);
    }
    fn packed_sint_delta(vals: &[i64]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut prev = 0;
        for &v in vals {
            put_svarint(&mut out, v - prev);
            prev = v;
        }
        out
    }

    /// A PrimitiveBlock built by hand in the wire format, with a lat offset and a non-default
    /// granularity, decodes to the nodes, the way and the relation put in, coordinates in OSM's
    /// 10^7 integers. Seen red 2026-11-09 with `to_e7` ignoring the offset: "Dyes Inlet's node
    /// at 47.64 N; left: Node { id: 101, lat_e7: 476395000, ..".
    #[test]
    fn a_hand_built_block_decodes() {
        let mut strings = Vec::new();
        for s in ["", "natural", "coastline", "outer", "type", "multipolygon"] {
            bytes_field(&mut strings, 1, s.as_bytes());
        }
        let mut dense = Vec::new();
        bytes_field(&mut dense, 1, &packed_sint_delta(&[101, 102, 205]));
        // granularity 200 and lat_offset 5_000_000 nanodegrees: e7 = (5_000_000 + 200 v) / 100.
        bytes_field(&mut dense, 8, &packed_sint_delta(&[238_197_500, 238_197_550, 238_200_000]));
        bytes_field(&mut dense, 9, &packed_sint_delta(&[-613_475_000, -613_470_000, -613_480_000]));
        let mut way = Vec::new();
        key(&mut way, 1, 0);
        put_varint(&mut way, 9001);
        let mut k = Vec::new();
        put_varint(&mut k, 1);
        bytes_field(&mut way, 2, &k);
        let mut v = Vec::new();
        put_varint(&mut v, 2);
        bytes_field(&mut way, 3, &v);
        bytes_field(&mut way, 8, &packed_sint_delta(&[101, 102, 205, 101]));
        let mut rel = Vec::new();
        key(&mut rel, 1, 0);
        put_varint(&mut rel, 77);
        let (mut k, mut v, mut roles, mut types) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        put_varint(&mut k, 4);
        put_varint(&mut v, 5);
        put_varint(&mut roles, 3);
        put_varint(&mut types, 1);
        bytes_field(&mut rel, 2, &k);
        bytes_field(&mut rel, 3, &v);
        bytes_field(&mut rel, 8, &roles);
        bytes_field(&mut rel, 9, &packed_sint_delta(&[9001]));
        bytes_field(&mut rel, 10, &types);
        let mut group = Vec::new();
        bytes_field(&mut group, 2, &dense);
        bytes_field(&mut group, 3, &way);
        bytes_field(&mut group, 4, &rel);
        let mut block = Vec::new();
        bytes_field(&mut block, 1, &strings);
        bytes_field(&mut block, 2, &group);
        key(&mut block, 17, 0);
        put_varint(&mut block, 200);
        key(&mut block, 19, 0);
        put_varint(&mut block, 5_000_000);
        let all = Want { nodes: true, ways: true, relations: true };
        let b = decode_block(&block, all).expect("decodes");
        assert_eq!(b.nodes[0], Node { id: 101, lat_e7: 476_445_000, lon_e7: -1_226_950_000 }, "Dyes Inlet's node at 47.64 N");
        assert_eq!(b.nodes.len(), 3);
        assert_eq!(b.ways, vec![Way { id: 9001, tags: vec![("natural".into(), "coastline".into())], refs: vec![101, 102, 205, 101] }]);
        assert_eq!(b.relations[0].members, vec![Member { kind: 1, id: 9001, role: "outer".into() }]);
        assert_eq!(tag(&b.relations[0].tags, "type"), Some("multipolygon"));
        assert!(decode_block(&block, Want { nodes: false, ways: true, relations: false }).unwrap().nodes.is_empty());
        // A cut-short message is an error, never a panic.
        assert!(decode_block(&block[..block.len() - 3], all).is_err());
    }

    /// A zlib blob round trips through the blob reader; a lying size is refused. Seen red
    /// 2026-11-09 with the raw_size check dropped: "a blob whose header lies about its size".
    #[test]
    fn blobs_inflate_and_check_their_size() {
        let payload = b"OSMData payload".repeat(20);
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&payload, 6);
        let make = |raw_size: usize| {
            let mut blob = Vec::new();
            key(&mut blob, 2, 0);
            put_varint(&mut blob, raw_size as u64);
            bytes_field(&mut blob, 3, &z);
            let mut header = Vec::new();
            bytes_field(&mut header, 1, b"OSMData");
            key(&mut header, 3, 0);
            put_varint(&mut header, blob.len() as u64);
            let mut file = (header.len() as u32).to_be_bytes().to_vec();
            file.extend_from_slice(&header);
            file.extend_from_slice(&blob);
            file
        };
        let file = make(payload.len());
        let got: Vec<Blob> = blobs(&file).collect::<Result<_, _>>().expect("reads");
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].kind.as_str(), got[0].data.as_slice()), ("OSMData", payload.as_slice()));
        assert!(blobs(&make(payload.len() + 1)).next().unwrap().is_err(), "a blob whose header lies about its size");
    }
}
