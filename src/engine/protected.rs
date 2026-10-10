//! The protected setup (step G of docs/design/blocking-and-safe-mode.md, section 10h,
//! 2026-10-10): the native client's half that touches the app state and the socket. The rules
//! themselves (the preset, the PIN's verifier and its wait, what needs the PIN, the review rows,
//! the phrase check) are src/net/protected.rs; Settings > Safety's section and the PIN prompt are
//! src/gui/pages/safety_protected.rs. Where 10h left a choice open, this does what the web chat
//! does (web/shared/protected.js, web/chat/chat-protected.js), so both apps behave alike.
//!
//! THE GATE (`allows`): every locked action's own entry point asks it first (the "Who can reach
//! me" row and tick in engine/reach.rs, Follow in engine/dm.rs, Accept and Send request in
//! engine/reach.rs, the friend-code and group-ticket and voice-room joins on the chat page, and
//! the setup's own switches through `perform`). With the setup off, or for an action that never
//! needs the PIN, it says yes. Otherwise it opens the PIN prompt for that action and says no; a
//! right PIN leaves a one-shot permission for exactly that action and runs it again (`perform`),
//! so each action has one code path whether or not the PIN was asked for.
//!
//! THE APPROVED LIST: while it is on, a pass goes only to a friend the PIN holder let be one (kept
//! in the review step, or befriended with the PIN since), checked where every pass is minted
//! (engine/dm.rs `mint_pass`). So a follow made before the setup and followed back after it makes
//! no friend without the PIN. Befriending someone already on the list needs no PIN; Unfollow and
//! Block take them off it.
//!
//! TURNING IT ON (10h, in order): Read, Choose a PIN, Review (with Remove for each friend, group
//! and voice room), then "Keep the rest" applies: the preset's rows go out as ONE ordinary
//! `reach_set` and everything else is local. It needs a live connection (offline it says so and
//! stays off), and it is never sent again on a reconnect. Nothing else is sent, no flag is set
//! anywhere a server can see, and nothing about it is written into the self-sync notes or any
//! export: the state is `GuiState::protected.setup`, saved in config.json only.
//!
//! It never asks the operating system for anyone's age (10h, and the California finding).

use crate::gui::GuiState;
use crate::net::protected::{PinPrompt, PinTry, PinVerifier, PromptMode, ProtectedAction, ProtectedSetup, ReviewItem, SetupStep};

/// Unix seconds now, for the PIN's wait.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Load the preset (data/gui/safety_presets.json, or the copy built into the exe) the first time
/// it is needed. A file that cannot be read is logged once; without a preset the setup cannot be
/// turned on.
pub(crate) fn ensure_preset(gs: &mut GuiState) -> bool {
    if gs.protected.preset.is_some() {
        return true;
    }
    if gs.protected.preset_error.is_some() {
        return false;
    }
    match crate::net::protected::load_preset(&crate::data_dir()) {
        Ok(preset) => {
            gs.protected.preset = Some(preset);
            true
        }
        Err(e) => {
            log::warn!("{e}; the protected setup cannot be turned on in this run");
            gs.protected.preset_error = Some(e);
            false
        }
    }
}

/// Is the protected setup on?
pub(crate) fn is_on(gs: &GuiState) -> bool {
    gs.protected.setup.on
}

/// A label from the preset (empty when it is not loaded, which `ensure_preset` makes rare).
fn label(gs: &GuiState, pick: impl Fn(&crate::net::protected::Labels) -> String) -> String {
    gs.protected.preset.as_ref().map(|p| pick(&p.labels)).unwrap_or_default()
}

// ── The gate and the PIN prompt ─────────────────────────────────────────────────────────────

