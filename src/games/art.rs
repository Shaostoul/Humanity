//! A game's pictures (2026-10-19), as the dated finding
//! docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md allows them (judgement
//! call 1): ONLY what is already on this computer, read in place, shown to this person on this
//! page.
//!
//! - STEAM: the pictures Steam's own client cached for its library, in
//!   `<Steam>\appcache\librarycache\<appid>\`. Two layouts are in use (both seen on the operator's
//!   computer, 2026-10-19): the files directly in that folder (`header.jpg`, `library_600x900.jpg`,
//!   `library_hero.jpg`, `logo.png`), or one folder per picture, named by a hash, each holding one
//!   (`library_header.jpg`, `library_capsule.jpg`, `library_hero.jpg`, `logo.png`). Older clients
//!   wrote them flat, as `<appid>_header.jpg` and the like, in the cache folder itself.
//! - EPIC AND GOG: the icon Windows shows for the game's own program, read from the program's own
//!   resources (the program is opened as data, never run).
//! - A game added by hand, or with none of these: no picture. The page draws a plain panel with its
//!   name (widgets/cover_tile.rs).
//!
//! THE RULES THE FINDING SETS, kept here: the pictures are read where Steam put them, when the
//! page's worker reads the stores, and decoded into memory for drawing (widgets/thumbnails.rs);
//! they are NEVER copied into any HumanityOS folder or file cache, never uploaded, synced or sent
//! anywhere, and never put on an in-world screen or anything other players can see (showing them
//! to others would be a public display, which is the publisher's to allow). Nothing is fetched
//! from any store's servers. The stores' own logos are never shown: a game's picture belongs to
//! its publisher, and the store's mark to the store (the finding, 2).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{list_dir, Store};

/// A game's pictures on this computer: each `None` when there is none to show.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Art {
    /// For a tile: a wide picture (the slot is 460 by 215 in shape).
    pub cover: Option<PathBuf>,
    /// For the top of its details: a banner.
    pub hero: Option<PathBuf>,
    /// The game's own title lettering, drawn over the banner.
    pub logo: Option<PathBuf>,
    /// The icon of the game's own program (Epic and GOG games), already read.
    pub icon: Option<Icon>,
}

/// A program's icon, read into memory: its size and its pixels (RGBA, 8 bits each).
#[derive(Clone, PartialEq)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Icon({}x{})", self.width, self.height)
    }
}

/// The largest an icon is kept (its longer side, pixels): a tile draws it far smaller.
const ICON_MAX: u32 = 128;

/// Steam's picture names, in the order each slot prefers them: the tile's wide slot takes the
/// 460 by 215 header first; the banner takes the hero first.
const COVER_NAMES: [&str; 3] = ["header.jpg", "library_header.jpg", "library_hero.jpg"];
const HERO_NAMES: [&str; 3] = ["library_hero.jpg", "header.jpg", "library_header.jpg"];
const LOGO_NAME: &str = "logo.png";

/// Whether `name` is one of the pictures read.
fn known(name: &str) -> bool {
    COVER_NAMES.contains(&name) || HERO_NAMES.contains(&name) || name == LOGO_NAME
}

/// The pictures Steam's client cached for game `appid` under Steam's folder `steam_root`, found
/// by name in either layout (or the old flat one). Read on the page's worker thread.
pub fn steam(steam_root: &Path, appid: &str) -> Art {
    // The id is part of a path: only digits.
    if appid.is_empty() || !appid.chars().all(|c| c.is_ascii_digit()) {
        return Art::default();
    }
    let cache = steam_root.join("appcache").join("librarycache");
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    fn take(found: &mut Vec<(String, PathBuf)>, name: String, path: PathBuf) {
        if known(&name) && !found.iter().any(|(n, _)| *n == name) {
            found.push((name, path));
        }
    }
    for entry in list_dir(&cache.join(appid)) {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_file() {
            take(&mut found, entry.file_name().to_string_lossy().to_lowercase(), entry.path());
        } else if kind.is_dir() {
            for inner in list_dir(&entry.path()) {
                if inner.file_type().is_ok_and(|t| t.is_file()) {
                    take(&mut found, inner.file_name().to_string_lossy().to_lowercase(), inner.path());
                }
            }
        }
    }
    if found.is_empty() {
        for name in COVER_NAMES.iter().chain([LOGO_NAME].iter()) {
            let flat = cache.join(format!("{appid}_{name}"));
            if flat.is_file() {
                take(&mut found, name.to_string(), flat);
            }
        }
    }
    let first = |names: &[&str]| names.iter().find_map(|n| found.iter().find(|(f, _)| f == n).map(|(_, p)| p.clone()));
    Art { cover: first(&COVER_NAMES), hero: first(&HERO_NAMES), logo: first(&[LOGO_NAME]), icon: None }
}

