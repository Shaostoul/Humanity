//! The notification rule held to the shared cases (`scripts/tests/fixtures/notify-cases.json`, the
//! website's twin runs the same file), and the pieces around it: the words a notice shows, what
//! each platform is handed, the mutes, and the data file.

use serde_json::Value;

use super::data::File;
use super::decide::{clean, decide, is_mention, words, Candidate, Moment, Synced};
use super::mutes::Mutes;
use super::settings::{LocalSettings, Show};
use super::toast::{linux_args, mac_args, tag_for, toast_xml, Toast};

fn fixture() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/notify-cases.json");
    serde_json::from_str(&std::fs::read_to_string(&path).expect("the shared cases")).expect("they parse")
}

fn shipped() -> File {
    File::parse(crate::embedded_data::get_embedded("notifications/notifications.json").expect("built into the exe")).expect("it parses")
}

/// `over`'s keys replace `base`'s (a shallow merge of one part).
fn merge(base: &Value, over: Option<&Value>) -> Value {
    let mut out = base.clone();
    if let (Some(o), Some(map)) = (over.and_then(Value::as_object), out.as_object_mut()) {
        for (k, v) in o {
            map.insert(k.clone(), v.clone());
        }
    }
    out
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn opt_i(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(Value::as_i64)
}

fn mutes_of(v: &Value) -> Mutes {
    let map = |k: &str| {
        v.get(k)
            .and_then(Value::as_object)
            .map(|o| o.iter().map(|(k, u)| (k.clone(), u.as_i64().unwrap_or(0))).collect())
            .unwrap_or_default()
    };
    Mutes { everything: opt_i(v, "everything"), servers: map("servers"), rooms: map("rooms"), people: map("people") }
}

fn levels_of(v: &Value) -> std::collections::BTreeMap<String, super::Level> {
    v.as_object()
        .map(|o| o.iter().map(|(k, l)| (k.clone(), super::Level::from_id(l.as_str().unwrap_or("")))).collect())
        .unwrap_or_default()
}

/// EVERY SHARED CASE, as the website answers it too. Seen red 2026-10-30 with the mute checks
/// moved after quiet hours: "a muted server during quiet hours reports the mute" answered
/// quiet_hours, not server_muted (the website's twin went red on the same case). Rung 2
/// (2026-10-31) seen red here with the conversation mute skipped: "a DM in a muted conversation"
/// answered notify. The website's twin was seen red on these cases with each other new check
/// broken in turn: the room level skipped, the exceptions ignored, the friends exception granted
/// to anyone, and the list given only with the sound.
#[test]
fn every_shared_case_gets_the_same_answer() {
    let fx = fixture();
    let file = shipped();
    let base = &fx["base"];
    let cases = fx["cases"].as_array().expect("cases");
    assert!(cases.len() >= 70, "the shared cases shrank to {}", cases.len());
    for case in cases {
        let name = s(case, "name");
        let mut cand = base["candidate"].clone();
        if b(case, "mention") {
            cand = merge(&cand, Some(&fx["mention_room"]));
        }
        let cand = merge(&cand, case.get("candidate"));
        let synced = merge(&base["synced"], case.get("synced"));
        let local = merge(&base["local"], case.get("local"));
        let moment = merge(&base["moment"], case.get("moment"));
        let candidate = Candidate {
            kind: s(&cand, "kind"),
            from_me: b(&cand, "from_me"),
            server: s(&cand, "server"),
            room: cand.get("room").and_then(Value::as_str).map(str::to_string),
            conversation: s(&cand, "conversation"),
            sender: s(&cand, "sender"),
            room_name: s(&cand, "room_name"),
            text: s(&cand, "text"),
            from_friend: b(&cand, "from_friend"),
        };
        let qs = synced.get("quiet_start").and_then(Value::as_str).map(str::to_string);
        let qe = synced.get("quiet_end").and_then(Value::as_str).map(str::to_string);
        let sy = Synced {
            dm: b(&synced, "dm"),
            mentions: b(&synced, "mentions"),
            tasks: b(&synced, "tasks"),
            quiet_start: qs.as_deref(),
            quiet_end: qe.as_deref(),
            quiet_friends: b(&synced, "quiet_friends"),
            quiet_mentions: b(&synced, "quiet_mentions"),
            loaded: b(&synced, "loaded"),
        };
        let lo = LocalSettings {
            desktop: b(&local, "desktop"),
            show: Show::from_id(&s(&local, "show")),
            sound: b(&local, "sound"),
            sound_id: "sfx.chat_message".into(),
            mutes: mutes_of(&local["mutes"]),
            room_levels: levels_of(&local["levels"]),
        };
        let at = Moment {
            now: opt_i(&moment, "now").unwrap(),
            minute: opt_i(&moment, "minute").unwrap() as u16,
            focused: b(&moment, "focused"),
            on_screen: b(&moment, "on_screen"),
            last_notice: opt_i(&moment, "last_notice"),
        };
        let got = decide(&file, &candidate, &sy, &lo, &at);
        let want = &case["expect"];
        assert_eq!(got.why.code(), s(want, "why"), "{name}: why");
        assert_eq!(got.toast, b(want, "toast"), "{name}: the notice");
        assert_eq!(got.sound, b(want, "sound"), "{name}: the sound");
        assert_eq!(got.list, b(want, "list"), "{name}: the in-app list");
    }
}

/// THE IN-APP LIST: a burst while the panel stays shut is one entry, opening the panel sees
/// everything, the oldest go past 100, and what was said is kept only when the person's setting
/// shows it. Seen red 2026-10-31 with the fold removed: "a burst is one entry" got two entries.
#[test]
fn the_in_app_list_folds_bursts_counts_unseen_and_keeps_text_only_by_choice() {
    use super::list::{List, KEPT};
    let mut l = List::default();
    let ben = dm("meet me at noon");
    l.add(&ben, Show::Name, 100);
    l.add(&Candidate { sender: "Ben".into(), ..ben.clone() }, Show::Name, 160);
    assert_eq!((l.len(), l.unseen()), (1, 1), "a burst is one entry");
    assert_eq!(l.entries().next().unwrap().at, 160, "with the latest time");
    assert!(l.entries().next().unwrap().text.is_none(), "who and when, never what, by default");
    let cy = Candidate { conversation: "dm:k2".into(), sender: "Cy".into(), ..dm("hi") };
    l.add(&cy, Show::NameAndText, 170);
    assert_eq!(l.entries().next().unwrap().text.as_deref(), Some("hi"), "kept: the person shows the message");
    assert_eq!(l.unseen(), 2);
    l.mark_seen();
    assert_eq!(l.unseen(), 0);
    l.add(&ben, Show::Name, 200);
    assert_eq!((l.len(), l.unseen()), (3, 1), "a seen entry is not folded into: a new one for the new message");
    for i in 0..KEPT + 5 {
        l.add(&Candidate { conversation: format!("dm:x{i}"), ..ben.clone() }, Show::Name, 300 + i as i64);
    }
    assert_eq!(l.len(), KEPT);
    assert_eq!(l.unseen(), KEPT, "the count follows what was dropped");
    l.clear();
    assert!(l.is_empty() && l.unseen() == 0);
}

/// A conversation's mute is that person's, on every server, and leaves rooms alone; a room's level
/// is kept only when it is not the default. Seen red 2026-10-31 with `prune` leaving ended
/// conversation mutes ("ended").
#[test]
fn conversation_mutes_and_room_levels() {
    let mut m = Mutes::default();
    m.mute_person("dm:k1", 100, 3600);
    assert_eq!(m.person_muted("dm:k1", 200), Some(3700));
    assert!(m.person_muted("dm:k2", 200).is_none());
    assert!(!m.is_empty_at(200));
    assert!(m.prune(5000), "ended");
    assert!(m.people.is_empty());
    let mut s = LocalSettings::default();
    assert_eq!(s.room_level("https://a.example", "general"), super::Level::Mentions, "every room starts at mentions");
    s.set_room_level("https://a.example", "general", super::Level::All);
    assert_eq!(s.room_level("https://a.example", "general"), super::Level::All);
    assert_eq!(s.room_level("https://b.example", "general"), super::Level::Mentions, "the same room on another server");
    s.set_room_level("https://a.example", "general", super::Level::Mentions);
    assert!(s.room_levels.is_empty(), "the default is no entry");
    let back: LocalSettings = serde_json::from_str(r#"{"room_levels":{"s|r":"nothing"},"mutes":{"people":{"dm:k":0}}}"#).unwrap();
    assert_eq!(back.room_level("s", "r"), super::Level::Nothing);
    assert_eq!(back.mutes.person_muted("dm:k", 1), Some(0));
}

/// Seen red 2026-10-30 with the boundary check removed: "@Benjamin hi" mentioned Ben.
#[test]
fn mentions_as_the_website_reads_them() {
    let fx = fixture();
    for m in fx["mentions"].as_array().expect("mentions") {
        assert_eq!(is_mention(&s(m, "text"), &s(m, "name")), b(m, "mentioned"), "{:?} for {:?}", s(m, "text"), s(m, "name"));
    }
}

/// Seen red 2026-10-30 with the digits-only check removed: "+5" read as 05:00.
#[test]
fn quiet_hours_clocks_and_windows_as_the_website_reads_them() {
    let fx = fixture();
    for c in fx["clocks"].as_array().expect("clocks") {
        let want = c.get("minutes").and_then(Value::as_u64).map(|m| m as u16);
        assert_eq!(super::quiet::parse_clock(&s(c, "text")), want, "{:?}", s(c, "text"));
    }
    for q in fx["quiet"].as_array().expect("quiet") {
        let (st, en, mi) = (q["start"].as_u64().unwrap() as u16, q["end"].as_u64().unwrap() as u16, q["minute"].as_u64().unwrap() as u16);
        assert_eq!(super::quiet::in_quiet(st, en, mi), b(q, "quiet"), "{st}..{en} at {mi}");
    }
    assert_eq!(super::quiet::format_clock(1320), "22:00");
    assert_eq!(super::quiet::format_clock(425), "07:05");
}

fn dm(text: &str) -> Candidate {
    Candidate {
        kind: "dm".into(),
        server: "https://a.example".into(),
        conversation: "dm:k1".into(),
        sender: "Ben".into(),
        text: text.into(),
        ..Default::default()
    }
}

/// THE PRIVACY DEFAULT: a notice says who, never what, unless the person chose it. Seen red
/// 2026-10-30 with `Show::Name` given the text as its body: the default notice carried "meet me at
/// noon".
#[test]
fn a_notice_shows_the_message_only_when_the_person_chose_it() {
    let file = shipped();
    assert_eq!(LocalSettings::default().show, Show::Name, "the default shows who, not what");
    let c = dm("meet me at noon");
    let (title, body) = words(&file, &c, Show::default());
    assert_eq!(title, "Message from Ben");
    assert!(!body.contains("noon"), "the default body: {body}");
    let (title, body) = words(&file, &c, Show::Nothing);
    assert!(!title.contains("Ben") && !body.contains("Ben") && !body.contains("noon"), "nothing: {title} / {body}");
    let (_, body) = words(&file, &c, Show::NameAndText);
    assert_eq!(body, "meet me at noon");
    let mut m = dm("hey @me");
    m.kind = "mention".into();
    m.room_name = "general".into();
    assert_eq!(words(&file, &m, Show::Name).0, "Ben mentioned you in #general");
}

/// A notice is two short lines: control characters and new lines go, runs of spaces close up, and
/// the cut never splits a character. Seen red 2026-10-30 with runs left open: "a  b   c".
#[test]
fn notice_text_is_cleaned_and_cut_between_characters() {
    assert_eq!(clean("  a\n\tb\u{7}  c ", 80), "a b c");
    let long = "\u{e9}".repeat(300);
    let cut = clean(&long, 10);
    assert_eq!(cut.chars().count(), 10);
    assert!(cut.ends_with('\u{2026}'));
    assert_eq!(clean("", 10), "");
}

/// A name cannot add a button, a link or a second line to a Windows notice: every value is
/// escaped. Seen red 2026-10-30 with the title left unescaped: a name holding `</text><actions>`
/// reached the XML as markup.
#[test]
fn a_toast_cannot_be_made_into_something_else() {
    let t = Toast {
        title: "Eve</text><actions><action content=\"x\" arguments=\"https://evil\"/></actions><text>".into(),
        body: "a & b < c 'd' \"e\"".into(),
        tag: tag_for("dm:k1"),
        group: "dm".into(),
    };
    let xml = toast_xml(&t);
    assert!(!xml.contains("<actions>"), "{xml}");
    assert!(xml.contains("Eve&lt;/text&gt;&lt;actions&gt;"), "{xml}");
    assert!(xml.contains("a &amp; b &lt; c &apos;d&apos; &quot;e&quot;"), "{xml}");
    assert!(xml.contains("<audio silent=\"true\"/>"), "the app plays its own sound: {xml}");
    let empty = Toast { body: String::new(), ..t.clone() };
    assert_eq!(toast_xml(&empty).matches("<text>").count(), 1, "no empty second line");

    let args = linux_args(&Toast { title: "-v".into(), body: "<b>hi</b>".into(), ..t.clone() });
    let dashdash = args.iter().position(|a| a == "--").expect("options end");
    assert_eq!(args[dashdash + 1], "-v", "a title starting with - is a title");
    assert_eq!(args[dashdash + 2], "&lt;b&gt;hi&lt;/b&gt;", "the body's markup is escaped");

    let mac = mac_args(&Toast { title: "-e".into(), body: "do shell script \"x\"".into(), ..t });
    let script: Vec<&str> = mac[..6].iter().map(String::as_str).collect();
    assert_eq!(
        script,
        ["-e", "on run argv", "-e", "display notification (item 2 of argv) with title (item 1 of argv)", "-e", "end run"],
        "the script is fixed: the words are never pasted into it"
    );
    assert_eq!(mac[mac.len() - 2], "\u{2060}-e", "a word starting with - is never an option");
    assert_eq!(mac[mac.len() - 1], "do shell script \"x\"", "the body is an argument, as it is");
}

/// The tag replaces a conversation's earlier notice: stable, short, and naming nothing. Seen red
/// 2026-10-30 with the conversation itself as its tag.
#[test]
fn a_notice_tag_is_stable_short_and_names_nothing() {
    let a = tag_for("dm:ab12cd34");
    assert_eq!(a, tag_for("dm:ab12cd34"));
    assert_ne!(a, tag_for("dm:ab12cd35"));
    assert_eq!(a.len(), 16);
    assert!(!a.contains("ab12"));
}

/// Mutes end by themselves, and the saved list drops what ended. Seen red 2026-10-30 with a mute
/// still on at its last second (`>=`).
#[test]
fn mutes_end_and_are_pruned() {
    let mut m = Mutes::default();
    m.mute_server("s", 100, 3600);
    m.mute_room("s", "general", 100, 0);
    m.mute_everything(100, 60);
    assert_eq!(m.server_muted("s", 200), Some(3700));
    assert_eq!(m.room_muted("s", "general", 1_000_000), Some(0), "until unmuted");
    assert!(m.room_muted("t", "general", 200).is_none(), "the same room on another server");
    assert_eq!(m.everything_muted(159), Some(160));
    assert!(m.everything_muted(160).is_none(), "ended on the second");
    assert!(m.prune(5000));
    assert!(m.servers.is_empty() && m.everything.is_none());
    assert_eq!(m.rooms.len(), 1, "until-unmuted stays");
    assert!(!m.prune(5000), "nothing more to drop");
    m.unmute_room("s", "general");
    assert!(m.is_empty_at(5000));
}

/// The data file built into the exe parses, and every word it names is in English, so no notice and
/// no Settings row ever shows a key. Seen red 2026-10-30 with one kind's `title_hidden` misspelt,
/// and 2026-10-31 with `reply` marked unbuilt.
#[test]
fn the_data_file_parses_and_every_word_it_names_is_in_english() {
    let file = shipped();
    assert!(file.kinds.iter().filter(|k| k.built).count() >= 5, "rung 2: DMs, mentions, replies, contact requests, every line");
    for id in ["dm", "mention", "reply", "contact_request", "room_line"] {
        assert!(file.kind(id).is_some_and(|k| k.built), "{id} is built");
    }
    // A room set to nothing never notifies: every room kind needs at least mentions.
    for k in file.kinds.iter().filter(|k| k.scope == super::data::Scope::Room) {
        assert!(k.level.unwrap_or_default() > super::Level::Nothing, "{} would notify in a room set to nothing", k.id);
    }
    let level_ids: Vec<super::Level> = file.levels.iter().map(|l| l.id).collect();
    assert_eq!(level_ids, [super::Level::All, super::Level::Mentions, super::Level::Nothing], "the right-click's choices");
    let mut keys: Vec<&str> = Vec::new();
    for k in &file.kinds {
        keys.extend([k.label.as_str(), k.hint.as_str(), k.title.as_str(), k.title_hidden.as_str(), k.body_hidden.as_str()]);
    }
    for l in &file.levels {
        keys.extend([l.label.as_str(), l.hint.as_str()]);
    }
    for m in &file.mute_for {
        keys.extend([m.label.as_str(), m.hint.as_str()]);
    }
    for snd in &file.sounds {
        keys.push(snd.label.as_str());
    }
    for key in keys {
        assert!(crate::i18n::has(key), "data/notifications/notifications.json names {key}, which en.json lacks");
    }
    assert!(file.mute_for.iter().any(|m| m.seconds == 0), "until I unmute");
    assert!(file.sound(&LocalSettings::default().sound_id).is_some_and(|s| s.id == "sfx.chat_message"), "the default sound is listed");
    let disk = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/notifications/notifications.json")).unwrap();
    assert_eq!(disk, crate::embedded_data::get_embedded("notifications/notifications.json").unwrap(), "the exe's copy is the data folder's");
}

/// This device's choices survive a restart, and a config.json from before them loads with the safe
/// defaults (nobody uses the app yet, so no other old shape is read). Seen red 2026-10-30 with an
/// unknown choice read as "who and the message".
#[test]
fn local_settings_survive_config_json() {
    let old: LocalSettings = serde_json::from_str("{}").unwrap();
    assert_eq!(old, LocalSettings::default());
    let mut s = LocalSettings { show: Show::NameAndText, desktop: false, sound_id: "sfx.notification".into(), ..Default::default() };
    s.mutes.mute_server("https://a.example", 10, 0);
    let back: LocalSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back, s);
    assert_eq!(Show::from_id("whatever"), Show::Name, "an unknown choice is the safe default");
}
