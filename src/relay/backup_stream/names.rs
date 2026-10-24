//! The names of what the continuous backup keeps at a destination, and the two decisions made
//! from a listing of them alone: what a restore uses, and what retention may delete.
//!
//! Layout (every name relative to the destination's folder or prefix):
//!
//! ```text
//! owner                                          which server keeps its backups here
//! <gen>/seg-<seq:10>-<captured ms:13>-<c|p>      WAL frames; c = ends at a commit, p = part of one
//! <gen>/snap-<seq:10>-<taken ms:13>/c<i:05>      a snapshot's chunks
//! <gen>/snap-<seq:10>-<taken ms:13>/manifest-<n:05>   written last; n = how many chunks
//! ```
//!
//! A generation (`<created ms:13>-<random:8 hex>`, so names sort in time order) is one unbroken
//! run of the log: it starts with a snapshot, and every segment after it continues from the one
//! before. A new generation starts whenever the shipper cannot prove the run is unbroken (the
//! relay was stopped uncleanly and its log is gone, a checkpoint it did not make). A snapshot
//! counts only when its manifest and every chunk the manifest's name promises are there, so a
//! snapshot half uploaded when the power went out is never used.

use std::collections::BTreeMap;

/// The owner mark's name.
pub const OWNER: &str = "owner";

/// One object as a destination lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub name: String,
    pub size: u64,
}

/// A new generation id for a run starting at `now_ms`.
pub fn new_generation(now_ms: u64) -> String {
    format!("{now_ms:013}-{:08x}", rand::random::<u32>())
}

pub fn segment_name(gen: &str, seq: u64, captured_ms: u64, ends_at_commit: bool) -> String {
    format!("{gen}/seg-{seq:010}-{captured_ms:013}-{}", if ends_at_commit { 'c' } else { 'p' })
}

pub fn snapshot_dir(gen: &str, seq: u64, taken_ms: u64) -> String {
    format!("{gen}/snap-{seq:010}-{taken_ms:013}")
}

pub fn chunk_name(snap_dir: &str, i: u32) -> String {
    format!("{snap_dir}/c{i:05}")
}

pub fn manifest_name(snap_dir: &str, chunks: u32) -> String {
    format!("{snap_dir}/manifest-{chunks:05}")
}

/// What a name is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Name {
    Owner,
    Segment { gen: String, seq: u64, ms: u64, commit: bool },
    Chunk { gen: String, seq: u64, ms: u64, idx: u32 },
    Manifest { gen: String, seq: u64, ms: u64, chunks: u32 },
    /// Not one of ours (a probe, a stray file someone put there): never used, never deleted.
    Other,
}

