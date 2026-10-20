//! Epic covers: the ADDRESSES of an Epic game's pictures, from the Epic Games Launcher's own
//! catalog on this computer, and the ONE function that asks Epic's servers for one (2026-10-19;
//! the operator: "can we not find an image for fortnite so it's not just simple text?").
//!
//! The launcher keeps its catalog of the store's entries at
//! `%ProgramData%\Epic\EpicGamesLauncher\Data\Catalog\catcache.bin`, beside the install manifests
//! (epic.rs): base64 of a JSON list, each entry with its `namespace` and `id` (the
//! `CatalogNamespace` and `CatalogItemId` an install's manifest carries), its `title`, and
//! `keyImages`, the store's pictures for it as addresses on Epic's own servers, each with a `type`
//! ("DieselGameBox", 2560 by 1440 for Fortnite on the operator's computer; "DieselGameBoxTall",
//! 1200 by 1600; "AndroidIcon"; "Featured"), its `url`, `width` and `height`. The pictures
//! themselves are NOT on this computer.
//!
//! WHAT IS DONE, per the dated finding
//! docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md (1.5, judgement call 1, and
//! the dated note at its end):
//! - The catalog is read here, read only, on the Apps page's worker with the manifests. Reading it
//!   fetches nothing: an Epic game's details carry only the addresses ([`Covers`]).
//! - Nothing is fetched unless the person turns on the Apps page's switch, which is off by default
//!   (gui/pages/apps/covers.rs); then only the pictures of the Epic games installed on this
//!   computer, only from the hosts in [`HOSTS`], through [`fetch`], the one function here that asks
//!   Epic's servers for anything. Test builds record what [`fetch`] was asked instead of fetching.
//! - What comes back is decoded into memory for that session and never written anywhere: no
//!   cache, no copy (the finding's never-copy rule, and CLAUDE.md's no-caches rule).

use std::path::{Path, PathBuf};

use base64::Engine;

/// One of a store entry's pictures, as the catalog records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyImage {
    /// Its kind ("DieselGameBox", "DieselGameBoxTall"...).
    pub kind: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// One entry of the launcher's catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogItem {
    /// Its catalog namespace and item id (a manifest's `CatalogNamespace` and `CatalogItemId`).
    pub namespace: String,
    pub id: String,
    pub title: String,
    pub images: Vec<KeyImage>,
}

/// The addresses of a game's two pictures on Epic's servers: one for its tile, one for the banner
/// at the top of its details. Addresses only: nothing is fetched by having them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Covers {
    pub tile: Option<String>,
    pub banner: Option<String>,
}

impl Covers {
    /// Both addresses, the tile's first, each once.
    pub fn urls(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for u in [&self.tile, &self.banner].into_iter().flatten() {
            if !out.contains(u) {
                out.push(u.clone());
            }
        }
        out
    }
}

/// The only hosts a cover is fetched from: Epic's own picture servers, as the catalog names them
/// (every picture in the operator's catalog on 2026-10-19 was on this one host). A picture on any
/// other host is never asked for.
pub const HOSTS: [&str; 1] = ["cdn1.epicgames.com"];

/// The kinds a tile takes, in order (the tile's slot is wide, so the wide box art first), and the
/// banner's.
const TILE_KINDS: [&str; 2] = ["DieselGameBox", "DieselGameBoxTall"];
const BANNER_KINDS: [&str; 1] = ["DieselGameBox"];

/// The most bytes a picture may be (Fortnite's widest is under 1 MB).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
/// The most bytes the catalog file may be.
const MAX_CATALOG: u64 = 64 * 1024 * 1024;

/// Whether `url` is one a cover may be fetched from: https, on a host in [`HOSTS`], no user name
/// or port.
pub fn host_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    HOSTS.contains(&host)
}

/// The catalog beside an Epic manifests folder (`...\Data\Manifests` -> `...\Data\Catalog\catcache.bin`).
pub fn catalog_beside(manifests: &Path) -> Option<PathBuf> {
    manifests.parent().map(|data| data.join("Catalog").join("catcache.bin"))
}

