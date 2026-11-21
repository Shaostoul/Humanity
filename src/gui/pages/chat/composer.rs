//! The chat's composer, one copy for every view of a conversation: the Chat page's input bar and
//! the chat footer's on the other pages (gui/bottom_strip/chat_footer.rs, docs/design/
//! app-bottom-strip.md section 4). Moved out of `chat.rs` whole on 2026-11-15: the reply banner,
//! the typing line, who can read the channel, the line limit's counter, the text box that grows
//! upward, the @mention list, and Send through the Chat page's own send path
//! (`send_composed_content`), so line limits, the send limiter, replies and the protected setup
//! apply wherever the person types.

use super::*;

/// Where a composer is drawn: the Chat page, or the chat footer on the other pages. The two keep
/// their measurements apart (each bar has its own width, so its own heights), and share the text
/// box itself ([`INPUT_ID`]): the draft (`chat_input`), its caret and a request for focus follow
/// the person from one to the other. Only one of them draws in a frame: the footer is off on the
/// Chat page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    Page,
    Footer,
}

impl Place {
    /// Where this place keeps the measurement `key` between frames. The Chat page's are the ids
    /// they always were.
    pub(crate) fn id(self, key: &str) -> egui::Id {
        match self {
            Place::Page => egui::Id::new(key),
            Place::Footer => egui::Id::new(key).with("chat_footer"),
        }
    }

    /// The most of its area the bar may take: on the Chat page 60%, the messages keeping the rest;
    /// in the footer's short strip 80%, so the box is never pushed out of a strip at its lowest.
    fn max_share(self) -> f32 {
        match self {
            Place::Page => 0.6,
            Place::Footer => 0.8,
        }
    }
}

/// The text box, by id: the same in both places.
pub(crate) const INPUT_ID: &str = "chat_composer_input";

/// The input bar's height to reserve at the bottom of an area `area_h` tall, and how much the text
/// box's lines beyond the first may add to it there: measured from the bar as last drawn in `place`
/// (BUG-311: from the box's allocation, never a scroll area's `inner_rect`).
pub(crate) fn reserve(ctx: &egui::Context, state: &GuiState, place: Place, area_h: f32) -> (f32, f32) {
    // Input height grows by 22 px when a reply context is active so the
    // "Replying to … [X]" banner has its own row above the text field.
    let reply_banner_h: f32 = if state.chat_reply_to.is_some() { 22.0 } else { 0.0 };
    // The "… is typing" indicator renders as its OWN row above the text field
    // too, so reserve its height the same way. Without this, an incoming typing
    // event grew the input bar's CONTENT but not its allocated rect, sliding the
    // composer down past the window's bottom edge — and behind the taskbar in
    // snapped-fullscreen (the operator's bug report). Mirror the freshness check
    // the renderer uses below (entries within TYPING_TTL = 3 s) so the reserved
    // height matches whether the row is actually drawn this frame.
    let typing_active = {
        let now = std::time::Instant::now();
        state
            .chat_typing_users
            .values()
            .any(|(_, t)| now.duration_since(*t) < std::time::Duration::from_secs(3))
    };
    let typing_h: f32 = if typing_active { 22.0 } else { 0.0 };
    // The text box's lines beyond the first, measured last frame (`COMPOSER_EXTRA_ID`, written
    // where the box is drawn). The bar was a fixed 52 px while the multiline box grew with the
    // text, so a long post spilled DOWN past the window's bottom edge and could not be read back
    // (operator, 2026-10-10). Now the bar takes the box's height and the messages give way, up to
    // COMPOSER_MAX_SHARE of the chat area; past that the box scrolls inside.
    let composer_extra = ctx.data(|d| d.get_temp::<f32>(place.id(COMPOSER_EXTRA_ID))).unwrap_or(0.0);
    let composer_cap = composer_extra_cap(area_h);
    // The bar's own parts as last drawn (`INPUT_BAR_REST_ID`: everything but the text box), plus
    // one line and the box's extra lines. The fixed 52 px stays as a floor; the bar's real
    // content is taller than that even at the default text size, so with the floor alone it
    // overflowed the window's bottom edge, under the taskbar at a larger text size (operator,
    // 2026-10-13). Measured from the bar itself, so a banner coming or going settles in a frame.
    let bar_rest = ctx.data(|d| d.get_temp::<f32>(place.id(INPUT_BAR_REST_ID))).unwrap_or(0.0);
    let extra_shown = composer_extra.min(composer_cap);
    // The text box's own height as last shown (`INPUT_BOX_H_ID`). One line is taller than
    // COMPOSER_ONE_LINE_H at a larger text size, so reserving the constant left the box's lower
    // border and padding under the taskbar (operator, 2026-10-14). Never less than one line plus
    // the extra lines, so a new line still grows the bar this frame.
    let _ = extra_shown;
    let box_needed = composer_text_h(ctx, place, composer_cap);
    // Once the bar has been drawn, it reserves exactly its own parts plus the box: no fixed floor,
    // so no empty band under the box. The first frame has nothing measured yet and uses the old
    // fixed estimate.
    let input_height = if bar_rest > 0.0 {
        bar_rest + box_needed
    } else {
        52.0 + reply_banner_h + typing_h + extra_shown
    }
    .min(area_h * place.max_share());
    (input_height, composer_cap)
}

