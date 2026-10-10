//! Help outside this server, in the report dialog (step D follow-up of
//! docs/design/blocking-and-safe-mode.md, section 10e-ii, 2026-10-10): the native client's rules,
//! with no socket and no GUI so each one is unit tested. The dialog that draws the block is
//! src/gui/pages/chat/report_dialog.rs; opening it on the right country is src/engine/report.rs.
//!
//! When the reason is that a child or anyone may be in danger, the dialog shows, under the
//! reason's help, the emergency number and the official place to report a child being sexually
//! exploited or abused online, for a country the person picks. The numbers are data,
//! `data/safety/outside_help.json`, each entry backed by the dated finding
//! docs/reference/findings/2026-10-09-outside-help-lines.md; nothing here types a number, and the
//! date the numbers were checked is read from the file, never written into the code.
//!
//! The country is the person's own choice and stays on this device: never looked up (no IP
//! lookup, no location service), never sent with the report or anywhere else. The first one shown
//! is the last one picked here (AppConfig), else "Another country". The web client also tries the
//! region of the browser's language; this app reads no OS locale, so 10e-ii says to skip that
//! step here rather than add a crate for it.
//!
//! The words and rules match the web client's (`outsideHelp*` in web/shared/report.js), so both
//! apps show the same block.

use serde_json::Value;

/// The file, under `data/`. Built into the desktop app as `embedded_data::OUTSIDE_HELP_JSON` so an
/// install whose data folder lacks it still shows the block.
pub const OUTSIDE_HELP_FILE: &str = "safety/outside_help.json";

/// The reasons the block is shown for (10e-ii: never for any other).
pub const SHOWN_FOR: [&str; 2] = ["child_danger", "someone_in_danger"];

/// "Another country" as a choice. Lower case, so it can never be a listed country's code (those
/// are two upper-case letters), and it is saved when picked like any country, so the person who
/// chose it keeps it.
pub const OTHER: &str = "other";

/// The block's title (10e-ii).
pub const TITLE: &str = "Help outside this server";
/// The picker's last entry (10e-ii).
pub const OTHER_LABEL: &str = "Another country";
/// The link text when a listed country has no reporting body of its own and the INHOPE directory
/// stands in (10e-ii); the directory's own name becomes the hover text.
pub const FIND_HOTLINE: &str = "Find the hotline for your country";
/// Said before the child line's link: what that body takes reports of, in the data file's own
/// description of the field. The same words as the web client's.
pub const CHILD_LEAD: &str = "To report a child being sexually exploited or abused online:";

/// A named place to report, with an address that opens in the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub name: String,
    pub url: String,
}

/// One of a country's other numbers, with what it is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlsoLine {
    pub number: String,
    pub what_for: String,
}

impl AlsoLine {
    /// The line as the block shows it: "<number>: <what it is for>".
    pub fn line(&self) -> String {
        if self.what_for.is_empty() {
            self.number.clone()
        } else {
            format!("{}: {}", self.number, self.what_for)
        }
    }
}

/// One listed country.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country {
    /// Two upper-case letters (ISO 3166-1), as the file has it.
    pub code: String,
    pub name: String,
    pub emergency: String,
    pub also: Vec<AlsoLine>,
    /// The body that takes child reports there; None when the file has null (or a link this
    /// file refuses, see `link`), and the INHOPE directory stands in.
    pub child: Option<Link>,
    /// A caveat the findings record for this entry, or empty.
    pub note: String,
}

/// The file's `default`: for any country not listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fallback {
    /// A sentence in place of a number ("Call your local emergency number.").
    pub emergency_text: String,
    /// The INHOPE directory of member hotlines.
    pub child: Option<Link>,
    pub note: String,
}

/// The file, read into what the block needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutsideHelp {
    /// The date the numbers were checked, exactly as the file writes it (`YYYY-MM-DD`), or empty
    /// when the file has no such date (then the block has no date line).
    pub researched: String,
    pub default: Fallback,
    /// In the file's order, which is the picker's order.
    pub countries: Vec<Country>,
}

