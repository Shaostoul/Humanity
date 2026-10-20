//! The links a game's details offer (2026-10-19), each opened only when the person clicks it:
//!
//! - IN THE STORE'S OWN APP: `steam://` links, sent through `games::launch` like Play and "Open in
//!   Steam" (launch.rs's `LINK_SCHEMES` already allows `steam://`). Valve's client opens the page.
//! - ON THE WEB: a page in the person's own browser, through egui's `open_url` as the chat's links
//!   are, and only on a host in [`WEB_HOSTS`].
//!
//! One line of data per link ([`STEAM_LINKS`], [`GOG_LINKS`]), so taking one out is deleting its
//! line. Which links are offered is the dated finding's
//! (docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md, section 4 and judgement
//! call 4): the store's page, the Steam community hub, PCGamingWiki through its own redirect by
//! Steam App ID or GOG ID, and SteamDB; links only, never fetched, previewed or read in the
//! background (SteamDB and PCGamingWiki ask programs not to).
//!
//! A program from the external catalog that Steam installed (Blender) gets the same Steam links,
//! [`steam_program_links`] and [`open_in_steam`] (2026-10-19), beside its maker's own website.
//!
//! Epic and GOG store pages are not linked: they are addressed by a name ("slug") the stores'
//! local records do not carry, and a guessed address could open the wrong game. An Epic game's
//! "Open in the Epic Games Store" (games/mod.rs) opens the store's front page instead.

use super::{Game, Launch, Store};

/// Where a link opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opens {
    /// In the store's own app, through `games::launch`.
    InStore(Launch),
    /// A web page, in the person's browser.
    Web(String),
}

/// One link: its words, its hover words, and where it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameLink {
    pub label: &'static str,
    pub tip: &'static str,
    pub opens: Opens,
}

/// Whether a link opens in the store's app or on the web.
#[derive(Clone, Copy)]
enum Kind {
    InSteam,
    Web,
}

/// A Steam game's links, one line each: its words, its hover words, where it opens, its address
/// (`{id}` is where the game's appid goes).
const STEAM_LINKS: &[(&str, &str, Kind, &str)] = &[
    ("Store page", "Opens its store page in Steam.", Kind::InSteam, "steam://store/{id}"),
    ("Community hub", "Opens its community hub in Steam: guides, discussions, screenshots and the Workshop.", Kind::InSteam, "steam://url/GameHub/{id}"),
    ("Steam store on the web", "Opens its Steam store page in your web browser.", Kind::Web, "https://store.steampowered.com/app/{id}"),
    ("PCGamingWiki", "Opens its PCGamingWiki page in your web browser: fixes, settings and where its files are, written by players.", Kind::Web, "https://www.pcgamingwiki.com/api/appid.php?appid={id}"),
    ("SteamDB", "Opens its SteamDB page in your web browser: its updates, prices and player counts, from a site not run by Valve.", Kind::Web, "https://steamdb.info/app/{id}/"),
];

/// A GOG game's links, the same way (`{id}` is its GOG game id).
const GOG_LINKS: &[(&str, &str, Kind, &str)] = &[
    ("PCGamingWiki", "Opens its PCGamingWiki page in your web browser: fixes, settings and where its files are, written by players.", Kind::Web, "https://www.pcgamingwiki.com/api/gog.php?page={id}"),
];

/// A Workshop item's own page, in Steam.
const WORKSHOP_ITEM: &str = "steam://url/CommunityFilePage/{id}";

/// The only hosts a web link here may open. A link whose host is not one of these is not opened.
pub const WEB_HOSTS: [&str; 3] = ["store.steampowered.com", "www.pcgamingwiki.com", "steamdb.info"];

/// Whether `url` is a web address this page may open: https, on a host in [`WEB_HOSTS`].
pub fn web_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    WEB_HOSTS.contains(&host)
}

/// The id goes into a link, so only an id of digits is used.
fn digits(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit())
}

/// `game`'s links, in the order shown: a Steam game's and a GOG game's (Epic's and an added
/// game's have none).
pub fn links(game: &Game) -> Vec<GameLink> {
    let lines = match game.store {
        Store::Steam => STEAM_LINKS,
        Store::Gog => GOG_LINKS,
        Store::Epic | Store::Added => &[],
    };
    if !digits(&game.id) {
        return Vec::new();
    }
    build(lines, &game.id)
}

/// `lines` with `id` put in.
fn build(lines: &[(&'static str, &'static str, Kind, &str)], id: &str) -> Vec<GameLink> {
    lines
        .iter()
        .map(|&(label, tip, kind, address)| {
            let address = address.replace("{id}", id);
            let opens = match kind {
                Kind::InSteam => Opens::InStore(Launch::Link(address)),
                Kind::Web => Opens::Web(address),
            };
            GameLink { label, tip, opens }
        })
        .collect()
}

/// The links a program installed through Steam gets beside its maker's own website (2026-10-19,
/// the operator: "blender shows it is through Steam but, it doesn't have a link to the steam
/// page"): its store page in Steam and on the web, and its community hub. `appid` is its Steam app
/// id; one that is not a number has none.
pub fn steam_program_links(appid: &str) -> Vec<GameLink> {
    if !digits(appid) {
        return Vec::new();
    }
    let wanted = ["Store page", "Community hub", "Steam store on the web"];
    build(&STEAM_LINKS.iter().filter(|l| wanted.contains(&l.0)).copied().collect::<Vec<_>>(), appid)
}

