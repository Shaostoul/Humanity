//! A SERVER'S PUBLISHED FUNDING FIGURES (2026-11-22; docs/design/live-funding.md). The operator,
//! that day: "Can we update the values live? So people can see what current funding is at least
//! via other things not my second job?"
//!
//! What donations bring in each month (one row per CHANNEL: Patreon, GitHub Sponsors, CashApp...)
//! against what running the project costs each month (one row per COST: the servers, the domain...),
//! each with the date the figure was last confirmed. Rows are DATA, not code (infinite-of-x): a
//! server's owner adds a channel or a cost in Server Settings, no release needed. The funding GOAL
//! stays where it was (`funding.goal_usd` in data/server-config.json).
//!
//! STORED as one JSON text column of the `server_settings` row (`funding_figures`, empty for a
//! server that publishes nothing: the shipped default), carried in `ServerSettings::funding_figures`
//! to Server Settings, set with `server_settings_update` (admin or owner only, the relay checks the
//! role: handlers/server_settings_update.rs), and served to everyone in `/api/server-info`'s
//! `funding.channels` and `funding.costs` (relay/api.rs). Nothing here is a person's income: the
//! page says what DONATIONS bring in and what the PROJECT costs.
//!
//! The relay holds the figures to their SHAPE ([`FundingFigures::checked`]): at most [`MAX_ROWS`]
//! channels and [`MAX_ROWS`] costs, a name of 1 to [`MAX_NAME`] characters, a link of at most
//! [`MAX_LINK`] characters that is empty or `https://`, an amount from 0 to [`MAX_USD`], and an
//! as-of date the RELAY stamps (today, for a row that is new, whose amount changed, or that the
//! owner confirmed by leaving its date blank). Compiled in both builds (the relay and the desktop
//! app, which reads the same shape from `/api/server-info`).

use serde::{Deserialize, Serialize};

/// The most channels, and the most costs, a server publishes.
pub const MAX_ROWS: usize = 32;
/// The longest name, in characters.
pub const MAX_NAME: usize = 80;
/// The longest link, in characters.
pub const MAX_LINK: usize = 300;
/// The largest monthly amount, in US dollars.
pub const MAX_USD: f64 = 1_000_000.0;

/// One way donations reach the project, and what it brings in each month.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FundingChannel {
    /// What the donor sees ("Patreon").
    pub name: String,
    /// Where to give: empty, or an `https://` link.
    #[serde(default)]
    pub link: String,
    /// What it brings in each month, in US dollars.
    #[serde(default)]
    pub monthly_usd: f64,
    /// The day the figure was last confirmed, `YYYY-MM-DD` (stamped by the relay).
    #[serde(default)]
    pub as_of: String,
}

/// One thing running the project costs each month.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FundingCost {
    /// What it is ("Servers").
    pub name: String,
    /// What it costs each month, in US dollars.
    #[serde(default)]
    pub monthly_usd: f64,
    /// The day the figure was last confirmed, `YYYY-MM-DD` (stamped by the relay).
    #[serde(default)]
    pub as_of: String,
}

/// A server's published figures. Empty (the default) publishes nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FundingFigures {
    /// The ways donations arrive, with each one's monthly amount.
    #[serde(default)]
    pub channels: Vec<FundingChannel>,
    /// What running the project costs each month.
    #[serde(default)]
    pub costs: Vec<FundingCost>,
}

/// Why an update was refused, in words an owner can read (shown to them by the relay).
pub type Refusal = &'static str;

fn amount_ok(v: f64) -> bool {
    v.is_finite() && (0.0..=MAX_USD).contains(&v)
}

fn name_ok(s: &str) -> bool {
    let n = s.trim().chars().count();
    (1..=MAX_NAME).contains(&n)
}

fn link_ok(s: &str) -> bool {
    s.is_empty() || (s.chars().count() <= MAX_LINK && s.starts_with("https://") && s.len() > "https://".len() && !s.chars().any(|c| c.is_whitespace() || c.is_control()))
}

fn date_ok(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// A dollar amount as a person reads it: `20`, `12.50` (whole dollars without cents).
pub fn money(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.2}") }
}

/// Today's date (UTC) as `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = crate::relay::backup_stream::civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The date a row carries after an update: the owner's own date when the row is unchanged and has
/// one, else `today` (new row, changed amount, or a blank date: "still right as of today").
fn stamp(sent: &str, was: Option<(f64, &str)>, amount: f64, today: &str) -> String {
    match was {
        Some((old, old_date)) if old == amount && date_ok(sent) && sent == old_date => old_date.to_string(),
        _ => today.to_string(),
    }
}

