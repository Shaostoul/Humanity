//! The talk card (rung 3 of docs/design/embeddable-ai.md, 2026-11-12): its choices come from data and
//! work with no AI; the "Say anything" box answers through the fake engine with the AI tag on the line;
//! an engine that fails falls back to a written line, in words.

use std::time::{Duration, Instant};

use egui::{Pos2, Rect, Vec2};

use super::chat::{Availability, Phase, Said};
use super::{book, Opening, TalkCard, View};
use crate::ai::client::{AiClient, ClientConfig};
use crate::ai::talk::{Character, Grants};
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::theme::Theme;
use crate::gui::GuiState;

fn data() -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("HUMANITY_REPO_ROOT"), "/data"))
}

fn registries() -> (humanity_systems::quests::QuestRegistry, humanity_systems::quests::DestinationList) {
    let quests = humanity_systems::quests::QuestRegistry::load(&data());
    let places = humanity_systems::quests::DestinationList::from_ron(&std::fs::read(data().join("entities/destinations.ron")).expect("destinations")).expect("destinations parse");
    (quests, places)
}

/// A card opened for `name` (a crew member of data/npc/crew.ron) as E does, with `availability` for the
/// AI box instead of what this computer has installed (so the test does not depend on it).
fn open(gs: &mut GuiState, name: &str, availability: Availability) {
    super::open_for_test(gs, name, availability);
}

fn frame(ctx: &egui::Context, theme: &Theme, gs: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))), events, ..Default::default() };
    ctx.run(input, |ctx| super::card::draw(ctx, theme, gs))
}

