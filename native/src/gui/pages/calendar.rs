//! Calendar page — month view with events list and add-event form.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiCalendarEvent, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            // Navigation header
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, theme, "<") {
                    if state.cal_month == 1 {
                        state.cal_month = 12;
                        state.cal_year -= 1;
                    } else {
                        state.cal_month -= 1;
                    }
                }
                let month_name = month_name(state.cal_month);
                ui.label(
                    RichText::new(format!("{} {}", month_name, state.cal_year))
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                if widgets::secondary_button(ui, theme, ">") {
                    if state.cal_month == 12 {
                        state.cal_month = 1;
                        state.cal_year += 1;
                    } else {
                        state.cal_month += 1;
                    }
                }
                if widgets::primary_button(ui, theme, "Today") {
                    let now = current_date();
                    state.cal_year = now.0;
                    state.cal_month = now.1;
                    state.cal_selected_day = now.2;
                }
            });

            ui.add_space(theme.spacing_sm);

            ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: month grid
                    ui.vertical(|ui| {
                        ui.set_min_width(320.0);

                        // Day-of-week headers
                        let days_header = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
                        egui::Grid::new("cal_header")
                            .spacing(Vec2::new(2.0, 2.0))
                            .show(ui, |ui| {
                                for d in &days_header {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::new(42.0, 20.0), egui::Sense::hover());
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        *d,
                                        egui::FontId::proportional(theme.font_size_small),
                                        theme.text_muted(),
                                    );
                                }
                                ui.end_row();
                            });

                        // Calendar day cells
                        let first_weekday = day_of_week(state.cal_year, state.cal_month, 1);
                        let days_in = days_in_month(state.cal_year, state.cal_month);
                        let today = current_date();

                        egui::Grid::new("cal_grid")
                            .spacing(Vec2::new(2.0, 2.0))
                            .show(ui, |ui| {
                                // Blank cells before first day
                                for _ in 0..first_weekday {
                                    ui.allocate_exact_size(Vec2::new(42.0, 36.0), egui::Sense::hover());
                                }

                                let mut col = first_weekday;
                                for day in 1..=days_in {
                                    let is_today = state.cal_year == today.0
                                        && state.cal_month == today.1
                                        && day == today.2;
                                    let is_selected = day == state.cal_selected_day;
                                    let has_events = state.cal_events.iter().any(|e| {
                                        e.year == state.cal_year && e.month == state.cal_month && e.day == day
                                    });

                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::new(42.0, 36.0),
                                        egui::Sense::click(),
                                    );

                                    if response.clicked() {
                                        state.cal_selected_day = day;
                                    }

                                    // Background
                                    let fill = if is_selected {
                                        theme.accent()
                                    } else if is_today {
                                        theme.bg_card()
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    ui.painter().rect_filled(rect, Rounding::same(4), fill);
                                    if is_today && !is_selected {
                                        ui.painter().rect_stroke(rect, Rounding::same(4), Stroke::new(1.0, theme.accent()), egui::StrokeKind::Outside);
                                    }

                                    // Day number
                                    let text_color = if is_selected {
                                        theme.text_on_accent()
                                    } else {
                                        theme.text_primary()
                                    };
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        day.to_string(),
                                        egui::FontId::proportional(theme.font_size_body),
                                        text_color,
                                    );

                                    // Event dot
                                    if has_events {
                                        let dot_pos = rect.center_bottom() + egui::vec2(0.0, -4.0);
                                        ui.painter().circle_filled(dot_pos, 2.5, theme.accent());
                                    }

                                    col += 1;
                                    if col % 7 == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });

                    ui.add_space(theme.spacing_md);

                    // Right: events for selected day
                    ui.vertical(|ui| {
                        ui.set_min_width(240.0);
                        ui.label(
                            RichText::new(format!(
                                "Events - {} {}",
                                month_name(state.cal_month),
                                state.cal_selected_day
                            ))
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                        );
                        ui.add_space(theme.spacing_xs);

                        let selected_events: Vec<_> = state
                            .cal_events
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| {
                                e.year == state.cal_year
                                    && e.month == state.cal_month
                                    && e.day == state.cal_selected_day
                            })
                            .map(|(i, e)| (i, e.clone()))
                            .collect();

                        if selected_events.is_empty() {
                            ui.label(
                                RichText::new("No events")
                                    .color(theme.text_muted()),
                            );
                        } else {
                            ScrollArea::vertical().id_salt("cal_events_list").max_height(150.0).show(ui, |ui| {
                                for (_, evt) in &selected_events {
                                    ui.horizontal(|ui| {
                                        let dot_rect = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover()).0;
                                        ui.painter().circle_filled(dot_rect.center(), 4.0, evt.color);
                                        ui.label(RichText::new(&evt.time).size(theme.font_size_small).color(theme.text_muted()));
                                        ui.label(RichText::new(&evt.title).color(theme.text_primary()));
                                    });
                                }
                            });
                        }

                        ui.add_space(theme.spacing_md);
                        ui.separator();
                        ui.add_space(theme.spacing_sm);

                        // Add event form
                        ui.label(RichText::new("Add Event").color(theme.text_secondary()));
                        ui.add_space(theme.spacing_xs);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Title:").color(theme.text_muted()));
                            ui.text_edit_singleline(&mut state.cal_new_title);
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Time:").color(theme.text_muted()));
                            ui.text_edit_singleline(&mut state.cal_new_time);
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Color:").color(theme.text_muted()));
                            let mut c = state.cal_new_color;
                            ui.color_edit_button_srgba(&mut c);
                            state.cal_new_color = c;
                        });

                        ui.add_space(theme.spacing_xs);
                        if widgets::primary_button(ui, theme, "Add") && !state.cal_new_title.trim().is_empty() {
                            state.cal_events.push(GuiCalendarEvent {
                                title: state.cal_new_title.trim().to_string(),
                                year: state.cal_year,
                                month: state.cal_month,
                                day: state.cal_selected_day,
                                time: if state.cal_new_time.is_empty() {
                                    "All day".to_string()
                                } else {
                                    state.cal_new_time.clone()
                                },
                                color: state.cal_new_color,
                            });
                            state.cal_new_title.clear();
                            state.cal_new_time.clear();
                        }
                    });
                });
            });
        });
}

// Date helpers

fn current_date() -> (i32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs / 86400;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(y) { 29 } else { 28 },
        _ => 30,
    }
}

fn day_of_week(y: i32, m: u32, d: u32) -> u32 {
    let t = [0i32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i32) % 7).unsigned_abs()
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}
