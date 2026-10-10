//! The rules of src/net/outside_help.rs (10e-ii's proof list), held against the REAL file: the
//! copy built into the exe (`embedded_data::OUTSIDE_HELP_JSON`, which is
//! data/safety/outside_help.json). Where a test needs a case today's file does not have (a null
//! `child_report`, another date, a link that is not https), it edits a copy of the real file's
//! text, never a hand-made file. Each test was seen red once on purpose, recorded at the test.

use super::*;

/// The real file, read the way the app reads it.
fn real() -> OutsideHelp {
    parse(crate::embedded_data::OUTSIDE_HELP_JSON.as_bytes()).expect("the shipped file parses")
}

/// The real file as plain JSON, for the expected values: a test never types a number.
fn raw() -> Value {
    serde_json::from_str(crate::embedded_data::OUTSIDE_HELP_JSON).expect("the shipped file is JSON")
}

/// The real file with one change, read the way the app reads it.
fn edited(change: impl FnOnce(&mut Value)) -> OutsideHelp {
    let mut v = raw();
    change(&mut v);
    parse(v.to_string().as_bytes()).expect("the edited copy parses")
}

fn entry<'a>(v: &'a Value, code: &str) -> &'a Value {
    v["countries"].as_array().expect("countries").iter().find(|c| c["code"] == code).expect("a listed code")
}

fn entry_mut<'a>(v: &'a mut Value, code: &str) -> &'a mut Value {
    v["countries"].as_array_mut().expect("countries").iter_mut().find(|c| c["code"] == code).expect("a listed code")
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// The block is offered for exactly the two danger reasons of the real reasons file, and the
/// view says nothing for any other. Seen red 2026-10-10 by taking the `shows_for` guard out of
/// `view`: "spam" got a block.
#[test]
fn the_block_shows_for_exactly_the_two_danger_reasons() {
    let help = real();
    let reasons = crate::net::report::parse_reasons(crate::embedded_data::REPORT_REASONS_JSON.as_bytes()).expect("the shipped reasons");
    for id in SHOWN_FOR {
        assert!(reasons.iter().any(|r| r.id == id), "{id} is a reason in data/safety/report_reasons.json");
    }
    for r in &reasons {
        let danger = r.id == "child_danger" || r.id == "someone_in_danger";
        assert_eq!(shows_for(&r.id), danger, "{}", r.id);
        assert_eq!(view(&help, &r.id, "US").is_some(), danger, "{}: the block is shown for the danger reasons only", r.id);
        assert_eq!(view(&help, &r.id, OTHER).is_some(), danger, "{}, Another country", r.id);
    }
    assert!(view(&help, "", "US").is_none(), "no reason chosen yet: no block");
}