impl OutsideHelp {
    pub fn country(&self, code: &str) -> Option<&Country> {
        self.countries.iter().find(|c| c.code == code)
    }
}

/// Is the block offered for this reason? Only the two danger reasons (10e-ii).
pub fn shows_for(reason: &str) -> bool {
    SHOWN_FOR.contains(&reason)
}

/// A link the dialog will open: only `https://`, so a damaged or hostile data file cannot put
/// another kind of address (`javascript:`, plain `http:`, a file path) behind a hotline's name.
/// The same test as the web client's (`^https://[^\s"'<>]+$`).
fn link(v: &Value) -> Option<Link> {
    let name = v.get("name")?.as_str()?.trim();
    let url = v.get("url")?.as_str()?;
    let rest = url.strip_prefix("https://")?;
    if name.is_empty() || rest.is_empty() || rest.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>')) {
        return None;
    }
    Some(Link { name: name.to_string(), url: url.to_string() })
}

/// A trimmed string field, or empty.
fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string()
}

fn is_code(s: &str) -> bool {
    s.len() == 2 && s.bytes().all(|b| b.is_ascii_uppercase())
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Read the file. Not the file 10e-ii describes (no `countries` list, or a `default` with no
/// `emergency_text`) is an error, so the copy built into the exe is used instead. An entry that is
/// not whole (no code, name or number, or a code already seen) is left out rather than shown
/// half.
pub fn parse(bytes: &[u8]) -> Result<OutsideHelp, String> {
    let data: Value = serde_json::from_slice(bytes).map_err(|e| format!("outside help: {e}"))?;
    let list = data.get("countries").and_then(Value::as_array).ok_or("outside help: no countries list")?;
    let d = data.get("default").filter(|d| d.is_object()).ok_or("outside help: no default entry")?;
    let emergency_text = text(d, "emergency_text");
    if emergency_text.is_empty() {
        return Err("outside help: the default entry has no emergency_text".to_string());
    }
    let mut countries: Vec<Country> = Vec::new();
    for c in list {
        let code = c.get("code").and_then(Value::as_str).unwrap_or_default();
        let (name, emergency) = (text(c, "name"), text(c, "emergency"));
        if !is_code(code) || countries.iter().any(|seen| seen.code == code) || name.is_empty() || emergency.is_empty() {
            continue;
        }
        let also: Vec<AlsoLine> = c
            .get("also")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|x| AlsoLine { number: text(x, "number"), what_for: text(x, "for") }).filter(|x| !x.number.is_empty()).collect())
            .unwrap_or_default();
        countries.push(Country {
            code: code.to_string(),
            name,
            emergency,
            also,
            child: c.get("child_report").and_then(link),
            note: text(c, "note"),
        });
    }
    let researched = data.get("researched").and_then(Value::as_str).filter(|s| is_date(s)).unwrap_or_default().to_string();
    Ok(OutsideHelp {
        researched,
        default: Fallback { emergency_text, child: d.get("child_report").and_then(link), note: text(d, "note") },
        countries,
    })
}

/// Load the file from the data folder, or the copy built into the exe when the folder's copy is
/// missing or this version cannot read it.
pub fn load(data_dir: &std::path::Path) -> Result<OutsideHelp, String> {
    crate::embedded_data::load_data_or_embedded(data_dir, OUTSIDE_HELP_FILE, parse)
}

/// Which country the block starts on (10e-ii): the one last picked on this device (`saved`, from
/// AppConfig) when it is still a choice (a listed code, or "Another country"), else "Another
/// country". A saved code the file no longer lists falls through. The web client puts its
/// browser-language step between the two; this app reads no OS locale, so it has none.
pub fn first_country(help: &OutsideHelp, saved: &str) -> String {
    let saved = saved.trim();
    if saved == OTHER {
        return OTHER.to_string();
    }
    let code = saved.to_ascii_uppercase();
    if help.country(&code).is_some() {
        return code;
    }
    OTHER.to_string()
}

