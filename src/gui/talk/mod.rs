//! THE TALK CARD (rung 3 of docs/design/embeddable-ai.md, 2026-11-12; the card the operator approved
//! on 2026-11-11). Walk up to a crew member, look at them, press E: the card shows their name, their
//! job and what they are doing, their line, and the choices of data/npc/talk.ron (Talk about, Work,
//! Trade, Show me the way, Goodbye), every one WRITTEN, so the whole card works with no AI installed
//! (CLAUDE.md: "AI is optional, never required"). With an AI model installed and Conversational
//! chosen (Settings > AI), a "Say anything" box joins them (`chat`): the character answers in its
//! persona within the fixed shape, through the filter, tagged AI, falling back to its written lines on
//! any failure, refusal or timeout, in words.
//!
//! - [`book`]: talk.ron and crew.ron as the client reads them.
//! - [`chat`]: the conversation with the AI engine (warm-up when the card opens, streaming, timeouts,
//!   fallbacks).
//! - [`card`]: drawing it (hud.rs `draw_npc_talk_card` hands over to it).
//!
//! The card's own state lives in `GuiState::talk` ([`TalkCard`]); the relay's name, chore line, dialog
//! and greetings stay in the `npc_talk_*` fields they always had. Choices that change the game do it
//! the way every page does, through an intent the frame settles: a task through
//! `pending_accept_quest`, trade through `vendor_open`, the way through [`Guide`], which the frame
//! resolves each tick into the HUD's ring (src/engine/talk.rs).

pub mod book;
pub mod card;
pub mod chat;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::ai::talk::{Character, Grants, PlaceMap};
use crate::gui::GuiState;

/// The talk card's entity id when the dev IPC opened it by name (no crew member stands there to walk
/// away from, so the frame's walk-away check leaves it open): the ai-talk rig's card.
pub const DEV_TALK_TARGET: u64 = u64::MAX;

/// Which list of choices the card shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Main,
    Topics,
    Work,
    Ways,
}

/// Where a task offered stands for this player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferState {
    Available,
    /// Taken on: in the player's quests now.
    Taken,
    Done,
    /// Needs another task first (its name).
    Locked(String),
}

/// A task a crew member offers (a quest of data/quests/).
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub quest_id: String,
    pub name: String,
    pub summary: String,
    pub state: OfferState,
}

/// A place a crew member can show the way to (data/entities/destinations.ron).
#[derive(Debug, Clone, PartialEq)]
pub struct Way {
    pub id: String,
    pub label: String,
}

/// "Show me the way": the place the HUD marks until the player gets there. Kept when the card closes.
#[derive(Debug, Clone, PartialEq)]
pub struct Guide {
    pub dest_id: String,
    pub label: String,
    /// Where it is now and how far (metres), worked out by the frame each tick (src/engine/talk.rs);
    /// None while it does not resolve here.
    pub mark: Option<(glam::Vec3, f64)>,
}

/// The card's state (`GuiState::talk`).
#[derive(Default)]
pub struct TalkCard {
    pub name: String,
    pub role: String,
    /// The job in words, from crew.ron's description ("Holding the clinic hour in the Commons").
    pub job: String,
    pub topics: Vec<(String, Vec<String>)>,
    /// The topic Talk about last asked, which E keeps cycling.
    pub topic: Option<String>,
    topic_at: BTreeMap<String, usize>,
    pub offers: Vec<Offer>,
    pub trades: bool,
    pub ways: Vec<Way>,
    pub way_lines: Vec<String>,
    pub accept_lines: Vec<String>,
    pub view: View,
    /// What the card shows as the character's line, once a choice or a question replaced the opening
    /// line (`npc_talk_line`).
    pub said: Option<chat::Said>,
    pub chat: chat::Chat,
    pub guide: Option<Guide>,
    /// Opened by the dev IPC by name (`DEV_TALK_TARGET`).
    pub dev_opened: bool,
    /// The size the one-time offer of AI names (worked out once when the card opens with no AI).
    pub offer_size: String,
}

