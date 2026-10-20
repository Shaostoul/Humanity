//! One entry of the external catalog (`data/external/catalog.json`), as the Apps page reads it:
//! a free program (`kind == "software"`) or a real-world help service (`kind == "service"`). One
//! catalog, both clients: the web Tools page reads the same file (v0.1063).
//!
//! Since 2026-10-19 a program's entry may also say how to recognise it on this computer and how it
//! could be installed, all as DATA (Infinite-of-X: a new program is a new entry, never code):
//!
//! - `detect`: `display_name`, a regular expression matched against the name Windows keeps for an
//!   installed program (its Uninstall record's DisplayName), letter case ignored; and `exe`, the
//!   program's own file as a path inside its install folder ("blender.exe", "bin/krita.exe"), or a
//!   list when its versions name it differently. The file name is also looked up in Windows' App
//!   Paths. Left out when its installer's naming is not known: the entry is then never "installed".
//!   And `shortcut` (2026-10-19), a regular expression for its Start-menu shortcut's name, for a
//!   program whose installed-programs record does not say where its program is (DaVinci Resolve).
//! - `steam_appid`: the program's Steam app id, when it is also sold or given away on Steam, so a
//!   copy Steam installed counts (read with the Games page's Steam reader, games/steam.rs).
//! - `winget`: its package id in winget, Windows' own package manager ("BlenderFoundation.Blender"),
//!   checked with `winget show --id <id> --exact`. Install is drawn only for an entry that has one.
//! - `download`: the project's own download page, when it has one apart from its home page.

use serde::Deserialize;

/// How to recognise an installed program (see the module's header).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Detect {
    /// A regular expression for Windows' name of the installed program, letter case ignored.
    #[serde(default)]
    pub display_name: Option<String>,
    /// The program's own file, inside its install folder: one path or several.
    #[serde(default, deserialize_with = "one_or_many")]
    pub exe: Vec<String>,
    /// A regular expression for the name of its Start-menu shortcut, letter case ignored, read
    /// when Windows' record gives no program file (2026-10-19, software/shortcuts.rs). Left out,
    /// `display_name` is used.
    #[serde(default)]
    pub shortcut: Option<String>,
}

/// `"a"` or `["a", "b"]`, read as a list.
fn one_or_many<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match Option::<OneOrMany>::deserialize(d)? {
        None => Vec::new(),
        Some(OneOrMany::One(s)) => vec![s],
        Some(OneOrMany::Many(v)) => v,
    })
}

/// An external entry loaded from `data/external/catalog.json`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    pub description: String,
    /// The project's (or the service's) own website.
    pub url: String,
    /// Software only; services leave this empty.
    #[serde(default)]
    pub license: String,
    /// Software only; services leave this empty. "web" alone means it runs in a browser.
    #[serde(default)]
    pub platforms: Vec<String>,
    /// Optional download size hint (e.g. "~350MB"). Software only.
    #[serde(default)]
    pub size: String,
    /// The project's own download page, when it is not its home page.
    #[serde(default)]
    pub download: Option<String>,
    /// How to recognise it installed on this computer (Windows).
    #[serde(default)]
    pub detect: Option<Detect>,
    /// Its Steam app id, when Steam can install it too.
    #[serde(default)]
    pub steam_appid: Option<u32>,
    /// Its winget package id.
    #[serde(default)]
    pub winget: Option<String>,
    /// The licence's own page, when the catalog has it (shown in the consent step).
    #[serde(default)]
    pub license_url: Option<String>,
    /// Its licence is not a free-software licence (VS Code, Paint.NET, Zed's terms): winget runs its
    /// own installer with its window (`--interactive`), so the maker's licence page appears as the
    /// maker intended (the 2026-10-19 finding, judgement call 2).
    #[serde(default)]
    pub install_interactive: bool,
    /// Why there is no Install for it, in plain words naming whose rule it is (DaVinci Resolve:
    /// the maker's own registration form). Shown in its details.
    #[serde(default)]
    pub website_only: Option<String>,
    /// Category name, populated during loading from the parent category.
    #[serde(skip)]
    pub category: String,
    /// Category id ("3d-modeling"), populated during loading: the hue of its tiles and the filter.
    #[serde(skip)]
    pub category_id: String,
    /// The file types the category's programs open (".blend", ".fbx"), from the parent category.
    #[serde(skip)]
    pub extensions: Vec<String>,
    /// Kind id from the parent category: "software" or "service".
    #[serde(skip)]
    pub kind: String,
    /// A help service's part of the Apps page, from the parent category (2026-10-19): "help" for
    /// urgent and protective needs (emergency, health, legal), "services" for everyday ones (food,
    /// housing, work, learning). Empty for software.
    #[serde(skip)]
    pub section: String,
}