/// The pictures for a game from `store` whose program is `program`: its icon, for Epic and GOG.
pub fn program(store: Store, program: Option<&Path>) -> Art {
    match (store, program) {
        (Store::Epic | Store::Gog, Some(exe)) => Art { icon: program_icon(exe), ..Art::default() },
        _ => Art::default(),
    }
}

/// The icon Windows shows for the program `exe`: its first icon group's largest picture, read
/// from the program's resources with the program opened as data (never run), and shrunk to at
/// most [`ICON_MAX`]. `None` when it has none, or elsewhere than Windows.
pub fn program_icon(exe: &Path) -> Option<Icon> {
    #[cfg(windows)]
    {
        if !exe.is_file() {
            return None;
        }
        super::note_disk_read();
        let (group, images) = windows_icons::read(exe)?;
        let ico = ico_of(&group, &images)?;
        let img = image::load_from_memory_with_format(&ico, image::ImageFormat::Ico).ok()?;
        let img = if img.width() > ICON_MAX || img.height() > ICON_MAX { img.thumbnail(ICON_MAX, ICON_MAX) } else { img };
        let rgba = img.to_rgba8();
        (rgba.width() > 0 && rgba.height() > 0).then(|| Icon { width: rgba.width(), height: rgba.height(), rgba: Arc::new(rgba.into_raw()) })
    }
    #[cfg(not(windows))]
    {
        let _ = exe;
        None
    }
}

/// One picture in an icon group: its size (0 is 256), its colour depth, and its resource id.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GroupEntry {
    width: u8,
    height: u8,
    colors: u8,
    planes: u16,
    bits: u16,
    id: u16,
}

/// The pictures an icon group (`RT_GROUP_ICON`, a GRPICONDIR) lists.
fn group_entries(group: &[u8]) -> Vec<GroupEntry> {
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    if group.len() < 6 || u16_at(group, 2) != 1 {
        return Vec::new();
    }
    let count = u16_at(group, 4) as usize;
    (0..count)
        .filter_map(|i| {
            let e = group.get(6 + 14 * i..6 + 14 * (i + 1))?;
            Some(GroupEntry { width: e[0], height: e[1], colors: e[2], planes: u16_at(e, 4), bits: u16_at(e, 6), id: u16_at(e, 12) })
        })
        .collect()
}

/// The largest, deepest picture of a group (its id), as an .ico file in memory with that one
/// picture, given each picture's bytes by id (`images`).
fn ico_of(group: &[u8], images: &[(u16, Vec<u8>)]) -> Option<Vec<u8>> {
    let side = |w: u8| if w == 0 { 256u32 } else { w as u32 };
    let best = group_entries(group).into_iter().filter(|e| images.iter().any(|(id, _)| *id == e.id)).max_by_key(|e| (side(e.width) * side(e.height), e.bits))?;
    let data = &images.iter().find(|(id, _)| *id == best.id)?.1;
    let mut ico = Vec::with_capacity(22 + data.len());
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    ico.extend_from_slice(&[best.width, best.height, best.colors, 0]);
    ico.extend_from_slice(&best.planes.to_le_bytes());
    ico.extend_from_slice(&best.bits.to_le_bytes());
    ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(data);
    Some(ico)
}

/// Reading a program's icon resources on Windows: the program is loaded as a data file and an
/// image resource only (`LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE`), so none of
/// its code runs, and is let go before this returns.
#[cfg(windows)]
mod windows_icons {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows::core::{BOOL, PCWSTR};
    use windows::Win32::Foundation::{FreeLibrary, HMODULE};
    use windows::Win32::System::LibraryLoader::{EnumResourceNamesW, FindResourceW, LoadLibraryExW, LoadResource, LockResource, SizeofResource, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE};