/// What the card needs from the game when it opens: the crew member and the player's tasks.
pub struct Opening<'a> {
    pub name: &'a str,
    pub role: &'a str,
    pub activity: &'a str,
    /// The relay's written lines for them (dialog, then greetings): the fallback lines.
    pub lines: Vec<String>,
    pub quests: Option<&'a humanity_systems::quests::QuestRegistry>,
    pub tracker: Option<&'a humanity_systems::quests::QuestTracker>,
    pub places: Option<&'a humanity_systems::quests::DestinationList>,
    /// The ship's labelled rooms, zones and corridors the places resolve to (`SHIP_PLACES_KEY`).
    pub ship: Option<&'a humanity_systems::quests::ShipPlaces>,
}

/// THE SHIP'S PLACES A CHARACTER KNOWS (2026-11-13; docs/design/embeddable-ai.md, "His answers on
/// rung 3"), from the data "Show me the way" resolves: the zones aboard with their labelled rooms
/// (where a `NearestVolume` destination such as the mess hall resolves), the corridors joining
/// them, and the destinations out in the open (data/entities/destinations.ron), plus the real
/// names the prompt leaves out (the districts, the player's own front door) for the filter.
pub fn place_map(dests: Option<&humanity_systems::quests::DestinationList>, ship: Option<&humanity_systems::quests::ShipPlaces>) -> PlaceMap {
    use humanity_systems::quests::in_a_sentence;
    let mut map = PlaceMap::default();
    // "Mess hall" inside a sentence: "the mess hall".
    let room = |label: &str| {
        let mut c = label.chars();
        c.next().map(|f| format!("the {}{}", f.to_lowercase(), c.as_str()))
    };
    if let Some(ship) = ship {
        for z in &ship.zones {
            let rooms: Vec<String> = ship.volumes.iter().filter(|v| v.zone_label == *z).filter_map(|v| room(&v.label)).collect();
            map.zones.push((in_a_sentence(z), rooms));
        }
        map.joins = ship.joins.iter().map(|(a, b)| (in_a_sentence(a), in_a_sentence(b))).collect();
        map.also_real.extend(ship.districts.iter().map(|d| in_a_sentence(d)));
    }
    for d in dests.map(|d| d.destinations.as_slice()).unwrap_or_default() {
        if d.at.is_none() {
            map.outdoors.push(d.label.clone());
        } else {
            map.also_real.push(d.label.clone());
        }
    }
    map
}

impl TalkCard {
    /// Fill the card for `o`: its choices from talk.ron, its tasks with their states, its places, and
    /// (when AI is installed and chosen) the persona, grants and grammar for the "Say anything" box.
    /// The guide on the HUD is kept.
    pub fn open(&mut self, o: &Opening, talk_mode: &str) {
        self.open_with(o, |has_persona| chat::availability(talk_mode, has_persona));
    }

    /// [`TalkCard::open`] with what the AI box can offer decided by `availability` (given whether the
    /// character has a persona): the tests' door, so a card's choices are tested without depending on
    /// what this computer has installed.
    pub fn open_with(&mut self, o: &Opening, availability: impl FnOnce(bool) -> chat::Availability) {
        let guide = self.guide.take();
        let mut fresh = TalkCard { guide, ..TalkCard::default() };
        let crew = book::crew();
        let row = crew.iter().find(|c| c.name == o.name);
        let role = if o.role.is_empty() { row.map(|r| r.role.clone()).unwrap_or_default() } else { o.role.to_string() };
        let r = book::TalkBook::current().resolve(o.name, &role);
        fresh.name = o.name.to_string();
        fresh.job = row.map(|r| r.description.clone()).unwrap_or_else(|| book::role_words(&role));
        fresh.role = role.clone();
        fresh.topics = r.topics;
        fresh.trades = r.trades;
        fresh.way_lines = r.way_lines;
        fresh.accept_lines = r.accept_lines;
        fresh.offers = r.work.iter().filter_map(|id| offer(id, o.quests?, o.tracker)).collect();
        fresh.ways = r
            .ways
            .iter()
            .map(|id| Way { id: id.clone(), label: o.places.and_then(|p| p.destinations.iter().find(|d| d.id == *id)).map(|d| d.label.clone()).unwrap_or_else(|| id.replace('_', " ")) })
            .collect();
        let grants = Grants {
            tasks: fresh.offers.iter().filter(|x| x.state == OfferState::Available).map(|x| (x.quest_id.clone(), x.name.clone())).collect(),
            trade: fresh.trades,
            ways: fresh.ways.iter().map(|w| (w.id.clone(), w.label.clone())).collect(),
        };
        let character = row.map(|row| Character {
            name: row.name.clone(),
            job: book::role_words(&row.role),
            description: row.description.clone(),
            post: book::post_words(&row.post),
            persona: row.persona.clone(),
            examples: row.dialog.iter().chain(row.greetings.iter()).cloned().collect(),
            doing: o.activity.to_string(),
            places: place_map(o.places, o.ship),
        });
        let mut fallback = o.lines.clone();
        if fallback.is_empty() {
            fallback = row.map(|r| r.dialog.iter().chain(r.greetings.iter()).cloned().collect()).unwrap_or_default();
        }
        fresh.chat.begin(availability(character.is_some()), character.as_ref(), grants, fallback);
        if fresh.chat.availability == chat::Availability::NoAi {
            fresh.offer_size = card::offer_size();
        }
        *self = fresh;
    }

