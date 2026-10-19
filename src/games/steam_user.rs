//! What Steam keeps on this computer about the PERSON, not the game (2026-10-19): how long they
//! have played each game and when they last did, and the screenshots they took. Read from Steam's
//! own files, never written to, on the page's worker thread when it opens and on Refresh.
//!
//! Steam keeps one folder per Steam account that has signed in on this computer,
//! `userdata\<account id>`, so a family's shared computer holds several people's records. ONE
//! account's are read, never several mixed (the 2026-10-19 finding,
//! docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md, judgement call 3): the
//! one signed in to Steam now (the registry's `ActiveProcess > ActiveUser`, while Steam runs);
//! else the one `config\loginusers.vdf` marks `"MostRecent" "1"` (or, where none is marked, the
//! latest `"Timestamp"`), its 64-bit Steam id turned into the account id the folder is named by
//! (the low 32 bits); else the only account folder there is. With several and no record of whose,
//! none.
//!
//! - PLAYTIME: `userdata\<id>\config\localconfig.vdf`, under `UserLocalConfigStore > Software >
//!   Valve > Steam > apps > <appid>`: `Playtime` (minutes) and `LastPlayed` (Unix time). That file
//!   also holds Steam's own sign-in records; only those two numbers per game are kept, and the
//!   rest is dropped as soon as the file is read. Nothing of it is shown, logged or sent.
//! - SCREENSHOTS: `userdata\<id>\760\remote\<appid>\screenshots\*.jpg`, the pictures Steam saved
//!   when the person pressed its screenshot key, with Steam's own small copies in `thumbnails\`.
//!   Listed newest first; at most [`MAX_SCREENSHOTS`] are kept on the list. Only the list is read
//!   here: the pictures themselves are read and shrunk when they are drawn
//!   (widgets/thumbnails.rs, on its own workers).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::keyvalues::{self, Value};
use super::{list_dir, read_small};

/// The difference between a 64-bit Steam id and the account id its folder is named by.
const STEAM_ID_BASE: u64 = 76_561_197_960_265_728;
/// At most this many screenshots of one game are listed (the newest).
pub const MAX_SCREENSHOTS: usize = 500;

/// The Steam account whose records are read.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    /// Its account id: the number its `userdata` folder is named by.
    pub id: u32,
    /// `userdata\<id>`.
    pub folder: PathBuf,
}

/// The account id in a 64-bit Steam id (its low 32 bits), when it is one.
pub fn account_id(steam_id: &str) -> Option<u32> {
    let n: u64 = steam_id.trim().parse().ok()?;
    (n >= STEAM_ID_BASE).then(|| (n - STEAM_ID_BASE) as u32).filter(|id| *id != 0)
}

/// The account whose records the page reads, when Steam has one here: the one signed in to Steam
/// right now (`active_user`, from the registry while Steam runs), else the one loginusers.vdf says
/// signed in last, else the only account folder there is. With several account folders and no
/// record of which is the person's (a shared computer), none: one person's playtime and
/// screenshots are never shown as another's, and never mixed (the 2026-10-19 finding, 3).
pub fn account(steam_root: &Path, active_user: Option<u32>) -> Option<Account> {
    let userdata = steam_root.join("userdata");
    let folder_of = |id: u32| Some(userdata.join(id.to_string())).filter(|p| p.is_dir()).map(|folder| Account { id, folder });
    if let Some(found) = active_user.and_then(folder_of) {
        return Some(found);
    }
    if let Some(found) = most_recent_login(steam_root).and_then(folder_of) {
        return Some(found);
    }
    // No record names one that is here: the account folders themselves ("0" is Steam's own, not
    // an account), only when there is exactly one.
    let ids: Vec<u32> = list_dir(&userdata).iter().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).filter_map(|e| e.file_name().to_str().and_then(|s| s.parse::<u32>().ok())).filter(|id| *id != 0).collect();
    match ids.as_slice() {
        [only] => folder_of(*only),
        _ => None,
    }
}

/// The account `config\loginusers.vdf` says signed in last: the one marked MostRecent, else the
/// latest Timestamp.
fn most_recent_login(steam_root: &Path) -> Option<u32> {
    let text = read_small(&steam_root.join("config").join("loginusers.vdf"))?;
    let kv = keyvalues::parse(&text).ok()?;
    let users = keyvalues::find(&kv, "users")?.block()?;
    let mut best: Option<(bool, u64, u32)> = None;
    for (steam_id, user) in users {
        let (Some(id), Some(user)) = (account_id(steam_id), user.block()) else { continue };
        let recent = keyvalues::find_text(user, "MostRecent").map(str::trim) == Some("1");
        let stamp = keyvalues::find_text(user, "Timestamp").and_then(|t| t.trim().parse().ok()).unwrap_or(0);
        if best.is_none_or(|b| (recent, stamp) > (b.0, b.1)) {
            best = Some((recent, stamp, id));
        }
    }
    best.map(|b| b.2)
}

/// How long a game has been played, and when last.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Played {
    /// Minutes.
    pub minutes: Option<u64>,
    /// Unix seconds.
    pub last: Option<u64>,
}

