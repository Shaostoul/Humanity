// Dev > Journal (dev_journal.rs): reading a journal folder, clearing it, the kinds' words, and the
// section drawn from a fixture.

use super::*;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("hos-dev-journal-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("marks")).unwrap();
    d
}

fn fixture(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("log.jsonl"), "{\"t\":1000,\"what\":\"first\",\"level\":\"WARN\"}\n{\"t\":2000,\"what\":\"newest warning\",\"level\":\"ERROR\"}\n").unwrap();
    std::fs::write(d.join("marks").join("2.png"), b"png").unwrap();
    let pic = d.join("marks").join("2.png");
    let gone = d.join("marks").join("1.png"); // pruned: listed with no picture
    std::fs::write(
        d.join("mark.jsonl"),
        format!(
            "{}\n{}\n",
            serde_json::json!({ "t": 1500, "what": "Mark 1", "n": 1, "picture": gone, "page": "None", "storey": {} }),
            serde_json::json!({ "t": 2500, "what": "Mark 2", "n": 2, "picture": pic, "page": "Chat", "storey": { "room": "Kitchen" } }),
        ),
    )
    .unwrap();
    d
}

fn kinds() -> Vec<(String, bool)> {
    vec![("log".into(), true), ("panic".into(), true), ("mark".into(), true), ("net".into(), false)]
}

#[test]
fn a_reading_counts_each_kind_shows_its_newest_and_lists_marks_newest_first() {
    let d = fixture("read");
    let s = read_snapshot(&d, &kinds());
    let log = &s.kinds[0];
    assert_eq!((log.id.as_str(), log.count), ("log", 2));
    assert_eq!(log.latest, Some((2000, "newest warning".to_string())));
    assert_eq!(s.kinds[1].count, 0, "no panics, no file");
    assert!(!s.kinds[3].built, "a kind not recorded yet says so");
    assert_eq!(s.marks.iter().map(|m| m.n).collect::<Vec<_>>(), vec![2, 1], "newest first");
    assert_eq!(s.marks[0].place, "Kitchen");
    assert!(s.marks[0].picture.is_some(), "a picture on disk is offered");
    assert!(s.marks[1].picture.is_none(), "a pruned picture is not offered");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn clearing_deletes_every_kind_and_picture_and_keeps_the_folder() {
    let d = fixture("clear");
    clear(&d);
    assert!(d.is_dir() && d.join("marks").is_dir(), "the folders stay for the writer");
    assert_eq!(std::fs::read_dir(&d).unwrap().flatten().filter(|e| e.path().is_file()).count(), 0, "no kind's file left");
    assert_eq!(std::fs::read_dir(d.join("marks")).unwrap().count(), 0, "no picture left");
    assert_eq!(read_snapshot(&d, &kinds()).marks.len(), 0);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn every_kind_in_the_data_has_its_words() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/dev/journal_kinds.json")).unwrap();
    let listed = listed_kinds(&json);
    assert_eq!(listed.len(), 7, "{listed:?}");
    let words: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/i18n/en.json")).unwrap()).unwrap();
    for (id, _) in &listed {
        assert!(words["dev_journal"][format!("kind_{id}")].is_string(), "dev_journal.kind_{id} has words");
    }
}

/// The section drawn from the fixture: the kinds' words and counts, the marks, and the buttons.
#[test]
fn the_section_shows_the_kinds_the_marks_and_its_buttons() {
    let d = fixture("draw");
    let snap = read_snapshot(&d, &kinds());
    let theme = crate::gui::theme::load_theme();
    let ctx = egui::Context::default();
    let out = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let _ = draw_section(ui, &theme, Some(&d), &snap);
        });
    });
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        if let egui::Shape::Text(t) = &clipped.shape {
            texts.push(t.galley.text().to_string());
        }
    }
    let all = texts.join(" | ");
    for want in [
        t("dev_journal.title"),
        t("dev_journal.kind_log"),
        "newest warning",
        t("dev_journal.kind_not_built"),
        t("dev_journal.mark_now"),
        t("dev_journal.open_folder"),
        t("dev_journal.clear"),
        t("dev_journal.open_picture"),
        t("dev_journal.no_picture"),
    ] {
        assert!(all.contains(want), "{want:?} is drawn: {all}");
    }
    assert!(all.contains("#2") && all.contains("Kitchen"), "the newest mark and its room: {all}");
    // Without a running journal the section says so and offers nothing to press.
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            assert_eq!(draw_section(ui, &theme, None, &Snapshot::default()), None);
        });
    });
    let none: Vec<String> = out.shapes.iter().filter_map(|c| if let egui::Shape::Text(t) = &c.shape { Some(t.galley.text().to_string()) } else { None }).collect();
    assert!(none.iter().any(|s| s == t("dev_journal.not_running")), "{none:?}");
    assert!(!none.iter().any(|s| s == t("dev_journal.mark_now")), "no buttons without a journal");
    let _ = std::fs::remove_dir_all(&d);
}
