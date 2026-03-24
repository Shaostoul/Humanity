//! Calculator page — full calculator with expression display, result line,
//! button grid with scientific functions, history panel, keyboard input.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Handle keyboard input
    ctx.input(|i| {
        for event in &i.events {
            match event {
                egui::Event::Text(text) => {
                    for ch in text.chars() {
                        match ch {
                            '0'..='9' | '.' => handle_calc_input(state, &ch.to_string()),
                            '+' | '-' | '*' | '/' => handle_calc_input(state, &ch.to_string()),
                            '(' | ')' => handle_calc_input(state, &ch.to_string()),
                            _ => {}
                        }
                    }
                }
                egui::Event::Key { key, pressed: true, .. } => {
                    match key {
                        egui::Key::Enter => handle_calc_input(state, "="),
                        egui::Key::Backspace => handle_calc_input(state, "BS"),
                        egui::Key::Escape => handle_calc_input(state, "C"),
                        egui::Key::Delete => handle_calc_input(state, "C"),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    });

    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Calculator")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);

            ui.horizontal(|ui| {
                // Left: calculator pad
                ui.vertical(|ui| {
                    ui.set_min_width(320.0);
                    ui.set_max_width(320.0);

                    // Display: expression + result
                    widgets::card(ui, theme, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.label(
                                RichText::new(&state.calc_expression)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.label(
                                RichText::new(&state.calc_display)
                                    .size(32.0)
                                    .color(theme.text_primary()),
                            );
                        });
                    });

                    ui.add_space(theme.spacing_sm);

                    // Scientific row
                    let sci_btns: &[&str] = &["sin", "cos", "tan", "log", "sqrt", "pow", "pi"];
                    ui.horizontal(|ui| {
                        for &label in sci_btns {
                            let btn = egui::Button::new(
                                RichText::new(label).size(theme.font_size_small).color(Theme::c32(&theme.info)),
                            )
                            .fill(theme.bg_card())
                            .min_size(Vec2::new(42.0, 32.0));
                            if ui.add(btn).clicked() {
                                handle_calc_input(state, label);
                            }
                        }
                    });

                    ui.add_space(theme.spacing_xs);

                    // Main button grid: 4 columns
                    let btn_size = Vec2::new(68.0, 48.0);
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
                                let text_color = if is_op {
                                    theme.text_on_accent()
                                } else if is_special {
                                    Color32::WHITE
                                } else {
                                    theme.text_primary()
                                };
                                let fill = if label == "=" {
                                    theme.accent()
                                } else if is_op {
                                    Color32::from_rgb(60, 60, 70)
                                } else if is_special {
                                    theme.danger()
                                } else {
                                    theme.bg_card()
                                };
                                let display_label = if label == "BS" { "<-" } else { label };
                                let btn = egui::Button::new(
                                    RichText::new(display_label)
                                        .size(theme.font_size_heading)
                                        .color(text_color),
                                )
                                .fill(fill)
                                .rounding(Rounding::same(6))
                                .min_size(btn_size);
                                if ui.add(btn).clicked() {
                                    handle_calc_input(state, label);
                                }
                            }
                        });
                    }

                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        RichText::new("Tip: Type numbers and operators with your keyboard")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });

                ui.add_space(theme.spacing_md);

                // Right: history panel
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);

                    widgets::card_with_header(ui, theme, "History", |ui| {
                        if state.calc_history.is_empty() {
                            ui.label(
                                RichText::new("No calculations yet")
                                    .color(theme.text_muted()),
                            );
                        } else {
                            ScrollArea::vertical()
                                .id_salt("calc_history")
                                .max_height(350.0)
                                .show(ui, |ui| {
                                    for entry in state.calc_history.iter().rev() {
                                        let frame = egui::Frame::none()
                                            .fill(Color32::TRANSPARENT)
                                            .rounding(Rounding::same(4))
                                            .inner_margin(4.0);
                                        frame.show(ui, |ui| {
                                            let resp = ui.label(
                                                RichText::new(entry)
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_secondary()),
                                            );
                                            // Click to reuse result
                                            if resp.interact(egui::Sense::click()).clicked() {
                                                if let Some(result) = entry.split('=').last() {
                                                    let result = result.trim();
                                                    state.calc_display = result.to_string();
                                                    state.calc_expression.clear();
                                                }
                                            }
                                            if resp.hovered() {
                                                ui.painter().rect_stroke(
                                                    resp.rect,
                                                    Rounding::same(4),
                                                    Stroke::new(1.0, theme.accent()),
                                                    egui::StrokeKind::Outside,
                                                );
                                            }
                                        });
                                    }
                                });
                        }
                    });

                    if !state.calc_history.is_empty() {
                        ui.add_space(theme.spacing_sm);
                        if widgets::danger_button(ui, theme, "Clear History") {
                            state.calc_history.clear();
                        }
                    }
                });
            });
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
            if state.calc_history.len() > 20 {
                state.calc_history.remove(0);
            }
            state.calc_display = result;
            state.calc_expression.clear();
        }
        "+" | "-" | "*" | "/" => {
            state.calc_expression = format!("{}{} {} ", state.calc_expression, state.calc_display, input);
            state.calc_display = "0".to_string();
        }
        "(" | ")" => {
            state.calc_expression.push_str(input);
        }
        // Scientific functions
        "sin" => {
            if let Ok(val) = state.calc_display.parse::<f64>() {
                state.calc_display = format_result(val.to_radians().sin());
            }
        }
        "cos" => {
            if let Ok(val) = state.calc_display.parse::<f64>() {
                state.calc_display = format_result(val.to_radians().cos());
            }
        }
        "tan" => {
            if let Ok(val) = state.calc_display.parse::<f64>() {
                state.calc_display = format_result(val.to_radians().tan());
            }
        }
        "log" => {
            if let Ok(val) = state.calc_display.parse::<f64>() {
                if val > 0.0 {
                    state.calc_display = format_result(val.log10());
                } else {
                    state.calc_display = "Error".to_string();
                }
            }
        }
        "sqrt" => {
            if let Ok(val) = state.calc_display.parse::<f64>() {
                if val >= 0.0 {
                    state.calc_display = format_result(val.sqrt());
                } else {
                    state.calc_display = "Error".to_string();
                }
            }
        }
        "pow" => {
            // x^2
            if let Ok(val) = state.calc_display.parse::<f64>() {
                state.calc_display = format_result(val * val);
            }
        }
        "pi" => {
            state.calc_display = format_result(std::f64::consts::PI);
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

fn format_result(val: f64) -> String {
    if val.is_nan() || val.is_infinite() {
        return "Error".to_string();
    }
    if val.fract() == 0.0 && val.abs() < 1e15 {
        format!("{}", val as i64)
    } else {
        format!("{:.8}", val)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
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

    format_result(result)
}
