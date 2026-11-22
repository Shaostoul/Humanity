//! The chat's message rows, one copy for every view of a conversation: the Chat page's message
//! list and the chat footer's unfolded strip on the other pages (gui/bottom_strip/chat_footer.rs,
//! docs/design/app-bottom-strip.md section 4). Moved out of `chat.rs` whole on 2026-11-15 so the
//! two can never drift: each view only gives the rows a scroll area to stand in.

use super::*;

/// The messages `active_channel` shows, oldest first, before the blocked are taken out: the
/// active server's lines in it, or for a Commons room every linked carrier's copy merged.
/// `linked` is `commons_carriers` for a Commons room. Borrowing only the two message lists (not
/// the whole state), so the rows can draw with the rest of the state while they hold these.
pub(crate) fn in_conversation<'a>(
    messages: &'a [ChatMessage],
    connections: &'a [crate::gui::ServerConnection],
    linked: Option<&super::commons::CommonsCarriers>,
    active_channel: &str,
) -> Vec<&'a ChatMessage> {
    // Commons merge (federation-ux.md): a Commons view merges the
    // conversation from every CARRIER of the bridged room -- and
    // ONLY carriers. A same-named local channel on a non-carrier
    // server is a different room and never blends in (the private
    // #global vs the bridged #global).
    // Dedup: a line native to one of my servers (origin_server
    // empty) beats its federated echoes; federated copies from
    // several carriers collapse on (origin, timestamp, content).
    let filtered: Vec<&ChatMessage> =
        if let (Some(room), Some(linked)) = (commons_room_of(active_channel), linked) {
            // Only the linked carriers' copies (chat/commons.rs): a server that names
            // a room the same but bridges with none of them holds a different room.
            // (`linked`: commons_carriers, read by the caller, which holds the whole state.)
            let mut merged: Vec<&ChatMessage> = Vec::new();
            if linked.active {
                merged.extend(
                    messages.iter().filter(|m| m.channel == room),
                );
            }
            for &i in &linked.parked {
                merged.extend(
                    connections[i].messages.iter().filter(|m| m.channel == room),
                );
            }
            let natives: std::collections::HashSet<(u64, &str)> = merged
                .iter()
                .filter(|m| m.origin_server.is_empty())
                .map(|m| (m.timestamp_ms, m.content.as_str()))
                .collect();
            let mut seen_fed: std::collections::HashSet<(&str, u64, &str)> =
                std::collections::HashSet::new();
            let mut seen_native: std::collections::HashSet<(&str, u64, &str)> =
                std::collections::HashSet::new();
            merged.retain(|m| {
                if m.origin_server.is_empty() {
                    // Two carriers can both hold the same native
                    // line (the snapshot repro proved the double
                    // render): collapse on sender + time + text.
                    return seen_native.insert((
                        m.sender_key.as_str(),
                        m.timestamp_ms,
                        m.content.as_str(),
                    ));
                }
                if natives.contains(&(m.timestamp_ms, m.content.as_str())) {
                    return false; // the native original is present
                }
                seen_fed.insert((
                    m.origin_server.as_str(),
                    m.timestamp_ms,
                    m.content.as_str(),
                ))
            });
            merged.sort_by_key(|m| m.timestamp_ms);
            merged
        } else {
            messages
                .iter()
                .filter(|m| m.channel == active_channel)
                .collect()
        };
    filtered
}

/// The newest message `active_channel` shows (a blocked person's never): the chat footer's folded
/// line. The same rule as the rows, so the line and the last row are always the same message.
pub(crate) fn newest<'a>(state: &'a GuiState, active_channel: &str) -> Option<&'a ChatMessage> {
    let linked = commons_room_of(active_channel).map(|room| commons_carriers(state, room));
    in_conversation(&state.chat_messages, &state.connections, linked.as_ref(), active_channel)
        .into_iter()
        .rev()
        .find(|m| !crate::engine::block::hides_message(state, m))
}

/// A message's words on one line, as the rows would show them: what the protected setup withholds
/// stays withheld (its own words in its place), a file is "(a file)", and the formatting marks are
/// taken out. The chat footer's folded line.
pub(crate) fn preview(state: &GuiState, msg: &ChatMessage) -> String {
    let words = if let Some(rest) = protected::withheld(state, msg) {
        rest
    } else if attach_view::file_in(msg).is_some() {
        return humanity_core::i18n::t("chat_footer.file").to_string();
    } else {
        msg.content.clone()
    };
    one_line_formatted(&words).0
}

