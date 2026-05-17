//! Server→Services privilege bridge (v0.262.16).
//!
//! Lets an **admin** start/stop a tiny ALLOWLISTED set of OS daemons
//! that back HumanityOS features (so "click to disable that feature" in
//! Server Settings works without SSHing the VPS), without ever giving
//! the relay arbitrary command execution.
//!
//! ── SECURITY MODEL (read before touching this file) ──────────────────
//!
//! The relay runs as the **non-root** user `humanity`. A repo-versioned
//! sudoers drop-in (`scripts/sudoers.d/humanity-relay-services`,
//! installed by the deploy pipeline) grants that user NOPASSWD for
//! EXACTLY four invocations and nothing else:
//!     /usr/bin/systemctl start  coturn.service
//!     /usr/bin/systemctl stop   coturn.service
//!     /usr/bin/systemctl start  transmission-daemon.service
//!     /usr/bin/systemctl stop   transmission-daemon.service
//!
//! Defence in depth — every layer independently prevents escalation:
//!
//! 1. **Authorization** — the WS handler MUST verify the caller is
//!    `admin`/`owner` BEFORE calling [`control`] (same gate as
//!    `server_settings_update`). [`control`] additionally takes the
//!    caller role and re-checks it (belt and suspenders).
//! 2. **Allowlist** — the ONLY place a client string maps to a unit is
//!    [`resolve`], which does an EXACT-EQUALITY lookup against a
//!    compile-time table. The unit + action handed to `sudo` are ALWAYS
//!    `&'static` constants from that table — NEVER a client-derived
//!    string. Unknown service / bad action ⇒ hard error (fails closed).
//! 3. **No shell** — `std::process::Command` with an argument vector;
//!    `sudo`/`systemctl` are exec'd directly. No `sh -c`, no string
//!    interpolation, so injection is structurally impossible.
//! 4. **`sudo -n`** — non-interactive: if the sudoers rule is absent or
//!    wrong, it fails IMMEDIATELY instead of hanging a relay thread on a
//!    password prompt (fails closed, relay unaffected).
//! 5. **Kernel/sudoers** — even if 1–4 all somehow failed, the box's
//!    sudoers grants only those 4 exact commands; nothing else is
//!    possible for the relay user.
//!
//! Status queries (`is-active`/`is-enabled`) are unprivileged (any user
//! may read systemd state) so they DON'T use sudo.
//!
//! Adding a service = add one row to [`ALLOWLIST`] AND the matching two
//! lines to the sudoers drop-in. Both are in-repo and reviewed together.

use serde::{Deserialize, Serialize};
use std::process::Command;

/// (logical id sent by clients, systemd unit, human label).
///
/// THE trust boundary. `logical id` is matched by EXACT equality only.
/// The `unit` is the only string ever passed to `systemctl` and is a
/// compile-time constant. To extend: add a row here AND two lines to
/// `scripts/sudoers.d/humanity-relay-services`, then security-review.
pub const ALLOWLIST: &[(&str, &str, &str)] = &[
    ("voice", "coturn.service", "Voice/Video relay (coturn)"),
    ("p2p", "transmission-daemon.service", "P2P distribution (transmission)"),
];

/// The only privileged actions. Anything else is rejected before any
/// process is spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
}

impl ServiceAction {
    /// Parse a client-supplied action. ONLY `"start"` / `"stop"`.
    /// Returns the systemctl subcommand as a `&'static str` so the
    /// value handed to `sudo` is a constant, never the input string.
    fn parse(s: &str) -> Option<(ServiceAction, &'static str)> {
        match s {
            "start" => Some((ServiceAction::Start, "start")),
            "stop" => Some((ServiceAction::Stop, "stop")),
            _ => None,
        }
    }
}

/// Live daemon state for the Services UI.
#[derive(Debug, Clone, Copy)]
pub struct DaemonStatus {
    pub active: bool,
    pub enabled: bool,
}

/// One row for the Server Settings → Services panel (sent over WS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Allowlist logical id ("voice" / "p2p").
    pub id: String,
    /// Human label for the panel.
    pub label: String,
    /// App-side SOFT gate — when false the relay stops OFFERING the
    /// feature (instant, no restart). Sourced from server_settings;
    /// the operator flips it through the normal server_settings path.
    pub soft_enabled: bool,
    /// Backing daemon currently running.
    pub daemon_active: bool,
    /// Backing daemon set to start at boot.
    pub daemon_enabled: bool,
}

/// Build the Services snapshot from the allowlist + the soft toggles in
/// `server_settings` + a live (unprivileged) daemon-status probe. The
/// soft-toggle ↔ service mapping lives HERE (one place) so the WS layer
/// and UI never re-derive it.
pub fn snapshot(settings: &crate::relay::storage::ServerSettings) -> Vec<ServiceInfo> {
    ALLOWLIST
        .iter()
        .map(|(id, _unit, label)| {
            let soft_enabled = match *id {
                "voice" => settings.voice_channels_enabled,
                "p2p" => settings.p2p_distribution_enabled,
                _ => false,
            };
            let st = status(id);
            ServiceInfo {
                id: (*id).to_string(),
                label: (*label).to_string(),
                soft_enabled,
                daemon_active: st.map(|s| s.active).unwrap_or(false),
                daemon_enabled: st.map(|s| s.enabled).unwrap_or(false),
            }
        })
        .collect()
}