fn digits<T: std::str::FromStr>(s: &str, len: usize) -> Option<T> {
    (s.len() == len && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

fn is_gen(s: &str) -> bool {
    let (ms, rnd) = match s.split_once('-') {
        Some(p) => p,
        None => return false,
    };
    digits::<u64>(ms, 13).is_some() && rnd.len() == 8 && rnd.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// `snap-<seq>-<ms>` to (seq, ms).
fn snap_head(s: &str) -> Option<(u64, u64)> {
    let rest = s.strip_prefix("snap-")?;
    let (seq, ms) = rest.split_once('-')?;
    Some((digits(seq, 10)?, digits(ms, 13)?))
}

pub fn parse(name: &str) -> Name {
    if name == OWNER {
        return Name::Owner;
    }
    let parts: Vec<&str> = name.split('/').collect();
    match parts.as_slice() {
        [gen, file] if is_gen(gen) => {
            let Some(rest) = file.strip_prefix("seg-") else { return Name::Other };
            let f: Vec<&str> = rest.split('-').collect();
            match f.as_slice() {
                [seq, ms, kind] if *kind == "c" || *kind == "p" => match (digits(seq, 10), digits(ms, 13)) {
                    (Some(seq), Some(ms)) => Name::Segment { gen: gen.to_string(), seq, ms, commit: *kind == "c" },
                    _ => Name::Other,
                },
                _ => Name::Other,
            }
        }
        [gen, snap, file] if is_gen(gen) => {
            let Some((seq, ms)) = snap_head(snap) else { return Name::Other };
            if let Some(n) = file.strip_prefix("manifest-") {
                return match digits(n, 5) {
                    Some(chunks) => Name::Manifest { gen: gen.to_string(), seq, ms, chunks },
                    None => Name::Other,
                };
            }
            match file.strip_prefix('c').and_then(|i| digits(i, 5)) {
                Some(idx) => Name::Chunk { gen: gen.to_string(), seq, ms, idx },
                None => Name::Other,
            }
        }
        _ => Name::Other,
    }
}

/// A snapshot found in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snap {
    pub gen: String,
    pub seq: u64,
    pub ms: u64,
    /// The chunk count its manifest's name promises (None: no manifest yet).
    pub chunks: Option<u32>,
    pub chunks_present: Vec<u32>,
}

impl Snap {
    pub fn dir(&self) -> String {
        snapshot_dir(&self.gen, self.seq, self.ms)
    }
    /// Its manifest and every chunk the manifest promises are there.
    pub fn complete(&self) -> bool {
        match self.chunks {
            Some(n) => (0..n).all(|i| self.chunks_present.contains(&i)),
            None => false,
        }
    }
    fn key(&self) -> (String, u64, u64) {
        (self.gen.clone(), self.seq, self.ms)
    }
}

/// A segment found in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seg {
    pub gen: String,
    pub seq: u64,
    pub ms: u64,
    pub commit: bool,
}

impl Seg {
    pub fn name(&self) -> String {
        segment_name(&self.gen, self.seq, self.ms, self.commit)
    }
}

/// Everything at a destination, sorted out.
#[derive(Debug, Default)]
pub struct Catalog {
    pub owner: bool,
    /// Per generation (in time order): its snapshots and its segments by sequence number.
    pub gens: BTreeMap<String, GenCat>,
    pub objects: u64,
    pub bytes: u64,
}

#[derive(Debug, Default)]
pub struct GenCat {
    pub snaps: Vec<Snap>,
    pub segs: BTreeMap<u64, Vec<Seg>>,
}

impl Catalog {
    pub fn from_listing(list: &[Listed]) -> Self {
        let mut c = Catalog::default();
        for l in list {
            c.objects += 1;
            c.bytes += l.size;
            match parse(&l.name) {
                Name::Owner => c.owner = true,
                Name::Segment { gen, seq, ms, commit } => {
                    c.gens.entry(gen.clone()).or_default().segs.entry(seq).or_default().push(Seg { gen, seq, ms, commit });
                }
                Name::Chunk { gen, seq, ms, idx } => {
                    snap_entry(&mut c, &gen, seq, ms).chunks_present.push(idx);
                }
                Name::Manifest { gen, seq, ms, chunks } => {
                    snap_entry(&mut c, &gen, seq, ms).chunks = Some(chunks);
                }
                Name::Other => {}
            }
        }
        for g in c.gens.values_mut() {
            g.snaps.sort_by_key(|s| (s.seq, s.ms));
            for v in g.segs.values_mut() {
                // Two objects with one sequence number cannot come from a working shipper; if
                // one ever appears, the newest is the one used, so the choice is the same every time.
                v.sort_by_key(|s| s.ms);
            }
        }
        c
    }

    /// Every complete snapshot, oldest first.
    pub fn complete_snaps(&self) -> Vec<&Snap> {
        let mut v: Vec<&Snap> = self.gens.values().flat_map(|g| g.snaps.iter()).filter(|s| s.complete()).collect();
        v.sort_by_key(|s| (s.ms, s.gen.clone(), s.seq));
        v
    }
}

fn snap_entry<'a>(c: &'a mut Catalog, gen: &str, seq: u64, ms: u64) -> &'a mut Snap {
    let g = c.gens.entry(gen.to_string()).or_default();
    if let Some(i) = g.snaps.iter().position(|s| s.seq == seq && s.ms == ms) {
        return &mut g.snaps[i];
    }
    g.snaps.push(Snap { gen: gen.to_string(), seq, ms, chunks: None, chunks_present: Vec::new() });
    g.snaps.last_mut().unwrap()
}

/// What a restore should rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// The newest state the destination holds.
    Latest,
    /// The newest state at or before this time (Unix ms).
    Time(u64),
    /// Exactly the state after this segment of this generation (the restore drill).
    Seq(&'a str, u64),
}

/// A restore: one snapshot, then these segments in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub snap: Snap,
    pub segs: Vec<Seg>,
    /// The time the restored database will be from: the last segment's, or the snapshot's.
    pub restores_to_ms: u64,
}