/// "Open in Steam" for Steam app `appid`: its page in the person's Steam library, as a Steam game's
/// own button opens (games/mod.rs `store_button`).
pub fn open_in_steam(appid: &str) -> Option<Launch> {
    digits(appid).then(|| Launch::Link(format!("steam://nav/games/details/{appid}")))
}

/// A Workshop item's own page in Steam, when its id is a number.
pub fn workshop_item(id: &str) -> Option<Launch> {
    digits(id).then(|| Launch::Link(WORKSHOP_ITEM.replace("{id}", id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::{launch::LINK_SCHEMES, Details, State};

    fn game(store: Store, id: &str) -> Game {
        Game { store, id: id.into(), name: "x".into(), folder: Default::default(), folder_here: false, state: State::Ready, play: None, show: Default::default(), details: Details::default() }
    }

    /// EVERY LINK IS EXACTLY ITS ADDRESS, AND EVERY WEB ONE IS ON THE LIST OF HOSTS: a Steam game's
    /// two links in Steam go through the launcher's own schemes and its three web links are on
    /// `WEB_HOSTS`; a GOG game's one is PCGamingWiki's redirect by its GOG id; an Epic game, or an
    /// id that is not a number, has none (no guessed store page); a host not on the list, or plain
    /// http, is refused.
    ///
    /// Seen red 2026-10-19 with SteamDB's line written "https://www.steamdb.info/app/{id}/" (a
    /// host not on the list): the addresses compared, left ending
    /// Web("https://www.steamdb.info/app/1000010/").
    #[test]
    fn every_link_is_its_address_and_on_the_list() {
        let gog = links(&game(Store::Gog, "1207658924"));
        assert_eq!(gog.iter().map(|l| &l.opens).collect::<Vec<_>>(), vec![&Opens::Web("https://www.pcgamingwiki.com/api/gog.php?page=1207658924".into())], "a GOG game: PCGamingWiki by its GOG id");
        assert!(gog.iter().all(|l| matches!(&l.opens, Opens::Web(u) if web_allowed(u))));
        let steam = links(&game(Store::Steam, "1000010"));
        let opens: Vec<&Opens> = steam.iter().map(|l| &l.opens).collect();
        assert_eq!(
            opens,
            vec![
                &Opens::InStore(Launch::Link("steam://store/1000010".into())),
                &Opens::InStore(Launch::Link("steam://url/GameHub/1000010".into())),
                &Opens::Web("https://store.steampowered.com/app/1000010".into()),
                &Opens::Web("https://www.pcgamingwiki.com/api/appid.php?appid=1000010".into()),
                &Opens::Web("https://steamdb.info/app/1000010/".into()),
            ]
        );
        for link in &steam {
            match &link.opens {
                Opens::InStore(Launch::Link(l)) => assert!(LINK_SCHEMES.iter().any(|s| l.starts_with(s)), "{l} goes through the launcher's schemes"),
                Opens::Web(u) => assert!(web_allowed(u), "every web link is on the list of hosts: {u}"),
                other => panic!("not a link: {other:?}"),
            }
        }
        assert!(links(&game(Store::Epic, "Lantern")).is_empty() && links(&game(Store::Added, "1")).is_empty(), "no guessed store pages");
        assert!(links(&game(Store::Gog, "12 OR 1")).is_empty());
        assert!(links(&game(Store::Steam, "10 OR 1")).is_empty(), "an id that is not a number has none");
        assert!(!web_allowed("https://evil.example/app/1") && !web_allowed("http://steamdb.info/app/1/") && !web_allowed("https://steamdb.info.evil.example/"));
        assert_eq!(workshop_item("3000001"), Some(Launch::Link("steam://url/CommunityFilePage/3000001".into())));
        assert_eq!(workshop_item("../x"), None);
    }

    /// A PROGRAM STEAM INSTALLED GETS STEAM'S LINKS (2026-10-19, Blender, app 365670): Open in Steam
    /// is its page in the Steam library, and its links are its store page in Steam, its community
    /// hub and its store page on the web, each exactly a Steam game's address and every web one on
    /// the list of hosts; an id that is not a number has none.
    ///
    /// Seen red 2026-10-19 with the links filtered by a label no line has ("Store" for "Store
    /// page"): "Blender's Steam links" got two of the three.
    #[test]
    fn a_program_steam_installed_gets_steams_links() {
        assert_eq!(open_in_steam("365670"), Some(Launch::Link("steam://nav/games/details/365670".into())));
        let opens: Vec<Opens> = steam_program_links("365670").into_iter().map(|l| l.opens).collect();
        assert_eq!(
            opens,
            vec![
                Opens::InStore(Launch::Link("steam://store/365670".into())),
                Opens::InStore(Launch::Link("steam://url/GameHub/365670".into())),
                Opens::Web("https://store.steampowered.com/app/365670".into()),
            ],
            "Blender's Steam links"
        );
        assert!(opens.iter().all(|o| match o {
            Opens::Web(u) => web_allowed(u),
            Opens::InStore(Launch::Link(l)) => LINK_SCHEMES.iter().any(|s| l.starts_with(s)),
            _ => false,
        }));
        assert!(steam_program_links("36 OR 1").is_empty() && open_in_steam("x").is_none());
    }
}