impl CatalogEntry {
    /// Whether it is a program (rather than a help service).
    pub fn is_software(&self) -> bool {
        self.kind == "software"
    }

    /// Whether it runs only in a web browser (nothing to download or install).
    pub fn web_only(&self) -> bool {
        !self.platforms.is_empty() && self.platforms.iter().all(|p| p == "web")
    }

    /// Whether Install is offered for it: a program to download (not one that runs in a browser)
    /// with a winget package and no reason to send people to its maker's site instead. Whether it
    /// is already on this computer is the page's to check.
    pub fn installable(&self) -> bool {
        self.is_software() && !self.web_only() && self.winget.is_some() && self.website_only.is_none()
    }

    /// The page its download button opens: its download page, else its website.
    pub fn download_page(&self) -> &str {
        self.download.as_deref().filter(|u| !u.is_empty()).unwrap_or(&self.url)
    }

    /// Its website's host, for a line under a service's name ("www.211.org" is "211.org").
    pub fn host(&self) -> String {
        let rest = self.url.split("://").nth(1).unwrap_or(&self.url);
        let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        host.strip_prefix("www.").unwrap_or(host).to_string()
    }
}

/// The JSON shape of `data/external/catalog.json`: categories, each tagged with a kind, each
/// holding entries.
#[derive(Deserialize)]
struct Catalog {
    categories: Vec<CatalogCategory>,
}

#[derive(Deserialize)]
struct CatalogCategory {
    #[serde(default)]
    id: String,
    name: String,
    /// "software" or "service"; see the `kinds` array in the file.
    #[serde(default)]
    kind: String,
    #[serde(default)]
    extensions: Vec<String>,
    /// A service category's part of the Apps page: "help" or "services".
    #[serde(default)]
    section: String,
    #[serde(default)]
    entries: Vec<CatalogEntry>,
}

