//! The "I can" statements answer to their review (2026-11-14).
//!
//! An adversarial review checked all 810 statements against primary sources and filed 103
//! findings (safety, legal, order, factual, vague). Its record,
//! `docs/reference/research/2026-11-14-skill-statement-review/findings.json`, lists every finding
//! with its outcome: APPLIED, with the changes made (a statement reworded, moved to another stage
//! under a new key, or added as a new key) and the words now in `data/i18n/en.json`, or REJECTED,
//! with the reason. These tests hold the data to that record, so a finding cannot be dropped, a
//! correction cannot be quietly undone, and each corrected statement keeps its source in
//! `data/skills/stages.json` (`sources`: url and date) for a later reader to check.

use std::collections::BTreeMap;

use serde_json::Value;

fn repo(rel: &str) -> Vec<u8> {
    std::fs::read(format!("{}/{rel}", env!("HUMANITY_REPO_ROOT"))).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_slice(&repo(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

const RECORD: &str = "docs/reference/research/2026-11-14-skill-statement-review/findings.json";

/// The words of a dotted en.json key, if it is there.
fn words<'a>(en: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.').try_fold(en, |v, part| v.get(part)).and_then(Value::as_str)
}

/// Every statement key in the ladders, with the skill and stage it sits at.
fn ladder_keys(stages: &Value) -> BTreeMap<String, (String, String)> {
    let mut out = BTreeMap::new();
    for (skill, ladder) in stages["ladders"].as_object().expect("ladders") {
        for step in ladder.as_array().expect("a ladder is a list") {
            let stage = step["stage"].as_str().unwrap_or_default().to_string();
            for k in step["can"].as_array().expect("can is a list") {
                out.insert(k.as_str().unwrap_or_default().to_string(), (skill.clone(), stage.clone()));
            }
        }
    }
    out
}

fn s<'a>(v: &'a Value, field: &str) -> &'a str {
    v[field].as_str().unwrap_or_default()
}

/// Every finding of the review is addressed: applied (its words changed, moved to the stage the
/// finding named, or a new statement added, each with its source in stages.json) or rejected with
/// a reason; and the record still holds all 103 (30 + 37 + 36 by review group).
///
/// Seen red 2026-11-14 with the record written and the data not yet changed: 243 problems across
/// the 103 findings, for example "skills.first_aid.s3_a: en.json has \"I can check whether a person is responding and
/// breathing, and place them in the recovery position.\", the review applied \"I can check whether
/// a person is responding and breathing normally. ...\"" and "skills.preserving.s3_c (moved from
/// skills.preserving.s4_a): not in the competent stage of preserving".
#[test]
fn every_finding_of_the_statement_review_is_addressed() {
    let record = json(RECORD);
    let en = json("data/i18n/en.json");
    let stages = json("data/skills/stages.json");
    let at = ladder_keys(&stages);
    let sources = stages["sources"].as_object().cloned().unwrap_or_default();
    let findings = record["findings"].as_array().expect("findings is a list");
    let mut bad = Vec::new();

    let mut by_group: BTreeMap<&str, usize> = BTreeMap::new();
    for f in findings {
        *by_group.entry(s(f, "group")).or_default() += 1;
    }
    let want: BTreeMap<&str, usize> = [("A", 30), ("B", 37), ("C", 36)].into_iter().collect();
    assert_eq!(by_group, want, "the record holds every finding of the three review groups");
    let mut seen = std::collections::BTreeSet::new();

    for f in findings {
        let key = s(f, "key");
        let skill = s(f, "skill");
        let was = s(f, "statement");
        if !seen.insert(key) {
            bad.push(format!("{key}: listed twice"));
        }
        match s(f, "outcome") {
            "rejected" => {
                if s(f, "reason").trim().is_empty() {
                    bad.push(format!("{key}: rejected with no reason"));
                }
                if words(&en, key) != Some(was) {
                    bad.push(format!("{key}: rejected, yet its words are no longer the reviewed ones"));
                }
            }
            "applied" => {
                let changes = f["changes"].as_array().cloned().unwrap_or_default();
                if changes.is_empty() {
                    bad.push(format!("{key}: applied with no change"));
                }
                for c in &changes {
                    let ck = s(c, "key");
                    let kind = s(c, "kind");
                    let text = s(c, "text");
                    let label = if kind == "moved" { format!("{ck} (moved from {key})") } else { ck.to_string() };
                    if text == was {
                        bad.push(format!("{label}: the change keeps the reviewed words"));
                    }
                    match words(&en, ck) {
                        Some(now) if now == text => {}
                        Some(now) => bad.push(format!("{label}: en.json has {now:?}, the review applied {text:?}")),
                        None => bad.push(format!("{label}: not in en.json")),
                    }
                    match at.get(ck) {
                        Some((sk, st)) if sk == skill && st == s(c, "stage") => {}
                        _ => bad.push(format!("{label}: not in the {} stage of {skill}", s(c, "stage"))),
                    }
                    match kind {
                        "reworded" => {}
                        "added" if ck != key => {}
                        "moved" if s(c, "from") == key && ck != key => {
                            if words(&en, key).is_some() || at.contains_key(key) {
                                bad.push(format!("{label}: the old key is still there"));
                            }
                        }
                        _ => bad.push(format!("{label}: a {kind:?} change that does not fit its finding")),
                    }
                    let src = &sources.get(ck).cloned().unwrap_or(Value::Null);
                    if s(src, "url") != s(&c["source"], "url") || s(src, "date") != s(&c["source"], "date") || s(src, "url").is_empty() {
                        bad.push(format!("{label}: stages.json sources does not carry its source {}", c["source"]));
                    }
                }
            }
            other => bad.push(format!("{key}: outcome {other:?}, want applied or rejected")),
        }
    }
    assert!(bad.is_empty(), "the review's findings are not all addressed ({} problems):\n  {}", bad.len(), bad.join("\n  "));
}

/// Every source in stages.json belongs to a statement some ladder names, and gives a url and a
/// date; and every statement reads like the rest: it starts "I can" and ends with a full stop.
///
/// Seen red 2026-11-14 before the review was applied: "skills.baking.s1_a: \"I can bake a
/// flatbread following the steps\" does not end with a full stop" (the six baking statements had
/// none, which the review noted as style).
#[test]
fn statement_sources_and_words_are_well_formed() {
    let en = json("data/i18n/en.json");
    let stages = json("data/skills/stages.json");
    let at = ladder_keys(&stages);
    let mut bad = Vec::new();
    for (k, src) in stages["sources"].as_object().cloned().unwrap_or_default() {
        if !at.contains_key(&k) {
            bad.push(format!("sources names {k}, which no ladder holds"));
        }
        if s(&src, "url").is_empty() || s(&src, "date").is_empty() {
            bad.push(format!("{k}: a source needs a url and a date"));
        }
    }
    for k in at.keys() {
        match words(&en, k) {
            Some(t) if t.starts_with("I can") && t.ends_with('.') => {}
            Some(t) if !t.starts_with("I can") => bad.push(format!("{k}: {t:?} does not start \"I can\"")),
            Some(t) => bad.push(format!("{k}: {t:?} does not end with a full stop")),
            None => bad.push(format!("{k}: not in en.json")),
        }
    }
    assert!(bad.is_empty(), "the statement data is not well formed:\n  {}", bad.join("\n  "));
}
