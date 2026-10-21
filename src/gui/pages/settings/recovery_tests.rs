// Child of settings/recovery.rs: Recovery through trusted people, in Settings > Account
// (2026-10-20, when the Platform tab was retired). Headless: the section is drawn into an egui
// context with no window and the text it painted is read back. No lookup is clicked, so no test
// reaches a server. Each test was seen failing once; what it saw red is at the test.

use super::*;
use crate::gui::theme::load_theme;

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

/// Every text drawn, with where.
fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    out.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, &mut found);
    }
    found
}

/// Draw `body` in a window tall enough that nothing is cut off, settled over three frames.
fn drawn(state: &mut GuiState, body: impl Fn(&mut egui::Ui, &Theme, &mut GuiState)) -> Vec<(String, egui::Rect)> {
    let (ctx, theme) = ctx_and_theme();
    let mut last = None;
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 12000.0))),
            ..Default::default()
        };
        last = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| body(ui, &theme, state));
        }));
    }
    texts(&last.expect("three frames ran"))
}

fn top_of(t: &[(String, egui::Rect)], text: &str) -> Option<f32> {
    t.iter().find(|(s, _)| s == text).map(|(_, r)| r.top())
}

/// SETTINGS > ACCOUNT HAS RECOVERY, RIGHT AFTER SECURITY: the section that was the Platform tab's
/// Recovery page is drawn in Settings > Account, below the Security subsection (which follows
/// Identity, where the recovery phrase is) and above Donation addresses.
///
/// Seen red 2026-10-20, before Settings drew it: "Recovery through trusted people is drawn in
/// Settings > Account".
#[test]
fn settings_account_has_recovery_right_after_security() {
    let mut state = GuiState::default();
    let t = drawn(&mut state, |ui, theme, state| crate::gui::pages::settings::draw_account_content(ui, theme, state));
    let identity = top_of(&t, "Identity").expect("Identity is drawn");
    let security = top_of(&t, "Security").expect("Security is drawn");
    let recovery = top_of(&t, TITLE).expect("Recovery through trusted people is drawn in Settings > Account");
    assert!(identity < security && security < recovery, "Recovery comes after Identity and Security: {identity} {security} {recovery}");
    if let Some(donation) = top_of(&t, "Donation addresses") {
        assert!(recovery < donation, "Recovery comes before Donation addresses: {recovery} {donation}");
    }
}

/// THE WORDS ARE FOR PEOPLE, AND SAY IT IS NOT FINISHED: none of the developer words the old page
/// used (the algorithm and object names, the routes, "DID"), a plain "Not finished yet", and the
/// two lookups still there.
///
/// Seen red 2026-10-20 with the old page's words in place: "the section says Shamir:
/// Recovery through trusted people ...".
#[test]
fn recovery_words_are_for_people_and_say_it_is_not_finished() {
    let mut state = GuiState::default();
    let t = drawn(&mut state, |ui, theme, state| draw(ui, theme, state, theme.info()));
    let all: Vec<&str> = t.iter().map(|(s, _)| s.as_str()).collect();
    let all = all.join("\n");
    for word in [
        "Shamir", "Kyber", "DID", "signed_object", "signed object", "key_rotation", "recovery_share",
        "recovery_request", "recovery_approval", "Dilithium", "pubkey", "/api/", "ciphertext", "threshold",
    ] {
        assert!(!all.contains(word), "the section says {word}:\n{all}");
    }
    assert!(all.contains("Not finished yet."), "it says plainly that it is not finished:\n{all}");
    assert!(all.contains("Does an account have guardians?"), "the setup lookup is there");
    assert!(all.contains("Are you a guardian?"), "the guardian's lookup is there");
    assert!(all.contains("Look up") && all.contains("Show the pieces I keep"), "both lookups have their button");
}

/// THE ANSWERS ARE READ IN THE SHAPE THE SERVER SENDS: the server's own `RecoverySetup` and
/// `RecoveryShareIndex`, turned to JSON the way its handlers do, come back as what the section
/// shows. A renamed field on either side breaks this, not a person's lookup.
///
/// Seen red 2026-10-20 with `parse_setup` reading "total" for "total_shares": "left: None, right:
/// Some(Setup { needed: 2, guardians_total: 3, guardians: [...] })".
#[test]
fn reads_the_answers_the_server_sends() {
    let setup = crate::relay::storage::RecoverySetup {
        holder_did: "did:hum:Owner1".into(),
        guardians: vec!["did:hum:G1".into(), "did:hum:G2".into(), "did:hum:G3".into()],
        threshold: 2,
        total_shares: 3,
    };
    let v = serde_json::to_value(&setup).unwrap();
    assert_eq!(
        parse_setup(&v),
        Some(Setup {
            needed: 2,
            guardians_total: 3,
            guardians: vec!["did:hum:G1".into(), "did:hum:G2".into(), "did:hum:G3".into()],
        })
    );
    let piece = crate::relay::storage::RecoveryShareIndex {
        share_object_id: "obj1".into(),
        holder_did: "did:hum:Owner1".into(),
        guardian_did: "did:hum:G1".into(),
        threshold: 2,
        total_shares: 3,
        created_at: 0,
    };
    let v = serde_json::to_value(vec![piece]).unwrap();
    assert_eq!(parse_pieces(&v), vec![HeldPiece { owner: "did:hum:Owner1".into(), needed: 2, guardians_total: 3 }]);
    assert!(parse_pieces(&serde_json::json!([])).is_empty(), "no pieces reads as none");
    assert_eq!(parse_setup(&serde_json::json!({"error": "no recovery setup"})), None);
}

/// AN ACCOUNT IS NAMED THE WAY THE SERVER FILES IT: your own account id is the server's id for
/// your key, an empty field asks about your own, and anything that is not an account id is turned
/// away with words before it reaches an address.
///
/// Seen red 2026-10-20 with `chosen` accepting any text: "assertion failed: chosen(\"bob\",
/// &mine).is_err()".
#[test]
fn an_account_is_named_the_way_the_server_files_it() {
    let key = [7u8; 64];
    let mine = account_id(&hex::encode(key));
    assert_eq!(mine, Some(crate::relay::core::did::did_for_pubkey(&key)));
    assert_eq!(account_id(""), None, "without a key there is nothing to name");
    assert_eq!(chosen("  ", &mine), Ok(mine.clone().unwrap()), "empty asks about your own");
    assert_eq!(chosen("did:hum:3xYz", &mine), Ok("did:hum:3xYz".to_string()));
    assert!(chosen("bob", &mine).is_err());
    assert!(chosen("did:hum:", &mine).is_err());
    assert!(chosen("did:hum:../x", &mine).is_err());
    assert!(chosen("", &None).is_err(), "no identity on this device and nothing typed");
}
