//! Testing / QA — data-driven checklist of features the developer (Claude)
//! wants the operator to verify. Each task has Mark Passed and Report Issue
//! buttons that post results to the active chat channel as a `[QA]` tagged
//! message — the developer reads them on the next session.
//!
//! Workflow:
//!   1. Operator opens Testing page (top nav).
//!   2. Reads task: feature name, what to test, expected behavior.
//!   3. Tests in-app.
//!   4. Clicks Mark Passed (auto-sends ✅ message) or Report Issue (opens
//!      a textarea, then sends ❌ message with the note).
//!   5. Local status updates so the operator can see at a glance what's done.

use egui::{Frame, RichText, ScrollArea, Stroke, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(1024.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        draw_header(ui, theme, state);
                        ui.add_space(theme.spacing_md);
                        draw_filter_bar(ui, theme, state);
                        ui.add_space(theme.spacing_md);
                        draw_tasks(ui, theme, state);
                    });
                });
            });
        });
}

fn draw_header(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_xl);
    ui.label(
        RichText::new("QA TESTING")
            .size(theme.font_size_small)
            .color(theme.accent())
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new("Verify features Claude shipped")
            .size(theme.font_size_title)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new(
            "Each card describes a feature, what to do, and what to expect. \
             Click Mark Passed if it works, or Report Issue with a note if it's broken. \
             Results are posted to the active chat channel as [QA] tagged messages so Claude \
             can read them next session.",
        )
        .size(theme.font_size_body)
        .color(theme.text_secondary()),
    );

    // Stats
    let total = state.qa_test_tasks.len();
    let passed = state.qa_test_status.values().filter(|v| v.as_str() == "passed").count();
    let issues = state.qa_test_status.values().filter(|v| v.as_str() == "issue").count();
    let pending = total.saturating_sub(passed).saturating_sub(issues);
    ui.add_space(theme.spacing_sm);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} total", total)).color(theme.text_muted()));
        ui.label(RichText::new(format!("· {} passed", passed)).color(theme.success()));
        ui.label(RichText::new(format!("· {} reported", issues)).color(theme.danger()));
        ui.label(RichText::new(format!("· {} pending", pending)).color(theme.text_secondary()));
    });
}

fn draw_filter_bar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let mut categories: Vec<String> = state.qa_test_tasks.iter()
        .map(|t| t.category.clone())
        .filter(|c| !c.is_empty())
        .collect();
    categories.sort();
    categories.dedup();

    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary("All")
            .active(state.qa_test_filter == "all")
            .show(ui, theme)
        {
            state.qa_test_filter = "all".to_string();
        }
        for cat in categories {
            let is_active = state.qa_test_filter == cat;
            if widgets::Button::secondary(&cat).active(is_active).show(ui, theme) {
                state.qa_test_filter = cat;
            }
        }
    });
}