/// THE GATE: may `action` go ahead now? Yes when the setup is off, when the action never needs
/// the PIN (Block, Report, Unfollow, leaving a group or room), when it befriends someone the PIN
/// holder already let be a friend, or when a right PIN was just entered for exactly this action
/// (a one-shot permission, used up here; for a befriending action it also puts them on the
/// approved list). Otherwise the PIN prompt opens for it and the answer is no.
pub(crate) fn allows(gs: &mut GuiState, action: ProtectedAction) -> bool {
    if !gs.protected.setup.on || !action.needs_pin() {
        return true;
    }
    if action.befriends().is_some_and(|k| gs.protected.setup.is_approved(k)) {
        return true;
    }
    if gs.protected.granted.as_ref() == Some(&action) {
        gs.protected.granted = None;
        if let Some(key) = action.befriends() {
            approve(gs, key);
        }
        return true;
    }
    ensure_preset(gs);
    gs.protected.prompt = Some(PinPrompt { action: Some(action), mode: PromptMode::Pin });
    gs.protected.prompt_input.clear();
    gs.protected.prompt_line.clear();
    false
}

/// The composer's check (chat.rs `send_composed_content`): a typed `/redeem <code>` makes a friend,
/// so it needs the PIN; anything else goes as before.
pub(crate) fn allows_typed(gs: &mut GuiState, text: &str) -> bool {
    match crate::net::protected::typed_command(text) {
        Some(action) => allows(gs, action),
        None => true,
    }
}

/// Put `key` on the approved list (no effect while the setup is off).
fn approve(gs: &mut GuiState, key: &str) {
    if gs.protected.setup.on && !key.is_empty() && !gs.protected.setup.is_approved(key) {
        gs.protected.setup.approved.push(key.to_string());
        gs.settings_dirty = true;
    }
}

/// `key` is no longer a friend (Unfollow, Block, from here or another of our devices): a new
/// friendship with them needs the PIN again.
pub(crate) fn forget(gs: &mut GuiState, key: &str) {
    let before = gs.protected.setup.approved.len();
    gs.protected.setup.approved.retain(|k| !k.eq_ignore_ascii_case(key));
    if gs.protected.setup.approved.len() != before {
        gs.settings_dirty = true;
    }
}

/// May this device give `key` a pass now? Always while the setup is off; while on, only a friend
/// the PIN holder let be one (engine/dm.rs `mint_pass` asks before every pass).
pub(crate) fn pass_allowed(gs: &GuiState, key: &str) -> bool {
    gs.protected.setup.pass_allowed(key)
}

/// One try at the PIN at `now`: counts wrong tries in a row and starts the preset's wait after
/// the preset's number of them. The count and the wait are saved with the settings, so closing
/// the app does not end a wait.
pub(crate) fn try_pin(gs: &mut GuiState, pin: &str, now: u64) -> PinTry {
    ensure_preset(gs);
    let Some((max_wrong, wait)) = gs.protected.preset.as_ref().map(|p| (p.pin_wrong_tries_before_wait, p.pin_wait_seconds)) else {
        return PinTry::Wrong; // no rules to check against: a lock that cannot check stays locked
    };
    let answer = gs.protected.setup.try_pin(pin, now, max_wrong, wait);
    if !matches!(answer, PinTry::Waiting(_) | PinTry::NoPin) {
        gs.settings_dirty = true;
    }
    answer
}

/// The PIN prompt's Continue in its PIN mode, at `now`: a right PIN closes the prompt, leaves the
/// one-shot permission for its action and does it; a wrong one says so (and, the preset's number
/// of times in a row, how long to wait). The typed PIN is cleared either way.
pub(crate) fn submit_prompt(gs: &mut GuiState, now: u64) {
    let Some(prompt) = gs.protected.prompt.clone().filter(|p| p.mode == PromptMode::Pin) else { return };
    let pin = std::mem::take(&mut gs.protected.prompt_input);
    let answer = try_pin(gs, &pin, now);
    match answer {
        PinTry::Right | PinTry::NoPin => {
            close_prompt(gs);
            if let Some(action) = prompt.action {
                gs.protected.granted = Some(action.clone());
                perform(gs, action);
                // A permission the action did not use (it found nothing to do) does not linger.
                gs.protected.granted = None;
            }
        }
        PinTry::Wrong => gs.protected.prompt_line = label(gs, |l| l.pin_wrong.clone()),
        PinTry::WaitStarted(secs) | PinTry::Waiting(secs) => gs.protected.prompt_line = label(gs, |l| l.pin_wait(secs)),
    }
}

/// Close the PIN prompt without doing anything; whatever was typed is cleared.
pub(crate) fn cancel_prompt(gs: &mut GuiState) {
    close_prompt(gs);
}