/// Draw the input bar in `input_rect` (from [`reserve`]), and note its parts' heights for the next
/// frame's reservation.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, place: Place, input_rect: egui::Rect, composer_cap: f32) {
    // The bar's top and bottom padding (see the frame below), also added to its drawn height.
    let bar_pad_v = theme.spacing_xs.round().clamp(0.0, 40.0);
    let input_bar = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(input_rect), |ui| {
        Frame::NONE
            .fill(theme.bg_sidebar_dark())
            // Top and bottom padding from the theme's smallest spacing (Settings > Widgets,
            // `spacing_xs`, 2 px by default), so the box sits a hair above the window's bottom
            // edge and under the bar's top line rather than 8 px off each (operator, 2026-10-14).
            .inner_margin(egui::Margin::symmetric(16, bar_pad_v as i8))
            .show(ui, |ui| {
                // Reply banner — only shown when a reply context is active.
                if let Some(ref target) = reply::made_in(state, &state.chat_active_channel) {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(humanity_core::i18n::tf(
                                "chat.replying_to",
                                &[("name", &target.sender_name), ("words", &reply::cut(&target.preview, reply::QUOTE_CHARS))],
                            ))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                        );
                        if widgets::Button::ghost("X").show(ui, theme) {
                            state.chat_reply_to = None;
                        }
                    });
                }

                // v0.282.0 typing indicator. Prune entries older than 3s so
                // a typer who quit typing 5s ago doesn't linger forever, then
                // render the names of users still in the active channel. We
                // intentionally don't filter by channel here — typing events
                // don't carry channel info (relay-side rate-limit is per-user,
                // not per-channel), so showing all typers app-wide is the
                // truthful answer until the protocol carries channel.
                const TYPING_TTL: std::time::Duration = std::time::Duration::from_secs(3);
                let now_typing_tick = std::time::Instant::now();
                state.chat_typing_users.retain(|_, (_, t)| now_typing_tick.duration_since(*t) < TYPING_TTL);
                if !state.chat_typing_users.is_empty() {
                    let names: Vec<String> = state.chat_typing_users.values()
                        .map(|(name, _)| name.clone())
                        .collect();
                    let label = match names.len() {
                        0 => String::new(), // unreachable per the is_empty check above
                        1 => format!("{} is typing…", names[0]),
                        2 => format!("{} and {} are typing…", names[0], names[1]),
                        _ => format!("{} and {} others are typing…", names[0], names.len() - 1),
                    };
                    if !label.is_empty() {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(label)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted())
                                    .italics(),
                            );
                        });
                        // Repaint so the prune-on-render keeps the line fresh
                        // (otherwise a UI without other inputs would stick the
                        // indicator past its TTL until something else redrew).
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                    }
                }

                // Who can read this channel, and what I may post here (chat/access.rs).
                access::draw_audience_line(ui, theme, state);
                // How long the line is against the server's limit, near and past it, as the
                // website's counter shows it (chat/line_limit.rs).
                if let Some(limit) = line_limit::limit_here(state) {
                    if let Some((words, color)) = line_limit::counter(state.chat_input.chars().count(), limit, theme) {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(words).size(theme.font_size_small).color(color));
                        });
                    }
                }

                Frame::NONE
                    // The message text field is a raised input surface -> the
                    // tertiary/extreme-bg token (what egui uses for text edits).
                    .fill(theme.bg_tertiary())
                    .corner_radius(CornerRadius::same(theme.border_radius_lg as u8))
                    .stroke(Stroke::new(1.0_f32, theme.border()))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let hint = if state.chat_active_channel.starts_with("dm:") {
                        let pk = &state.chat_active_channel["dm:".len()..];
                        let name = state.chat_dms.iter().find(|d| d.user_key == pk)
                            .map(|d| d.user_name.as_str()).unwrap_or("user");
                        format!("Message {}", name)
                    } else if let Some(gid) = state.chat_active_channel.strip_prefix("p2pgroup:") {
                        let name = state.p2p_groups.iter().find(|g| g.group_id == gid)
                            .map(|g| g.name.as_str()).unwrap_or("group");
                        format!("Message {} (encrypted)", name)
                    } else if let Some(room) = commons_room_of(&state.chat_active_channel) {
                        // Name the outgoing server EXPLICITLY: in the first
                        // field test a Commons send silently routed through a
                        // background carrier while the operator sat on an
                        // offline private server -- the composer must say
                        // where the message will actually go. The send's own route
                        // (chat/commons.rs), so the two cannot disagree.
                        let via = match commons_route(state, room) {
                            (true, _) => Some(server_display_name(&state.server_url)),
                            (false, Some(i)) => Some(server_display_name(&state.connections[i].url)),
                            (false, None) => None,
                        };
                        if commons_room_read_only(state, room) == Some(true) {
                            format!("#{} is read-only (announcements from the operators)", room)
                        } else {
                            match via {
                                Some(v) => format!("Message #{} (sends via {})", room, v),
                                None => format!("#{}: no connected server carries this room", room),
                            }
                        }
                    } else {
                        format!("Message #{}", state.chat_active_channel)
                    };
                    // Room for the trailing widgets on this row (search, pins,
                    // help, Attach, Send) is MEASURED, not guessed: the row
                    // records last frame's width of everything that is not
                    // the text area (`composer_overhead_id` below) and the box
                    // takes the rest. A fixed guess kept going stale: 104 px
                    // fit three icons, 210 px fit four, and with Attach added
                    // and Noto Sans widths the Send button was cut off at the
                    // right edge (2026-09-27 snapshot review). The first frame
                    // falls back to the old guess and corrects itself on the
                    // next one.
                    let composer_overhead_id = place.id("chat_composer_overhead");
                    let composer_row_w = ui.available_width();
                    let composer_overhead = ui
                        .ctx()
                        .data(|d| d.get_temp::<f32>(composer_overhead_id))
                        .unwrap_or(210.0);
                    let composer_text_w = (composer_row_w - composer_overhead).max(120.0);
                    // Stable id (v0.972, operator: the "is typing" indicator
                    // appearing made the box lose focus): egui auto-ids
                    // derive from layout position, and the indicator row
                    // shifts the composer, so the focused id changed under
                    // the caret. A fixed id keeps focus across any layout
                    // shift (typing rows, reply banners, resizes).
                    // Multiline (v0.1206.x composer expand) so a
                    // multi-paragraph post can be written AND proofread in
                    // place instead of a cramped single line (operator
                    // 2026-08-24: "never expands to show the whole post;
                    // editing is miserable"). desired_rows(1) keeps it
                    // one line tall until you actually write more, then it
                    // grows with the content. Enter still SENDS (the
                    // universal chat gesture + matches the web client);
                    // return_key = Shift+Enter makes Shift+Enter the
                    // newline. NOTE: with a non-plain return_key, egui does
                    // NOT surrender focus on plain Enter for a multiline box
                    // (see egui text_edit builder.rs ~L990) -- so the send
                    // trigger below uses has_focus()+Enter, NOT lost_focus().
                    // Past COMPOSER_MAX_SHARE of the chat area the box scrolls inside (its
                    // reserved height is capped there, in `draw_center_panel` above), so the
                    // box grows UPWARD and a long post can be read back (operator, 2026-10-10).
                    // The box is offered EXACTLY its text's height (as last laid out, `TEXT_H_ID`),
                    // capped. A scroll area takes all the height it is offered whatever its text,
                    // so offered the row's few lines it stopped there and the top lines scrolled out
                    // of view, while the bar kept growing empty space below it (operator's
                    // screenshots, 2026-10-14: eight lines typed, the last four shown); offered the
                    // whole cap it stood 273 px tall for one line. The bar reserves the same height
                    // (`box_needed` in `draw_center_panel`), so the two always agree.
                    let box_want = composer_text_h(ui.ctx(), place, composer_cap);
                    // The characters past the server's limit on a line, in red on dark red
                    // (operator, 2026-10-24; chat/line_limit.rs). Only where a limit applies.
                    let mut over_limit = line_limit::limit_here(state).map(|n| widgets::over_limit::layouter(theme, n, false));
                    // Its own space in the row, exactly that tall: the row beside the buttons offers
                    // only a few lines on its own.
                    let composer_alloc = ui
                        .allocate_ui_with_layout(egui::vec2(composer_text_w, box_want), Layout::top_down(Align::Min), |ui| {
                        egui::ScrollArea::vertical()
                        .id_salt(place.id("chat_composer_scroll"))
                        .max_height(box_want)
                        .auto_shrink([true, true])
                        .show(ui, |ui| {
                            let mut edit = egui::TextEdit::multiline(&mut state.chat_input)
                                .id(egui::Id::new(INPUT_ID))
                                .desired_width(composer_text_w)
                                .desired_rows(1)
                                .return_key(Some(egui::KeyboardShortcut::new(
                                    egui::Modifiers::SHIFT,
                                    egui::Key::Enter,
                                )))
                                .hint_text(hint);
                            if let Some(l) = over_limit.as_mut() {
                                edit = edit.layouter(l);
                            }
                            edit.show(ui)
                        })
                        });
                    // The space the box took in the row (its allocation, as drawn).
                    let composer_drawn_h = composer_alloc.response.rect.height();
                    let composer = composer_alloc.inner;
                    // The text's own height, for next frame's offer and the bar's reservation.
                    let text_h = composer.inner.response.rect.height();
                    let text_id = place.id(TEXT_H_ID);
                    if (text_h - ui.ctx().data(|d| d.get_temp::<f32>(text_id)).unwrap_or(0.0)).abs() > 0.5 {
                        ui.ctx().data_mut(|d| d.insert_temp(text_id, text_h));
                        ui.ctx().request_repaint();
                    }
                    // The box's height as shown, so the bar's other parts can be measured below: the
                    // space it took in the row, NOT the scroll area's `inner_rect`, which egui reports at
                    // its minimum scrolled height (64 px) whatever the box shows. With that figure the
                    // bar's other parts read 64 px too small. While they were under 64 px the difference
                    // went negative, the first-frame 52 px estimate stood in and happened to fit; once the
                    // line above the box (who can read this channel, 2026-11-02) made them taller, the bar
                    // reserved about 22 px for 70 px of content and the box drew under the window's
                    // bottom edge (operator, 2026-11-15: "I can't see the chat bar to type in").
                    ui.ctx().data_mut(|d| d.insert_temp(place.id(INPUT_BOX_H_ID), composer_drawn_h));
                    // Lines beyond the first, for next frame's bar height.
                    let extra = composer_extra_of(&composer.inner.galley);
                    let extra_id = place.id(COMPOSER_EXTRA_ID);
                    if (extra - ui.ctx().data(|d| d.get_temp::<f32>(extra_id)).unwrap_or(0.0)).abs() > 0.5 {
                        ui.ctx().data_mut(|d| d.insert_temp(extra_id, extra));
                        ui.ctx().request_repaint();
                    }
                    let response = composer.inner.response;

                    // v0.282.0 outgoing typing event, on an actual edit
                    // (egui's response.changed() debounces to "edit, not
                    // focus/scroll"); the rules are in `send_typing`.
                    if response.changed() {
                        send_typing(state, std::time::Instant::now());
                    }

                    // (Clipboard image paste detection moved to the top of
                    // pub fn draw() in v0.233 because egui's TextEdit consumes
                    // the Ctrl+V key event via filtered_events() before any
                    // code below it can detect it. See `pub fn draw` for the
                    // working detection.)

                    // ── @mention autocomplete ──
                    // If the input ends with `@partial` (no whitespace after the @),
                    // show a popup of matching users from chat_users.
                    let mention_partial: Option<String> = {
                        let text = &state.chat_input;
                        if let Some(at_pos) = text.rfind('@') {
                            let after_at = &text[at_pos + 1..];
                            // No whitespace after the @ means we're still typing the mention.
                            if !after_at.contains(char::is_whitespace) && after_at.len() < 32 {
                                Some(after_at.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    // True if Enter was consumed this frame to pick a mention
                    // (so the message-send below must NOT also fire).
                    let mut mention_took_enter = false;

                    if let Some(partial) = mention_partial {
                        let partial_lower = partial.to_lowercase();
                        let matches: Vec<String> = state.chat_users.iter()
                            .filter(|u| u.name.to_lowercase().starts_with(&partial_lower))
                            .take(8)
                            .map(|u| u.name.clone())
                            .collect();

                        // NOTE: no `response.has_focus()` gate — clicking a
                        // popup row defocuses the TextEdit FIRST, which (under
                        // the old guard) removed the popup the same frame the
                        // click would land, so mouse-select never worked
                        // (operator-reported 2026-05-15). The popup is now
                        // scoped purely by "input ends in @partial with
                        // matches", which disappears naturally once a name is
                        // inserted (the @partial is gone).
                        if !matches.is_empty() {
                            // Clamp highlight index into range.
                            if state.chat_mention_index >= matches.len() {
                                state.chat_mention_index = 0;
                            }

                            // Keyboard nav. A focused single-line TextEdit's
                            // event filter has vertical_arrows:false, so
                            // Up/Down are NOT consumed by it — ui.input sees
                            // them. Enter triggers the TextEdit's lost_focus
                            // but the key_pressed flag is still readable.
                            let (k_up, k_down, k_enter) = ui.input(|i| (
                                i.key_pressed(egui::Key::ArrowUp),
                                i.key_pressed(egui::Key::ArrowDown),
                                i.key_pressed(egui::Key::Enter),
                            ));
                            if k_down {
                                state.chat_mention_index =
                                    (state.chat_mention_index + 1) % matches.len();
                            }
                            if k_up {
                                state.chat_mention_index =
                                    (state.chat_mention_index + matches.len() - 1) % matches.len();
                            }

                            // Selection can come from Enter (highlighted row)
                            // or a mouse click on any row. Computed into
                            // locals so the Area closure doesn't need to
                            // borrow `state`.
                            let cur_index = state.chat_mention_index;
                            let mut selected: Option<String> = None;
                            let mut new_index = cur_index;
                            if k_enter {
                                selected = matches.get(cur_index).cloned();
                            }

                            // Render the suggestion list as a foreground Area
                            // anchored just ABOVE the input. Highlighted row
                            // uses the accent fill; hover moves the highlight.
                            let row_h = 24.0_f32;
                            let area_h = matches.len() as f32 * row_h + 34.0;
                            let area_pos = egui::pos2(
                                response.rect.left(),
                                response.rect.top() - area_h - 4.0,
                            );
                            crate::gui::layers::area(ui.ctx(), egui::Id::new("mention_autocomplete_area"), crate::gui::layers::Layer::PageFloat)
                                .fixed_pos(area_pos)
                                .show(ui.ctx(), |ui| {
                                    Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.set_min_width(220.0);
                                        ui.label(
                                            RichText::new(humanity_core::i18n::tf("chat.mention_title", &[("partial", &partial)]))
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                        ui.separator();
                                        for (idx, name) in matches.iter().enumerate() {
                                            let is_sel = idx == cur_index;
                                            let btn = egui::Button::new(
                                                RichText::new(format!("@{}", name)).color(
                                                    if is_sel {
                                                        theme.text_on_accent()
                                                    } else {
                                                        theme.text_primary()
                                                    },
                                                ),
                                            )
                                            .fill(if is_sel {
                                                theme.accent()
                                            } else {
                                                Color32::TRANSPARENT
                                            })
                                            .min_size(Vec2::new(210.0, row_h - 2.0));
                                            let r = ui.add(btn);
                                            if r.hovered() {
                                                new_index = idx;
                                            }
                                            if r.clicked() {
                                                selected = Some(name.clone());
                                            }
                                        }
                                    });
                                });

                            state.chat_mention_index = new_index;

                            if let Some(name) = selected {
                                // Replace the trailing @partial with @name + space.
                                if let Some(at_pos) = state.chat_input.rfind('@') {
                                    state.chat_input.truncate(at_pos);
                                    state.chat_input.push('@');
                                    state.chat_input.push_str(&name);
                                    state.chat_input.push(' ');
                                }
                                state.chat_mention_index = 0;
                                // Move the text caret to the END of the input
                                // (just past the trailing space we appended).
                                // egui keeps the caret in TextEditState keyed
                                // by the widget id; without this the caret
                                // stays where it was (mid-name) so the next
                                // keystroke lands inside the inserted name
                                // (operator-reported 2026-05-15).
                                {
                                    let end = egui::text::CCursor::new(
                                        state.chat_input.chars().count(),
                                    );
                                    let mut tes = egui::text_edit::TextEditState::load(
                                        ui.ctx(), response.id,
                                    ).unwrap_or_default();
                                    tes.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::one(end),
                                    ));
                                    tes.store(ui.ctx(), response.id);
                                }
                                // Always re-focus the input (whether the pick
                                // came from Enter, click, or arrow+Enter) so
                                // the caret move sticks and the user keeps
                                // typing seamlessly.
                                ui.memory_mut(|m| m.request_focus(response.id));
                                if k_enter {
                                    // Don't let the same Enter also send the
                                    // message.
                                    mention_took_enter = true;
                                }
                            }
                        }
                    }

                    // Suppress the message-send when Enter was just used to
                    // pick a mention from the autocomplete popup.
                    //
                    // The composer is now a multiline TextEdit with
                    // return_key = Shift+Enter, so plain Enter is NOT consumed
                    // by the box (it neither inserts a newline nor surrenders
                    // focus). We therefore detect the send on has_focus()+Enter
                    // (shift excluded, since Shift+Enter is the newline) rather
                    // than the old singleline lost_focus() path.
                    let enter_pressed = !mention_took_enter
                        && response.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);

                    // The Chat page's own tools (search, pins, the help, Attach) open its own panels;
                    // the footer leaves them out, Open in Chat beside it (gui/bottom_strip/chat_footer.rs).
                    if place == Place::Page {
                        // Search button — opens the message search modal.
                        if widgets::Button::ghost("🔍").tooltip_above(humanity_core::i18n::t("chat.search_tip"), input_rect.top()).show(ui, theme) {
                            state.chat_search_open = true;
                        }

                        // Pins button — shows pin count + opens the pins modal.
                        let pin_count = state.chat_pins.get(&state.chat_active_channel).map(|p| p.len()).unwrap_or(0);
                        let pin_label = if pin_count > 0 { format!("📌 {}", pin_count) } else { "📌".to_string() };
                        if widgets::Button::ghost(&pin_label).tooltip_above(humanity_core::i18n::t("chat.pins_tip"), input_rect.top()).show(ui, theme) {
                            state.chat_pins_open = true;
                        }

                        // Help button (?) - opens slash commands reference
                        if widgets::Button::ghost("?").tooltip_above(humanity_core::i18n::t("chat.help_tip"), input_rect.top()).show(ui, theme) {
                            state.show_help_modal = !state.show_help_modal;
                        }

                        // In-app file attach (v0.708, all-in-one direction: our
                        // OWN browser widget, not an OS dialog).
                        let attach_limit = attach_send::limit_for(attach_send::Limits::of(state), attach_send::Destination::of(&state.chat_active_channel));
                        let attach_tip = format!(
                            "Attach a file from your computer (images, documents, video, 3D models - up to {} MB here). \
                             3D files from a server's room are also published to its Shared Files library.",
                            attach_limit / (1024 * 1024)
                        );
                        // Hidden in a DM or a group when the person turned it off ("receive only").
                        if !attach_step::attach_off_here(state) && widgets::Button::secondary(humanity_core::i18n::t("chat.attach")).tooltip_above(&attach_tip, input_rect.top()).show(ui, theme) {
                            state.chat_attach_picker = Some(
                                crate::gui::widgets::file_browser::FilePickerState::new(ATTACH_EXTS, attach_limit).multiple(),
                            );
                        }
                    }
                    let send_clicked = widgets::Button::primary(humanity_core::i18n::t("chat.send"))
                        .tooltip_above(humanity_core::i18n::t("chat.send_tip"), input_rect.top())
                        .show(ui, theme);
                    // Everything on this row that is not the text area: the
                    // box's own margins beyond its desired width, the spacing,
                    // and the five buttons. Next frame's box is sized from it.
                    let row_used = ui.min_rect().right() - response.rect.left();
                    let overhead = (row_used - composer_text_w).max(0.0);
                    if (overhead - composer_overhead).abs() > 0.5 {
                        ui.ctx().data_mut(|d| d.insert_temp(composer_overhead_id, overhead));
                        ui.ctx().request_repaint();
                    }

                    if (enter_pressed || send_clicked) && !state.chat_input.trim().is_empty() {
                        let content = state.chat_input.trim().to_string();
                        // ALL content routing (P2P group / scratchpad / E2EE
                        // DM / group / channel + the local echo) lives in
                        // send_composed_content -- the single authority
                        // shared with the clipboard-paste and file-attach
                        // flows so the three paths can never drift. (The web
                        // client had exactly that drift as the DM-attachment
                        // privacy leak, fixed v0.698.2; native's clipboard
                        // flow had the same class of bug until v0.708.)
                        if send_composed_content(state, &content) {
                            state.chat_input.clear();
                            response.request_focus();
                        }
                    }

                    if enter_pressed {
                        response.request_focus();
                    }
                });
                });
                // The height of what was DRAWN in the bar, not of the space it was given. The
                // frame's own rect stretches to fill the reservation, so measuring that read the
                // reservation back as the bar's height: every new line grew it and nothing shrank
                // it, until it filled 60% of the chat (v0.1499.0, operator, 2026-10-14). The
                // allocation's response before that was held to the space given, so it read the
                // bar as fitting when it was not (v0.1497.0).
                ui.min_rect().height()
            })
            .inner
            + 2.0 * bar_pad_v
    });
    // The bar's height as drawn this frame less its text box: next frame's reservation (see
    // `bar_rest` above). A change bigger than half a pixel asks for one more frame to settle.
    let box_h = ui.ctx().data(|d| d.get_temp::<f32>(place.id(INPUT_BOX_H_ID))).unwrap_or(0.0);
    let rest = (input_bar.inner - box_h).max(0.0);
    let rest_id = place.id(INPUT_BAR_REST_ID);
    if (rest - ui.ctx().data(|d| d.get_temp::<f32>(rest_id)).unwrap_or(0.0)).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(rest_id, rest));
        ui.ctx().request_repaint();
    }
}

