//! The Dev page's Simulation card (the headless sim runner's increment 2, 2026-11-15;
//! docs/design/headless-sim-runner.md): the in-app twin of `HumanityOS --sim soak` (CLAUDE.md's
//! GUI-first rule: nobody has to open a terminal to run one). The button starts the same command
//! `just sim-soak` runs, this exe with `--sim soak --days 7`, as a process of its own at below
//! normal priority with no console window, and the card shows its report when it finishes.
//!
//! A process of its own, not a thread in the game: the soak steps a fresh homestead whose systems
//! mark the save layer's world hold (save_load `run_world`) every step, and that hold is the
//! running game's, one per process. In its own process the soak cannot touch the person's world,
//! save or GPU; it opens no window and binds no listener (engine/sim_soak.rs), and the machine
//! guard does not count it as a game (scripts/lib/machine-guard.js `isSimRun`).

use std::sync::Mutex;
use std::time::Instant;

use egui::RichText;
use humanity_core::i18n::t;

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, with_hint};
use crate::gui::GuiState;

/// The command line the card runs (after the exe): a week of the homestead at the safe step.
pub(crate) const SOAK_ARGS: &[&str] = &["--sim", "soak", "--days", "7"];

/// A soak the card started, and what came of it.
enum Run {
    Running(Instant),
    Done { report: String, ok: bool, secs: f64 },
}

/// The card's one soak (one at a time: the button hides while it runs).
static RUN: Mutex<Option<Run>> = Mutex::new(None);

fn run_slot() -> std::sync::MutexGuard<'static, Option<Run>> {
    RUN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start the soak: this exe with `SOAK_ARGS`, its output read on a thread of its own.
fn start() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            *run_slot() = Some(Run::Done { report: e.to_string(), ok: false, secs: 0.0 });
            return;
        }
    };
    *run_slot() = Some(Run::Running(Instant::now()));
    std::thread::spawn(move || {
        let t0 = Instant::now();
        let mut cmd = std::process::Command::new(exe);
        cmd.args(SOAK_ARGS).stdin(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS: no console flashes up, and the soak
            // yields the CPU to the game and to whatever the person is doing.
            cmd.creation_flags(0x0800_0000 | 0x0000_4000);
        }
        let done = match cmd.output() {
            Ok(out) => Run::Done {
                report: format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_string(),
                ok: out.status.success(),
                secs: t0.elapsed().as_secs_f64(),
            },
            Err(e) => Run::Done { report: e.to_string(), ok: false, secs: t0.elapsed().as_secs_f64() },
        };
        *run_slot() = Some(done);
    });
}

/// Draw the card.
pub fn draw_card(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("dev.sim.title")).size(theme.font_size_body).strong().color(theme.text_primary()));
        widgets::setting_hint(ui, theme, hint, t("dev.sim.about"));
        let running = match &*run_slot() {
            Some(Run::Running(since)) => Some(since.elapsed().as_secs_f64()),
            _ => None,
        };
        if let Some(secs) = running {
            ui.label(RichText::new(format!("{} {secs:.0} s", t("dev.sim.running"))).size(theme.font_size_small).color(theme.text_secondary()));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        } else if with_hint(ui, "dev.sim.run_hint", |ui| widgets::Button::secondary(t("dev.sim.run")).show(ui, theme)) {
            start();
        }
        widgets::setting_hint(ui, theme, hint, t("dev.sim.run_hint"));
        if let Some(Run::Done { report, ok, secs }) = &*run_slot() {
            let head = if *ok { t("dev.sim.done") } else { t("dev.sim.failed") };
            let color = if *ok { theme.text_secondary() } else { theme.warning() };
            ui.label(RichText::new(format!("{head} ({secs:.1} s)")).size(theme.font_size_small).color(color));
            egui::ScrollArea::both().id_salt("dev_sim_report").max_height(360.0).show(ui, |ui| {
                ui.label(RichText::new(report.as_str()).monospace().size(theme.font_size_small).color(theme.text_primary()));
            });
        }
    });
}