/// Draw the rows of `active_channel` into `ui`, a scroll area's content: every message the
/// conversation shows, oldest first (`last`: only that many of the newest), with its header,
/// words, pictures, files, reactions and controls, and what the controls asked done after the
/// rows (a reaction, a reply, an edit).
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, active_channel: &str, last: Option<usize>) {
    ui.add_space(8.0);

    let linked = commons_room_of(active_channel).map(|room| commons_carriers(state, room));
    let filtered = in_conversation(&state.chat_messages, &state.connections, linked.as_ref(), active_channel);
    // Step C: a post by someone we blocked is not drawn, in any channel, group or DM.
    let filtered: Vec<&ChatMessage> = filtered.into_iter().filter(|m| !crate::engine::block::hides_message(state, m)).collect();
    let filtered = match last {
        Some(n) => filtered[filtered.len().saturating_sub(n)..].to_vec(),
        None => filtered,
    };

    if filtered.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            if active_channel.starts_with("p2pgroup:") {
                // P2P group: distinguish "no key yet" from "no
                // messages yet" so the user knows what to do.
                let gid = &active_channel["p2pgroup:".len()..];
                let gname = state.p2p_groups.iter()
                    .find(|g| g.group_id == gid)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| gid.to_string());
                ui.label(
                    RichText::new(&gname)
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.add_space(8.0);
                let hint = if state.p2p_group_loading {
                    "Loading…"
                } else if state.p2p_group_chat_epoch_key.is_none() {
                    "No epoch key yet. The group creator must open this group once to issue the first key, after that, everyone with an invite can read and write."
                } else {
                    "No messages yet. Your messages here are end-to-end encrypted under the group key."
                };
                ui.label(
                    RichText::new(hint)
                        .size(theme.font_size_body)
                        .color(theme.text_muted()),
                );
            } else {
                // A Commons view greets by the ROOM name, never
                // the internal "commons:" id (field test 5).
                let display_room =
                    commons_room_of(&active_channel).unwrap_or(&active_channel);
                ui.label(
                    RichText::new(humanity_core::i18n::tf("chat.welcome_room", &[("room", &display_room)]))
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.add_space(8.0);
                let hint = if commons_room_of(&active_channel).is_some() {
                    "No messages here yet. History from your servers loads in \
                     the background; new posts appear as they arrive."
                } else {
                    "No messages yet. Say something!"
                };
                ui.label(
                    RichText::new(hint)
                        .size(theme.font_size_body)
                        .color(theme.text_muted()),
                );
            }
        });
    }

    // Track alternating user colors. The even/odd row stripes are the
    // theme's dedicated table-striping tokens (bg_primary = base panel,
    // row_stripe = the subtle odd-row stripe egui also uses for lists),
    // so the message list restyles with the rest of the app.
    let mut last_sender = String::new();
    let mut sender_parity = false; // toggles each time sender changes
    let bg_even = theme.bg_primary();
    let bg_odd = theme.row_stripe();

    // Reactions clicked during render — applied after the loop ends
    // so we don't try to send WS messages while iterating &state.
    let mut pending_reactions: Vec<(String, u64, String)> = Vec::new();
    // Reply button clicks: defer setting state.chat_reply_to until after the loop.
    let mut pending_reply: Option<crate::gui::ReplyContext> = None;
    // Pin button clicks — defer pin_request WS sends.
    let mut pending_pins: Vec<(String, String, String, u64)> = Vec::new();
    // Edit button clicks — defer setting edit target.
    let mut pending_edit: Option<(u64, String)> = None;
    // Edit-save submissions from the inline editor.
    let mut pending_edit_save: Option<(u64, String)> = None;
    // Cancel flag — Cancel button in the inline editor sets this
    // so we clear chat_edit_target after the loop ends.
    let mut pending_edit_cancel = false;
    // Report clicks (from the context menu), buffered: each opens the Report dialog (step D).
    let mut pending_reports: Vec<ChatMessage> = Vec::new();
    let mut pending_blocks: Vec<String> = Vec::new(); // Block beside Report (step C)
    // "Edit and send again" under a line of mine the server refused: its words (line_limit.rs).
    let mut pending_send_again: Option<String> = None;
    // v0.281.0: Delete button clicks (from context menu) — buffered
    // so the borrow on `state.chat_messages` ends before we mutate
    // state to send WS. Pairs as (sender_key, timestamp_ms); the
    // server fills the rest from its own auth context.
    let mut pending_deletes: Vec<(String, u64)> = Vec::new();

    // Remove default item spacing so rows sit flush
    ui.spacing_mut().item_spacing = Vec2::ZERO;

    // Deferred avatars — collected during the loop, painted
    // AFTER all rows are drawn so 32×32 avatars on short
    // single-line header rows don't get clipped by the next
    // row's bg fill. Eliminates the empty-row gap below
    // short messages (operator feedback 2026-05-12).
    let mut deferred_avatars: Vec<crate::gui::widgets::row::DeferredAvatar> = Vec::new();

    for msg in &filtered {
        // The app's own voice (origin set only by local code): its mark, no name, none of a member's
        // row (widgets/app_line.rs; the owner branch's seventh review, H1 (a)). Any other row is
        // someone's words under the name it claims, and a row only continues the one above when
        // both came the same way from the same key under the same name, so a frame named "System"
        // never reads as the line before it.
        if msg.origin == crate::gui::LineOrigin::App {
            sender_parity = !sender_parity;
            last_sender = "app".to_string();
            crate::gui::widgets::app_line::draw(ui, theme, &msg.content, if sender_parity { bg_even } else { bg_odd });
            continue;
        }
        let sender_id = format!("{:?}\u{1f}{}\u{1f}{}", msg.origin, msg.sender_key, msg.sender_name);
        let show_header = sender_id != last_sender;
        if show_header {
            sender_parity = !sender_parity;
        }
        last_sender = sender_id;

        let row_bg = if sender_parity { bg_even } else { bg_odd };
        let icon_color = name_color(&msg.sender_name);
        let icon_letter = msg.sender_name.chars().next().unwrap_or('?');
        let channeling = state.chat_user_modal_open
            && msg.sender_key == state.chat_user_modal_key;
        // A private file (10k): a FILE_MARKER carries an encrypted
        // file's address and key, never text. It has no words of its
        // own; chat/attach_view.rs draws it under the row.
        let enc_att = attach_view::file_in(msg);
        // A poster below Verified here: links, pictures and files as plain words (chat/access.rs).
        let plain = !access::shows_links(state, msg);
        // Extract image URLs from the message so we can render them
        // as inline thumbnails instead of raw /uploads/... text.
        let image_urls = if enc_att.is_some() || plain {
            Vec::new()
        } else {
            crate::gui::widgets::image_cache::extract_image_urls(&msg.content)
        };
        // Step G: a non-friend's pictures and files (audio, video, documents too) are
        // left out, one line instead (chat/protected.rs `withheld`).
        let withheld = protected::withheld(state, msg);
        let pictures_hidden = withheld.is_some();
        // Files posted into a public channel and Media items: a card each under the
        // message, their bare addresses taken out of its words (chat/public_files.rs).
        let posted = public_files::Posted::of(state, msg, enc_att.is_some() || pictures_hidden || plain);
        let display_text = if let Some(rest) = withheld {
            rest
        } else if enc_att.is_some() {
            String::new()
        } else {
            let text = posted.words_of(&msg.content);
            if image_urls.is_empty() { text } else { crate::gui::widgets::image_cache::strip_image_urls(&text) }
        };
        // Inline markdown + link detection (v0.702, parity with web:
        // the help modal has advertised markdown all along). Markers
        // are stripped HERE so the mention detection below and the
        // row's char-indexed ranges all see the same display text.
        // Fenced ```code``` blocks (v0.1208, parity with web's
        // formatBody step 1): pull them OUT of the inline flow before
        // bulletize/parse can mangle their contents (backticks, '*',
        // etc.), and render each as a monospace panel with a Copy
        // button below the message text. Mirrors how inline images
        // are stripped here and drawn as separate widgets. The
        // de-fenced text flows on through bulletize + parse as normal.
        let (display_text, code_blocks) = extract_code_blocks(&display_text);
        // Line-leading "- "/"* " -> a real bullet, parity with the
        // web client's list rendering. Width-preserving and done
        // BEFORE parse so the inline spans below stay char-aligned.
        let display_text = bulletize_list_lines(&display_text);
        let (display_text, mut format_spans) =
            crate::gui::widgets::msg_format::parse(&display_text);
        if plain { format_spans.retain(|sp| !matches!(sp.kind, crate::gui::widgets::msg_format::SpanKind::Link(_))); }
        let link_targets: Vec<String> = format_spans
            .iter()
            .filter_map(|sp| match &sp.kind {
                crate::gui::widgets::msg_format::SpanKind::Link(url) => {
                    Some(url.clone())
                }
                _ => None,
            })
            .collect();
        // Step F: a stranger's DM holds its links for the link line's Open (chat/warnings.rs).
        let links_held = !link_targets.is_empty() && crate::engine::warnings::links_held(state, msg);
        let format_spans = warnings::hold_links(format_spans, links_held);

        // ── Reply-to context (if this is a reply) ──
        if let Some(ref reply) = msg.reply_to {
            let row_w = ui.available_width();
            let (row_rect, _) = ui.allocate_exact_size(
                Vec2::new(row_w, 18.0),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(row_rect, 0.0, row_bg);
            let preview = reply::cut(&reply.preview, reply::QUOTE_CHARS); // never inside a character
            // A reply to someone we blocked shows neither their name nor their words (step C).
            let quoted = if crate::engine::block::is_blocked(state, &reply.sender_key) { "↩ a reply to someone you blocked".to_string() } else { format!("↩ {}: {}", reply.sender_name, preview) };
            ui.painter().text(
                egui::pos2(row_rect.left() + 40.0, row_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &quoted,
                egui::FontId::proportional(theme.font_size_small),
                theme.text_muted(),
            );
        }

        // Inline editor for this message (only on user's own messages
        // matching chat_edit_target). Otherwise render the regular row.
        let is_editing = state.chat_edit_target
            .as_ref()
            .map(|(ts, _)| *ts == msg.timestamp_ms && msg.sender_key == state.profile_public_key)
            .unwrap_or(false);

        let need_text_row = show_header || !display_text.trim().is_empty();
        // Track the pill rect so the expanded
        // pill popup (rendered later) can anchor to the right
        // place. The popup is an egui::Area, doesn't allocate
        // layout space — hovering a message no longer shifts
        // every message below.
        let mut pill_rect_for_msg: egui::Rect = egui::Rect::NOTHING;
        // Þ icon rect (within the pill); reaction popup hover
        // is now gated on the cursor being over THIS rect or
        // over the popup itself, NOT the full pill (operator
        // feedback 2026-05-12 — hovering message text should
        // not open the popup so the user can copy/select text).
        let mut thorn_rect_for_msg: egui::Rect = egui::Rect::NOTHING;
        if is_editing {
            // Render an editable row in place of the message text.
            if let Some((_, ref mut draft)) = state.chat_edit_target {
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    let resp = ui.add(
                        egui::TextEdit::multiline(draft)
                            // Stable id (v0.972): the edit row
                            // lives inside the scrolling message
                            // list, where new messages shift
                            // layout constantly - without a
                            // fixed id an incoming message
                            // dropped focus mid-edit.
                            .id(egui::Id::new("chat_edit_input"))
                            .desired_width(ui.available_width() - 130.0)
                            .desired_rows(2),
                    );
                    ui.add_space(theme.spacing_xs);
                    if widgets::Button::primary(humanity_core::i18n::t("common.save")).show(ui, theme)
                        || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        pending_edit_save = Some((msg.timestamp_ms, draft.trim().to_string()));
                    }
                    if widgets::Button::secondary(humanity_core::i18n::t("common.cancel")).show(ui, theme) {
                        pending_edit_cancel = true;
                    }
                });
            }
        } else if need_text_row {
            // Measure exact pill width — must match what
            // paint_timestamp_pill draws or the reserved space
            // in message_row will be wrong and content text
            // overlaps the pill (operator-reported bug).
            // Reactions of people we blocked are not counted or shown (step C).
            let reactions = crate::engine::block::visible_reactions(state, &msg.reactions);
            let pill_width = compute_pill_width(ui.ctx(), theme, &msg.timestamp, &reactions);

            // Parse @mentions that resolve to a known user, for
            // accent highlighting + click-to-open-modal (Discord-
            // style). Longest-match against the user list so
            // multi-word names like "@Deploy Bot" work. Ranges
            // are char-indexed into display_text. Operator
            // request 2026-05-15.
            let mut mention_ranges: Vec<(usize, usize)> = Vec::new();
            let mut mention_targets: Vec<(String, String)> = Vec::new();
            {
                let chars: Vec<char> = display_text.chars().collect();
                let mut i = 0;
                while i < chars.len() {
                    if chars[i] == '@' {
                        let after: String = chars[i + 1..].iter().collect();
                        let after_lower = after.to_lowercase();
                        let mut best: Option<&crate::gui::ChatUser> = None;
                        let mut best_len = 0usize;
                        for u in &state.chat_users {
                            if u.name.is_empty() { continue; }
                            let nl = u.name.to_lowercase();
                            if after_lower.starts_with(&nl) {
                                let nlen = u.name.chars().count();
                                // Boundary: char after the name
                                // must be missing or non-word so
                                // "@Eve" doesn't match user "Ev".
                                let boundary_ok = after
                                    .chars()
                                    .nth(nlen)
                                    .map(|c| !c.is_alphanumeric())
                                    .unwrap_or(true);
                                if boundary_ok && nlen > best_len {
                                    best = Some(u);
                                    best_len = nlen;
                                }
                            }
                        }
                        if let Some(u) = best {
                            mention_ranges.push((i, 1 + best_len));
                            mention_targets.push((u.name.clone(), u.public_key.clone()));
                            i += 1 + best_len;
                            continue;
                        }
                    }
                    i += 1;
                }
            }

            let row_resp = crate::gui::widgets::row::message_row(
                ui,
                theme,
                icon_letter,
                icon_color,
                &msg.sender_name,
                &msg.timestamp,
                &display_text,
                show_header,
                row_bg,
                channeling,
                pill_width,
                &mention_ranges,
                &format_spans,
            );
            // Click on a highlighted @mention → open that user's
            // modal (same as clicking them in the user list).
            if let Some(idx) = row_resp.clicked_mention {
                if let Some((nm, key)) = mention_targets.get(idx) {
                    // Field by field (the message list holds a borrow of `state`), as
                    // modals.rs `open_profile` sets them.
                    state.chat_user_modal_open = true;
                    state.chat_user_modal_name = nm.clone();
                    state.chat_user_modal_key = key.clone();
                }
            }
            // Click on a link → OS default browser (same pattern
            // as the Browser page's bookmarks). v0.702.
            if let Some(idx) = row_resp.clicked_link {
                if let Some(url) = link_targets.get(idx) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
            }
            pill_rect_for_msg = row_resp.pill_rect;
            if let Some(ref a) = row_resp.deferred_avatar {
                deferred_avatars.push(a.clone());
            }

            // Paint the timestamp pill into the rect message_row reserved.
            // Returns the Þ icon's rect — used below to constrain
            // the reaction-popup hover area so the popup doesn't
            // open when the user is just trying to read/copy the
            // message text (operator feedback 2026-05-12).
            // `popup_active` switches the Þ from static accent to
            // the channeling RGB cycle while the popup is open
            // for this message (visual feedback that the Þ is
            // "live"; operator feedback 2026-05-12).
            let popup_active_for_this = state.chat_open_popup_ts == Some(msg.timestamp_ms);
            if pill_rect_for_msg != egui::Rect::NOTHING {
                thorn_rect_for_msg = paint_timestamp_pill(
                    ui,
                    theme,
                    pill_rect_for_msg,
                    &msg.timestamp,
                    &reactions,
                    &state.profile_public_key,
                    msg.timestamp_ms,
                    msg.sender_key.clone(),
                    &mut pending_reactions,
                    popup_active_for_this,
                );
            }
            if row_resp.userbox_clicked(ui.ctx()) {
                state.chat_user_modal_open = true;
                state.chat_user_modal_name = msg.sender_name.clone();
                state.chat_user_modal_key = msg.sender_key.clone();
            }

            // Right-click context menu — same actions as the inline
            // pill buttons but quicker to reach.
            let is_own = msg.sender_key == state.profile_public_key;
            // v0.281.0: derive my role from the populated user list.
            // chat_users is the authoritative client-side mirror of
            // server-known roles (driven by relay's user_list /
            // peer_joined events); looking up my own pubkey there
            // gives the server-visible role without a roundtrip.
            // Falls back to "" when not yet populated (pre-list
            // frames) — the Delete entry just won't render those
            // frames; harmless.
            let my_role = state.chat_users.iter()
                .find(|u| u.public_key == state.profile_public_key)
                .map(|u| u.role.clone())
                .unwrap_or_default();
            let is_admin_or_mod = crate::net::roles::is_staff_role(&my_role); // the owner too (BUG-332)
            row_resp.response.context_menu(|ui| {
                ui.set_min_width(160.0);
                // Plain text labels — leading emoji glyphs were
                // unreliable across the loaded font (📋 📌 ✎ all
                // rendered as tofu in some sessions). The context
                // menu prioritizes legibility over ornament.
                if ui.button(humanity_core::i18n::t("chat.copy_text")).clicked() {
                    ui.ctx().copy_text(msg.content.clone());
                    ui.close_menu();
                }
                // ↩ U+21A9 is in the Arrows block which IS in the
                // loaded font — safe to keep.
                if msg.timestamp_ms > 0 && ui.button(humanity_core::i18n::t("chat.quote_reply")).clicked() {
                    pending_reply = Some(reply::context_for(msg, &state.chat_active_channel, msg.timestamp_ms));
                    ui.close_menu();
                }
                if msg.timestamp_ms > 0 && !is_private_channel(&msg.channel) && ui.button(humanity_core::i18n::t("chat.pin_message")).clicked() {
                    pending_pins.push((
                        msg.sender_key.clone(),
                        msg.sender_name.clone(),
                        msg.content.clone(),
                        msg.timestamp_ms,
                    ));
                    ui.close_menu();
                }
                if is_own && msg.timestamp_ms > 0 && !is_private_channel(&msg.channel) && ui.button(humanity_core::i18n::t("common.edit")).clicked() {
                    pending_edit = Some((msg.timestamp_ms, msg.content.clone()));
                    ui.close_menu();
                }
                // v0.281.0: Delete entry — own messages always; any
                // message when the current user is admin or mod.
                // Server enforces the same predicate so a stale UI
                // can't bypass; we just hide the option when it's
                // certain to be rejected.
                if msg.timestamp_ms > 0 && (is_own || is_admin_or_mod) && !is_private_channel(&msg.channel) {
                    let label = if is_own { "Delete" } else { "Delete (admin)" };
                    if ui.button(label).clicked() {
                        pending_deletes.push((
                            msg.sender_key.clone(),
                            msg.timestamp_ms,
                        ));
                        ui.close_menu();
                    }
                }
                ui.separator();
                // Report and Block name a person's own key only: never ours, never a
                // bridged line's server id.
                let a_person = !is_own && !msg.sender_key.is_empty() && msg.origin_server.is_empty();
                if a_person && ui.button(humanity_core::i18n::t("chat.report")).on_hover_text(report_dialog::REPORT_TIP).clicked() {
                    pending_reports.push(ChatMessage::clone(msg));
                    ui.close_menu();
                }
                if a_person
                    && ui.button(humanity_core::i18n::t("chat.block")).on_hover_text(blocking::BLOCK_TIP).clicked()
                {
                    pending_blocks.push(msg.sender_key.clone());
                    ui.close_menu();
                }
            });
        }

        // Fenced code blocks (v0.1208): render each extracted ```
        // block as an indented monospace panel with an optional
        // language label + Copy button, directly under the message
        // text. Skipped while editing (the raw ``` is in the editor).
        if !is_editing && !code_blocks.is_empty() {
            draw_code_blocks(ui, theme, &code_blocks);
        }

        // (Old inline reaction PILLS row removed — reactions
        // now live INSIDE the timestamp pill itself. See
        // paint_timestamp_pill() above for the inline display.)
        let target_ts = msg.timestamp_ms;
        let target_from = msg.sender_key.clone();

        // ── Expanded pill popup (right of Þ, sticky hover) ──
        // Opens when the cursor is over the pill OR over the popup
        // itself (sticky combined region), so moving from pill to
        // popup doesn't dismiss it. Extends RIGHTWARD from the
        // pill so it reads as the pill "growing" with new
        // controls — functions on the LEFT (separated from the
        // existing Þ that's already in the inline pill), reactions
        // on the RIGHT, ∞ at the far right.
        if pill_rect_for_msg != egui::Rect::NOTHING && target_ts > 0 {
            // Reactions list. As of v0.190.0 we install the OS's
            // emoji font (Windows seguiemj.ttf, macOS Apple Color
            // Emoji, Linux Noto Color Emoji) as an egui font
            // fallback at startup (see src/gui/fonts.rs). That
            // covers basically every emoji in BMP + supplementary
            // plane, so we can use a real reaction palette here.
            // The icon_glyph_lint test still catches U+FE0F
            // variation selectors and known-broken glyphs.
            // Palette lives in data/reactions.json (one source for
            // native, relay allowlist, and web) via humanity_core::reactions.
            // No reactions in a DM or a P2P group: the server would tell everyone who reacted to
            // whom there (BUG-178), so the popup offers none.
            let top_reactions: &'static [String] = if is_private_channel(&msg.channel) { &[] } else { humanity_core::reactions::top() };
            let all_reactions: &'static [String] = if is_private_channel(&msg.channel) { &[] } else { humanity_core::reactions::all() };
            let is_own = msg.sender_key == state.profile_public_key;

            // Estimated popup geometry — needed for the sticky
            // hover region. Function buttons are now text labels
            // (Pin / Edit) which are wider than the prior icon
            // attempts, so widen the estimate accordingly.
            let func_w = 26.0      // ↩ reply
                       + 36.0      // Pin
                       + (if is_own { 40.0 } else { 0.0 }); // Edit (own only)
            let est_popup_w =
                func_w
                + 18.0                       // Þ separator
                + top_reactions.len() as f32 * 28.0 // quick-row reactions
                + 30.0                       // ∞ button
                + 16.0;                      // padding
            // Popup rect adjacent to the pill (no gap so cursor
            // sliding rightward stays in a connected hover region).
            let est_popup_rect = egui::Rect::from_min_size(
                egui::pos2(pill_rect_for_msg.right(), pill_rect_for_msg.top() - 2.0),
                Vec2::new(est_popup_w, pill_rect_for_msg.height() + 4.0),
            );
            // And wherever the popup really was drawn last frame: its reactions are the emoji
            // pack's pictures since 2026-11-20 (widgets/emoji/button.rs), a little wider than the
            // estimate's glyphs, and a pointer on ∞ past the estimate closed the popup.
            let est_popup_rect = match ui.ctx().memory(|m| m.area_rect(egui::Id::new(("pill_expand", target_ts)))) {
                Some(drawn) if state.chat_open_popup_ts == Some(target_ts) => est_popup_rect.union(drawn),
                _ => est_popup_rect,
            };
            // Sticky hover gate: open if cursor is over THE PILL
            // or over THE POPUP REGION specifically. Earlier code
            // used pill.union(popup) which created a bounding
            // rect that included the message TEXT area between
            // them — so hovering message body opened the popup.
            // (Operator-reported bug 2026-05-04.)
            let pointer = ui.ctx().input(|i| i.pointer.hover_pos());
            // Block the popup whenever a foreground modal is
            // open over the chat. Egui's positional `.contains()`
            // checks don't respect z-ordering, so without this
            // gate the popup opens when the cursor is over a
            // modal that happens to sit above a message's pill
            // region — blocking the modal's own buttons.
            let modal_blocking = state.chat_user_modal_open
                || state.show_create_group_modal
                || state.show_join_group_modal
                || state.image_viewer_url.is_some()
                || state.chat_model_view.is_some()
                || state.chat_video.window_open();
            // Þ-only hover detection — does NOT use the wider
            // est_popup_rect because that overlaps the message
            // text on the same line (operator feedback
            // 2026-05-12 — "if I mouse over the text of a reply
            // the reaction pill comes up even though I never
            // clicked on the Þ"). Popups OPEN only via thorn.
            let thorn_hovered = !modal_blocking
                && thorn_rect_for_msg != egui::Rect::NOTHING
                && pointer.map(|p| thorn_rect_for_msg.contains(p)).unwrap_or(false);
            // popup_hovered is ONLY honored for sticky behavior
            // once a popup is ALREADY open for this message
            // (state.chat_open_popup_ts == Some(target_ts)).
            // Otherwise hovering the message text right of the
            // pill (which is geometrically inside est_popup_rect)
            // would open the popup spuriously.
            //
            // Sticky region while popup is open INCLUDES the
            // full pill_rect — not just est_popup_rect — so the
            // cursor can travel rightward through the pill
            // (past Þ, across timestamp text + existing reaction
            // badges) on its way to the popup without falling
            // into a "dead zone" that closes the popup.
            // Operator feedback 2026-05-12, "the problem now
            // is that I can't move the mouse off of the Þ
            // without the reaction pill disappearing."
            let popup_already_open_for_msg = state.chat_open_popup_ts == Some(target_ts);
            let popup_hovered = !modal_blocking
                && popup_already_open_for_msg
                && pointer.map(|p| {
                    pill_rect_for_msg.contains(p) || est_popup_rect.contains(p)
                }).unwrap_or(false);
            let combined_hovered = thorn_hovered || popup_hovered;

            // Update the open-popup tracker. Open on first Þ
            // hover; clear when cursor leaves both Þ AND popup.
            if thorn_hovered {
                state.chat_open_popup_ts = Some(target_ts);
            } else if popup_already_open_for_msg && !combined_hovered {
                state.chat_open_popup_ts = None;
            }

            if combined_hovered {
                // ── Timestamp expansion overlay (LEFT of Þ) ──
                // Shows YYYY-MM-DD HH:MM:SS UTC anchored just to
                // the left of the Þ. Operator feedback 2026-05-12.
                // Rendered before the reaction popup (which goes
                // on the RIGHT) so the two overlays appear
                // symmetrically around the Þ pull tab.
                if msg.timestamp_ms > 0 && thorn_rect_for_msg != egui::Rect::NOTHING {
                    let ts_overlay_pos = egui::pos2(
                        thorn_rect_for_msg.left() - 4.0,
                        thorn_rect_for_msg.center().y,
                    );
                    let full_ts = format_full_timestamp(msg.timestamp_ms);
                    draw_full_timestamp(ui.ctx(), theme, egui::Id::new(("pill_ts_expand", msg.timestamp_ms)), ts_overlay_pos, &full_ts);
                }

                let overlay_pos = egui::pos2(
                    pill_rect_for_msg.right(),
                    pill_rect_for_msg.center().y,
                );
                // Animated channeling color (RGB cycle / pulse / red on attack).
                // Used for every glyph + the Þ separator so the popup feels
                // alive and matches the active-page nav border.
                let chan_time = ui.ctx().input(|i| i.time) as f32;
                let chan_attack = state.attack_pulse_active;
                let chan = crate::gui::pages::escape_menu::channeling_color(
                    theme, chan_time, chan_attack, theme.accent(),
                );
                ui.ctx().request_repaint();
                // The page's own drop-down (layer 40): its reactions take clicks.
                crate::gui::layers::area(ui.ctx(), egui::Id::new(("pill_expand", target_ts)), crate::gui::layers::Layer::PageFloat)
                    .fixed_pos(overlay_pos)
                    .pivot(egui::Align2::LEFT_CENTER)
                    .interactable(true)
                    .show(ui.ctx(), |ui| {
                        Frame::NONE
                            .fill(theme.bg_card())
                            .stroke(Stroke::new(1.0_f32, theme.border()))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(4.0)
                            .shadow(egui::epaint::Shadow {
                                offset: [1, 2],
                                blur: 6,
                                spread: 0,
                                color: Color32::from_black_alpha(80),
                            })
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.horizontal(|ui| {
                                    // Functions on the LEFT of Þ. Use plain
                                    // unicode arrows + ASCII letters so they
                                    // render reliably in the default font
                                    // (emoji glyphs like 📌 ✎ show as squares
                                    // without an emoji font installed).
                                    if ui.add(
                                        egui::Button::new(RichText::new("↩").size(theme.font_size_body).color(chan))
                                            .min_size(Vec2::new(26.0, 22.0))
                                            .corner_radius(CornerRadius::same(4))
                                    ).on_hover_text(humanity_core::i18n::t("chat.reply")).clicked() {
                                        pending_reply = Some(reply::context_for(msg, &state.chat_active_channel, target_ts));
                                    }
                                    if !is_private_channel(&msg.channel) && ui.add(
                                        egui::Button::new(RichText::new(humanity_core::i18n::t("chat.pin")).size(theme.font_size_small).color(chan))
                                            .min_size(Vec2::new(34.0, 22.0))
                                            .corner_radius(CornerRadius::same(4))
                                    ).on_hover_text(humanity_core::i18n::t("chat.pin_message")).clicked() {
                                        pending_pins.push((msg.sender_key.clone(), msg.sender_name.clone(), msg.content.clone(), target_ts));
                                    }
                                    if is_own && !is_private_channel(&msg.channel) {
                                        if ui.add(
                                            egui::Button::new(RichText::new(humanity_core::i18n::t("common.edit")).size(theme.font_size_small).color(chan))
                                                .min_size(Vec2::new(38.0, 22.0))
                                                .corner_radius(CornerRadius::same(4))
                                        ).on_hover_text(humanity_core::i18n::t("chat.edit_message")).clicked() {
                                            pending_edit = Some((target_ts, msg.content.clone()));
                                        }
                                    }

                                    // Þ separator (matches the inline pill).
                                    // Painted in the channeling color too so the
                                    // whole popup pulses together.
                                    ui.add_space(2.0);
                                    ui.label(RichText::new("Þ").size(theme.font_size_body).color(chan).strong());
                                    ui.add_space(2.0);

                                    // Top-10 reactions on the RIGHT of Þ. Strip
                                    // any U+FE0F variation selector — it ends up
                                    // as a tofu square next to ❤ in fonts that
                                    // don't honor the emoji presentation hint.
                                    // Each drawn as the pack's picture when one is installed
                                    // (`emoji::button`, docs/design/emoji.md), else the glyph.
                                    for emoji in top_reactions {
                                        let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect(); // glyph-exempt: stripping the selector, not rendering it
                                        let font = egui::FontId::proportional(theme.font_size_body);
                                        if crate::gui::widgets::emoji::button::button(ui, &clean, font, Vec2::new(26.0, 22.0), Some(CornerRadius::same(4))).clicked() {
                                            pending_reactions.push((target_from.clone(), target_ts, clean.clone()));
                                        }
                                    }

                                    // ∞ all-emoji picker (popup with full set).
                                    let inf_resp = ui.add(
                                        egui::Button::new(RichText::new("∞").size(theme.font_size_body).color(chan))
                                            .min_size(Vec2::new(26.0, 22.0))
                                            .corner_radius(CornerRadius::same(4))
                                    ).on_hover_text(humanity_core::i18n::t("chat.all_reactions"));
                                    let inf_popup_id = egui::Id::new(("react_inf_popup", target_ts));
                                    if inf_resp.clicked() {
                                        ui.memory_mut(|m| m.toggle_popup(inf_popup_id));
                                    }
                                    egui::popup::popup_below_widget(
                                        ui, inf_popup_id, &inf_resp,
                                        egui::PopupCloseBehavior::CloseOnClickOutside,
                                        |ui| {
                                            ui.set_min_width(280.0);
                                            ui.horizontal_wrapped(|ui| {
                                                for emoji in all_reactions {
                                                    let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect(); // glyph-exempt: stripping the selector, not rendering it
                                                    let font = egui::TextStyle::Button.resolve(ui.style());
                                                    let min = Vec2::new(0.0, ui.spacing().interact_size.y); // as tall as egui's button
                                                    if crate::gui::widgets::emoji::button::button(ui, &clean, font, min, None).clicked() {
                                                        pending_reactions.push((target_from.clone(), target_ts, clean.clone()));
                                                        ui.memory_mut(|m| m.close_popup());
                                                    }
                                                }
                                            });
                                            // OpenMoji's credit where its pictures are picked (the
                                            // web panel's twin, pillPopup.js), while they are drawn.
                                            if crate::gui::widgets::emoji::pack_installed(ui.ctx()) {
                                                ui.add_space(theme.spacing_xs);
                                                crate::gui::widgets::emoji::button::credit(ui, theme);
                                            }
                                        },
                                    );
                                });
                            });
                    });
            }
        }

        // Render each attached image as a clickable thumbnail
        // indented under the message text.
        if pictures_hidden && enc_att.is_none() {
            protected::draw_picture_hidden(ui, theme, row_bg, 40.0, protected::picture_hidden_line(state));
        }
        if !image_urls.is_empty() && !pictures_hidden {
            // The server this message came through (its
            // connection's address), so a relative /uploads/ path
            // resolves against, and "our own server" means, the
            // server that delivered it. Older rows carry none.
            let server_url = if msg.server.is_empty() {
                state.server_url.clone()
            } else {
                msg.server.clone()
            };
            const THUMB_INDENT: f32 = 40.0;
            const THUMB_W: f32 = 240.0;
            for raw_url in image_urls {
                // Pictures from the server load by themselves; any
                // other website's wait for a click, because
                // fetching one shows that website this device's
                // network address (2026-10-09, defect 3.7.6 of
                // docs/design/blocking-and-safe-mode.md).
                let route = crate::gui::widgets::image_cache::route_image(&raw_url, &server_url);
                let url = route.url().to_string();
                if let crate::gui::widgets::image_cache::ImageRoute::AskFirst { host, .. } = &route {
                    if !state.image_cache.is_allowed(&url) {
                        if draw_click_to_load(ui, theme, row_bg, THUMB_INDENT, host) {
                            state.image_cache.allow(&url);
                        }
                        continue;
                    }
                }
                state.image_cache.request(&url);
                let status = state.image_cache.status(&url);

                match status {
                    crate::gui::widgets::image_cache::ImageStatus::Ready { width, height } => {
                        let aspect = width as f32 / height.max(1) as f32;
                        let thumb_h = (THUMB_W / aspect.max(0.1)).min(360.0).max(60.0);
                        let row_w = ui.available_width();
                        let (row_rect, resp) = ui.allocate_exact_size(
                            Vec2::new(row_w, thumb_h + 4.0),
                            egui::Sense::click(),
                        );
                        ui.painter().rect_filled(row_rect, 0.0, row_bg);
                        if let Some(tex) = state.image_cache.get_texture(&url) {
                            let img_rect = egui::Rect::from_min_size(
                                egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.top() + 2.0),
                                Vec2::new(THUMB_W, thumb_h),
                            );
                            let mut mesh = egui::Mesh::with_texture(tex.id());
                            mesh.add_rect_with_uv(
                                img_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                            ui.painter().add(egui::Shape::mesh(mesh));
                            // Faint border
                            ui.painter().rect_stroke(
                                img_rect,
                                CornerRadius::same(3),
                                Stroke::new(1.0_f32, theme.border()),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            state.image_viewer_url = Some(url.clone());
                        }
                    }
                    crate::gui::widgets::image_cache::ImageStatus::Fetching
                    | crate::gui::widgets::image_cache::ImageStatus::Idle => {
                        let row_w = ui.available_width();
                        let (row_rect, _) = ui.allocate_exact_size(
                            Vec2::new(row_w, 20.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(row_rect, 0.0, row_bg);
                        ui.painter().text(
                            egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &humanity_core::i18n::tf("chat.loading_image", &[("file", &crate::gui::widgets::image_cache::filename_from_url(&url))]),
                            egui::FontId::proportional(theme.font_size_small),
                            theme.text_muted(),
                        );
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                    }
                    crate::gui::widgets::image_cache::ImageStatus::Failed(err) => {
                        let row_w = ui.available_width();
                        let (row_rect, _) = ui.allocate_exact_size(
                            Vec2::new(row_w, 20.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(row_rect, 0.0, row_bg);
                        ui.painter().text(
                            egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &humanity_core::i18n::tf("chat.image_failed", &[("error", &err)]),
                            egui::FontId::proportional(theme.font_size_small),
                            theme.danger(),
                        );
                    }
                }
            }
        }
        // Public files and Media items: a card each (chat/public_files.rs, files/chat_card.rs).
        public_files::draw_all(ui, theme, &mut state.image_cache, &mut state.chat_model_view, &mut state.chat_video, &state.settings.ffmpeg_path, &posted.files, &msg.server, &state.server_url, row_bg);
        let (cards, needs) = crate::gui::pages::files::chat_card::views(state, &msg.sender_key, msg.timestamp_ms, &posted.items);
        state.files.card_acts.extend(needs);
        crate::gui::pages::files::chat_card::draw_all(ui, theme, &cards, row_bg, &mut state.files.card_acts);
        // A private file (10k): its picture inline, or a card with Save.
        if let Some(ref att) = enc_att {
            let plan = attach_view::plan(state, msg, att);
            attach_view::draw(ui, theme, &mut state.image_cache, &mut state.image_viewer_url, &msg.sender_name, att, &plan, row_bg);
        }
        // Step F: the warnings and the stranger link line under a received DM or group message.
        if let Some(act) = warnings::draw_under_message(ui, theme, state, msg, &link_targets) {
            warnings::apply(&mut state.warnings, act); // "Got it" or Open, for the session
        }
        // A line of mine not (yet) posted: "Sending...", or "Not sent" with why (line_limit.rs).
        if msg.send_state != line_limit::LineSend::Posted {
            if let Some(words) = line_limit::draw_status(ui, theme, msg) {
                pending_send_again = Some(words);
            }
        }
    }

    ui.add_space(8.0);

    // ── Deferred avatar post-pass ──
    // Paint avatars AFTER all rows + reply rows + image rows have
    // rendered, so the avatar's bottom doesn't get clipped by
    // subsequent row bgs (header rows are now sized to the text,
    // not to the avatar, so 32×32 avatars often overflow). Painting
    // last puts them on top of any covering bg fill.
    let avatar_ctx_time = ui.ctx().input(|i| i.time);
    for avatar in &deferred_avatars {
        crate::gui::widgets::row::paint_avatar(ui, theme, avatar, avatar_ctx_time);
    }

    // Edit and send again: the refused line's words back in the box, ready to shorten.
    if let Some(words) = pending_send_again.take() {
        state.chat_input = words;
        ui.ctx().memory_mut(|m| m.request_focus(egui::Id::new(composer::INPUT_ID)));
    }

    // Apply pending reply selection (set chat composing context).
    if let Some(ctx) = pending_reply.take() {
        state.chat_reply_to = Some(ctx);
    }

    // Apply pending edit-target click (open the inline editor for this message).
    if let Some((ts, draft)) = pending_edit.take() {
        state.chat_edit_target = Some((ts, draft));
    }

    // Apply pending edit cancel (clear the inline editor).
    if pending_edit_cancel {
        state.chat_edit_target = None;
    }

    // Apply pending edit save (send the WS edit message + clear edit target). An edit
    // holding the recovery phrase is stopped (step F) and its editor stays open.
    // Never in a DM or a P2P group (BUG-178): the edit would carry its private text to the server.
    if is_private_channel(&state.chat_active_channel) {
        pending_edit_save = None;
        pending_pins.clear();
        pending_reactions.clear();
        pending_deletes.clear();
    }
    // An edit that is a `/claim` line is never sent to the room: the claim goes to the server alone,
    // or the claim help shows (the second review, W5), and the message stays as it was.
    let pending_edit_save = pending_edit_save.filter(|(_, c)| !crate::engine::owner::typed_claim(state, c));
    if let Some((ts, new_content)) = pending_edit_save.filter(|(_, c)| !crate::engine::warnings::guard_stops(state, &[c])) {
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let msg = serde_json::json!({
                    "type": "edit",
                    "from": state.profile_public_key,
                    "timestamp": ts,
                    "new_content": new_content.clone(),
                    "channel": state.chat_active_channel,
                });
                client.send(&msg.to_string());
            }
        }
        // Optimistic local update so the UI shows the new text immediately.
        for m in state.chat_messages.iter_mut() {
            if m.sender_key == state.profile_public_key && m.timestamp_ms == ts {
                m.content = new_content;
                break;
            }
        }
        state.chat_edit_target = None;
    }

    for key in pending_blocks {
        crate::engine::block::block(state, &key); // nothing is sent to them (step C)
    }
    // Step D: Report opens the Report dialog (reasons, evidence, a note, Also block
    // them). It replaced a `/report <name>` slash command that dropped the message.
    for msg in pending_reports {
        crate::engine::report::open_for_message(state, &msg);
    }

    // v0.281.0: send pending delete requests via WebSocket.
    // Protocol: RelayMessage::Delete { from, timestamp }. The
    // relay decides whether to honor based on requester's role
    // (own / admin / mod), so we don't gate locally beyond
    // hiding the menu entry for the non-eligible cases.
    for (from_key, ts) in pending_deletes {
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let r = serde_json::json!({
                    "type": "delete",
                    "from": from_key,
                    "timestamp": ts,
                });
                client.send(&r.to_string());
            }
        }
    }

    // Send pending pin requests via WebSocket.
    for (from_key, from_name, content, ts) in pending_pins {
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let r = serde_json::json!({
                    "type": "pin_request",
                    "from_key": from_key,
                    "from_name": from_name,
                    "content": content,
                    "timestamp": ts,
                    "channel": state.chat_active_channel,
                });
                client.send(&r.to_string());
            }
        }
    }

    // Apply any pending reaction sends collected during render.
    for (target_from, target_ts, emoji) in pending_reactions {
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let from_name = if !state.user_name.is_empty() {
                    state.user_name.clone()
                } else {
                    "Anonymous".to_string()
                };
                let r = serde_json::json!({
                    "type": "reaction",
                    "target_from": target_from,
                    "target_timestamp": target_ts,
                    "emoji": emoji,
                    "from": state.profile_public_key,
                    "from_name": from_name,
                    "channel": state.chat_active_channel,
                });
                client.send(&r.to_string());
            }
        }
    }
}