    /// A topic's next written line (Talk about, and E while a topic is chosen).
    pub fn say_topic(&mut self, topic: &str) {
        let Some((_, lines)) = self.topics.iter().find(|(t, _)| t == topic) else { return };
        if lines.is_empty() {
            return;
        }
        let at = self.topic_at.entry(topic.to_string()).or_insert(usize::MAX);
        *at = if *at == usize::MAX { 0 } else { (*at + 1) % lines.len() };
        self.said = Some(chat::Said::Written(lines[*at].clone()));
        self.topic = Some(topic.to_string());
    }

    /// E pressed while the card is open: a letter while typing in the box (true, nothing else
    /// happens), the chosen topic's next line (true), or else the line clears and the caller cycles
    /// the person's own dialogue (false). Moved out of gui/mod.rs to keep it under its line budget.
    pub fn on_e(&mut self) -> bool {
        if self.chat.typing {
            return true;
        }
        if let Some(topic) = self.topic.clone() {
            self.say_topic(&topic);
            return true;
        }
        self.said = None;
        false
    }

    /// Take on an offered task: the intent the frame settles (`pending_accept_quest`), the line they
    /// say, and the offer shown as taken.
    pub fn take_task(&mut self, quest_id: &str) -> Option<String> {
        let o = self.offers.iter_mut().find(|o| o.quest_id == quest_id && o.state == OfferState::Available)?;
        o.state = OfferState::Taken;
        let name = o.name.clone();
        self.said = Some(chat::Said::Written(pick(&self.accept_lines, &name).replace("{task}", &name)));
        self.chat.grants.tasks.retain(|(id, _)| id != quest_id);
        Some(quest_id.to_string())
    }

    /// Show the way to one of the card's places: the guide the HUD marks, and the line they say.
    pub fn show_way(&mut self, dest_id: &str) {
        let Some(w) = self.ways.iter().find(|w| w.id == dest_id) else { return };
        self.guide = Some(Guide { dest_id: w.id.clone(), label: w.label.clone(), mark: None });
        let line = pick(&self.way_lines, &w.label).replace("{place}", &w.label);
        self.said = Some(chat::Said::Written(capitalise(&line)));
    }
}

/// The offer for quest `id`, with its state for this player; None when the quest does not exist.
fn offer(id: &str, quests: &humanity_systems::quests::QuestRegistry, tracker: Option<&humanity_systems::quests::QuestTracker>) -> Option<Offer> {
    let q = quests.get(id)?;
    let state = match tracker {
        Some(t) if t.is_completed(id) => OfferState::Done,
        Some(t) if t.is_active(id) => OfferState::Taken,
        _ => match q.prerequisite.as_deref() {
            Some(pre) if !tracker.is_some_and(|t| t.is_completed(pre)) => OfferState::Locked(quests.get(pre).map(|p| p.name.clone()).unwrap_or_else(|| pre.to_string())),
            _ => OfferState::Available,
        },
    };
    Some(Offer { quest_id: id.to_string(), name: q.name.clone(), summary: q.description.clone(), state })
}