fn click(ctx: &egui::Context, theme: &Theme, gs: &mut GuiState, label: &str) {
    let out = frame(ctx, theme, gs, Vec::new());
    let pos = find_text_in_shapes(&out.shapes, label).unwrap_or_else(|| panic!("{label:?} is drawn")).rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, gs, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, gs, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, gs, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

fn ctx_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

/// Whether the AI tag ("AI", exactly, on its own) is on the card.
fn has_ai_tag(out: &egui::FullOutput) -> bool {
    find_text_in_shapes(&out.shapes, humanity_core::i18n::t("ai.tag")).is_some_and(|f| f.text == humanity_core::i18n::t("ai.tag"))
}

/// THE CARD'S CHOICES COME FROM DATA AND WORK WITH NO AI (CLAUDE.md: "AI is optional, never
/// required"; the card the operator approved on 2026-11-11). Botanist Yara with nothing installed:
/// Talk about gives talk.ron's lines for the topic and cycles them; Work lists her tasks from talk.ron
/// with their states, and Take it on asks the game for the quest; Show me the way marks the place for
/// the HUD with her line; she does not trade, so there is no Trade; CB-7 trades, and Trade opens the
/// trading post. No AI tag anywhere, and the one-time offer of AI is there and can be dismissed.
/// Seen red 2026-11-12 with the role level left out of `TalkBook::resolve`: "\"Your job\" is drawn"
/// (she had no Your job topic at all: only `everyone` has lines, and it has none for a job).
#[test]
fn the_cards_choices_come_from_data_and_work_with_no_ai() {
    let (ctx, theme) = ctx_theme();
    let mut gs = GuiState::default();
    open(&mut gs, "Botanist Yara", Availability::NoAi);
    let book = book::TalkBook::current();
    let r = book.resolve("Botanist Yara", "botanist");
    assert_eq!(gs.talk.chat.phase, Phase::Off, "no AI: the box is off");
    frame(&ctx, &theme, &mut gs, Vec::new());
    let out = frame(&ctx, &theme, &mut gs, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, "Botanist Yara").is_some() && find_text_in_shapes(&out.shapes, "Tending the Commons garden").is_some(), "her name and job");
    assert!(find_text_in_shapes(&out.shapes, humanity_core::i18n::t("talk.trade")).is_none(), "she does not trade");
    assert!(!has_ai_tag(&out), "no AI tag with no AI");

    click(&ctx, &theme, &mut gs, humanity_core::i18n::t("talk.talk_about"));
    assert_eq!(gs.talk.view, View::Topics);
    click(&ctx, &theme, &mut gs, humanity_core::i18n::t("talk.topic.job"));
    let job_lines = &r.topics.iter().find(|(t, _)| t == "job").expect("Yara talks about her job from talk.ron's botanist lines").1;
    assert_eq!(gs.talk.said, Some(Said::Written(job_lines[0].clone())), "Yara talks about her job from talk.ron's botanist lines");
    gs.npc_talk_advance();
    assert_eq!(gs.talk.said, Some(Said::Written(job_lines[1].clone())), "E keeps to the topic");

    click(&ctx, &theme, &mut gs, humanity_core::i18n::t("talk.back"));
    click(&ctx, &theme, &mut gs, humanity_core::i18n::t("talk.work"));
    let out = frame(&ctx, &theme, &mut gs, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, "Start Farming").is_some(), "her first task, by its quest name");
    let locked = humanity_core::i18n::tf("talk.offer.locked", &[("name", &"Build First Habitat")]);
    assert!(find_text_in_shapes(&out.shapes, &locked).is_some(), "a task that needs another first says which: {locked}");
    assert!(gs.talk.offers.iter().all(|o| o.state != super::OfferState::Available), "a new player has finished none of the tasks these need");
    // A player who finished the first habitat can take Start Farming on.
    let mut tracker = humanity_systems::quests::QuestTracker::default();
    tracker.completed_quests.push("tutorial_first_habitat".into());
    let (quests, places) = registries();
    let o = Opening { name: "Botanist Yara", role: "botanist", activity: "", lines: Vec::new(), quests: Some(&quests), tracker: Some(&tracker), places: Some(&places) };
    gs.talk.open_with(&o, |_| Availability::NoAi);
    gs.talk.view = View::Work;
    click(&ctx, &theme, &mut gs, humanity_core::i18n::t("talk.take_task"));
    assert_eq!(gs.pending_accept_quest.as_deref(), Some("tutorial_farming_basics"), "Take it on asks the game for the quest");

    gs.talk.view = View::Main;
    click(&ctx, &theme, &mut gs, humanity_core::i18n::t("talk.show_way"));
    click(&ctx, &theme, &mut gs, "the mess hall");
    assert_eq!(gs.talk.guide.as_ref().map(|g| g.dest_id.as_str()), Some("mess_hall"), "the way is marked for the HUD");
    assert!(matches!(&gs.talk.said, Some(Said::Written(l)) if l.to_lowercase().contains("the mess hall")), "and she says where: {:?}", gs.talk.said);

    let offer = humanity_core::i18n::t("talk.ai.offer_dismiss");
    let out = frame(&ctx, &theme, &mut gs, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, offer).is_some(), "the one-time offer of AI");
    click(&ctx, &theme, &mut gs, offer);
    assert!(gs.settings.ai_talk_offer_dismissed, "dismissed, it stays dismissed");

    let mut gs = GuiState::default();
    open(&mut gs, "CB-7", Availability::NoAi);
    click(&ctx, &theme, &mut gs, humanity_core::i18n::t("talk.trade"));
    assert!(gs.vendor_open && gs.npc_talk_target.is_none(), "CB-7 trades: the trading post opens and the card closes");
}

/// Every topic in talk.ron has its words on the card, and every crew member has a card.
#[test]
fn every_topic_has_its_words_and_every_crew_member_a_card() {
    let book = book::TalkBook::current();
    for t in &book.topics {
        let key = format!("talk.topic.{t}");
        assert_ne!(humanity_core::i18n::t_dyn(&key), key, "the topic {t} has words");
    }
    let (quests, places) = registries();
    for c in book::crew() {
        let r = book.resolve(&c.name, &c.role);
        assert!(!r.topics.is_empty(), "{} has something to talk about", c.name);
        assert!(!c.persona.trim().is_empty(), "{} has a persona for AI", c.name);
        for q in &r.work {
            assert!(quests.get(q).is_some(), "{}'s task {q} is a quest", c.name);
        }
        for w in &r.ways {
            assert!(places.destinations.iter().any(|d| d.id == *w), "{}'s place {w} is a destination", c.name);
        }
    }
}