/// The block's last line, with the date read from the file (10e-ii, word for word).
pub fn date_line(researched: &str) -> String {
    format!("Numbers checked on {researched}. If one is wrong, tell us.")
}

/// The child line's link as the block shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildLine {
    /// The link text.
    pub name: String,
    pub url: String,
    /// Shown when the pointer rests on the link: the INHOPE directory's own name when it stands
    /// in for a listed country's body; None otherwise (the dialog shows the address then).
    pub hover: Option<String>,
}

/// What the block shows for a reason and a chosen country.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpView {
    /// The picker's entries, `(code, name)`: the listed countries by name in the file's order,
    /// then "Another country".
    pub choices: Vec<(String, String)>,
    /// The choice shown: a listed code, or `OTHER`.
    pub code: String,
    /// Its name in the picker ("Country: <name>").
    pub name: String,
    /// The emergency number (None for "Another country", which has `emergency_text`).
    pub emergency: Option<String>,
    pub emergency_text: String,
    pub also: Vec<AlsoLine>,
    pub child: Option<ChildLine>,
    /// For someone in danger the child line is there too, smaller (10e-ii).
    pub child_small: bool,
    pub note: String,
    /// Empty when the file carries no date.
    pub date_line: String,
}

impl HelpView {
    /// The large line: "Emergency: <number>", or for "Another country" the default's sentence.
    pub fn emergency_line(&self) -> String {
        match &self.emergency {
            Some(n) => format!("Emergency: {n}"),
            None => self.emergency_text.clone(),
        }
    }
}

/// What the block shows for `reason` and the chosen `code`, or None when the reason does not
/// offer it. A code not listed reads as "Another country".
pub fn view(help: &OutsideHelp, reason: &str, code: &str) -> Option<HelpView> {
    if !shows_for(reason) {
        return None;
    }
    let entry = help.country(code);
    let child = match (entry, entry.and_then(|c| c.child.as_ref()), help.default.child.as_ref()) {
        (_, Some(own), _) => Some(ChildLine { name: own.name.clone(), url: own.url.clone(), hover: None }),
        // A listed country with no body of its own: the INHOPE directory, named for what it does,
        // with its own name on hover.
        (Some(_), None, Some(d)) => Some(ChildLine { name: FIND_HOTLINE.to_string(), url: d.url.clone(), hover: Some(d.name.clone()) }),
        // "Another country" is the default entry itself, so it shows the default's own name.
        (None, None, Some(d)) => Some(ChildLine { name: d.name.clone(), url: d.url.clone(), hover: None }),
        (_, None, None) => None,
    };
    let mut choices: Vec<(String, String)> = help.countries.iter().map(|c| (c.code.clone(), c.name.clone())).collect();
    choices.push((OTHER.to_string(), OTHER_LABEL.to_string()));
    Some(HelpView {
        choices,
        code: entry.map(|c| c.code.clone()).unwrap_or_else(|| OTHER.to_string()),
        name: entry.map(|c| c.name.clone()).unwrap_or_else(|| OTHER_LABEL.to_string()),
        emergency: entry.map(|c| c.emergency.clone()),
        emergency_text: if entry.is_some() { String::new() } else { help.default.emergency_text.clone() },
        also: entry.map(|c| c.also.clone()).unwrap_or_default(),
        child,
        child_small: reason == "someone_in_danger",
        note: entry.map(|c| c.note.clone()).unwrap_or_else(|| help.default.note.clone()),
        date_line: if help.researched.is_empty() { String::new() } else { date_line(&help.researched) },
    })
}

#[cfg(test)]
#[path = "outside_help_tests.rs"]
mod tests;