/// The full timestamp (YYYY-MM-DD HH:MM:SS UTC) beside a message's pull tab, its right middle at
/// `right_center`; where it stood. A tooltip (layer 1), so it takes no click: PAINTED
/// (`widgets::painted_card`, 2026-11-15). As the egui Area it was it took the click of whatever
/// stood under it, the name or a link in the pill while the reaction popup was open (the BUG-316
/// class). The website's `.pill-ts-expand` takes no pointer the same way.
pub(crate) fn draw_full_timestamp(ctx: &egui::Context, theme: &Theme, id: egui::Id, right_center: egui::Pos2, full_ts: &str) -> egui::Rect {
    use crate::gui::widgets::painted_card::{PaintedCard, Place, Words};
    let frame = Frame::NONE
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0_f32, theme.border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .shadow(egui::epaint::Shadow { offset: [1, 2], blur: 6, spread: 0, color: Color32::from_black_alpha(80) });
    PaintedCard::new(frame)
        .text(Words::new(full_ts, egui::FontId::monospace(theme.small_size), theme.text_secondary()))
        .paint(ctx, id, crate::gui::layers::Layer::Tooltip, Place::At(right_center, egui::Align2::RIGHT_CENTER))
}

#[cfg(test)]
#[path = "app_voice_tests.rs"]
mod app_voice_tests;