/// The client against the developer's fake engine (built by the client tests), with `args`.
fn fake_client(args: &[&str]) -> AiClient {
    let engine = crate::ai::client_tests::fake_engine();
    // 60 s to say hello, as in ai::client_tests: the fake engine starts below normal priority and
    // missed 10 s while other worktrees compiled (flaky 2026-11-13, data/coordination/flaky-tests.jsonl).
    let config = ClientConfig { args: args.iter().map(|s| s.to_string()).collect(), hello_timeout: std::time::Duration::from_secs(60), ..ClientConfig::default() };
    AiClient::new(Some(engine), config)
}

fn character() -> Character {
    Character { name: "Botanist Yara".into(), job: "botanist".into(), description: "Tending the Commons garden".into(), post: "the Commons".into(), persona: String::new(), examples: vec!["Seeds, soil, light and patience.".into()], doing: "Watering".into() }
}

/// Step the card's box until it finishes a line (or `within` passes).
fn until_said(chat: &mut super::chat::Chat, c: &mut AiClient, within: Duration) -> Option<Said> {
    let until = Instant::now() + within;
    while Instant::now() < until {
        c.poll();
        if let Some(s) = chat.step(c, crate::ai::models::BRAIN_AUTO) {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    None
}

/// THE BOX ANSWERS IN THE SHAPE, WITH THE AI TAG (5.4: "Written by AI on this computer, not by a
/// person", on every line the brain wrote): through the real fake engine (it answers in the talk
/// card's shape), the warm-up reads the persona when the card opens, a question gets a line that
/// passed the shape and the filter, and the card draws it with the AI tag; a written line has none.
/// Seen red 2026-11-12 with the tag left out of `draw_line`'s AI arm: "the AI's line carries the AI
/// tag".
#[test]
fn the_box_answers_in_the_shape_with_the_ai_tag() {
    let mut c = fake_client(&["--fake", "--fake-ms-per-token", "2", "--fake-ms-prefill", "20"]);
    let mut chat = super::chat::Chat::default();
    chat.begin(Availability::Offered, Some(&character()), Grants::default(), vec!["Seeds, soil, light and patience.".into()]);
    chat.dev_fake = true;
    chat.ask("What are you growing?");
    let said = until_said(&mut chat, &mut c, Duration::from_secs(90)).expect("an answer");
    let Said::Ai { line, action } = said.clone() else { panic!("an AI line: {said:?}") };
    assert!(line.contains("engine") && !line.contains('{'), "the fake's scripted line, out of its JSON: {line:?}");
    assert_eq!(action, None);
    assert_eq!(chat.turns.len(), 2, "the question and the answer join the conversation");
    assert!(chat.turns[1].text.starts_with("{\"action\":"), "the model's own reply is what it sees next time: {:?}", chat.turns[1].text);
    c.stop();

    let (ctx, theme) = ctx_theme();
    let mut gs = GuiState::default();
    open(&mut gs, "Botanist Yara", Availability::NoAi);
    gs.talk.said = Some(said);
    frame(&ctx, &theme, &mut gs, Vec::new());
    let out = frame(&ctx, &theme, &mut gs, Vec::new());
    assert!(has_ai_tag(&out), "the AI's line carries the AI tag");
    assert!(find_text_in_shapes(&out.shapes, &line).is_some(), "and the line itself");
    gs.talk.said = Some(Said::Written("Seeds, soil, light and patience.".into()));
    let out = frame(&ctx, &theme, &mut gs, Vec::new());
    assert!(!has_ai_tag(&out), "a written line has no AI tag");
}

/// THE FALLBACK ON ENGINE FAILURE (8.3 and the brief: "fallbacks to the written lines on any failure,
/// refusal or timeout, in words"): an engine that dies on the question (`--fake-exit-on generate`)
/// leaves the character saying one of its written lines, with why in words; a reply outside the shape,
/// and one the filter stops, each fall back the same way. Seen red 2026-11-12 with the warm-up's
/// failure leaving the waiting question unanswered: "a crash falls back to a written line: None" (after
/// 20 s).
#[test]
fn an_engine_failure_falls_back_to_a_written_line_in_words() {
    let written = vec!["Seeds, soil, light and patience.".to_string(), "The lemon tree won't fruit.".to_string()];
    let mut c = fake_client(&["--fake", "--fake-exit-on", "generate"]);
    let mut chat = super::chat::Chat::default();
    chat.begin(Availability::Offered, Some(&character()), Grants::default(), written.clone());
    chat.dev_fake = true;
    chat.ask("Hello?");
    let said = until_said(&mut chat, &mut c, Duration::from_secs(90));
    let Some(Said::Fallback { line, why }) = said.clone() else { panic!("a crash falls back to a written line: {said:?}") };
    assert!(written.contains(&line), "one of her written lines: {line:?}");
    assert_eq!(why, humanity_core::i18n::t("ai.fallback.crashed"), "why, in words");
    assert!(matches!(chat.phase, Phase::Unavailable(_)), "and the box says it cannot answer now: {:?}", chat.phase);

    let mut chat = super::chat::Chat::default();
    chat.begin(Availability::Offered, Some(&character()), Grants::default(), written.clone());
    chat.ask("Hi");
    assert!(matches!(chat.finish("Sure! Here you go."), Said::Fallback { ref why, .. } if why == humanity_core::i18n::t("ai.talk.shape.not_the_shape")), "a reply outside the shape falls back");
    chat.ask("Hi");
    assert!(matches!(chat.finish(r#"{"line":"I am a human, honestly.","action":"none"}"#), Said::Fallback { ref why, .. } if why == humanity_core::i18n::t("ai.talk.filter.person")), "a line the filter stops falls back");
    chat.ask("Hi");
    assert!(matches!(chat.finish(r#"{"line":"Come and trade.","action":"open_trade"}"#), Said::Fallback { ref why, .. } if why == humanity_core::i18n::t("ai.talk.shape.not_granted")), "an action not granted falls back");
}

/// THE FILTER JUDGES THE LINE AS IT STREAMS (8.3, a filter after generation, here run on every piece):
/// a reply whose line breaks a rule partway is stopped there, the request cancelled before its end,
/// and a written line is shown, with why. The fake engine's reply for a first question is "A real
/// engine would answer ..."; a rule naming "engine" stops it at its third word. Seen red 2026-11-12
/// with the filter run only on the whole reply: "only the warm-up was answered: the reply was stopped
/// before its end: Counters { ..., answered: 2, ... }" (the whole reply arrived, then was replaced).
#[test]
fn a_line_that_breaks_a_rule_is_stopped_as_it_streams() {
    let mut c = fake_client(&["--fake", "--fake-ms-per-token", "60", "--fake-ms-prefill", "20"]);
    let mut chat = super::chat::Chat::default();
    chat.begin(Availability::Offered, Some(&character()), Grants::default(), vec!["The lemon tree won't fruit.".into()]);
    chat.dev_fake = true;
    chat.filter = Some(crate::ai::talk::Filter { max_chars: 300, markers: Vec::new(), rules: vec![crate::ai::talk::Rule { why: "person".into(), phrases: vec!["engine".into()] }] });
    chat.ask("Who are you?");
    let said = until_said(&mut chat, &mut c, Duration::from_secs(90));
    assert_eq!(said, Some(Said::Fallback { line: "The lemon tree won't fruit.".into(), why: humanity_core::i18n::t("ai.talk.filter.person").into() }));
    assert_eq!(c.counters.answered, 1, "only the warm-up was answered: the reply was stopped before its end: {:?}", c.counters);
    c.stop();
    let _ = TalkCard::default();
}