/// Every listed country gives its own number, its `also` lines as "<number>: <what it is for>",
/// its child body as the link (no hover name: it is not a stand-in), and its note, exactly as the
/// file has them; the picker lists them by name in the file's order, then "Another country". For
/// someone in danger the same child line is there, smaller. Seen red 2026-10-10 with `parse`
/// reading every `child_report` as None: each country showed the INHOPE stand-in instead of its
/// own body.
#[test]
fn a_listed_country_gives_its_number_also_lines_and_child_body() {
    let help = real();
    let raw = raw();
    let listed = raw["countries"].as_array().expect("countries");
    assert_eq!(help.countries.len(), listed.len(), "every entry of the shipped file is whole and kept");
    assert!(listed.iter().any(|c| c["also"].as_array().is_some_and(|a| !a.is_empty())), "the file still has an `also` line to check");
    assert!(listed.iter().any(|c| c["note"].is_string()), "the file still has a note to check");
    for c in listed {
        let code = s(&c["code"]);
        let v = view(&help, "child_danger", &code).expect("a block");
        assert_eq!((v.code.as_str(), v.name.clone()), (code.as_str(), s(&c["name"])));
        assert_eq!(v.emergency_line(), format!("Emergency: {}", s(&c["emergency"])), "{code}");
        let also: Vec<String> = c["also"]
            .as_array()
            .map(|a| a.iter().map(|x| if x["for"].is_string() { format!("{}: {}", s(&x["number"]), s(&x["for"])) } else { s(&x["number"]) }).collect())
            .unwrap_or_default();
        assert_eq!(v.also.iter().map(AlsoLine::line).collect::<Vec<_>>(), also, "{code}");
        // A null one (none today) would be the INHOPE stand-in, tested on its own below.
        let own = if c["child_report"].is_null() {
            let d = &raw["default"]["child_report"];
            ChildLine { name: FIND_HOTLINE.to_string(), url: s(&d["url"]), hover: Some(s(&d["name"])) }
        } else {
            ChildLine { name: s(&c["child_report"]["name"]), url: s(&c["child_report"]["url"]), hover: None }
        };
        assert_eq!(v.child.as_ref(), Some(&own), "{code}: its own body, as the file names it");
        assert!(!v.child_small, "full size for a child in danger");
        assert_eq!(v.note, s(&c["note"]), "{code}");
        let small = view(&help, "someone_in_danger", &code).expect("a block");
        assert_eq!((small.child.as_ref(), small.child_small), (Some(&own), true), "{code}: smaller for someone in danger");
    }
    let v = view(&help, "child_danger", "US").expect("a block");
    let names: Vec<String> = listed.iter().map(|c| s(&c["name"])).chain([OTHER_LABEL.to_string()]).collect();
    assert_eq!(v.choices.iter().map(|(_, n)| n.clone()).collect::<Vec<_>>(), names);
    assert_eq!(v.choices.last().map(|(c, _)| c.as_str()), Some(OTHER));
}

/// A country whose `child_report` is null (none is today, so a copy of the real file with the
/// United States' set to null) shows the default's INHOPE directory as "Find the hotline for your
/// country", with the directory's own name on hover; its number is untouched. Seen red 2026-10-10
/// with the stand-in arm of `view` giving the directory's own name: the link read "INHOPE member
/// hotlines (find the one for your country)".
#[test]
fn a_null_child_report_falls_back_to_the_inhope_list() {
    let raw = raw();
    let help = edited(|v| entry_mut(v, "US")["child_report"] = Value::Null);
    let v = view(&help, "child_danger", "US").expect("a block");
    let inhope = &raw["default"]["child_report"];
    assert_eq!(
        v.child,
        Some(ChildLine { name: FIND_HOTLINE.to_string(), url: s(&inhope["url"]), hover: Some(s(&inhope["name"])) })
    );
    assert_eq!(v.emergency_line(), format!("Emergency: {}", s(&entry(&raw, "US")["emergency"])), "its number is untouched");
}

/// "Another country" shows the default's sentence in place of a number, no `also` lines, and the
/// INHOPE directory under its own name (it is the default entry itself, not a stand-in). A code
/// the file does not list reads the same. Seen red 2026-10-10 with `emergency_line` putting
/// "Emergency: " before the sentence too: Another country read "Emergency: Call your local
/// emergency number."
#[test]
fn another_country_gives_the_sentence() {
    let help = real();
    let raw = raw();
    let d = &raw["default"];
    let v = view(&help, "child_danger", OTHER).expect("a block");
    assert_eq!((v.code.as_str(), v.name.as_str(), v.emergency.as_deref()), (OTHER, OTHER_LABEL, None));
    assert_eq!(v.emergency_line(), s(&d["emergency_text"]));
    assert!(v.also.is_empty());
    assert_eq!(v.child, Some(ChildLine { name: s(&d["child_report"]["name"]), url: s(&d["child_report"]["url"]), hover: None }));
    assert_eq!(view(&help, "child_danger", "XX"), Some(v), "an unlisted code is Another country");
}