/// One of `lines` chosen by `seed`'s letters (the same place or task gets the same line, a different
/// one may get another), or the plain `{seed}` when there are none.
fn pick(lines: &[String], seed: &str) -> String {
    if lines.is_empty() {
        return seed.to_string();
    }
    let n = seed.bytes().map(usize::from).sum::<usize>();
    lines[n % lines.len()].clone()
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Open the card for the crew member `npc` (E pressed while looking at them): the relay's fields into
/// `npc_talk_*` as before, and the card's choices and AI from the data.
pub fn open_for(gs: &mut GuiState, npc: &crate::net::sync::RemoteNpc, data: &humanity_core::hot_reload::data_store::DataStore, world: &hecs::World) {
    gs.npc_talk_name = npc.name.clone();
    gs.npc_talk_activity = npc.activity.clone();
    gs.npc_talk_dialog = npc.dialog.clone();
    gs.npc_talk_greetings = npc.greetings.clone();
    gs.npc_talk_index = None;
    let mut q = world.query::<(&humanity_systems::quests::QuestTracker, &humanity_core::ecs::components::Controllable)>();
    let tracker = q.iter().next().map(|(_, (t, _))| t);
    let opening = Opening {
        name: &npc.name,
        role: &npc.role,
        activity: &npc.activity,
        lines: npc.dialog.iter().chain(npc.greetings.iter()).cloned().collect(),
        quests: data.get::<humanity_systems::quests::QuestRegistry>("quest_registry"),
        tracker,
        places: data.get::<humanity_systems::quests::DestinationList>("quest_destinations"),
        ship: data.get::<humanity_systems::quests::ShipPlaces>(humanity_systems::quests::SHIP_PLACES_KEY),
    };
    let mode = gs.settings.ai_talk_mode.clone();
    gs.talk.open(&opening, &mode);
}

/// A card opened for the crew member `name` as E does, with the repository's quests and places and
/// `availability` for the AI box instead of what this computer has installed: the tests' and the
/// snapshots' card.
#[cfg(test)]
pub(crate) fn open_for_test(gs: &mut GuiState, name: &str, availability: chat::Availability) {
    let data = std::path::PathBuf::from(concat!(env!("HUMANITY_REPO_ROOT"), "/data"));
    let quests = humanity_systems::quests::QuestRegistry::load(&data);
    let places = humanity_systems::quests::DestinationList::from_ron(&std::fs::read(data.join("entities/destinations.ron")).expect("destinations")).expect("destinations parse");
    let ship = humanity_systems::quests::ShipPlaces::of_ship(&humanity_ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, None).expect("the shipped ship assembles"));
    let row = book::crew().into_iter().find(|c| c.name == name).expect("a crew member");
    gs.npc_talk_name = row.name.clone();
    gs.npc_talk_activity = "Watering the beds".into();
    gs.npc_talk_dialog = row.dialog.clone();
    gs.npc_talk_greetings = row.greetings.clone();
    gs.npc_talk_line = row.greetings[0].clone();
    gs.npc_talk_target = Some(7);
    let o = Opening { name, role: &row.role, activity: "Watering the beds", lines: row.dialog.clone(), quests: Some(&quests), tracker: None, places: Some(&places), ship: Some(&ship) };
    gs.talk.open_with(&o, |_| availability);
}

/// Open the card by name for the dev IPC (the ai-talk rig): the crew member from this client's
/// crew.ron, no figure in the world needed. Returns false when no crew member has that name.
pub fn open_by_name(gs: &mut GuiState, name: &str, data: &humanity_core::hot_reload::data_store::DataStore, world: &hecs::World) -> bool {
    let Some(row) = book::crew().into_iter().find(|c| c.name == name) else { return false };
    let npc = crate::net::sync::RemoteNpc {
        entity_id: DEV_TALK_TARGET,
        name: row.name.clone(),
        activity: row.description.clone(),
        working: true,
        role: row.role.clone(),
        dialog: row.dialog.clone(),
        greetings: row.greetings.clone(),
        last_position: glam::Vec3::ZERO,
        target_position: glam::Vec3::ZERO,
        facing: humanity_core::turning::Turn::default(),
        heading: 0.0,
        interpolation_t: 1.0,
    };
    open_for(gs, &npc, data, world);
    gs.npc_talk_line = row.greetings.first().or(row.dialog.first()).cloned().unwrap_or_default();
    gs.npc_talk_target = Some(DEV_TALK_TARGET);
    gs.talk.dev_opened = true;
    true
}
