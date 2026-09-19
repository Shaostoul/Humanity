//! The chat page's PEOPLE surfaces: the right rail and the live strip.
//!
//! The right rail lists who you can talk to (Friends, then the current
//! server's Members) and the live strip across the top of the page lists who is
//! broadcasting right now. They are one cluster because they are the same
//! thing drawn twice: both are lists of people, both open the user modal on
//! click, and both go through the single `draw_user_row` that owns what a
//! person looks like in this page (identicon, presence dot, role badge, name
//! colour). Keeping them together is what stops that row drifting into two.
//!
//! Extracted VERBATIM from `gui/pages/chat.rs` (file-size ratchet), which stood
//! at 9,053 lines against an 8,000 budget. Third of four clusters out in that
//! pass. The only delta beyond the move is two `pub(super)` prefixes, on
//! `draw_right_panel` and `draw_live_strip`, whose caller (`draw`, the page
//! frame) stayed behind.
//!
//! Note the live strip is drawn by `draw` at the TOP of the page, not inside
//! the rail: it was moved out of this column in v0.1150 so a stream card gets
//! the full page width. It lives in this file because of what it draws, not
//! where it is painted.
//!
//! Takes `use super::*` like the page's other children, so the shared section
//! headers, `name_color`, the role-badge painter and the page's imports all
//! arrive without any of them being widened.