fn close_prompt(gs: &mut GuiState) {
    gs.protected.prompt = None;
    gs.protected.prompt_input.clear();
    gs.protected.forgot_phrase.clear();
    gs.protected.prompt_line.clear();
    clear_pin_fields(gs);
}

/// "Forgot the PIN?", from the prompt (the waiting action stays waiting) or from Settings >
/// Safety (nothing waits): the prompt asks for this identity's recovery phrase.
pub(crate) fn open_forgot(gs: &mut GuiState) {
    if !gs.protected.setup.on {
        return;
    }
    ensure_preset(gs);
    let action = gs.protected.prompt.as_ref().and_then(|p| p.action.clone());
    gs.protected.prompt = Some(PinPrompt { action, mode: PromptMode::Forgot });
    gs.protected.forgot_phrase.clear();
    gs.protected.prompt_line.clear();
}

/// The prompt's Continue in its "Forgot the PIN?" mode: when the typed words are exactly the
/// recovery phrase this device derives for the identity the setup was turned on under, the prompt
/// asks for a new PIN (the setup stays on). The phrase is worked out from the seed already in
/// memory for this check and dropped after it; the typed words are cleared either way. A locked
/// identity, or another identity than the setup's, gets the same "not the recovery phrase" line.
pub(crate) fn submit_phrase(gs: &mut GuiState) -> bool {
    let typed = std::mem::take(&mut gs.protected.forgot_phrase);
    if !gs.protected.setup.on || !gs.protected.prompt.as_ref().is_some_and(|p| p.mode == PromptMode::Forgot) {
        return false;
    }
    // A recorded identity must be the one in use; a missing or malformed one, which only damage to
    // the saved setup can leave (turning it on always records one), falls back to the identity in
    // use, so the recovery phrase still opens a damaged setup instead of it locking for good. The
    // web chat follows the same rule (`protectedIdentityMatches`, 10h as built).
    let recorded = gs.protected.setup.identity.trim().to_ascii_lowercase();
    let well_formed = !recorded.is_empty() && recorded.chars().all(|c| c.is_ascii_hexdigit());
    let same_identity = !gs.profile_public_key.is_empty() && (!well_formed || recorded == gs.profile_public_key.to_ascii_lowercase());
    let derived = gs.private_key_bytes.as_deref().and_then(crate::net::identity::mnemonic_from_seed);
    let matched = same_identity && derived.as_deref().is_some_and(|d| crate::net::protected::phrase_matches(&typed, d));
    if !matched {
        gs.protected.prompt_line = label(gs, |l| l.phrase_wrong.clone());
        return false;
    }
    if let Some(p) = gs.protected.prompt.as_mut() {
        p.mode = PromptMode::NewPin;
    }
    clear_pin_fields(gs);
    gs.protected.prompt_line.clear();
    true
}

/// The prompt's Continue in its new-PIN mode: the two fields, when they agree and have the
/// preset's shape, replace the verifier and the wrong-try count and any wait start again; the
/// setup stays on. Then, with an action waiting (after "Forgot the PIN?" in a prompt it opened),
/// the prompt asks for the new PIN before the action runs; otherwise it closes.
pub(crate) fn submit_new_pin(gs: &mut GuiState) -> bool {
    if !gs.protected.setup.on || !gs.protected.prompt.as_ref().is_some_and(|p| p.mode == PromptMode::NewPin) {
        return false;
    }
    match verifier_from_fields(gs) {
        Ok(v) => {
            gs.protected.setup.pin = Some(v);
            gs.protected.setup.wrong_tries = 0;
            gs.protected.setup.wait_until = 0;
            gs.settings_dirty = true;
            let waiting = gs.protected.prompt.as_ref().and_then(|p| p.action.clone());
            match waiting {
                Some(action) => {
                    gs.protected.prompt = Some(PinPrompt { action: Some(action), mode: PromptMode::Pin });
                    gs.protected.prompt_line.clear();
                }
                None => close_prompt(gs),
            }
            true
        }
        Err(why) => {
            gs.protected.prompt_line = why;
            false
        }
    }
}

