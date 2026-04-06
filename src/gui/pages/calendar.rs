//! Calendar page — month view with 7-column grid, event dots, day selection,
//! event list in right panel, and add-event form with time/color/description.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiCalendarEvent, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// Page-local state for additional fields not in GuiState.
struct CalendarPageState {
    new_end_time: String,
    new_description: String,
}

impl Default for CalendarPageState {
    fn default() -> Self {
        Self {
            new_end_time: String::new(),
            new_description: String::new(),
        }
    }
}

thread_local! {
    static LOCAL: RefCell<CalendarPageState> = RefCell::new(CalendarPageState::default());
}

fn with_local<R>(f: impl FnOnce(&mut CalendarPageState) -> R) -> R {
    LOCAL.with(|s| f(&mut s.borrow_mut()))
}

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
                    state.cal_selected_day = 1;
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
                    state.cal_selected_day = 1;
                }
                ui.add_space(theme.spacing_sm);
                if widgets::primary_button(ui, theme, "Today") {
                    let now = current_date();
                    state.cal_year = now.0;
                    state.cal_month = now.1;
                    state.cal_selected_day = now.2;
                }
            });

            ui.add_space(theme.spacing_sm);

            ui.horizontal(|ui| {
                // Left: month grid
                ui.vertical(|ui| {
                    ui.set_min_width(330.0);

                    // Day-of-week headers
                    let days_header = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
                    egui::Grid::new("cal_header")
                        .spacing(Vec2::new(2.0, 2.0))
                        .show(ui, |ui| {
                            for d in &days_header {
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(44.0, 22.0), egui::Sense::hover());
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
                                ui.allocate_exact_size(Vec2::new(44.0, 40.0), egui::Sense::hover());
                            }

                            let mut col = first_weekday;
                            for day in 1..=days_in {
                                let is_today = state.cal_year == today.0
                                    && state.cal_month == today.1
                                    && day == today.2;
                                let is_selected = day == state.cal_selected_day;
                                let event_count = state.cal_events.iter().filter(|e| {
                                    e.year == state.cal_year && e.month == state.cal_month && e.day == day
                                }).count();

                                let (rect, response) = ui.allocate_exact_size(
                                    Vec2::new(44.0, 40.0),
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
                                    ui.painter().rect_stroke(rect, Rounding::same(4), Stroke::new(1.5, theme.accent()), egui::StrokeKind::Outside);
                                }

                                // Day number
                                let text_color = if is_selected {
                                    theme.text_on_accent()
                                } else {
                                    theme.text_primary()
                                };
                                ui.painter().text(
                                    rect.center() - egui::vec2(0.0, 4.0),
                                    egui::Align2::CENTER_CENTER,
                                    day.to_string(),
                                    egui::FontId::proportional(theme.font_size_body),
                                    text_color,
                                );

                                // Event dots (up to 3)
                                if event_count > 0 {
                                    let dot_y = rect.center_bottom().y - 6.0;
                                    let dots = event_count.min(3);
                                    let start_x = rect.center().x - ((dots as f32 - 1.0) * 5.0) / 2.0;
                                    for d in 0..dots {
                                        let dot_color = if is_selected { theme.text_on_accent() } else { theme.accent() };
                                        ui.painter().circle_filled(
                                            egui::pos2(start_x + d as f32 * 5.0, dot_y),
                                            2.0,
                                            dot_color,
                                        );
                                    }
                                }

                                col += 1;
                                if col % 7 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });

                ui.add_space(theme.spacing_md);

                // Right: events for selected day + add form
                ui.vertical(|ui| {
                    ui.set_min_width(280.0);
                    ui.label(
                        RichText::new(format!(
                            "Events - {} {} {}",
                            month_name(state.cal_month),
                            state.cal_selected_day,
                            state.cal_year,
                        ))
                        .size(theme.font_size_heading)
                        .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_sm);

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
                            RichText::new("No events for this day")
                                .color(theme.text_muted()),
                        );
                    } else {
                        ScrollArea::vertical()
                            .id_salt("cal_events_list")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for (_, evt) in &selected_events {
                                    widgets::card(ui, theme, |ui| {
                                        ui.horizontal(|ui| {
                                            let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                                            ui.painter().circle_filled(dot_rect.center(), 5.0, evt.color);
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&evt.title)
                                                        .size(theme.font_size_body)
                                                        .color(theme.text_primary()),
                                                );
                                                ui.label(
                                                    RichText::new(&evt.time)
                                                        .size(theme.font_size_small)
                                                        .color(theme.text_muted()),
                                                );
                                            });
                                        });
                                    });
                                    ui.add_space(2.0);
                                }
                            });
                    }

                    ui.add_space(theme.spacing_md);
                    ui.separator();
                    ui.add_space(theme.spacing_sm);

                    // Add event form
                    ui.label(
                        RichText::new("Add Event")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_xs);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Title:").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::singleline(&mut state.cal_new_title)
                                .desired_width(200.0)
                                .hint_text("Event name"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Start:").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::singleline(&mut state.cal_new_time)
                                .desired_width(80.0)
                                .hint_text("09:00"),
                        );
                        ui.label(RichText::new("End:").color(theme.text_secondary()));
                        with_local(|local| {
                            ui.add(
                                egui::TextEdit::singleline(&mut local.new_end_time)
                                    .desired_width(80.0)
                                    .hint_text("10:00"),
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Color:").color(theme.text_secondary()));
                        let mut c = state.cal_new_color;
                        ui.color_edit_button_srgba(&mut c);
                        state.cal_new_color = c;
                    });
                    with_local(|local| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Description:").color(theme.text_secondary()));
                        });
                        ui.add(
                            egui::TextEdit::multiline(&mut local.new_description)
                                .desired_width(f32::INFINITY)
                                .desired_rows(2)
                                .hint_text("Optional details..."),
                        );
                    });

                    ui.add_space(theme.spacing_sm);
                    if widgets::primary_button(ui, theme, "Add Event") && !state.cal_new_title.trim().is_empty() {
                        let end_time = with_local(|local| local.new_end_time.clone());
                        let time_str = if state.cal_new_time.is_empty() {
                            "All day".to_string()
                        } else if end_time.is_empty() {
                            state.cal_new_time.clone()
                        } else {
                            format!("{} - {}", state.cal_new_time, end_time)
                        };
                        state.cal_events.push(GuiCalendarEvent {
                            title: state.cal_new_title.trim().to_string(),
                            year: state.cal_year,
                            month: state.cal_month,
                            day: state.cal_selected_day,
                            time: time_str,
                            color: state.cal_new_color,
                        });
                        state.cal_new_title.clear();
                        state.cal_new_time.clear();
                        with_local(|local| {
                            local.new_end_time.clear();
                            local.new_description.clear();
                        });
                    }
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