/// Choose what to restore for `target`, from a listing alone. None when nothing complete fits.
///
/// The snapshot is the newest complete one the target allows, from the newest generation that
/// has one; then that generation's segments in unbroken order after it, stopping at the first
/// gap, at the first one later than the target, and finally cut back to the last one that ends
/// at a commit, so a restore never stops halfway through a transaction.
pub fn plan_restore(c: &Catalog, target: Target<'_>) -> Option<Plan> {
    let snap = c
        .gens
        .iter()
        .rev()
        .filter(|(g, _)| match target {
            Target::Seq(gen, _) => g.as_str() == gen,
            _ => true,
        })
        .find_map(|(_, gc)| {
            gc.snaps
                .iter()
                .filter(|s| s.complete())
                .filter(|s| match target {
                    Target::Latest => true,
                    Target::Time(t) => s.ms <= t,
                    Target::Seq(_, seq) => s.seq <= seq,
                })
                .max_by_key(|s| (s.seq, s.ms))
        })?
        .clone();
    let gc = &c.gens[&snap.gen];
    let mut segs = Vec::new();
    let mut next = snap.seq + 1;
    while let Some(v) = gc.segs.get(&next) {
        let s = v.last().unwrap();
        let past = match target {
            Target::Latest => false,
            Target::Time(t) => s.ms > t,
            Target::Seq(_, seq) => s.seq > seq,
        };
        if past {
            break;
        }
        segs.push(s.clone());
        next += 1;
    }
    while segs.last().is_some_and(|s| !s.commit) {
        segs.pop();
    }
    let restores_to_ms = segs.last().map(|s| s.ms).unwrap_or(snap.ms);
    Some(Plan { snap, segs, restores_to_ms })
}

/// What retention may delete now, given how long history is kept (`keep_ms`) and the
/// generation the shipper is writing (`current`, never deleted from).
///
/// The rule: keep every complete snapshot taken within the window, plus the newest one taken
/// before it (the base that lets the whole window be restored), and every segment after the
/// oldest snapshot kept. Everything older goes: earlier generations entirely, older snapshots
/// and the segments they made unnecessary. Also goes: a generation that never got a complete
/// snapshot and is older than the current one (it can never be restored), and the leftovers of
/// a snapshot whose upload was cut short, once a newer complete one exists in its generation.
/// When no complete snapshot exists at all, nothing is deleted: the destination's only copy is
/// never thrown away before its replacement has fully arrived.
pub fn plan_prune(c: &Catalog, now_ms: u64, keep_ms: u64, current: Option<&str>) -> Vec<String> {
    let complete = c.complete_snaps();
    if complete.is_empty() {
        return Vec::new();
    }
    let cutoff = now_ms.saturating_sub(keep_ms);
    // The base: the newest complete snapshot at or before the cutoff. When there is none, the
    // oldest complete snapshot is the base (all of them are inside the window).
    let base = complete.iter().rev().find(|s| s.ms <= cutoff).copied().unwrap_or(complete[0]);
    let base_key = base.key();
    let mut out = Vec::new();
    for (gen, gc) in &c.gens {
        let is_current = current == Some(gen.as_str());
        let has_complete = gc.snaps.iter().any(|s| s.complete());
        let older_than_base = gen.as_str() < base.gen.as_str();
        if is_current {
            // Only debris: a snapshot without its manifest or chunks, older than a complete one.
            let newest_complete = gc.snaps.iter().filter(|s| s.complete()).map(|s| (s.seq, s.ms)).max();
            for s in gc.snaps.iter().filter(|s| !s.complete()) {
                if newest_complete.is_some_and(|n| (s.seq, s.ms) < n) {
                    out.extend(snap_objects(s));
                }
            }
            if gen.as_str() == base.gen.as_str() {
                out.extend(older_in_base_gen(gc, &base_key));
            }
            continue;
        }
        if older_than_base || (!has_complete && current.is_some_and(|cur| gen.as_str() < cur)) {
            // Everything in it.
            for s in &gc.snaps {
                out.extend(snap_objects(s));
            }
            for v in gc.segs.values() {
                out.extend(v.iter().map(|s| s.name()));
            }
            continue;
        }
        if gen.as_str() == base.gen.as_str() {
            out.extend(older_in_base_gen(gc, &base_key));
            for s in gc.snaps.iter().filter(|s| !s.complete()) {
                out.extend(snap_objects(s));
            }
        }
    }
    out
}