    /// Resource types (winuser.h): an icon's picture, and a group of them.
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    /// The most icon pictures read from one group.
    const MAX_IMAGES: usize = 32;

    /// A resource's name: a number, or text.
    enum Name {
        Id(u16),
        Text(Vec<u16>),
    }

    fn int_resource(id: u16) -> PCWSTR {
        PCWSTR(id as usize as *const u16)
    }

    struct Loaded(HMODULE);

    impl Drop for Loaded {
        fn drop(&mut self) {
            // SAFETY: the module came from a successful LoadLibraryExW and is freed once, here.
            unsafe {
                let _ = FreeLibrary(self.0);
            }
        }
    }

    /// Called for each icon group's name: keeps the first, and stops.
    unsafe extern "system" fn first_name(_module: HMODULE, _kind: PCWSTR, name: PCWSTR, out: isize) -> BOOL {
        // SAFETY: `out` is the `&mut Option<Name>` `read` passed, alive for the whole enumeration;
        // a name that is not a number is a NUL-ended string Windows keeps alive for this call.
        unsafe {
            let out = &mut *(out as *mut Option<Name>);
            *out = Some(if (name.0 as usize) >> 16 == 0 { Name::Id(name.0 as usize as u16) } else { Name::Text(name.as_wide().iter().copied().chain(std::iter::once(0)).collect()) });
        }
        BOOL(0)
    }

    /// A resource's bytes, copied out.
    fn resource(module: HMODULE, name: PCWSTR, kind: u16) -> Option<Vec<u8>> {
        // SAFETY: `module` is loaded for the whole call; `name` is a number or a NUL-ended string
        // that outlives it; the locked bytes are `size` long and copied before the module is freed.
        unsafe {
            let found = FindResourceW(Some(module), name, int_resource(kind));
            if found.is_invalid() {
                return None;
            }
            let size = SizeofResource(Some(module), found) as usize;
            let loaded = LoadResource(Some(module), found).ok()?;
            let ptr = LockResource(loaded) as *const u8;
            if ptr.is_null() || size == 0 || size > 4 * 1024 * 1024 {
                return None;
            }
            Some(std::slice::from_raw_parts(ptr, size).to_vec())
        }
    }