impl FundingFigures {
    /// Nothing published.
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty() && self.costs.is_empty()
    }

    /// What every channel brings in each month.
    pub fn channels_total(&self) -> f64 {
        self.channels.iter().map(|c| c.monthly_usd).sum()
    }

    /// What every cost comes to each month.
    pub fn costs_total(&self) -> f64 {
        self.costs.iter().map(|c| c.monthly_usd).sum()
    }

    /// Hold an owner's figures to their shape, stamping the as-of dates. `previous` is what is
    /// saved now (a row keeps its date only when its name and amount are unchanged), `today` is
    /// `YYYY-MM-DD`. Any bad row refuses the whole update, with the reason.
    pub fn checked(mut self, previous: &FundingFigures, today: &str) -> Result<FundingFigures, Refusal> {
        if self.channels.len() > MAX_ROWS || self.costs.len() > MAX_ROWS {
            return Err("At most 32 channels and 32 costs can be published.");
        }
        for c in &mut self.channels {
            c.name = c.name.trim().to_string();
            c.link = c.link.trim().to_string();
            if !name_ok(&c.name) {
                return Err("Every channel needs a name of 1 to 80 characters.");
            }
            if !link_ok(&c.link) {
                return Err("A channel's link must be empty or start with https:// (at most 300 characters).");
            }
            if !amount_ok(c.monthly_usd) {
                return Err("A monthly amount must be from 0 to 1,000,000 dollars.");
            }
            let was = previous.channels.iter().find(|p| p.name == c.name).map(|p| (p.monthly_usd, p.as_of.as_str()));
            c.as_of = stamp(&c.as_of, was, c.monthly_usd, today);
        }
        for c in &mut self.costs {
            c.name = c.name.trim().to_string();
            if !name_ok(&c.name) {
                return Err("Every cost needs a name of 1 to 80 characters.");
            }
            if !amount_ok(c.monthly_usd) {
                return Err("A monthly amount must be from 0 to 1,000,000 dollars.");
            }
            let was = previous.costs.iter().find(|p| p.name == c.name).map(|p| (p.monthly_usd, p.as_of.as_str()));
            c.as_of = stamp(&c.as_of, was, c.monthly_usd, today);
        }
        Ok(self)
    }

    /// Read the stored column. Anything unreadable is "nothing published".
    pub fn from_column(s: &str) -> FundingFigures {
        serde_json::from_str(s).unwrap_or_default()
    }

    /// The column's text: empty when nothing is published.
    pub fn column(&self) -> String {
        if self.is_empty() {
            String::new()
        } else {
            serde_json::to_string(self).unwrap_or_default()
        }
    }

    /// Read the figures out of a server's `funding` object (`/api/server-info`), as a person's app
    /// does: a row that is not shaped right is skipped, so a server's slip never blanks the page,
    /// and the same bounds hold (a rogue server cannot make the page draw a thousand rows or a
    /// link that is not https).
    pub fn from_funding_json(funding: &serde_json::Value) -> FundingFigures {
        let rows = |key: &str| funding.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let channels = rows("channels")
            .into_iter()
            .filter_map(|v| serde_json::from_value::<FundingChannel>(v).ok())
            .filter(|c| name_ok(&c.name) && link_ok(&c.link) && amount_ok(c.monthly_usd))
            .take(MAX_ROWS)
            .collect();
        let costs = rows("costs")
            .into_iter()
            .filter_map(|v| serde_json::from_value::<FundingCost>(v).ok())
            .filter(|c| name_ok(&c.name) && amount_ok(c.monthly_usd))
            .take(MAX_ROWS)
            .collect();
        FundingFigures { channels, costs }
    }
}