/// The catalog's entries from the file's bytes: base64 of a JSON list. Entries without an id or
/// with pictures not on [`HOSTS`] keep only what is usable; a file that does not read is no entries.
pub fn parse(bytes: &[u8]) -> Vec<CatalogItem> {
    let text: Vec<u8> = bytes.iter().copied().filter(|b| !b.is_ascii_whitespace()).collect();
    let decoded = base64::engine::general_purpose::STANDARD.decode(&text).or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&text));
    let Ok(json) = decoded else { return Vec::new() };
    let Ok(serde_json::Value::Array(list)) = serde_json::from_slice::<serde_json::Value>(&json) else { return Vec::new() };
    let text_of = |v: &serde_json::Value, key: &str| v.get(key).and_then(|x| x.as_str()).map(str::trim).unwrap_or("").to_string();
    list.iter()
        .filter_map(|item| {
            let (namespace, id) = (text_of(item, "namespace"), text_of(item, "id"));
            if namespace.is_empty() || id.is_empty() {
                return None;
            }
            let images = item
                .get("keyImages")
                .and_then(|k| k.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|img| {
                            let url = text_of(img, "url");
                            let n = |key: &str| img.get(key).and_then(|x| x.as_u64()).unwrap_or(0).min(u32::MAX as u64) as u32;
                            host_allowed(&url).then(|| KeyImage { kind: text_of(img, "type"), url, width: n("width"), height: n("height") })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(CatalogItem { namespace, id, title: text_of(item, "title"), images })
        })
        .collect()
}

/// The catalog at `path`, read only (none when it is not there or does not read).
pub fn read(path: &Path) -> Vec<CatalogItem> {
    super::read_bytes(path, MAX_CATALOG).map(|b| parse(&b)).unwrap_or_default()
}

/// The covers of the game whose manifest names `namespace` and `id`, from `catalog`.
pub fn covers_for(catalog: &[CatalogItem], namespace: &str, id: &str) -> Option<Covers> {
    let item = catalog.iter().find(|c| c.namespace == namespace && c.id == id)?;
    let first = |kinds: &[&str]| kinds.iter().find_map(|k| item.images.iter().find(|i| i.kind == *k).map(|i| i.url.clone()));
    let covers = Covers { tile: first(&TILE_KINDS), banner: first(&BANNER_KINDS) };
    (covers.tile.is_some() || covers.banner.is_some()).then_some(covers)
}

/// Fetch one cover's bytes from Epic's servers: the ONE place anything is asked of Epic here, and
/// only for an address on [`HOSTS`], with no redirect followed (so no other host is reached), at
/// most [`MAX_BYTES`]. Called only by the Apps page's covers worker, only while the person's switch
/// is on. Test builds record the address and answer what the test set, and never fetch.
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    if !host_allowed(url) {
        return Err("that address is not on Epic's picture servers".to_string());
    }
    #[cfg(test)]
    {
        recorder::record(url)
    }
    #[cfg(not(test))]
    {
        let agent = ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(20)).timeout_read(std::time::Duration::from_secs(30)).redirects(0).build();
        let resp = agent.get(url).call().map_err(|e| format!("Epic's server did not send it ({e})"))?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_reader(), MAX_BYTES as u64 + 1), &mut bytes).map_err(|e| format!("it did not arrive whole ({e})"))?;
        if bytes.len() > MAX_BYTES {
            return Err("it is larger than a cover could be".to_string());
        }
        Ok(bytes)
    }
}

/// Test builds: what [`fetch`] was asked, per test thread, instead of asking Epic.
#[cfg(test)]
pub(crate) mod recorder {
    use std::cell::RefCell;

    thread_local! {
        static ASKED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        static ANSWER: RefCell<Option<Result<Vec<u8>, String>>> = const { RefCell::new(None) };
    }

    pub(super) fn record(url: &str) -> Result<Vec<u8>, String> {
        ASKED.with(|a| a.borrow_mut().push(url.to_string()));
        ANSWER.with(|r| r.borrow().clone()).unwrap_or_else(|| Ok(super::tests::png(8, 4)))
    }

    /// Every address [`super::fetch`] was asked on this thread since the last call, in order.
    pub(crate) fn recorded() -> Vec<String> {
        ASKED.with(|a| std::mem::take(&mut *a.borrow_mut()))
    }