/// The first country (10e-ii, on this app): the saved choice when it is still one (a listed code,
/// read in any case; or "Another country", which is kept), else "Another country". A code the
/// file stops listing falls through. Seen red 2026-10-10 with `first_country` returning any
/// non-empty saved code as it was: "XX" stayed "XX".
#[test]
fn the_first_country_is_the_saved_choice_else_another_country() {
    let help = real();
    let first = s(&raw()["countries"][0]["code"]);
    assert_eq!(first_country(&help, &first), first, "a saved listed code wins");
    assert_eq!(first_country(&help, &format!(" {} ", first.to_lowercase())), first, "in any case, spaces trimmed");
    assert_eq!(first_country(&help, OTHER), OTHER, "Another country, once picked, is kept");
    assert_eq!(first_country(&help, "XX"), OTHER, "an unlisted code falls to Another country");
    assert_eq!(first_country(&help, ""), OTHER, "nothing saved: Another country");
    let without = edited(|v| v["countries"].as_array_mut().expect("countries").retain(|c| c["code"] != first.as_str()));
    assert_eq!(first_country(&without, &first), OTHER, "a saved code the file no longer lists falls through");
}

/// The date line reads the file's `researched`, exactly as written, in 10e-ii's sentence; a copy
/// carrying another date shows that date; a copy with no date shows no line rather than a wrong
/// one. Seen red 2026-10-10 with `view` writing `date_line("2026-10-09")`: the copy dated
/// 2027-03-01 still said 2026-10-09.
#[test]
fn the_date_line_reads_the_files_researched_date() {
    let raw = raw();
    let date = s(&raw["researched"]);
    assert!(!date.is_empty(), "the shipped file carries its date");
    let v = view(&real(), "someone_in_danger", OTHER).expect("a block");
    assert_eq!(v.date_line, format!("Numbers checked on {date}. If one is wrong, tell us."));
    let later = edited(|v| v["researched"] = Value::from("2027-03-01"));
    assert_eq!(view(&later, "child_danger", "US").expect("a block").date_line, "Numbers checked on 2027-03-01. If one is wrong, tell us.");
    let undated = edited(|v| v["researched"] = Value::from("soon"));
    assert_eq!(view(&undated, "child_danger", "US").expect("a block").date_line, "", "not a date: no line");
}

/// Only `https://` links come from the file: a country's link of any other kind is dropped, and
/// the INHOPE stand-in takes its place; a default link of any other kind leaves no child line at
/// all. Seen red 2026-10-10 with `link` no longer requiring `https://`: the United States' child
/// line opened "http://report.cybertip.org/".
#[test]
fn a_link_that_is_not_https_is_dropped() {
    let raw = raw();
    let inhope_url = s(&raw["default"]["child_report"]["url"]);
    let bad = ["http://report.cybertip.org/", "javascript:alert(1)", "https://", "https://report.cybertip.org/ x", "HTTPS://report.cybertip.org/"];
    for url in bad {
        let help = edited(|v| entry_mut(v, "US")["child_report"]["url"] = Value::from(url));
        let child = view(&help, "child_danger", "US").expect("a block").child.expect("the stand-in");
        assert_eq!((child.name.as_str(), child.url.as_str()), (FIND_HOTLINE, inhope_url.as_str()), "{url} is dropped");
    }
    let no_default = edited(|v| {
        v["default"]["child_report"]["url"] = Value::from("http://www.inhope.org/EN/our-members");
        entry_mut(v, "US")["child_report"] = Value::Null;
    });
    assert_eq!(view(&no_default, "child_danger", OTHER).expect("a block").child, None, "Another country: no link rather than a bad one");
    assert_eq!(view(&no_default, "child_danger", "US").expect("a block").child, None, "and no stand-in either");
    assert!(view(&no_default, "child_danger", "GB").expect("a block").child.is_some(), "a good link stays");
}