/// Where the chat's input bar keeps its height as last drawn WITHOUT its text box (toolbar,
/// margins, reply banner, typing line), so the reservation fits the bar at any text size.
pub(crate) const INPUT_BAR_REST_ID: &str = "chat_input_bar_rest_h";
/// Where the chat's text box keeps its height as shown this frame (for `INPUT_BAR_REST_ID`).
pub(crate) const INPUT_BOX_H_ID: &str = "chat_input_box_h";

/// Where the composer's measured extra height (its lines beyond the first) is kept between frames.
pub(crate) const COMPOSER_EXTRA_ID: &str = "chat_composer_extra_h";
/// The most of the chat area the composer may take before its text scrolls inside: the web's
/// `autoResizeTextarea` cap (45% of the window), so both clients behave alike.
pub(crate) const COMPOSER_MAX_SHARE: f32 = 0.45;
/// About one line of the composer, for the scroll area's height (the bar's fixed 52 px holds it).
pub(crate) const COMPOSER_ONE_LINE_H: f32 = 22.0;

/// Where the composer keeps its text's own height as last laid out (one line plus its extra
/// lines, at whatever text size), for the box's offer and the bar's reservation.
pub(crate) const TEXT_H_ID: &str = "chat_composer_text_h";

/// The height the composer box is offered and the bar reserves for it: its text's height as last
/// laid out, at least one line, at most one line plus `composer_cap` (past that the text scrolls
/// inside the box).
pub(crate) fn composer_text_h(ctx: &egui::Context, place: Place, composer_cap: f32) -> f32 {
    let text = ctx.data(|d| d.get_temp::<f32>(place.id(TEXT_H_ID))).unwrap_or(COMPOSER_ONE_LINE_H);
    // Rounded up, plus one pixel: the box's rect is snapped to whole pixels, and a box one pixel
    // short of its text clips its own bottom border line, the line the operator asked to see.
    let text = text.ceil() + 1.0;
    text.clamp(COMPOSER_ONE_LINE_H.min(text.max(1.0)), COMPOSER_ONE_LINE_H + composer_cap)
}

/// How much the composer's extra lines may add to the bar in a chat area `area_h` tall.
pub(crate) fn composer_extra_cap(area_h: f32) -> f32 {
    (area_h * COMPOSER_MAX_SHARE - 52.0).max(0.0)
}

/// The height of a laid-out composer's lines beyond the first.
pub(crate) fn composer_extra_of(galley: &egui::Galley) -> f32 {
    let rows = galley.rows.len().max(1) as f32;
    let row_h = galley.rows.first().map(|r| r.height()).unwrap_or(COMPOSER_ONE_LINE_H);
    (rows - 1.0) * row_h
}