/// The `funding` object `/api/server-info` serves: the server-config's (when enabled) with the
/// published figures as `channels` and `costs`. A server that publishes no figures serves exactly
/// what it did before; one that publishes some serves them even with the config's funding off.
pub fn served(config_funding: Option<serde_json::Value>, figures: &FundingFigures) -> Option<serde_json::Value> {
    if figures.is_empty() {
        return config_funding;
    }
    let mut f = config_funding.unwrap_or_else(|| serde_json::json!({ "enabled": true }));
    if let Some(obj) = f.as_object_mut() {
        obj.insert("channels".into(), serde_json::to_value(&figures.channels).unwrap_or_default());
        obj.insert("costs".into(), serde_json::to_value(&figures.costs).unwrap_or_default());
    }
    Some(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(name: &str, link: &str, usd: f64, as_of: &str) -> FundingChannel {
        FundingChannel { name: name.into(), link: link.into(), monthly_usd: usd, as_of: as_of.into() }
    }
    fn co(name: &str, usd: f64) -> FundingCost {
        FundingCost { name: name.into(), monthly_usd: usd, as_of: String::new() }
    }

    /// THE SHAPE VALIDATION REFUSES BAD ROWS (2026-11-22): a link that is not https, a negative or
    /// over-large or non-number amount, an empty or over-long name, a 33rd row, each refuse the
    /// WHOLE update (nothing half-saved); good figures pass, with names trimmed.
    ///
    /// Seen red 2026-11-22 with `link_ok` accepting any link: "an http link was accepted".
    #[test]
    fn the_shape_validation_refuses_bad_rows() {
        let none = FundingFigures::default();
        let one = |c: FundingChannel| FundingFigures { channels: vec![c], costs: vec![] };
        let good = one(ch("  Patreon ", "https://www.patreon.com/x", 20.0, ""));
        let ok = good.checked(&none, "2026-11-22").expect("good figures pass");
        assert_eq!(ok.channels[0].name, "Patreon", "names are trimmed");
        assert_eq!(ok.channels[0].as_of, "2026-11-22", "a new row is stamped today");
        for (why, bad) in [
            ("an http link", one(ch("A", "http://x.example", 1.0, ""))),
            ("a link with a space", one(ch("A", "https://x.example/a b", 1.0, ""))),
            ("a javascript link", one(ch("A", "javascript:alert(1)", 1.0, ""))),
            ("a negative amount", one(ch("A", "", -1.0, ""))),
            ("an amount over a million", one(ch("A", "", 1_000_000.5, ""))),
            ("a not-a-number amount", one(ch("A", "", f64::NAN, ""))),
            ("an empty name", one(ch("  ", "", 1.0, ""))),
            ("a name of 81 characters", one(ch(&"n".repeat(81), "", 1.0, ""))),
            ("a link of 301 characters", one(ch("A", &format!("https://{}", "a".repeat(293)), 1.0, ""))),
            ("a 33rd channel", FundingFigures { channels: (0..33).map(|i| ch(&format!("c{i}"), "", 1.0, "")).collect(), costs: vec![] }),
            ("a 33rd cost", FundingFigures { channels: vec![], costs: (0..33).map(|i| co(&format!("k{i}"), 1.0)).collect() }),
            ("a cost over a million", FundingFigures { channels: vec![], costs: vec![co("Servers", 2_000_000.0)] }),
        ] {
            assert!(bad.checked(&none, "2026-11-22").is_err(), "{why} was accepted");
        }
        let edge = FundingFigures { channels: vec![ch("A", "", 0.0, ""), ch("B", "", 1_000_000.0, "")], costs: vec![] };
        assert!(edge.checked(&none, "2026-11-22").is_ok(), "0 and 1,000,000 are inside the range");
    }

    /// THE AS-OF DATE IS THE RELAY'S (2026-11-22): a row whose name and amount are unchanged keeps
    /// its date, a changed amount or a blank date ("still right today") is stamped today, and a date
    /// an owner types for a changed row is not believed.
    ///
    /// Seen red 2026-11-22 with `stamp` keeping any date the owner sent: "a changed amount is stamped
    /// today / left: \"2026-01-01\"".
    #[test]
    fn the_as_of_date_is_the_relays() {
        let before = FundingFigures { channels: vec![ch("Patreon", "", 20.0, "2026-11-01")], costs: vec![] };
        let same = FundingFigures { channels: vec![ch("Patreon", "", 20.0, "2026-11-01")], costs: vec![] };
        assert_eq!(same.checked(&before, "2026-11-22").unwrap().channels[0].as_of, "2026-11-01", "unchanged keeps its date");
        let changed = FundingFigures { channels: vec![ch("Patreon", "", 25.0, "2026-01-01")], costs: vec![] };
        assert_eq!(changed.checked(&before, "2026-11-22").unwrap().channels[0].as_of, "2026-11-22", "a changed amount is stamped today");
        let confirmed = FundingFigures { channels: vec![ch("Patreon", "", 20.0, "")], costs: vec![] };
        assert_eq!(confirmed.checked(&before, "2026-11-22").unwrap().channels[0].as_of, "2026-11-22", "a blank date confirms it today");
    }

    /// THE COLUMN ROUND-TRIPS AND NOTHING PUBLISHED IS EMPTY (2026-11-22): the shipped default
    /// stores "", unreadable text reads as nothing, and a reader skips rows a rogue server malformed.
    ///
    /// Seen red 2026-11-22 with `link_ok` accepting any link: "only the well-shaped https row is
    /// read / left: 2".
    #[test]
    fn the_column_round_trips_and_nothing_published_is_empty() {
        assert_eq!(FundingFigures::default().column(), "");
        assert!(FundingFigures::from_column("").is_empty());
        assert!(FundingFigures::from_column("{not json").is_empty());
        let f = FundingFigures { channels: vec![ch("A", "https://a.example", 3.0, "2026-11-22")], costs: vec![co("Servers", 40.0)] };
        assert_eq!(FundingFigures::from_column(&f.column()), f);
        assert_eq!((f.channels_total(), f.costs_total()), (3.0, 40.0));
        let served = serde_json::json!({ "channels": [
            { "name": "Ok", "link": "https://ok.example", "monthly_usd": 5.0, "as_of": "2026-11-22" },
            { "name": "Bad", "link": "http://bad.example", "monthly_usd": 5.0 },
            { "link": "https://noname.example" } ] });
        assert_eq!(FundingFigures::from_funding_json(&served).channels.len(), 1, "only the well-shaped https row is read");
    }
}
