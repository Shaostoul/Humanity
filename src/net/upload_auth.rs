//! The session's upload token on the desktop app's reads of `/uploads/` (BUG-282, 2026-11-07).
//!
//! A file posted in a members-only room is served only to someone who may read that room
//! (relay/upload_rooms.rs). The desktop app fetches every file itself (ureq, on worker threads), so
//! it proves who it is the way it does for members-only Files items: the session's upload token in
//! the `X-Upload-Token` header, never in an address. The token comes with this person's own entry
//! in the `peer_list` (`gui/pages/chat/attach_send.rs` `note_upload_limit`) and is kept here with
//! the server it is for, so the picture loader, the file card, the video player and the 3D view
//! (which have no `GuiState`) can add it to a read of that server's `/uploads/`, and to no other
//! server's.

use std::sync::Mutex;

/// The header a members-only file read carries the token in (the relay's `UPLOAD_TOKEN_HEADER`).
pub const HEADER: &str = "X-Upload-Token";

/// The server (its address, no trailing `/`) and the token this person was given there.
static HERE: Mutex<Option<(String, String)>> = Mutex::new(None);

/// Keep `token` as this person's upload token on `server`.
pub fn set(server: &str, token: &str) {
    let server = server.trim().trim_end_matches('/').to_string();
    *HERE.lock().unwrap_or_else(|p| p.into_inner()) = Some((server, token.to_string()));
}

/// The token to send with a read of `url`: only for that server's `/uploads/`, and only over the
/// same scheme the token was given on (an `https://` server's token never goes over `http://`).
pub fn token_for(url: &str) -> Option<String> {
    let here = HERE.lock().unwrap_or_else(|p| p.into_inner());
    let (server, token) = here.as_ref()?;
    url.starts_with(&format!("{server}/uploads/")).then(|| token.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_goes_to_its_own_servers_uploads_alone() {
        set("https://united-humanity.us/", "tok");
        assert_eq!(token_for("https://united-humanity.us/uploads/1_ab_plan.png").as_deref(), Some("tok"));
        assert_eq!(token_for("https://united-humanity.us/api/files/1"), None, "not another route");
        assert_eq!(token_for("https://public.guide/uploads/1_ab_plan.png"), None, "not another server");
        assert_eq!(token_for("http://united-humanity.us/uploads/1_ab_plan.png"), None, "not over plain http");
        assert_eq!(token_for("https://united-humanity.us.evil.example/uploads/x"), None, "not a look-alike host");
    }
}
