//! What the page shows of a how-to and a run, as plain rules the native page and the tests share.
//! The website's copy of these rules is web/shared/how-to-logic.js (`visibleSteps`, `inShort`,
//! `currentStep`, `progress`, `removedSteps`, `search`), the same names.

use super::model::{HowTo, Step};
use super::runs::Run;

/// The two ways of reading a how-to (docs/design/how-to.md, "Safety, hazard, and the two reading
/// views"). Short: the steps' words, every safety note's short line, the checks and timers, the
/// stop-and-call box. Full: everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Full,
    Short,
}

/// The steps a run shows, in order: a step with an `if` on a choose step shows once that option
/// is picked. A step with an `if` on `place` always shows, labelled with its place, until the
/// person's place is a saved setting (increment 7): hiding a step on a place we do not know could
/// hide a legal or safety note from the person it applies to.
pub fn visible_steps<'a>(h: &'a HowTo, run: Option<&Run>) -> Vec<&'a Step> {
    h.steps
        .iter()
        .filter(|s| {
            s.when.iter().all(|(key, want)| {
                key == "place" || run.and_then(|r| r.choices.get(key)).map(|got| got == want).unwrap_or(false)
            })
        })
        .collect()
}

/// The steps `view` shows of those visible.
pub fn shown<'a>(h: &'a HowTo, run: Option<&Run>, view: View) -> Vec<&'a Step> {
    visible_steps(h, run).into_iter().filter(|s| view == View::Full || s.in_short()).collect()
}

/// The step a run is on: the first visible step not yet done.
pub fn current_step<'a>(h: &'a HowTo, run: &Run) -> Option<&'a Step> {
    visible_steps(h, Some(run)).into_iter().find(|s| !run.is_done(&s.id))
}

/// Where a run is, for "Step 3 of 9": the current step's place among the visible steps, and how
/// many there are; None when every visible step is done. The place, not the count done, because a
/// check answered out of order (or a No that sent the run back) leaves steps done after it.
pub fn position(h: &HowTo, run: &Run) -> Option<(usize, usize)> {
    let v = visible_steps(h, Some(run));
    let at = v.iter().position(|s| !run.is_done(&s.id))?;
    Some((at + 1, v.len()))
}

/// Steps done and visible steps.
pub fn progress(h: &HowTo, run: &Run) -> (usize, usize) {
    let v = visible_steps(h, Some(run));
    (v.iter().filter(|s| run.is_done(&s.id)).count(), v.len())
}

/// Step ids a run has state for that the how-to no longer has: "This step was removed since you
/// started".
pub fn removed_steps(h: &HowTo, run: &Run) -> Vec<String> {
    run.steps.keys().filter(|id| !h.steps.iter().any(|s| &s.id == *id)).cloned().collect()
}

/// Whether `h` matches a search: its title, summary, who it is for, its steps, its tools and
/// materials, and the SIGNS of its problems, because people arrive with a symptom ("lid pops up"),
/// not a title. `q` is matched word by word, every word somewhere.
pub fn matches(h: &HowTo, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let hay = search_text(h);
    q.split_whitespace().all(|w| hay.contains(w))
}

/// Everything search reads, lower case.
pub fn search_text(h: &HowTo) -> String {
    let mut parts: Vec<&str> = vec![&h.title, &h.summary, &h.for_whom];
    for s in &h.steps {
        parts.push(&s.text);
    }
    for p in &h.problems {
        parts.push(&p.sign);
        parts.push(&p.fix);
    }
    let items: Vec<String> = h.tools.iter().chain(&h.materials).filter_map(|n| n.item.as_deref().or(n.proposed_item.as_deref())).map(item_words).collect();
    let mut out = parts.join(" ").to_lowercase();
    for i in items {
        out.push(' ');
        out.push_str(&i);
    }
    out
}

/// An item id as words: `furnace_filter_0` reads "furnace filter".
pub fn item_words(id: &str) -> String {
    let base = id.strip_suffix("_0").unwrap_or(id);
    base.replace('_', " ").to_lowercase()
}