    /// From now on, on this thread, [`super::fetch`] answers `answer` (`None`: a small picture).
    pub(crate) fn answer_with(answer: Option<Result<Vec<u8>, String>>) {
        ANSWER.with(|r| *r.borrow_mut() = answer);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small PNG picture, `w` by `h` (what the recorder answers by default).
    pub(crate) fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbaImage::from_pixel(w, h, image::Rgba([200, 90, 40, 255])).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    /// A catalog as the launcher writes it: base64 of a JSON list. Fortnite's entry is the one on
    /// the operator's computer on 2026-10-19, its addresses shortened; Lantern Valley is the games
    /// fixture's (namespace ns1, item1), with a picture on a host not on the list.
    pub(crate) fn catalog_bytes() -> Vec<u8> {
        let json = serde_json::json!([
            { "id": "4fe75bbc5a674f4f9b356b5c90567da5", "namespace": "fn", "title": "Fortnite", "keyImages": [
                { "type": "AndroidIcon", "url": "https://cdn1.epicgames.com/item/fn/icon-512", "width": 512, "height": 512 },
                { "type": "DieselGameBox", "url": "https://cdn1.epicgames.com/item/fn/box-2560x1440", "width": 2560, "height": 1440 },
                { "type": "DieselGameBoxTall", "url": "https://cdn1.epicgames.com/item/fn/tall-1200x1600", "width": 1200, "height": 1600 },
                { "type": "Featured", "url": "https://cdn1.epicgames.com/item/fn/featured-800", "width": 800, "height": 800 }
            ]},
            { "id": "item1", "namespace": "ns1", "title": "Lantern Valley", "keyImages": [
                { "type": "DieselGameBoxTall", "url": "https://cdn1.epicgames.com/item/ns1/lantern-tall", "width": 1200, "height": 1600 },
                { "type": "DieselGameBox", "url": "https://evil.example/lantern-wide", "width": 2560, "height": 1440 }
            ]},
            { "id": "", "namespace": "x", "title": "No id" }
        ]);
        base64::engine::general_purpose::STANDARD.encode(json.to_string()).into_bytes()
    }

    /// THE LAUNCHER'S CATALOG IS READ, AND ONLY EPIC'S OWN PICTURE HOSTS ARE KEPT: Fortnite's entry
    /// gives its wide box art for the tile and the banner; Lantern Valley's wide picture is on a
    /// host not on the list, so its tile takes its tall one and it has no banner; an entry with no
    /// id is left out; a file that is not the catalog's shape reads as nothing; and the host list
    /// refuses plain http, another host, a host that only starts like Epic's, and a user name.
    ///
    /// Seen red 2026-10-19 with the host check taken out of `parse` (every picture kept): "Lantern
    /// Valley's picture on another host is left out" got Some("https://evil.example/lantern-wide").
    #[test]
    fn the_launchers_catalog_and_epics_own_hosts() {
        let catalog = parse(&catalog_bytes());
        assert_eq!(catalog.len(), 2, "an entry with no id is left out");
        let fortnite = covers_for(&catalog, "fn", "4fe75bbc5a674f4f9b356b5c90567da5").expect("Fortnite's covers");
        assert_eq!(fortnite.tile.as_deref(), Some("https://cdn1.epicgames.com/item/fn/box-2560x1440"), "the wide box art for its tile");
        assert_eq!(fortnite.banner.as_deref(), Some("https://cdn1.epicgames.com/item/fn/box-2560x1440"), "and its banner");
        assert_eq!(fortnite.urls().len(), 1, "one picture asked for, not two");
        let lantern = covers_for(&catalog, "ns1", "item1").expect("Lantern Valley's");
        assert_eq!(lantern.banner, None, "Lantern Valley's picture on another host is left out");
        assert_eq!(lantern.tile.as_deref(), Some("https://cdn1.epicgames.com/item/ns1/lantern-tall"), "its tall one for its tile");
        assert_eq!(covers_for(&catalog, "ns1", "nothing"), None);
        assert!(parse(b"{ not base64 }").is_empty() && parse(base64::engine::general_purpose::STANDARD.encode("{\"a\":1}").as_bytes()).is_empty());
        assert!(host_allowed("https://cdn1.epicgames.com/item/fn/x"));
        for bad in ["http://cdn1.epicgames.com/x", "https://cdn1.epicgames.com.evil.example/x", "https://evil.example/cdn1.epicgames.com", "https://user@cdn1.epicgames.com/x", "https://cdn1.epicgames.com:8443/x"] {
            assert!(!host_allowed(bad), "{bad} is refused");
        }
    }

    /// FETCH ASKS ONLY FOR AN ADDRESS ON EPIC'S HOSTS (the test build's fetch records): an address
    /// on another host is refused before anything is asked.
    ///
    /// Seen red 2026-10-19 with the host check taken out of `fetch`: `fetch("https://evil.example/x")
    /// .is_err()` failed (the address was asked for, and answered).
    #[test]
    fn fetch_asks_only_epics_hosts() {
        let _ = recorder::recorded();
        assert!(fetch("https://evil.example/x").is_err());
        assert!(recorder::recorded().is_empty(), "another host is never asked");
        assert!(fetch("https://cdn1.epicgames.com/item/fn/box").is_ok());
        assert_eq!(recorder::recorded(), vec!["https://cdn1.epicgames.com/item/fn/box".to_string()]);
    }
}
