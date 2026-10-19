//! A Steam game's achievements, read from the copies Steam keeps on this computer (2026-10-19),
//! never written to and never asked of Steam:
//!
//! - `appcache\stats\UserGameStatsSchema_<appid>.bin`: the game's achievements. Under `<appid> >
//!   stats`, each block whose `type` is achievements (written "ACHIEVEMENTS", "4", or `type_int`
//!   4) holds `bits`: one entry per achievement, numbered by its bit (or carrying a `bit`), with
//!   its `name` (the game's own id for it) and `display > name` and `display > desc` in each
//!   language (English is read), and `display > hidden` for one whose description the game keeps
//!   secret until it is unlocked.
//! - `appcache\stats\UserGameStats_<account id>_<appid>.bin`: this person's. Under `cache`, the
//!   block of the same number holds `data`, whose bits are the achievements unlocked, and
//!   `AchievementTimes`, when each was (Unix time, by bit).
//!
//! Both are binary KeyValues (binary_keyvalues.rs). No achievement's picture is shown: those are
//! on Valve's servers, and nothing here fetches anything.
//!
//! WRONG NUMBERS ARE WORSE THAN NONE. A schema that does not read, an achievement numbered past
//! its block's 32 bits, or a record of the person's that is there but does not read: the game
//! shows no achievements at all. A game whose person's record is simply not there (Steam has not
//! saved one on this computer) shows its achievements as not known, never as "0 unlocked".

use std::collections::HashMap;
use std::path::Path;

use super::binary_keyvalues;
use super::keyvalues::{find, find_text, Value};
use super::read_bytes;

/// The largest stats file read (the largest on the operator's computer was 1.6 MB).
const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// One achievement.
#[derive(Debug, Clone, PartialEq)]
pub struct Achievement {
    /// Its name, in English (the game's own id for it when it has no name).
    pub name: String,
    /// What it asks for, in English ("" when it says nothing).
    pub description: String,
    /// The game keeps its description secret until it is unlocked.
    pub hidden: bool,
    /// Whether the person has it: `None` when Steam has no record of theirs on this computer.
    pub unlocked: Option<bool>,
    /// When it was unlocked (Unix seconds), where Steam recorded it.
    pub unlocked_at: Option<u64>,
}

/// A game's achievements: the unlocked first, the most recent of them first, then the rest in the
/// game's own order.
#[derive(Debug, Clone, PartialEq)]
pub struct Achievements {
    pub list: Vec<Achievement>,
}

impl Achievements {
    pub fn total(&self) -> usize {
        self.list.len()
    }

    /// How many the person has unlocked, when that is known.
    pub fn unlocked(&self) -> Option<usize> {
        let known = self.list.iter().all(|a| a.unlocked.is_some());
        known.then(|| self.list.iter().filter(|a| a.unlocked == Some(true)).count())
    }
}