    /// The program's first icon group, and each of its pictures' bytes by id.
    pub(super) fn read(exe: &Path) -> Option<(Vec<u8>, Vec<(u16, Vec<u8>)>)> {
        let wide: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is NUL-ended and outlives the call; loaded as data, nothing of it runs.
        let module = unsafe { LoadLibraryExW(PCWSTR(wide.as_ptr()), None, LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE) }.ok()?;
        let loaded = Loaded(module);
        let mut first: Option<Name> = None;
        // SAFETY: `first` outlives the enumeration, which the callback ends after one name. Its
        // answer is FALSE when stopped that way, so it is not read.
        unsafe {
            let _ = EnumResourceNamesW(Some(loaded.0), int_resource(RT_GROUP_ICON), Some(first_name), &mut first as *mut Option<Name> as isize);
        }
        let group = match first? {
            Name::Id(id) => resource(loaded.0, int_resource(id), RT_GROUP_ICON)?,
            Name::Text(text) => resource(loaded.0, PCWSTR(text.as_ptr()), RT_GROUP_ICON)?,
        };
        let images = super::group_entries(&group).iter().take(MAX_IMAGES).filter_map(|e| resource(loaded.0, int_resource(e.id), RT_ICON).map(|b| (e.id, b))).collect();
        Some((group, images))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures;

    /// STEAM'S CACHED PICTURES IN BOTH LAYOUTS: Orbital Farmstead's are directly in its folder (the
    /// header for the tile, the hero for the banner, its logo); Old Lighthouse's are one per
    /// hash-named folder, with no header (the tile takes its library header); Deep Rover's are in
    /// the old flat layout; Sunken Archive has none; an id that is not a number reads nothing.
    ///
    /// Seen red 2026-10-19 with the hash-named folders not looked into (files only): "one per
    /// hash-named folder" got Art { cover: None, .. } for Old Lighthouse.
    #[test]
    fn steams_cached_pictures_in_both_layouts() {
        let fx = fixtures::steam("art_layouts");
        let cache = fx.root.join("appcache").join("librarycache");
        let farm = steam(&fx.root, "1000010");
        assert_eq!(farm.cover, Some(cache.join("1000010").join("header.jpg")));
        assert_eq!(farm.hero, Some(cache.join("1000010").join("library_hero.jpg")));
        assert_eq!(farm.logo, Some(cache.join("1000010").join("logo.png")));
        let old = steam(&fx.root, "1000030");
        assert!(old.cover.as_ref().is_some_and(|p| p.ends_with("library_header.jpg") && p.parent().unwrap().parent().unwrap().ends_with("1000030")), "one per hash-named folder: {old:?}");
        assert!(old.hero.as_ref().is_some_and(|p| p.ends_with("library_hero.jpg")));
        assert_eq!(steam(&fx.root, "1000020").cover, Some(cache.join("1000020_header.jpg")), "the old flat layout");
        assert_eq!(steam(&fx.root, "1000040"), Art::default(), "none");
        assert_eq!(steam(&fx.root, "..\\1000010"), Art::default(), "an id that is not a number");
    }

    /// AN ICON GROUP'S LARGEST, DEEPEST PICTURE becomes a one-picture .ico: of a 32-pixel 32-bit,
    /// a 256-pixel (written 0) 32-bit and a 256-pixel 8-bit picture, the 256-pixel 32-bit one.
    ///
    /// Seen red 2026-10-19 with the size read as written (0 for 256): the 32-pixel picture was
    /// chosen (left: [32, 32, ..], right: [0, 0, ..]).
    #[test]
    fn an_icon_groups_largest_deepest_picture() {
        let entry = |w: u8, bits: u16, id: u16| {
            let mut e = vec![w, w, 0, 0];
            e.extend_from_slice(&1u16.to_le_bytes());
            e.extend_from_slice(&bits.to_le_bytes());
            e.extend_from_slice(&100u32.to_le_bytes());
            e.extend_from_slice(&id.to_le_bytes());
            e
        };
        let mut group = vec![0, 0, 1, 0, 3, 0];
        group.extend(entry(32, 32, 1));
        group.extend(entry(0, 32, 2));
        group.extend(entry(0, 8, 3));
        let images = vec![(1, vec![1u8; 10]), (2, vec![2u8; 20]), (3, vec![3u8; 30])];
        let ico = ico_of(&group, &images).expect("an .ico");
        assert_eq!(&ico[6..8], &[0, 0], "the 256-pixel picture");
        assert_eq!(u16::from_le_bytes([ico[12], ico[13]]), 32, "the deeper one");
        assert_eq!(&ico[22..], &[2u8; 20][..], "its own bytes");
        assert_eq!(u32::from_le_bytes([ico[14], ico[15], ico[16], ico[17]]), 20, "their length");
    }

    /// A PROGRAM'S ICON, read from Windows' own Explorer (every Windows has it, and it has an icon);
    /// a file named .exe that is not a program has none.
    ///
    /// Seen red 2026-10-19 with the group's first picture taken instead of its largest: "no larger
    /// than kept, and not tiny" got Icon(16x16).
    #[cfg(windows)]
    #[test]
    fn a_programs_icon() {
        let explorer = std::path::PathBuf::from(std::env::var("SystemRoot").expect("Windows sets SystemRoot")).join("explorer.exe");
        let icon = program_icon(&explorer).expect("Explorer has an icon");
        assert!(icon.width <= ICON_MAX && icon.height <= ICON_MAX && icon.width >= 32, "no larger than kept, and not tiny: {icon:?}");
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        assert!(icon.rgba.chunks_exact(4).any(|p| p[3] > 0), "it has pixels you can see");
        let fake = crate::test_temp::dir("art_fake_exe");
        std::fs::write(fake.join("game.exe"), "not a program").unwrap();
        assert_eq!(program_icon(&fake.join("game.exe")), None);
        assert_eq!(program(Store::Steam, Some(&explorer)), Art::default(), "a Steam game's pictures are Steam's, not its program's");
    }
}
