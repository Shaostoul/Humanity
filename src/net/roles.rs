//! Who counts as an admin or a moderator on a server: the one place the desktop app reads a
//! person's role for a gate (BUG-332's sweep, 2026-11-20).
//!
//! The server's owner (role `owner`) counts as an admin everywhere, and a moderator is spelled
//! `mod` or `moderator` (the relay accepts both). A page that shows or hides an admin's or a
//! moderator's control asks these, never `role == "admin"` on its own: that copy of the rule hid
//! the owner's controls while the relay allowed them. The relay's twins are `is_admin_role` and
//! `is_staff_role` (src/relay/handlers/msg_handlers.rs); the website's are web/shared/roles.js.
//! The relay checks every action again; these only decide what the app offers.
//!
//! A role's own NAME (a badge's letter, the role a menu sets, a server's limits for a role) is not
//! a gate and stays a plain string. Read the way the website reads a role: trimmed, any case.

/// Whether `role` is an admin's or the owner's.
pub fn is_admin_role(role: &str) -> bool {
    let r = role.trim();
    r.eq_ignore_ascii_case("admin") || r.eq_ignore_ascii_case("owner")
}

/// Whether `role` is the server owner's itself (the Ownership panel, which only the owner sees;
/// handlers/owner.rs on the relay). Written from `is_admin_role`, admin or owner, less admin, so
/// the rule's names stay in the one place role_gate_lint knows.
pub fn is_owner_role(role: &str) -> bool {
    is_admin_role(role) && !role.trim().eq_ignore_ascii_case("admin")
}

/// Whether `role` is an admin's, the owner's or a moderator's (both spellings).
pub fn is_staff_role(role: &str) -> bool {
    let r = role.trim();
    is_admin_role(r) || r.eq_ignore_ascii_case("mod") || r.eq_ignore_ascii_case("moderator")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seen red 2026-11-20 with `is_admin_role` reading "admin" alone (the gates' old rule):
    /// "the owner is an admin: \"owner\"".
    #[test]
    fn the_owner_is_an_admin_and_staff_a_moderator_staff_anyone_else_neither() {
        for r in ["admin", "owner", " Owner ", "ADMIN"] {
            assert!(is_admin_role(r), "the owner is an admin: {r:?}");
            assert!(is_staff_role(r), "staff: {r:?}");
        }
        for r in ["mod", "moderator", "Moderator"] {
            assert!(!is_admin_role(r), "not an admin: {r:?}");
            assert!(is_staff_role(r), "staff: {r:?}");
        }
        for r in ["", "member", "verified", "donor", "muted", "shipwright", "administrator", "owners"] {
            assert!(!is_admin_role(r) && !is_staff_role(r), "neither: {r:?}");
        }
        assert!(is_owner_role("owner") && is_owner_role(" Owner "), "the owner is the owner");
        for r in ["admin", "mod", "", "owners"] {
            assert!(!is_owner_role(r), "not the owner: {r:?}");
        }
    }
}