/// Every game's playtime in the account's `localconfig.vdf`, by appid. Empty when it does not read.
pub fn playtimes(account: &Account) -> HashMap<String, Played> {
    let mut out = HashMap::new();
    let Some(text) = read_small(&account.folder.join("config").join("localconfig.vdf")) else { return out };
    let Ok(kv) = keyvalues::parse(&text) else { return out };
    let apps = ["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"].iter().try_fold(&kv[..], |entries, key| keyvalues::find(entries, key).and_then(Value::block));
    for (appid, app) in apps.unwrap_or(&[]) {
        let Some(app) = app.block() else { continue };
        if appid.is_empty() || !appid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let number = |key: &str| keyvalues::find_text(app, key).and_then(|t| t.trim().parse::<u64>().ok());
        let played = Played { minutes: number("Playtime"), last: number("LastPlayed").filter(|t| *t > 0) };
        if played != Played::default() {
            out.insert(appid.clone(), played);
        }
    }
    out
}

/// One screenshot: the picture, and Steam's own small copy of it when there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct Screenshot {
    pub path: PathBuf,
    pub thumb: Option<PathBuf>,
}

/// A game's screenshots: newest first, at most [`MAX_SCREENSHOTS`]; `total` counts them all.
#[derive(Debug, Clone, PartialEq)]
pub struct Screenshots {
    /// The folder they are in (Show in folder).
    pub folder: PathBuf,
    pub shots: Vec<Screenshot>,
    pub total: usize,
}

/// Whether a file is a screenshot Steam saves (a JPEG, or a PNG some games write).
fn is_shot(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png")
}

/// The account's screenshots of game `appid`, when it has any.
pub fn screenshots(account: &Account, appid: &str) -> Option<Screenshots> {
    let folder = account.folder.join("760").join("remote").join(appid).join("screenshots");
    let mut found: Vec<(std::time::SystemTime, String, PathBuf)> = Vec::new();
    for entry in list_dir(&folder) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_shot(&name) || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let when = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        found.push((when, name, entry.path()));
    }
    if found.is_empty() {
        return None;
    }
    // Newest first: by when it was saved, and Steam's names (the date and time) when that ties.
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    let total = found.len();
    let thumbs: HashSet<String> = list_dir(&folder.join("thumbnails")).iter().map(|e| e.file_name().to_string_lossy().to_lowercase()).collect();
    let shots = found
        .into_iter()
        .take(MAX_SCREENSHOTS)
        .map(|(_, name, path)| {
            let thumb = thumbs.contains(&name.to_lowercase()).then(|| folder.join("thumbnails").join(&name));
            Screenshot { path, thumb }
        })
        .collect();
    Some(Screenshots { folder, shots, total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures;

    /// THE ACCOUNT, ITS PLAYTIMES AND ITS SCREENSHOTS: loginusers.vdf names two accounts, the one
    /// marked MostRecent is read (not the one with the later Timestamp); its localconfig.vdf gives
    /// each game's minutes and last-played time, and nothing else of the file is kept; its
    /// screenshots are listed newest first, each with Steam's small copy where one is there.
    ///
    /// Seen red 2026-10-19 with `most_recent_login` taking the latest Timestamp before MostRecent:
    /// "the account marked MostRecent" (left: 2002, right: 1001).
    #[test]
    fn the_account_its_playtimes_and_its_screenshots() {
        let fx = fixtures::steam("steam_user");
        assert_eq!(account(&fx.root, Some(2002)).map(|a| a.id), Some(2002), "the account signed in now, while Steam runs");
        assert_eq!(account(&fx.root, Some(777)).map(|a| a.id), Some(fixtures::ACCOUNT), "one signed in with no folder here: the login record's");
        let account = account(&fx.root, None).expect("an account");
        assert_eq!(account.id, fixtures::ACCOUNT, "the account marked MostRecent");
        let played = playtimes(&account);
        assert_eq!(played.get("1000010"), Some(&Played { minutes: Some(754), last: Some(1_790_000_000) }));
        assert_eq!(played.get("1000030"), Some(&Played { minutes: Some(42), last: None }), "a game never started has no last-played time");
        assert!(!played.contains_key("SharedAuth") && played.len() == 2, "only the games' two numbers: {played:?}");

        let shots = screenshots(&account, "1000010").expect("Orbital Farmstead has screenshots");
        assert_eq!(shots.total, 3);
        let names: Vec<String> = shots.shots.iter().map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["20261003120000_1.jpg", "20261002120000_1.jpg", "20261001120000_1.jpg"], "newest first");
        assert!(shots.shots[0].thumb.is_some() && shots.shots[2].thumb.is_none(), "Steam's small copy where there is one");
        assert!(screenshots(&account, "1000030").is_none(), "a game with none has none");
    }

    /// NO LOGIN RECORD: the only account folder is used ("0" is Steam's own, not an account); with
    /// two and nothing saying whose, neither (a shared computer: never one person's records shown
    /// as another's). A 64-bit Steam id becomes the account id its folder is named by.
    ///
    /// Seen red 2026-10-19 with folder "0" counted as an account: "folder 0 is not an account"
    /// (left: None, right: Some(31)), two folders then found and neither taken.
    #[test]
    fn no_login_record() {
        let root = crate::test_temp::dir("steam_user_no_login");
        std::fs::create_dir_all(root.join("userdata").join("0")).unwrap();
        std::fs::create_dir_all(root.join("userdata").join("31").join("config")).unwrap();
        assert_eq!(account(&root, None).map(|a| a.id), Some(31), "folder 0 is not an account");
        std::fs::create_dir_all(root.join("userdata").join("32")).unwrap();
        assert_eq!(account(&root, None), None, "two accounts and no record of whose: neither");
        assert_eq!(account_id("76561198002590885"), Some(42_325_157));
        assert_eq!(account_id("12"), None);
    }
}