/// The English text of a `display` entry: `display > <key> > english`, or the text itself where a
/// file has it unnested.
fn english(display: &[(String, Value)], key: &str) -> Option<String> {
    let text = match find(display, key)? {
        Value::Text(t) => t.as_str(),
        Value::Block(langs) => find_text(langs, "english")?,
    };
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

/// Whether a stat block holds achievements.
fn is_achievements(stat: &[(String, Value)]) -> bool {
    let ty = find_text(stat, "type").map(str::trim);
    let ty_int = find_text(stat, "type_int").map(str::trim);
    matches!(ty, Some("ACHIEVEMENTS") | Some("4")) || ty_int == Some("4") || (ty.is_none() && find(stat, "bits").is_some())
}

/// One achievement as the schema defines it: its stat block, its bit, and its words.
struct Defined {
    stat: String,
    bit: u32,
    achievement: Achievement,
}

/// The achievements game `appid`'s schema defines, in its order. `None` when the file does not
/// read or is not this game's; `Some` of an empty list when it defines none.
fn defined(stats_dir: &Path, appid: &str) -> Option<Vec<Defined>> {
    let bytes = read_bytes(&stats_dir.join(format!("UserGameStatsSchema_{appid}.bin")), MAX_BYTES)?;
    let kv = binary_keyvalues::parse(&bytes).ok()?;
    let (root_id, root) = kv.first()?;
    if root_id != appid {
        return None;
    }
    let stats = find(root.block()?, "stats")?.block()?;
    let mut out = Vec::new();
    for (stat_id, stat) in stats {
        let Some(stat) = stat.block() else { continue };
        if !is_achievements(stat) {
            continue;
        }
        let Some(bits) = find(stat, "bits").and_then(Value::block) else { continue };
        for (bit_key, entry) in bits {
            let entry = entry.block()?;
            let bit: u32 = find_text(entry, "bit").unwrap_or(bit_key).trim().parse().ok()?;
            // A block's achievements are the bits of one 32-bit number: a bit past that is a
            // file this reader does not understand.
            if bit >= 32 {
                return None;
            }
            let id = find_text(entry, "name").map(str::trim).unwrap_or("").to_string();
            let display = find(entry, "display").and_then(Value::block).unwrap_or(&[]);
            let name = english(display, "name").unwrap_or_else(|| if id.is_empty() { format!("Achievement {}", out.len() + 1) } else { id.clone() });
            let description = english(display, "desc").unwrap_or_default();
            let hidden = find_text(display, "hidden").map(str::trim) == Some("1");
            out.push(Defined { stat: stat_id.clone(), bit, achievement: Achievement { name, description, hidden, unlocked: None, unlocked_at: None } });
        }
    }
    Some(out)
}

/// What the person's record says: per stat block, its bits and the times by bit. `None` when it is
/// there but does not read; `Some(None)` when it is not there.
#[allow(clippy::type_complexity)]
fn record(stats_dir: &Path, account: Option<u32>, appid: &str) -> Option<Option<HashMap<String, (u32, HashMap<u32, u64>)>>> {
    let Some(account) = account else { return Some(None) };
    let path = stats_dir.join(format!("UserGameStats_{account}_{appid}.bin"));
    if !path.is_file() {
        return Some(None);
    }
    let bytes = read_bytes(&path, MAX_BYTES)?;
    let kv = binary_keyvalues::parse(&bytes).ok()?;
    let cache = find(&kv, "cache")?.block()?;
    let mut out = HashMap::new();
    for (stat_id, stat) in cache {
        let Some(stat) = stat.block() else { continue };
        // Valve writes the bits as a signed 32-bit number: read it whole, keep its 32 bits.
        let Some(data) = find_text(stat, "data").and_then(|d| d.trim().parse::<i64>().ok()) else { continue };
        let mut times = HashMap::new();
        for (bit, at) in find(stat, "AchievementTimes").and_then(Value::block).unwrap_or(&[]) {
            if let (Ok(bit), Some(Ok(at))) = (bit.trim().parse::<u32>(), at.text().map(|t| t.trim().parse::<i64>())) {
                if at > 0 {
                    times.insert(bit, at as u64);
                }
            }
        }
        out.insert(stat_id.clone(), (data as u32, times));
    }
    Some(Some(out))
}

/// Game `appid`'s achievements, as the person has them where that is known (`account` is the
/// account id whose record is read). `None` when the game has none here, or its files do not read.
pub fn read(stats_dir: &Path, account: Option<u32>, appid: &str) -> Option<Achievements> {
    let defined = defined(stats_dir, appid)?;
    if defined.is_empty() {
        return None;
    }
    let record = record(stats_dir, account, appid)?;
    let mut list: Vec<(usize, Achievement)> = defined
        .into_iter()
        .enumerate()
        .map(|(order, d)| {
            let mut a = d.achievement;
            if let Some(record) = &record {
                // A stat block the record leaves out has nothing unlocked.
                let (bits, times) = record.get(&d.stat).map(|(b, t)| (*b, Some(t))).unwrap_or((0, None));
                let on = bits & (1u32 << d.bit) != 0;
                a.unlocked = Some(on);
                a.unlocked_at = if on { times.and_then(|t| t.get(&d.bit).copied()) } else { None };
            }
            (order, a)
        })
        .collect();
    // The unlocked first, the most recent first (one with no time after those with one); then the
    // rest in the game's order.
    list.sort_by(|(oa, a), (ob, b)| {
        let rank = |x: &Achievement| x.unlocked != Some(true);
        rank(a).cmp(&rank(b)).then_with(|| b.unlocked_at.cmp(&a.unlocked_at)).then_with(|| oa.cmp(ob))
    });
    Some(Achievements { list: list.into_iter().map(|(_, a)| a).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures;

    /// ACHIEVEMENTS READ FROM STEAM'S TWO FILES: Orbital Farmstead's schema defines five in two
    /// blocks (one numbered by key, one carrying `bit`); the person's record unlocks three, one of
    /// them in the second block and one with no time recorded; the unlocked come first, newest
    /// first; a hidden one keeps its flag; a stat that is not achievements is left out.
    ///
    /// Seen red 2026-10-19 with the bit read from the block's key only (the `bit` entry ignored):
    /// left: (5, Some(2)), right: (5, Some(3)), First Harvest (bit 0 of the second block) read as
    /// bit 7.
    #[test]
    fn achievements_read_from_steams_two_files() {
        let fx = fixtures::steam("ach_two_files");
        let a = read(&fx.stats, Some(fixtures::ACCOUNT), "1000010").expect("it reads");
        assert_eq!((a.total(), a.unlocked()), (5, Some(3)));
        let names: Vec<&str> = a.list.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["Seedling", "First Harvest", "Long Night", "Deep Roots", "Secret Garden"], "unlocked first, newest first, then the game's order");
        let harvest = &a.list[1];
        assert_eq!((harvest.unlocked, harvest.unlocked_at), (Some(true), Some(1_789_000_000)), "First Harvest is unlocked");
        assert_eq!((a.list[2].unlocked, a.list[2].unlocked_at), (Some(true), None), "unlocked with no time recorded");
        let secret = &a.list[4];
        assert!(secret.hidden && secret.unlocked == Some(false), "the hidden one keeps its flag");
        assert_eq!(a.list[0].description, "Plant your first seed.");
    }

    /// NOT KNOWN IS NOT NONE, AND A BROKEN FILE SHOWS NOTHING: with no record of the person's the
    /// achievements are listed but none is counted as unlocked; a record that is there and does not
    /// read, or a schema that does not read, gives no achievements at all; a game with no schema
    /// has none.
    ///
    /// Seen red 2026-10-19 with a record that does not read treated as no record (its parse error
    /// turned into `Some(None)`): "a broken record shows nothing" failed, the five listed as not
    /// known.
    #[test]
    fn not_known_is_not_none_and_a_broken_file_shows_nothing() {
        let fx = fixtures::steam("ach_unknown");
        let unknown = read(&fx.stats, Some(999), "1000010").expect("listed without a record");
        assert_eq!((unknown.total(), unknown.unlocked()), (5, None), "not known, not 0");
        assert!(unknown.list.iter().all(|a| a.unlocked.is_none()));

        std::fs::write(fx.stats.join(format!("UserGameStats_{}_1000010.bin", fixtures::ACCOUNT)), [0u8, b'c', 0, 0x09]).unwrap();
        assert!(read(&fx.stats, Some(fixtures::ACCOUNT), "1000010").is_none(), "a broken record shows nothing");
        std::fs::write(fx.stats.join("UserGameStatsSchema_1000030.bin"), [0u8, b'1']).unwrap();
        assert!(read(&fx.stats, Some(fixtures::ACCOUNT), "1000030").is_none(), "a broken schema shows nothing");
        assert!(read(&fx.stats, Some(fixtures::ACCOUNT), "1000020").is_none(), "no schema, none");
    }
}