use super::*;

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`; it was private when the two lived in one file.
pub(super) fn draw_right_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ScrollArea::vertical()
        .id_salt("chat_right_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // The Studio quick-access section moved OUT of this rail into the
            // full-width live strip at the top of the page (v0.1150, operator
            // direction). See draw_live_strip.

            // ── Friends Section ──
            draw_friends_section(ui, theme, state);

            ui.add_space(4.0);

            // ── Server Members Section ──
            draw_members_section(ui, theme, state);
        });
}

/// Studio (broadcast/livestream) quick-access section at the very top of the
/// chat right rail — a native mirror of the website's studio widget. The point
/// is durability: when the top nav condenses into Real/Play pages, the current
/// nav-bar Studio button disappears, so streamers reach Go Live and the full
/// Studio page from here instead. Collapse state persists via AppConfig.
/// The live strip: a full-width expandable panel at the top of the Chat page
/// (v0.1150, the studio-watch design's increment 1). Collapsed it is one slim
/// clickable row that says whether YOU are on air and how many streams are
/// live on this server; expanded it carries the broadcast controls (the old
/// right-rail Studio section, promoted) and the live-now directory with
/// one-click Watch. Collapse state reuses `chat_studio_collapsed` so existing
/// configs keep their preference.
// `pub(super)` only because its caller (`draw`, which paints the strip across
// the top of the page rather than inside the rail) stayed in `chat.rs`.
pub(super) fn draw_live_strip(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Keep the directory fresh while the Chat page is open, whether or not
    // the strip is expanded: the collapsed header shows the live count, and a
    // stale zero would read as "nobody streams here".
    crate::gui::pages::watch::poll_directory(ctx, state);

    let collapsed = state.chat_studio_collapsed;
    egui::TopBottomPanel::top("chat_live_strip")
        .frame(
            Frame::NONE
                .fill(theme.bg_tertiary())
                .inner_margin(egui::Margin::symmetric(8, 4)),
        )
        .show(ctx, |ui| {
            // ── Header row: always visible, one click toggles the body ──
            let on_air = state.studio.broadcast_live;
            let live_count = state.watch_streams.len();
            let summary = if on_air {
                format!("ON AIR, {} watching", state.studio.broadcast_viewers)
            } else if state.studio.is_live {
                "Connecting...".to_string()
            } else if live_count == 1 {
                "1 stream live".to_string()
            } else if live_count > 1 {
                format!("{live_count} streams live")
            } else {
                "Go live, watch streams".to_string()
            };
            let title = format!("Live: {summary}");
            let bg = if on_air { theme.success().gamma_multiply(0.25) } else { theme.bg_tertiary() };
            if section_header(ui, theme, &title, collapsed, bg) {
                state.chat_studio_collapsed = !state.chat_studio_collapsed;
                crate::config::AppConfig::from_gui_state(state).save();
            }
            if collapsed {
                return;
            }

            ui.spacing_mut().item_spacing.y = theme.row_gap;
            ui.add_space(4.0);

            // ── Broadcast controls (promoted from the old right-rail section) ──
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.spacing_mut().item_spacing.x = 6.0;

                if state.studio.is_live {
                    if widgets::Button::danger("End Stream").show(ui, theme) {
                        state.studio.is_live = false;
                        state.studio.is_paused = false;
                        // The engine loop owns the publisher; without this the
                        // broadcast kept running after Chat said it ended (the
                        // pre-v0.1150 dead-button rot).
                        state.studio.broadcast_request = Some(false);
                    }
                } else if widgets::Button::primary("Go Live").show(ui, theme) {
                    state.studio.is_live = true;
                    state.studio.is_paused = false;
                    state.studio.live_start_time = ui.ctx().input(|i| i.time);
                    state.studio.broadcast_error.clear();
                    // Ask the engine to start the REAL publisher, exactly like
                    // the Studio page's Go Live. Before v0.1150 this was
                    // missing, so Chat's Go Live broadcast nothing.
                    state.studio.broadcast_request = Some(true);
                }

                // push_nav_to so Esc on the Studio page returns to Chat.
                if widgets::Button::secondary("Open Studio").show(ui, theme) {
                    state.push_nav_to(crate::gui::GuiPage::Studio);
                }

                // Honest status, mirrored from the real publisher.
                if on_air {
                    ui.label(
                        RichText::new(format!(
                            "{} kbps, watch at {}",
                            state.studio.broadcast_kbps, state.studio.broadcast_url
                        ))
                        .size(theme.font_size_small)
                        .color(theme.success()),
                    );
                } else if !state.studio.broadcast_error.is_empty() {
                    ui.label(
                        RichText::new(state.studio.broadcast_error.clone())
                            .size(theme.font_size_small)
                            .color(theme.danger()),
                    );
                }
            });

            // ── Live now: the same directory the Watch page lists ──
            let streams = state.watch_streams.clone();
            if !streams.is_empty() {
                ui.add_space(2.0);
                for (id, stream_title, viewers, bound_chat) in streams {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let label =
                            if stream_title.is_empty() { id.clone() } else { stream_title.clone() };
                        ui.label(RichText::new(label).strong());
                        ui.label(
                            RichText::new(format!("{viewers} watching"))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                        if widgets::Button::secondary("Watch").show(ui, theme) {
                            crate::gui::pages::watch::start_watching(state, &id);
                            state.push_nav_to(crate::gui::GuiPage::Watch);
                        }
                    });
                }
            }
            ui.add_space(4.0);
        });
}

/// Shared user row renderer for both friends and members lists.
/// Ensures consistent spacing, hover, click handling, and layout.
fn draw_user_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    name: &str,
    public_key: &str,
    role: &str,
    status: &str,
    state: &mut GuiState,
    ctx_time: f64,
) {
    let is_modal_target = state.chat_user_modal_open
        && state.chat_user_modal_key == public_key;

    let row_height = theme.row_height;
    let (full_rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(full_rect) {
        let bg = if response.hovered() {
            theme.bg_tertiary()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(full_rect, 0.0, bg);

        // RGB border when this user's modal is open
        if is_modal_target {
            let border_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
            ui.painter().rect_stroke(
                full_rect, 2.0,
                egui::Stroke::new(1.5, border_color),
                egui::epaint::StrokeKind::Inside,
            );
            ui.ctx().request_repaint();
        }

        let mut cx = full_rect.left() + theme.item_padding;
        let cy = full_rect.center().y;

        // Online/offline dot
        let dot_color = match status {
            "offline" => theme.text_muted(),
            "away" => theme.warning(),
            "busy" | "dnd" => theme.danger(),
            _ => theme.success(),
        };
        let dot_r = theme.status_dot_size / 2.0;
        ui.painter().circle_filled(egui::pos2(cx + dot_r, cy), dot_r, dot_color);
        cx += theme.status_dot_size + 4.0;

        // Name
        let nc = if status == "offline" { theme.text_muted() } else { theme.text_primary() };
        let name_galley = ui.painter().layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(theme.body_size),
            nc,
        );
        ui.painter().galley(egui::pos2(cx, cy - name_galley.size().y / 2.0), name_galley, nc);
        // Advance past name for role badges
        let name_w = ui.painter().layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(theme.body_size),
            nc,
        ).size().x;
        cx += name_w + 4.0;

        // Role badge (single letter pill)
        if !role.is_empty() && role != "member" {
            // Same role -> token mapping as widgets::role_badge, so the single
            // badge palette in theme.ron drives every role pill in the app.
            let (badge_color, badge_letter) = match role {
                "admin" => (theme.badge_admin(), "A"),
                "mod" | "moderator" => (theme.badge_mod(), "M"),
                "verified" => (theme.badge_verified(), "V"),
                "donor" => (theme.badge_donor(), "D"),
                _ => (theme.text_muted(), "?"),
            };
            let badge_rect = egui::Rect::from_min_size(
                egui::pos2(cx, cy - 7.0),
                Vec2::new(14.0, 14.0),
            );
            ui.painter().rect_filled(badge_rect, Rounding::same(3), badge_color);
            ui.painter().text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                badge_letter,
                egui::FontId::proportional(9.0),
                Color32::WHITE,
            );
            cx += 18.0;
        }

        // Follow-direction badge (v0.721, mirrors the web peer-list
        // indicators): both-ways arrow = mutual (friends), right arrow =
        // I follow them, left arrow = they follow me (not followed back).
        // Painted shapes, NOT text glyphs — the U+2190 "←" char renders as
        // tofu in the app font (screenshot-confirmed). No badge on my own row.
        if public_key != state.profile_public_key {
            let i_follow = state.chat_following_keys.contains(public_key)
                || state.chat_friends.iter().any(|f| f.public_key == public_key);
            let follows_me = state.chat_followers.contains(public_key);
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(cx + 2.0, cy - 6.0),
                Vec2::splat(12.0),
            );
            match (i_follow, follows_me) {
                (true, true) => crate::gui::widgets::icons::paint_arrow_both(ui.painter(), icon_rect, theme.success()),
                (true, false) => crate::gui::widgets::icons::paint_arrow_right(ui.painter(), icon_rect, theme.text_muted()),
                (false, true) => crate::gui::widgets::icons::paint_arrow_left(ui.painter(), icon_rect, theme.warning()),
                (false, false) => {}
            }
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        // Explain the follow-direction arrow on hover.
        let i_follow = state.chat_following_keys.contains(public_key)
            || state.chat_friends.iter().any(|f| f.public_key == public_key);
        let follows_me = state.chat_followers.contains(public_key);
        if public_key != state.profile_public_key && (i_follow || follows_me) {
            let txt = match (i_follow, follows_me) {
                (true, true) => "Friends (you follow each other)",
                (true, false) => "You follow them (not followed back yet)",
                (false, true) => "Follows you (you don't follow back yet)",
                (false, false) => "",
            };
            if !txt.is_empty() {
                response.clone().on_hover_text(txt);
            }
        }
    }
    if response.clicked() {
        state.chat_user_modal_open = true;
        state.chat_user_modal_name = name.to_string();
        state.chat_user_modal_key = public_key.to_string();
    }
}

fn draw_friends_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_friends_collapsed;
    let friend_count = state.chat_friends.len();

    if section_header(ui, theme, &format!("Friends ({})", friend_count), collapsed, theme.bg_tertiary()) {
        state.chat_friends_collapsed = !state.chat_friends_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        ui.spacing_mut().item_spacing.y = theme.row_gap;

        if state.chat_friends.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("No friends added yet")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(4.0);
        }

        let ctx_time = ui.ctx().input(|i| i.time);
        let friends = state.chat_friends.clone();
        for friend in &friends {
            // NO role badge here (v0.693, operator): follows are UNIVERSAL,
            // admin/mod is server-scoped truth -- a mod of one server is not
            // a mod of your friends list. Pass an empty role so the shared
            // row renders name + status only.
            draw_user_row(ui, theme, &friend.name, &friend.public_key, "", &friend.status, state, ctx_time);
        }
    }
}

fn draw_members_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_members_collapsed;
    let member_count = state.chat_users.len();
    let server_name = server_display_name(&state.server_url);

    if section_header(ui, theme, &format!("{} ({})", server_name, member_count), collapsed, theme.bg_tertiary()) {
        state.chat_members_collapsed = !state.chat_members_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        ui.spacing_mut().item_spacing.y = theme.row_gap;

        if state.chat_users.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("No users online")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(4.0);
        }

        // Sort: online first, then alphabetical
        let mut users = state.chat_users.clone();
        users.sort_by(|a, b| {
            let a_online = a.status != "offline";
            let b_online = b.status != "offline";
            b_online.cmp(&a_online).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let ctx_time = ui.ctx().input(|i| i.time);
        for user in &users {
            draw_user_row(ui, theme, &user.name, &user.public_key, &user.role, &user.status, state, ctx_time);
        }

        // ── DEV: native WebRTC P2P transport self-test (increment 1) ──
        // Minimal proof button: pick a target peer (the user whose profile
        // modal is open, else the first online user that isn't us) and open a
        // DataChannel to them. On open, lib.rs auto-sends "native p2p test";
        // received frames + open/close show in the in-app Debug console. This
        // is a transport proof, not polished UI — it lives only in native
        // builds and only when a WebRTC manager is up.
        #[cfg(feature = "native")]
        {
            // Resolve a target: prefer the modal-open user, else first online
            // non-self user from the sorted list.
            let me = state.profile_public_key.clone();
            let target: Option<(String, String)> = {
                let modal_key = if state.chat_user_modal_open {
                    Some(state.chat_user_modal_key.clone())
                } else {
                    None
                };
                modal_key
                    .filter(|k| !k.is_empty() && *k != me)
                    .and_then(|k| {
                        users.iter().find(|u| u.public_key == k)
                            .map(|u| (u.public_key.clone(), u.name.clone()))
                    })
                    .or_else(|| {
                        users.iter()
                            .find(|u| u.status != "offline" && u.public_key != me && !u.public_key.is_empty())
                            .map(|u| (u.public_key.clone(), u.name.clone()))
                    })
            };

            ui.add_space(6.0);
            // Dev diagnostic, tucked away (operator 2026-07-04: what is this P2P
            // test button?): it hole-punches a direct WebRTC channel to a peer to
            // prove serverless P2P works. Useful when debugging connectivity, not
            // an everyday control -- collapsed by default.
            ui.collapsing(RichText::new("Dev tools").size(theme.font_size_small).color(theme.text_muted()), |ui| {
                if let Some((peer_key, peer_name)) = target {
                    let label = format!("P2P test \u{2192} {}", peer_name);
                    if widgets::Button::secondary(&label).full_width().show(ui, theme) {
                        if let Some(ref webrtc) = state.webrtc {
                            // Arm the one-shot test send for when the channel opens.
                            state.webrtc_test_peer = Some(peer_key.clone());
                            // Offer (honors the offerer rule internally: only the
                            // larger pubkey actually offers; the smaller side waits
                            // for the peer's offer — both presses on both machines
                            // are harmless).
                            webrtc.offer_to(peer_key.clone());
                            crate::debug::push_debug(format!(
                                "WebRTC: P2P test initiated to {}",
                                if peer_key.len() > 12 { &peer_key[..12] } else { &peer_key }
                            ));
                        } else {
                            crate::debug::push_debug("WebRTC: manager not ready (connect to a server first)");
                        }
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("P2P test: no online peer")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                }
            });
        }
    }
}