fn draw_tasks(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if state.qa_test_tasks.is_empty() {
        ui.label(
            RichText::new("No QA tasks loaded. Check data/testing/qa_tasks.json.")
                .size(theme.font_size_small)
                .color(theme.text_muted())
                .italics(),
        );
        return;
    }

    let filter = state.qa_test_filter.clone();
    let tasks = state.qa_test_tasks.clone();

    // Snapshot active channel for outgoing chat, profile_public_key, user_name.
    let active_channel = state.chat_active_channel.clone();
    let from_key = state.profile_public_key.clone();
    let from_name = if state.user_name.is_empty() { "QA".to_string() } else { state.user_name.clone() };

    // Pending sends collected during render to avoid double-borrow with state.ws_client.
    let mut pending_sends: Vec<(String, String)> = Vec::new(); // (task_id, message_content)

    for task in &tasks {
        if filter != "all" && task.category != filter { continue; }
        let status = state.qa_test_status.get(&task.id).cloned().unwrap_or_default();

        let border_color = match status.as_str() {
            "passed" => theme.success(),
            "issue"  => theme.danger(),
            _        => theme.border(),
        };

        Frame::none()
            .fill(theme.bg_card())
            .stroke(Stroke::new(1.5, border_color))
            .rounding(egui::Rounding::same(theme.border_radius as u8))
            .inner_margin(theme.card_padding * 1.5)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&task.feature)
                            .size(theme.font_size_body)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !task.version.is_empty() {
                            ui.label(
                                RichText::new(format!("v{}", task.version))
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted())
                                    .monospace(),
                            );
                        }
                        if !task.category.is_empty() {
                            ui.label(
                                RichText::new(&task.category)
                                    .size(theme.font_size_small)
                                    .color(theme.accent()),
                            );
                        }
                        let badge = match status.as_str() {
                            "passed" => Some(("✓ Passed", theme.success())),
                            "issue"  => Some(("⚠ Reported", theme.danger())),
                            _ => None,
                        };
                        if let Some((label, color)) = badge {
                            ui.label(RichText::new(label).color(color).strong());
                        }
                    });
                });
                ui.add_space(theme.spacing_sm);

                ui.label(RichText::new("What to test").size(theme.font_size_small).color(theme.text_muted()));
                ui.label(RichText::new(&task.what_to_test).color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);
                ui.label(RichText::new("Expected").size(theme.font_size_small).color(theme.text_muted()));
                ui.label(RichText::new(&task.expected).color(theme.text_secondary()));
                if let Some(note) = &task.note {
                    ui.add_space(theme.spacing_xs);
                    widgets::alert(ui, theme, widgets::AlertKind::Info, note);
                }

                ui.add_space(theme.spacing_sm);

                // Mark Passed / Report Issue buttons.
                ui.horizontal(|ui| {
                    if widgets::Button::primary("✓ Mark Passed").show(ui, theme) {
                        let body = format!("[QA ✓] {}: passed", task.feature);
                        pending_sends.push((task.id.clone(), body));
                    }
                    ui.add_space(theme.spacing_sm);
                    if widgets::Button::danger("⚠ Report Issue").show(ui, theme) {
                        // Move to "issue" status; the note field below appears next render.
                        state.qa_test_status.insert(task.id.clone(), "issue_pending_note".to_string());
                    }
                });

                // If user clicked Report Issue, show a note field + Send button.
                if state.qa_test_status.get(&task.id).map(|s| s.as_str()) == Some("issue_pending_note")
                    || state.qa_test_status.get(&task.id).map(|s| s.as_str()) == Some("issue")
                {
                    ui.add_space(theme.spacing_sm);
                    // Render the note field — borrow scoped to this row.
                    {
                        let draft = state.qa_test_note.entry(task.id.clone()).or_insert_with(String::new);
                        widgets::form_row(ui, theme, "Issue note", |ui| {
                            ui.add(
                                egui::TextEdit::multiline(draft)
                                    .desired_width(500.0)
                                    .desired_rows(2)
                                    .hint_text("What went wrong?"),
                            );
                        });
                    }

                    // Snapshot the note text so the action closures don't need to re-borrow.
                    let note_text = state.qa_test_note.get(&task.id).cloned().unwrap_or_default();
                    let mut cancel_clicked = false;
                    ui.horizontal(|ui| {
                        if widgets::Button::primary("Send report").show(ui, theme)
                            && !note_text.trim().is_empty()
                        {
                            let body = format!(
                                "[QA ⚠] {}: {}",
                                task.feature,
                                note_text.trim()
                            );
                            pending_sends.push((task.id.clone(), body));
                        }
                        if widgets::Button::secondary("Cancel").show(ui, theme) {
                            cancel_clicked = true;
                        }
                    });
                    if cancel_clicked {
                        state.qa_test_status.remove(&task.id);
                        state.qa_test_note.remove(&task.id);
                    }
                }
            });
        ui.add_space(theme.spacing_md);
    }

    // Apply pending sends after the loop.
    for (task_id, body) in pending_sends {
        let is_issue = body.starts_with("[QA ⚠]");
        // Send chat message to active channel.
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let msg = serde_json::json!({
                    "type": "chat",
                    "from": from_key,
                    "from_name": from_name,
                    "content": body,
                    "timestamp": ts,
                    "channel": active_channel,
                });
                client.send(&msg.to_string());
            }
        }
        // Update local status.
        state.qa_test_status.insert(
            task_id.clone(),
            if is_issue { "issue".to_string() } else { "passed".to_string() },
        );
        if !is_issue {
            state.qa_test_note.remove(&task_id);
        }
    }

    ui.add_space(theme.spacing_xl);
    ui.label(
        RichText::new("Tip: switch to the chat tab to see your reports queued for the developer.")
            .size(theme.font_size_small)
            .color(theme.text_muted())
            .italics(),
    );
    let _ = GuiPage::Chat; // suppress unused-import warning if user navigates away
}