/// Resolve a client `service` id to its allowlisted entry by EXACT
/// equality. `None` = not allowlisted ⇒ caller must fail closed.
/// Returns `&'static` data — the unit can never be a client string.
pub fn resolve(service: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    ALLOWLIST.iter().find(|(id, _, _)| *id == service)
}

fn is_admin(role: &str) -> bool {
    role == "admin" || role == "owner"
}

/// Start/stop an allowlisted daemon. `caller_role` is the resolved role
/// of the requesting account (the WS handler must have authenticated
/// it). Returns `Ok(human message)` or `Err(reason)` — NEVER panics,
/// NEVER runs anything not in the allowlist, ALWAYS fails closed.
pub fn control(caller_role: &str, service: &str, action: &str) -> Result<String, String> {
    // 1. Authorization (defence in depth — handler also gates).
    if !is_admin(caller_role) {
        return Err("not authorized (admin only)".into());
    }
    // 2. Allowlist — exact match; unit is a compile-time constant.
    let (_, unit, label) = match resolve(service) {
        Some(e) => *e,
        None => return Err(format!("unknown service '{service}' (not allowlisted)")),
    };
    // 3. Action — only start/stop; the subcommand is a constant.
    let (_, sub) = match ServiceAction::parse(action) {
        Some(a) => a,
        None => return Err(format!("invalid action '{action}' (start|stop only)")),
    };
    // 4. Exec WITHOUT a shell. sudo -n: never block on a password —
    //    fail fast if the sudoers rule is missing. Both `sub` and
    //    `unit` are &'static constants resolved above.
    let out = Command::new("sudo")
        .arg("-n")
        .arg("/usr/bin/systemctl")
        .arg(sub)
        .arg(unit)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            tracing::info!(target: "svc_bridge", "{} {} (by {})", sub, unit, caller_role);
            Ok(format!("{label}: {sub} OK"))
        }
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            let err: String = String::from_utf8_lossy(&o.stderr)
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(160)
                .collect();
            tracing::warn!(target: "svc_bridge", "{} {} FAILED rc={} {}", sub, unit, code, err);
            Err(format!("{label}: {sub} failed (rc={code}) {err}"))
        }
        Err(e) => {
            tracing::warn!(target: "svc_bridge", "spawn sudo failed: {e}");
            Err(format!("{label}: cannot run systemctl ({e})"))
        }
    }
}

/// Unprivileged status of an allowlisted daemon. `None` if the service
/// id isn't allowlisted. Never panics; treats any query failure as
/// "not active / not enabled" (fail closed for display).
pub fn status(service: &str) -> Option<DaemonStatus> {
    let (_, unit, _) = *resolve(service)?;
    // `is-active` / `is-enabled` need no privilege. Direct exec, no shell.
    let q = |verb: &str| -> bool {
        Command::new("/usr/bin/systemctl")
            .arg(verb)
            .arg(unit)
            .output()
            .map(|o| {
                // is-active prints "active"; is-enabled prints "enabled".
                let s = String::from_utf8_lossy(&o.stdout);
                let s = s.trim();
                s == "active" || s == "enabled"
            })
            .unwrap_or(false)
    };
    Some(DaemonStatus {
        active: q("is-active"),
        enabled: q("is-enabled"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_resolves_only_exact_known_ids() {
        assert_eq!(resolve("voice").unwrap().1, "coturn.service");
        assert_eq!(resolve("p2p").unwrap().1, "transmission-daemon.service");
    }

    #[test]
    fn allowlist_rejects_everything_else() {
        // Empty, unknown, the unit name itself, injection-y, traversal,
        // case variants — all must be None (fail closed).
        for bad in [
            "",
            " voice",
            "voice ",
            "VOICE",
            "coturn",
            "coturn.service",
            "transmission",
            "sshd",
            "voice;reboot",
            "voice && rm -rf /",
            "../../etc/passwd",
            "voice\n",
            "*",
        ] {
            assert!(resolve(bad).is_none(), "MUST reject {bad:?}");
        }
    }

    #[test]
    fn only_start_stop_actions_parse() {
        assert!(matches!(ServiceAction::parse("start"), Some((ServiceAction::Start, "start"))));
        assert!(matches!(ServiceAction::parse("stop"), Some((ServiceAction::Stop, "stop"))));
        for bad in [
            "restart", "reload", "enable", "disable", "status", "mask",
            "start;reboot", "start ", "Start", "", "kill", "daemon-reexec",
        ] {
            assert!(ServiceAction::parse(bad).is_none(), "MUST reject action {bad:?}");
        }
    }

    #[test]
    fn control_fails_closed_for_non_admin_and_unknown() {
        // Non-admin is rejected BEFORE any allowlist/exec.
        assert!(control("verified", "voice", "start").is_err());
        assert!(control("", "voice", "start").is_err());
        // Admin but unknown service / bad action → still error, no exec
        // of anything (resolve/parse reject before Command).
        assert!(control("admin", "sshd", "start").is_err());
        assert!(control("admin", "voice", "restart").is_err());
        assert!(control("owner", "../x", "stop").is_err());
    }
}
