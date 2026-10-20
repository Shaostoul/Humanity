//! The Tools page's readers (2026-10-19; the operator: "What if we make the tools page like the
//! games page? Then what if through that interface users can install/launch/go to the website(s)
//! for the various non-game apps?"). The page is src/gui/pages/tools.rs.
//!
//! WHAT IS DONE HERE, and no more:
//! - READ which catalog programs are installed on this computer, from what Windows records about
//!   installed programs (its installed-programs list and App Paths, `records`) and from Steam's own
//!   install records (the Games page's Steam reader, games/steam.rs), matched to the catalog's
//!   entries by their DATA fields (`catalog`: `detect`, `steam_appid`). Read only: no key or file is
//!   written. Read on the page's worker thread when the page opens and on Refresh, never every frame.
//! - LAUNCH an installed program only when the person presses Launch, through the Games page's one
//!   launcher (games/launch.rs: its own program, or Steam's own link for a copy Steam installed).
//! - INSTALL through winget, Windows' own package manager (`install`): on a click, only after the
//!   person agrees to the package's licence and agreements in HumanityOS's own window, that exact
//!   package from winget's own source, never accepting a source agreement for anyone. The rules are
//!   the dated finding docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md.
//! - PICTURES: an installed program shows the icon in its own file (games/art.rs, the program
//!   opened as data, never run). No logo is fetched from any website.
//!
//! Nothing read here leaves this computer.
//!
//! DESKTOP ONLY, and the finding of what is installed is Windows only: a web page cannot see what
//! is installed on the computer it runs on, and the records read are Windows' own.

pub mod catalog;
pub mod install;
pub mod matcher;
pub mod records;

use std::path::PathBuf;

pub use catalog::ToolEntry;
pub use matcher::{Installed, Via};

/// Where the readers look: this computer's records (`Sources::this_computer`, on the worker), or a
/// test's own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sources {
    /// What Windows records about installed programs.
    pub records: records::Records,
    /// Steam's own folder, when Steam is installed.
    pub steam_root: Option<PathBuf>,
}

impl Sources {
    /// Read this computer's records (the registry, read only). Slow enough to belong on a worker.
    pub fn this_computer() -> Self {
        #[cfg(windows)]
        {
            Self { records: records::this_computer(), steam_root: crate::games::steam::root_on_this_computer() }
        }
        #[cfg(not(windows))]
        {
            Self::default()
        }
    }
}

/// What was found: for each catalog entry, in the catalog's order, how it is installed, or `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    pub installed: Vec<Option<Installed>>,
}

impl Found {
    /// How entry `i` is installed, when it is.
    pub fn get(&self, i: usize) -> Option<&Installed> {
        self.installed.get(i).and_then(Option::as_ref)
    }

    /// How many entries are installed.
    pub fn count(&self) -> usize {
        self.installed.iter().filter(|i| i.is_some()).count()
    }
}

/// Which of `catalog`'s programs are installed, by `sources`. Help services are never installed.
pub fn read(sources: &Sources, catalog: &[ToolEntry]) -> Found {
    let steam = sources.steam_root.as_deref().filter(|p| p.is_dir()).map(crate::games::steam::installed).unwrap_or_default();
    let installed = catalog.iter().map(|e| if e.is_software() { matcher::find_with_icon(e, &sources.records, &steam) } else { None }).collect();
    Found { installed }
}

/// The detector against THIS computer, printed for a look by hand (the other tests read only their
/// own records): `node scripts/lib-tests.js software::this_computer --ignored --nocapture`. Ignored
/// because what it finds depends on the machine. It reads only, as the page does, and starts
/// nothing.
#[cfg(test)]
mod this_computer {
    #[test]
    #[ignore]
    fn print_what_the_tools_page_would_find_here() {
        let catalog = super::catalog::parse(&std::fs::read_to_string("data/external/catalog.json").unwrap()).unwrap();
        let sources = super::Sources::this_computer();
        println!("Windows' installed-programs list: {} programs; App Paths: {} programs; Steam: {:?}", sources.records.uninstall.len(), sources.records.app_paths.len(), sources.steam_root);
        let found = super::read(&sources, &catalog);
        let software: Vec<_> = catalog.iter().enumerate().filter(|(_, e)| e.is_software()).collect();
        println!("{} of {} catalog programs installed here:", found.count(), software.len());
        for (i, e) in &software {
            match found.get(*i) {
                None => println!("  -  {}{}", e.name, if e.detect.is_none() && e.steam_appid.is_none() { "  (nothing to recognise it by)" } else { "" }),
                Some(f) => println!(
                    "  OK {} | via {:?} | Windows' name {:?} | version {:?} | folder {:?} | program {:?} | launch {:?} | icon {}",
                    e.name,
                    f.via,
                    f.name_in_windows,
                    f.version,
                    f.folder,
                    f.program,
                    f.launch,
                    f.icon.as_ref().map(|i| format!("{}x{}", i.width, i.height)).unwrap_or_else(|| "-".into())
                ),
            }
        }
    }
}