/// Do `action`, through the same entry point a click uses: each locked one asks `allows` first
/// (and so needs the PIN, or a permission a right PIN just left). The setup's own switches live
/// here; everything else is the feature's own function.
pub(crate) fn perform(gs: &mut GuiState, action: ProtectedAction) {
    use ProtectedAction::*;
    match action {
        ReachRow(kind, audience) => crate::engine::reach::ask(gs, kind, audience),
        Tick(peer, kind, on) => crate::engine::reach::set_tick(gs, &peer, kind, on),
        Follow(peer) => crate::engine::dm::set_follow(gs, &peer, true),
        AcceptRequest(key) => crate::engine::reach::accept_request(gs, &key),
        SendRequest(peer) => {
            if let Err(why) = crate::engine::reach::send_contact_request(gs, &peer) {
                gs.reach.status = why;
            }
        }
        RedeemFriendCode(code) => {
            if allows(gs, RedeemFriendCode(code.clone())) {
                let command = format!("/redeem {code}");
                crate::gui::pages::chat::send_slash_command(gs, &command);
                // A typed command that waited for the PIN has gone now: the composer lets it go.
                if gs.chat_input.trim() == command {
                    gs.chat_input.clear();
                }
            }
        }
        JoinGroup(ticket) => crate::gui::pages::chat::join_group_with_ticket(gs, &ticket),
        JoinVoice(room) => crate::gui::pages::chat::set_voice_room(gs, &room, true),
        WarningsOff => {
            if allows(gs, WarningsOff) {
                gs.settings.warnings_on_messages = false;
                gs.settings_dirty = true;
            }
        }
        ShowPictures => {
            if allows(gs, ShowPictures) {
                gs.protected.setup.pictures_hidden = false;
                gs.settings_dirty = true;
            }
        }
        ShowPublicRooms => {
            if allows(gs, ShowPublicRooms) {
                gs.protected.setup.public_rooms_hidden = false;
                gs.settings_dirty = true;
            }
        }
        ChangePin => {
            if allows(gs, ChangePin) {
                clear_pin_fields(gs);
                gs.protected.prompt = Some(PinPrompt { action: None, mode: PromptMode::NewPin });
                gs.protected.prompt_line.clear();
            }
        }
        TurnOff => {
            if allows(gs, TurnOff) {
                turn_off(gs);
            }
        }
        ShowRecoveryPhrase => {
            if allows(gs, ShowRecoveryPhrase) {
                gs.protected.phrase_shown = true;
            }
        }
        Block(key) => crate::engine::block::block(gs, &key),
        Report(key) => crate::engine::report::open_for_person(gs, &key),
        Unfollow(key) => crate::engine::dm::set_follow(gs, &key, false),
        LeaveGroup(id) => crate::gui::pages::chat::leave_p2p_group(gs, &id),
        LeaveRoom(id) => crate::gui::pages::chat::set_voice_room(gs, &id, false),
    }
}

/// Turn the setup off (after the PIN): nothing of it is kept, the verifier included. The "Who can
/// reach me" rows stay as the server holds them; they are the person's own to change now.
fn turn_off(gs: &mut GuiState) {
    gs.protected.setup = ProtectedSetup::default();
    gs.protected.phrase_shown = false;
    cancel(gs);
    gs.settings_dirty = true;
}

// ── Turning it on ───────────────────────────────────────────────────────────────────────────

/// "Turn on the protected setup": step 1, the sentences. It needs the preset and this device's
/// settings for the server (the DM store: an unlocked identity and a server), because step 3
/// lists the friends kept there.
pub(crate) fn begin(gs: &mut GuiState) {
    if !ensure_preset(gs) {
        gs.protected.line = gs.protected.preset_error.clone().unwrap_or_default();
        return;
    }
    if !crate::engine::dm::ensure_dm_store(gs) {
        gs.protected.line = label(gs, |l| l.waiting_store.clone());
        return;
    }
    clear_pin_fields(gs);
    gs.protected.chosen = None;
    gs.protected.line.clear();
    gs.protected.step = Some(SetupStep::Read);
}

/// Step 1's Continue: on to choosing a PIN.
pub(crate) fn read_done(gs: &mut GuiState) {
    if gs.protected.step == Some(SetupStep::Read) {
        gs.protected.step = Some(SetupStep::ChoosePin);
        gs.protected.line.clear();
    }
}