/// In the base's generation: snapshots older than the base, and the segments it makes unnecessary.
fn older_in_base_gen(gc: &GenCat, base: &(String, u64, u64)) -> Vec<String> {
    let mut out = Vec::new();
    for s in &gc.snaps {
        if s.complete() && (s.seq, s.ms) < (base.1, base.2) {
            out.extend(snap_objects(s));
        }
    }
    for (seq, v) in &gc.segs {
        if *seq <= base.1 {
            out.extend(v.iter().map(|s| s.name()));
        }
    }
    out
}

fn snap_objects(s: &Snap) -> Vec<String> {
    let dir = s.dir();
    let mut v: Vec<String> = s.chunks_present.iter().map(|i| chunk_name(&dir, *i)).collect();
    if let Some(n) = s.chunks {
        v.push(manifest_name(&dir, n));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u64 = 3_600_000;

    fn gen(ms: u64) -> String {
        format!("{ms:013}-0000abcd")
    }

    fn snap(list: &mut Vec<Listed>, g: &str, seq: u64, ms: u64, chunks: u32) {
        let dir = snapshot_dir(g, seq, ms);
        for i in 0..chunks {
            list.push(Listed { name: chunk_name(&dir, i), size: 10 });
        }
        list.push(Listed { name: manifest_name(&dir, chunks), size: 1 });
    }

    fn seg(list: &mut Vec<Listed>, g: &str, seq: u64, ms: u64, commit: bool) {
        list.push(Listed { name: segment_name(g, seq, ms, commit), size: 5 });
    }

    #[test]
    fn names_parse_back() {
        let g = gen(1_790_000_000_000);
        assert_eq!(parse(&segment_name(&g, 3, 99, true)), Name::Segment { gen: g.clone(), seq: 3, ms: 99, commit: true });
        let d = snapshot_dir(&g, 4, 100);
        assert_eq!(parse(&chunk_name(&d, 2)), Name::Chunk { gen: g.clone(), seq: 4, ms: 100, idx: 2 });
        assert_eq!(parse(&manifest_name(&d, 7)), Name::Manifest { gen: g.clone(), seq: 4, ms: 100, chunks: 7 });
        assert_eq!(parse(OWNER), Name::Owner);
        for other in ["probe-1", "x/seg-1-2-c", &format!("{g}/seg-0000000001-0000000000001-x"), "../owner", &format!("{g}/snap-1-2/c00001")] {
            assert_eq!(parse(other), Name::Other, "{other}");
        }
        assert!(new_generation(5) < new_generation(6), "generations sort in time order");
    }

    /// The restore plan: newest complete snapshot, unbroken segments after it, cut back to a
    /// commit; a half-uploaded snapshot is skipped for the one before it; a gap stops the run;
    /// a time target stops at the last segment at or before it.
    ///
    /// Seen red 2026-10-22 with the cut back to a commit removed: "assertion `left == right`
    /// failed: cut back to the last commit, never past a gap / left: [1, 2, 3, 4] / right: [1, 2, 3]".
    #[test]
    fn a_restore_uses_the_newest_complete_snapshot_and_the_unbroken_run_after_it() {
        let g1 = gen(1000);
        let g2 = gen(2000);
        let mut l = Vec::new();
        snap(&mut l, &g1, 0, 1000, 2);
        for s in 1..=5 {
            seg(&mut l, &g1, s, 1000 + s * 10, true);
        }
        snap(&mut l, &g2, 0, 2000, 3);
        seg(&mut l, &g2, 1, 2010, true);
        seg(&mut l, &g2, 2, 2020, false);
        seg(&mut l, &g2, 3, 2030, true);
        seg(&mut l, &g2, 4, 2040, false); // the last part of a transaction never arrived
        seg(&mut l, &g2, 6, 2060, true); // after a gap: unusable
        // A newer snapshot in g2 whose chunk 1 never arrived.
        let d = snapshot_dir(&g2, 3, 2035);
        l.push(Listed { name: chunk_name(&d, 0), size: 1 });
        l.push(Listed { name: manifest_name(&d, 2), size: 1 });
        let c = Catalog::from_listing(&l);

        let p = plan_restore(&c, Target::Latest).unwrap();
        assert_eq!((p.snap.gen.as_str(), p.snap.seq), (g2.as_str(), 0), "the incomplete newer snapshot is skipped");
        assert_eq!(p.segs.iter().map(|s| s.seq).collect::<Vec<_>>(), vec![1, 2, 3], "cut back to the last commit, never past a gap");
        assert_eq!(p.restores_to_ms, 2030);

        let p = plan_restore(&c, Target::Time(1035)).unwrap();
        assert_eq!(p.snap.gen, g1);
        assert_eq!(p.segs.iter().map(|s| s.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
        let p = plan_restore(&c, Target::Time(2025)).unwrap();
        assert_eq!(p.segs.iter().map(|s| s.seq).collect::<Vec<_>>(), vec![1], "seq 2 is half a transaction whose end is after the time");
        assert_eq!(plan_restore(&c, Target::Time(999)), None, "nothing is that old");
        let p = plan_restore(&c, Target::Seq(&g1, 4)).unwrap();
        assert_eq!(p.segs.len(), 4);
    }

    /// Retention keeps the window plus its base and never deletes the only copy.
    ///
    /// Seen red 2026-10-22 with the base always taken as the oldest complete snapshot (so nothing
    /// before the window was ever let go): "assertion `left == right` failed", two names deleted
    /// where eight should have been.
    #[test]
    fn retention_keeps_the_window_and_its_base_and_deletes_the_rest() {
        let now = 100 * H;
        let keep = 24 * H;
        let g1 = gen(10 * H); // old generation, wholly before the window
        let g2 = gen(60 * H); // the base's generation
        let g3 = gen(90 * H); // current
        let g_dead = gen(95 * H); // never got a snapshot, older than... no: newer than current
        let g_lost = gen(85 * H); // never got a snapshot, older than current: unrestorable
        let mut l = Vec::new();
        snap(&mut l, &g1, 0, 10 * H, 1);
        seg(&mut l, &g1, 1, 11 * H, true);
        snap(&mut l, &g2, 0, 60 * H, 1);
        seg(&mut l, &g2, 1, 61 * H, true);
        snap(&mut l, &g2, 1, 70 * H, 2); // the base: newest complete at or before 76 h
        seg(&mut l, &g2, 2, 71 * H, true);
        snap(&mut l, &g2, 2, 80 * H, 1); // inside the window
        seg(&mut l, &g2, 3, 81 * H, true);
        seg(&mut l, &g_lost, 1, 86 * H, true);
        snap(&mut l, &g3, 0, 90 * H, 1);
        seg(&mut l, &g3, 1, 91 * H, true);
        // Debris in the current generation: chunks without a manifest, older than its snapshot.
        l.push(Listed { name: chunk_name(&snapshot_dir(&g3, 0, 89 * H), 0), size: 1 });
        seg(&mut l, &g_dead, 1, 96 * H, true);
        l.push(Listed { name: OWNER.into(), size: 1 });
        l.push(Listed { name: "someone-elses-file.txt".into(), size: 1 });
        let c = Catalog::from_listing(&l);
        let mut del = plan_prune(&c, now, keep, Some(&g3));
        del.sort();
        let mut want = vec![
            chunk_name(&snapshot_dir(&g1, 0, 10 * H), 0),
            manifest_name(&snapshot_dir(&g1, 0, 10 * H), 1),
            segment_name(&g1, 1, 11 * H, true),
            chunk_name(&snapshot_dir(&g2, 0, 60 * H), 0),
            manifest_name(&snapshot_dir(&g2, 0, 60 * H), 1),
            segment_name(&g2, 1, 61 * H, true),
            segment_name(&g_lost, 1, 86 * H, true),
            chunk_name(&snapshot_dir(&g3, 0, 89 * H), 0),
        ];
        want.sort();
        assert_eq!(del, want);
        // What is left restores to any time inside the window.
        let left: Vec<Listed> = l.iter().filter(|o| !del.contains(&o.name)).cloned().collect();
        let c2 = Catalog::from_listing(&left);
        assert_eq!(plan_restore(&c2, Target::Time(now - keep)).unwrap().snap.ms, 70 * H, "the window's start is restorable");
        assert_eq!(plan_restore(&c2, Target::Latest).unwrap().restores_to_ms, 91 * H);

        // Nothing complete anywhere: nothing is deleted, however old.
        let mut only = Vec::new();
        seg(&mut only, &g1, 1, H, true);
        only.push(Listed { name: chunk_name(&snapshot_dir(&g1, 5, H), 0), size: 1 });
        assert!(plan_prune(&Catalog::from_listing(&only), now, keep, Some(&g3)).is_empty());
    }
}