/// The catalog's entries in one flat list, each tagged with its category and kind. `Err` carries
/// the parser's words.
pub fn parse(text: &str) -> Result<Vec<CatalogEntry>, String> {
    let catalog: Catalog = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for cat in catalog.categories {
        for mut entry in cat.entries {
            entry.category = cat.name.clone();
            entry.category_id = if cat.id.is_empty() { cat.name.to_lowercase() } else { cat.id.clone() };
            entry.extensions = cat.extensions.clone();
            entry.kind = cat.kind.clone();
            entry.section = cat.section.clone();
            out.push(entry);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE CATALOG'S NEW FIELDS LOAD, FROM THE SHIPPED FILE: Blender's entry says how to recognise
    /// it (Windows' name, its program), its Steam app id, its winget package and its download page;
    /// GIMP's lists two programs (its versions name theirs differently); every entry keeps its
    /// category's id, name and file types; a help service has none of the new fields; every winget
    /// id and every name pattern is well formed; and no entry is listed twice.
    ///
    /// Seen red 2026-10-19 with the category's file types not carried onto its entries (`parse`
    /// leaving `entry.extensions` empty): "its category's file types" failed. Its Help and Services
    /// half (the same day) seen red with `parse` not carrying the category's `section`:
    /// "Appropedia (Water): section \"\"".
    #[test]
    fn the_catalogs_new_fields_load() {
        let text = std::fs::read_to_string("data/external/catalog.json").expect("the catalog is there");
        let all = parse(&text).expect("the shipped catalog parses");
        let blender = all.iter().find(|e| e.name == "Blender").expect("Blender");
        let detect = blender.detect.as_ref().expect("Blender says how to recognise it");
        assert!(detect.display_name.is_some() && detect.exe == vec!["blender.exe".to_string()], "{detect:?}");
        assert_eq!(blender.steam_appid, Some(365670));
        assert_eq!(blender.winget.as_deref(), Some("BlenderFoundation.Blender"));
        assert_eq!((blender.category_id.as_str(), blender.kind.as_str()), ("3d-modeling", "software"));
        assert!(blender.extensions.iter().any(|e| e == ".blend"), "its category's file types");
        assert_ne!(blender.download_page(), blender.url, "its own download page");
        let gimp = all.iter().find(|e| e.name == "GIMP").unwrap();
        assert!(gimp.detect.as_ref().is_some_and(|d| d.exe.len() >= 2), "GIMP's versions name their programs differently");
        // The 2026-10-19 install finding (2.9): the licences as the programs are downloaded, Zed's
        // Windows build, the three whose installer shows its own licence page, and the five that
        // are website only, each saying why.
        let by = |n: &str| all.iter().find(|e| e.name == n).unwrap_or_else(|| panic!("{n} is in the catalog"));
        for (name, not) in [("Paint.NET", "MIT"), ("Aseprite", "GPL"), ("VS Code", "MIT")] {
            assert!(!by(name).license.starts_with(not), "{name}'s licence as downloaded is not {not}: {:?}", by(name).license);
        }
        assert!(by("Zed").platforms.iter().any(|p| p == "windows"), "Zed has a Windows build");
        for name in ["VS Code", "Paint.NET", "Zed"] {
            assert!(by(name).install_interactive && by(name).installable(), "{name}: installable, its own installer shown");
        }
        assert!(!by("GIMP").install_interactive, "a free-software program installs quietly");
        for name in ["DaVinci Resolve", "Aseprite", "Ardour", "Vectr", "Boxy SVG"] {
            let e = by(name);
            assert!(e.website_only.as_deref().is_some_and(|w| w.contains("own")) && e.winget.is_none() && !e.installable(), "{name} is website only, saying whose rule");
        }
        let service = all.iter().find(|e| e.name == "Feeding America").unwrap();
        assert!(service.detect.is_none() && service.winget.is_none() && service.steam_appid.is_none() && !service.is_software());
        assert_eq!(service.host(), "feedingamerica.org");
        // Every help service is in the Apps page's Help or its Services (2026-10-19), as data:
        // urgent and protective needs in Help, everyday ones in Services; software in neither.
        for e in &all {
            let want: &[&str] = if e.is_software() { &[""] } else { &["help", "services"] };
            assert!(want.contains(&e.section.as_str()), "{} ({}): section {:?}", e.name, e.category, e.section);
        }
        assert_eq!((by("Crisis Text Line").section.as_str(), by("FEMA").section.as_str(), by("Legal Aid").section.as_str()), ("help", "help", "help"));
        assert_eq!((service.section.as_str(), by("Khan Academy").section.as_str()), ("services", "services"));
        assert!(by("DaVinci Resolve").detect.as_ref().is_some_and(|d| d.shortcut.is_some()), "Resolve is found by its Start-menu shortcut");
        for e in &all {
            if let Some(id) = &e.winget {
                assert!(crate::software::install::package_id_ok(id), "{}: winget id {id:?}", e.name);
            }
            for re in e.detect.iter().flat_map(|d| d.display_name.iter().chain(d.shortcut.iter())) {
                assert!(regex::RegexBuilder::new(re).case_insensitive(true).build().is_ok(), "{}: pattern {re:?}", e.name);
            }
            assert!(!(e.winget.is_some() && e.web_only()), "{}: a web app has nothing to install", e.name);
        }
        let mut seen = std::collections::HashSet::new();
        for e in &all {
            assert!(seen.insert((e.category_id.clone(), e.name.clone())), "{} is listed twice in {}", e.name, e.category);
        }
    }

    /// ONE PROGRAM OR SEVERAL: "exe" as one string or a list; left out, none.
    ///
    /// Seen red 2026-10-19 with `One(s)` read as an empty list: "one string is one program" (left:
    /// [], right: ["blender.exe"]).
    #[test]
    fn one_program_or_several() {
        let one: Detect = serde_json::from_str(r#"{"display_name":"^Blender","exe":"blender.exe"}"#).unwrap();
        assert_eq!(one.exe, vec!["blender.exe".to_string()], "one string is one program");
        let many: Detect = serde_json::from_str(r#"{"exe":["bin/gimp-3.exe","bin/gimp-2.10.exe"]}"#).unwrap();
        assert_eq!(many.exe.len(), 2);
        let none: Detect = serde_json::from_str(r#"{"display_name":"^X"}"#).unwrap();
        assert!(none.exe.is_empty());
    }
}