/// Cancel turning it on, wherever it had got to. Nothing was changed or sent.
pub(crate) fn cancel(gs: &mut GuiState) {
    gs.protected.step = None;
    gs.protected.chosen = None;
    clear_pin_fields(gs);
    gs.protected.line.clear();
}

fn clear_pin_fields(gs: &mut GuiState) {
    gs.protected.pin_first.clear();
    gs.protected.pin_second.clear();
}

/// The two PIN fields as a verifier, when they hold the same PIN of the preset's shape (both are
/// cleared either way); otherwise the preset's PIN rule, which says what is wanted.
fn verifier_from_fields(gs: &mut GuiState) -> Result<PinVerifier, String> {
    let first = std::mem::take(&mut gs.protected.pin_first);
    let second = std::mem::take(&mut gs.protected.pin_second);
    let preset = gs.protected.preset.as_ref().ok_or_else(|| gs.protected.preset_error.clone().unwrap_or_default())?;
    if !preset.pin_shape_ok(&first) || first != second {
        return Err(preset.pin_rule());
    }
    PinVerifier::new(&first)
}

/// Step 2's Continue: the PIN, twice, becomes a verifier (kept until step 4), and step 3 lists
/// who can already reach this device. The group list is asked for again so it is current.
pub(crate) fn pin_chosen(gs: &mut GuiState) -> bool {
    if gs.protected.step != Some(SetupStep::ChoosePin) {
        return false;
    }
    match verifier_from_fields(gs) {
        Ok(v) => {
            gs.protected.chosen = Some(v);
            gs.protected.step = Some(SetupStep::Review);
            gs.protected.line.clear();
            if gs.ws_client.as_ref().is_some_and(|c| c.is_connected()) {
                crate::gui::pages::chat::spawn_groups_list_refresh(gs);
            }
            true
        }
        Err(why) => {
            gs.protected.line = why;
            false
        }
    }
}

/// Step 3's rows (10h): every friend (everyone holding a pass from us, and every mutual follow
/// still owed one: `people_to_choose`, the "People I choose" list), every group we are in, and
/// every voice room we joined on this server. Someone we blocked holds no pass and is not listed.
pub(crate) fn review_items(gs: &GuiState) -> Vec<ReviewItem> {
    let mut out = Vec::new();
    if let Some(store) = gs.dm_store.as_ref() {
        let mut friends: Vec<ReviewItem> = store
            .people_to_choose()
            .into_iter()
            .filter(|k| !crate::engine::block::is_blocked(gs, k))
            .map(|key| ReviewItem::Friend { name: crate::engine::dm::dm_display_name(gs, &key), key })
            .collect();
        friends.sort_by_key(|f| f.name().to_lowercase());
        out.extend(friends);
    }
    out.extend(gs.p2p_groups.iter().map(|g| ReviewItem::Group {
        id: g.group_id.clone(),
        name: if g.name.is_empty() { g.group_id.clone() } else { g.name.clone() },
    }));
    out.extend(gs.chat_channels.iter().filter(|c| c.voice_joined).map(|c| ReviewItem::Room { id: c.id.clone(), name: c.name.clone() }));
    out
}

/// A row's Remove in step 3: Unfollow for a friend (which withdraws every pass we gave them),
/// Leave for a group or a voice room. None of these needs a PIN.
pub(crate) fn review_remove(gs: &mut GuiState, item: &ReviewItem) {
    perform(gs, item.removal());
    if let ReviewItem::Group { id, .. } = item {
        // The list refreshes itself after a leave; until it has, the row goes now.
        gs.p2p_groups.retain(|g| &g.group_id != id);
    }
}

