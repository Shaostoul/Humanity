//! Calculator page — basic arithmetic with expression display and history.

use egui::{RichText, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Calculator")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(520.0, 420.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // ── Left: calculator pad ──
                ui.vertical(|ui| {
                    ui.set_min_width(280.0);

                    // Display: expression
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new(&state.calc_expression)
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                        ui.label(
                            RichText::new(&state.calc_display)
                                .size(theme.font_size_title)
                                .color(theme.text_primary()),
                        );
                    });

                    ui.add_space(theme.spacing_sm);

                    // Button grid: 4 columns
                    let btn_size = Vec2::new(60.0, 40.0);
                    let rows: &[&[&str]] = &[
                        &["C", "(", ")", "/"],
                        &["7", "8", "9", "*"],
                        &["4", "5", "6", "-"],
                        &["1", "2", "3", "+"],
                        &["0", ".", "BS", "="],
                    ];

                    for row in rows {
                        ui.horizontal(|ui| {
                            for &label in *row {
                                let is_op = matches!(label, "+" | "-" | "*" | "/" | "=");
                                let is_special = matches!(label, "C" | "BS");
                                let text = if is_op {
                                    RichText::new(label).color(theme.text_on_accent())
                                } else {
                                    RichText::new(label).color(theme.text_primary())
                                };
                                let fill = if is_op {
                                    theme.accent()
                                } else if is_special {
                                    theme.danger()
                                } else {
                                    theme.bg_card()
                                };
                                let btn = egui::Button::new(text).fill(fill).min_size(btn_size);
                                if ui.add(btn).clicked() {
                                    handle_calc_input(state, label);
                                }
                            }
                        });
                    }
                });

                ui.add_space(theme.spacing_md);

                // ── Right: history ──
                ui.vertical(|ui| {
                    ui.set_min_width(200.0);
                    ui.label(
                        RichText::new("History")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_xs);

                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        if state.calc_history.is_empty() {
                            ui.label(
                                RichText::new("No calculations yet")
                                    .color(theme.text_muted()),
                            );
                        }
                        for entry in state.calc_history.iter().rev() {
                            ui.label(
                                RichText::new(entry)
                                    .size(theme.font_size_small)
                                    .color(theme.text_secondary()),
                            );
                        }
                    });

                    if !state.calc_history.is_empty() {
                        ui.add_space(theme.spacing_sm);
                        if widgets::secondary_button(ui, theme, "Clear History") {
                            state.calc_history.clear();
                        }
                    }
                });
            });

            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Close") {
                state.active_page = crate::gui::GuiPage::EscapeMenu;
            }
        });
}

fn handle_calc_input(state: &mut GuiState, input: &str) {
    match input {
        "C" => {
            state.calc_display = "0".to_string();
            state.calc_expression.clear();
        }
        "BS" => {
            if state.calc_display.len() > 1 {
                state.calc_display.pop();
            } else {
                state.calc_display = "0".to_string();
            }
        }
        "=" => {
            let expr = if state.calc_expression.is_empty() {
                state.calc_display.clone()
            } else {
                format!("{}{}", state.calc_expression, state.calc_display)
            };
            let result = evaluate_expression(&expr);
            let history_entry = format!("{} = {}", expr, result);
            state.calc_history.push(history_entry);
            if state.calc_history.len() > 10 {
                state.calc_history.remove(0);
            }
            state.calc_display = result;
            state.calc_expression.clear();
        }
        "+" | "-" | "*" | "/" => {
            state.calc_expression = format!("{}{} {} ", state.calc_expression, state.calc_display, input);
            state.calc_display = "0".to_string();
        }
        _ => {
            // Number or dot
            if state.calc_display == "0" && input != "." {
                state.calc_display = input.to_string();
            } else {
                state.calc_display.push_str(input);
            }
        }
    }
}

/// Simple left-to-right expression evaluator for basic arithmetic.
fn evaluate_expression(expr: &str) -> String {
    let tokens: Vec<&str> = expr.split_whitespace().collect();
    if tokens.is_empty() {
        return "0".to_string();
    }

    let mut result: f64 = match tokens[0].parse() {
        Ok(n) => n,
        Err(_) => return "Error".to_string(),
    };

    let mut i = 1;
    while i + 1 < tokens.len() {
        let op = tokens[i];
        let operand: f64 = match tokens[i + 1].parse() {
            Ok(n) => n,
            Err(_) => return "Error".to_string(),
        };
        result = match op {
            "+" => result + operand,
            "-" => result - operand,
            "*" => result * operand,
            "/" => {
                if operand == 0.0 {
                    return "Error: div/0".to_string();
                }
                result / operand
            }
            _ => return "Error".to_string(),
        };
        i += 2;
    }

    // Format: strip trailing zeros for clean display
    if result.fract() == 0.0 && result.abs() < 1e15 {
        format!("{}", result as i64)
    } else {
        format!("{:.6}", result).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