/// Step 3's "Keep the rest", which is step 4, Apply. It needs a live connection: offline it says
/// so (`not_connected`) and nothing changes. Then the setup goes on with the PIN chosen in step 2,
/// under this identity, with the friends kept in the review as its approved list; "Warnings on
/// messages" goes on; and the preset's rows go out as one ordinary `reach_set`. Nothing else is
/// sent, then or on any later reconnect. Saved with the other settings.
pub(crate) fn apply(gs: &mut GuiState) {
    if gs.protected.step != Some(SetupStep::Review) {
        return;
    }
    let Some(preset) = gs.protected.preset.clone() else { return };
    let (Some(frame), Some(settings)) = (preset.reach_set_frame(), preset.reach_settings()) else { return };
    if !gs.ws_client.as_ref().is_some_and(|c| c.is_connected()) {
        gs.protected.line = preset.labels.not_connected.clone();
        return;
    }
    let Some(pin) = gs.protected.chosen.take() else { return };
    let kept: Vec<String> = review_items(gs)
        .into_iter()
        .filter_map(|i| match i {
            ReviewItem::Friend { key, .. } => Some(key),
            _ => None,
        })
        .collect();
    if let Some(client) = gs.ws_client.as_ref() {
        client.send(&frame.to_string());
    }
    gs.reach.asked = Some((settings, std::time::Instant::now()));
    gs.protected.setup = ProtectedSetup::applied(&preset, pin, &gs.profile_public_key, kept);
    gs.protected.step = None;
    gs.protected.line.clear();
    gs.settings.warnings_on_messages = true;
    gs.settings_dirty = true;
}

// ── What it changes on screen ───────────────────────────────────────────────────────────────

/// Is a public room listed? Read-only ones always; the rest only while the setup is off or they
/// were shown with the PIN.
pub(crate) fn lists_room(gs: &GuiState, read_only: bool) -> bool {
    gs.protected.setup.lists_room(read_only)
}

/// `lists_room` for a channel of a server's list.
pub(crate) fn lists_channel(gs: &GuiState, ch: &crate::gui::ChatChannel) -> bool {
    lists_room(gs, ch.read_only)
}

/// `lists_room` for a Commons room (one bridged room carried by several servers): read-only when
/// any carrier holds it read-only.
pub(crate) fn lists_commons_room(gs: &GuiState, name: &str) -> bool {
    if gs.protected.setup.lists_room(false) {
        return true;
    }
    let read_only = gs.chat_channels.iter().chain(gs.connections.iter().flat_map(|c| c.channels.iter())).any(|c| c.id == name && c.read_only);
    lists_room(gs, read_only)
}

/// Are public rooms being left out of the lists right now (so the line saying so shows)?
pub(crate) fn hides_public_rooms(gs: &GuiState) -> bool {
    !gs.protected.setup.lists_room(false)
}

/// Is the conversation open in the centre a public room the setup leaves out? Then the chat shows
/// the line instead of its messages. Direct messages, groups and the scratchpad are never public
/// rooms; a channel this server has not listed yet counts as hidden until it is known to be
/// read-only.
pub(crate) fn hides_active_room(gs: &GuiState) -> bool {
    if !hides_public_rooms(gs) {
        return false;
    }
    let ac = gs.chat_active_channel.as_str();
    if ac.starts_with("dm:") || ac.starts_with("p2pgroup:") || ac.starts_with("group:") || ac == "scratchpad" || ac.is_empty() {
        return false;
    }
    if let Some(room) = ac.strip_prefix("commons:") {
        return !lists_commons_room(gs, room);
    }
    !gs.chat_channels.iter().any(|c| c.id == ac && lists_channel(gs, c))
}

/// Who counts as a friend for the pictures rule (as on the web chat): a mutual follow who also
/// holds a pass from us. Before this server's settings have loaded nobody does, so nothing from
/// anyone shows early.
fn picture_friend(gs: &GuiState, key: &str) -> bool {
    gs.dm_store.as_ref().is_some_and(|s| s.is_friend(key) && s.cert_sent_to(key))
}

/// Are the pictures and files in a message from `sender` left out altogether (10h: from someone
/// who is not a friend, while the setup's pictures rule is on)? Never our own.
pub(crate) fn hides_pictures_from(gs: &GuiState, sender: &str) -> bool {
    if sender.is_empty() || sender == gs.profile_public_key {
        return false;
    }
    gs.protected.setup.hides_pictures(picture_friend(gs, sender))
}

/// Do friends' messages get the strangers' warnings too (engine/warnings.rs)?
pub(crate) fn warns_friends_as_strangers(gs: &GuiState) -> bool {
    gs.protected.setup.warns_friends_as_strangers()
}

#[cfg(test)]
#[path = "protected_tests.rs"]
mod tests;
