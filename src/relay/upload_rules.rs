//! The rules every upload is held to, in one place (2026-10-11). Two routes take files: the single
//! request, `POST /api/upload` (api.rs `upload_file`), and the resumable upload, which sends a big
//! Media file in pieces (relay/resumable.rs; docs/design/media-showcase.md, "Resumable uploads,
//! exact"). Both call these functions, so a rule changed here changes for both, and a file one
//! route refuses cannot be slipped in through the other.
//!
//! What is here, in the order a route uses it:
//! - [`token_key`]: the session's upload token, turned into the person's key;
//! - [`uploader_role`]: whether that person's role may upload at all;
//! - [`check_type`]: the file's name and declared type against the blocked and allowed lists;
//! - [`sharing_check`]: the server's picture and file sharing switches, with the role's own;
//! - [`finish_picture`]: a picture's magic bytes checked and its hidden metadata stripped;
//! - [`finish_video`]: where a video was filmed removed from it, in place (BUG-190);
//! - [`FileKind::stored_ext`], [`file_type`] and [`stored_name`]: the name it is kept under and
//!   the kind the answer reports;
//! - [`upload_answer`]: the answer itself, the same JSON from both routes.
//!
//! The size limits are in upload_limits.rs, and the room on the server's disk (the general cap
//! for chat files, the Media budget for Media) in media.rs `upload_room`.

use axum::http::StatusCode;

use crate::relay::relay::RelayState;
use crate::relay::storage::{RoleDef, ServerSettings, Storage};

type Refused = (StatusCode, String);

/// Declared types an upload may have. `application/octet-stream` is the fallback a client sends
/// when it does not know: such a file is judged by its extension instead.
pub(crate) const ALLOWED_TYPES: &[&str] = &[
    "image/png", "image/jpeg", "image/gif", "image/webp",
    "audio/mpeg", "audio/ogg", "audio/wav", "audio/webm", "audio/mp4",
    "video/mp4", "video/webm", "video/ogg",
    "application/pdf", "text/plain", "text/markdown",
    "application/json", "application/zip",
    "application/gzip", "application/x-tar",
    "application/x-gzip", "application/x-compressed-tar",
    "application/octet-stream", // fallback, validated by extension
];

/// Extensions an upload may have (one of these, or one of [`ALLOWED_TYPES`], is enough).
pub(crate) const ALLOWED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp",
    "mp3", "ogg", "wav", "mp4", "webm",
    "pdf", "txt", "md", "json", "zip", "tar.gz", "gz",
    "blend", "stl", "obj", "gltf", "glb",
    // NOTE: "svg" is deliberately NOT allowed. SVG is an active-content
    // format (can carry <script>/event handlers) and our /uploads is
    // served same-origin, so an uploaded SVG opened directly executes in
    // our origin = stored XSS -> localStorage seed theft. 3D models use
    // stl/obj/gltf/glb, so nothing legitimate needs SVG upload. (Audit
    // 2026-06-12; defense-in-depth with the Content-Disposition header on
    // the /uploads service + svg in BLOCKED_EXTENSIONS below.)
];

/// Extensions refused whatever type is declared: programs, scripts, and markup a browser would
/// run if the file were opened straight from this server.
pub(crate) const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "sh", "bat", "cmd", "msi", "dmg", "app", "com", "scr", "pif",
    "html", "htm", "xhtml", "xml", "js", "mjs",
    // Active-content image/markup formats: the same stored-XSS class as the
    // markup types above (audit 2026-06-12).
    "svg", "svgz",
];

/// The person a session's upload token belongs to. Every upload names its session by token
/// (`?token=`): a bare `?key=` proved nothing about who was asking, since anyone can list the
/// connected keys (GET /api/peers), so anyone could upload as anyone connected (2026-10-11).
/// Tokens change on every reconnect; the key they name does not.
pub(crate) async fn token_key(state: &RelayState, token: Option<&str>) -> Result<String, Refused> {
    let Some(token) = token else {
        return Err((StatusCode::BAD_REQUEST, "Missing required 'token' query parameter.".into()));
    };
    if token.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Empty upload token.".into()));
    }
    state
        .upload_tokens
        .read()
        .await
        .get(token)
        .cloned()
        .ok_or_else(|| (StatusCode::FORBIDDEN, "Invalid upload token.".into()))
}

/// The uploader's role, if it may upload: someone with no role at all may not (an admin verifies
/// people), and neither may a role with its "upload" switch off (Server Settings > Roles). The
/// role also carries the person's limits, which the caller reads from what this returns.
pub(crate) fn uploader_role(db: &Storage, key: &str) -> Result<RoleDef, Refused> {
    let role = db.get_role(key).unwrap_or_default();
    if role.is_empty() {
        return Err((StatusCode::FORBIDDEN, "Upload denied: only verified users can upload files. Ask an admin to verify you.".into()));
    }
    let def = db.role_def(&role);
    super::upload_limits::upload_role_check(&def)?;
    Ok(def)
}

/// What the type rules made of a file's name: its extension as given (lower-cased; the whole
/// name when it has no dot), whether it is a `.tar.gz`, and whether the extension is an allowed
/// one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FileKind {
    pub ext: String,
    pub tar_gz: bool,
    pub ext_ok: bool,
}

/// The type rules: a blocked extension is refused; otherwise the declared type or the extension
/// must be an allowed one. An encrypted file (a private conversation's attachment) is opaque
/// ciphertext, stored as an inert `.enc` blob that is never sniffed or run, so neither rule
/// applies to it.
pub(crate) fn check_type(filename: &str, content_type: &str, encrypted: bool) -> Result<FileKind, Refused> {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    let tar_gz = filename.to_lowercase().ends_with(".tar.gz");
    if !encrypted && BLOCKED_EXTENSIONS.contains(&ext.as_str()) {
        return Err((StatusCode::BAD_REQUEST, format!("File type .{} is not allowed.", ext)));
    }
    let type_ok = ALLOWED_TYPES.contains(&content_type);
    let ext_ok = ALLOWED_EXTENSIONS.contains(&ext.as_str()) || tar_gz;
    if !encrypted && !type_ok && !ext_ok {
        return Err((StatusCode::BAD_REQUEST, format!("Unsupported file type: {} (.{})", content_type, ext)));
    }
    Ok(FileKind { ext, tar_gz, ext_ok })
}

/// The sharing switches (v0.261): a picture needs the server's image sharing switch AND the
/// role's own; anything else the file sharing pair. (They were settable but never checked on
/// the server before v0.261, only the chat hid the button.) An encrypted file uses the file
/// pair, since the server cannot tell it is a picture. Checked before a byte of the file is read.
pub(crate) fn sharing_check(s: &ServerSettings, role: &RoleDef, content_type: &str, encrypted: bool) -> Result<(), Refused> {
    let is_img = content_type.starts_with("image/") && !encrypted;
    if is_img {
        if !(s.image_sharing_enabled && role.can_image_share) {
            return Err((
                StatusCode::FORBIDDEN,
                if !s.image_sharing_enabled {
                    "Image sharing is disabled on this server.".into()
                } else {
                    "Your role isn't allowed to share images here. Ask an admin.".into()
                },
            ));
        }
    } else if !(s.file_sharing_enabled && role.can_file_share) {
        return Err((
            StatusCode::FORBIDDEN,
            if !s.file_sharing_enabled {
                "File sharing is disabled on this server.".into()
            } else {
                "Your role isn't allowed to share files here. Ask an admin.".into()
            },
        ));
    }
    Ok(())
}

impl FileKind {
    /// The picture type the file is (upload_limits.rs `picture_type`), or None: never for an
    /// encrypted file.
    pub(crate) fn picture(&self, content_type: &str, encrypted: bool) -> Option<&'static str> {
        if encrypted {
            None
        } else {
            super::upload_limits::picture_type(content_type, &self.ext)
        }
    }

    /// The extension the file is stored under: `enc` for ciphertext, `tar.gz` for a tarball, the
    /// usual one for a known declared type, else its own if that is allowed, else `bin`.
    pub(crate) fn stored_ext(&self, content_type: &str, encrypted: bool) -> String {
        if encrypted {
            // Opaque ciphertext: inert extension, never sniffed/executed.
            return "enc".into();
        }
        if self.tar_gz {
            return "tar.gz".into();
        }
        let known = match content_type {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "audio/mpeg" => "mp3",
            "audio/ogg" => "ogg",
            "audio/wav" => "wav",
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "application/pdf" => "pdf",
            "application/json" => "json",
            "application/zip" => "zip",
            "text/plain" => "txt",
            "text/markdown" => "md",
            _ => return if self.ext_ok { self.ext.clone() } else { "bin".into() },
        };
        known.into()
    }
}

/// The kind the upload's answer reports (`type`): what the apps show it as.
pub(crate) fn file_type(content_type: &str, stored_ext: &str, encrypted: bool) -> &'static str {
    let ext = stored_ext;
    if encrypted {
        "encrypted"
    } else if content_type.starts_with("image/") {
        "image"
    } else if content_type.starts_with("audio/") || ["mp3", "ogg", "wav"].contains(&ext) {
        "audio"
    } else if content_type.starts_with("video/") || ["mp4", "webm"].contains(&ext) {
        "video"
    } else if ["blend", "stl", "obj", "gltf", "glb"].contains(&ext) {
        "3d_model"
    } else if ["zip", "tar.gz", "gz"].contains(&ext) {
        "archive"
    } else {
        "document"
    }
}

/// The name a file is stored under: `<ms>_<safe name>.<ext>`, where the safe name is the first
/// 32 letters, digits, `-` and `_` of the name it came with (`file` when none are left), and
/// `<ms>` the time now in milliseconds.
pub(crate) fn stored_name(filename: &str, stored_ext: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let safe_name: String = filename
        .chars()
        .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();
    format!("{}_{}.{}", ts, if safe_name.is_empty() { "file" } else { &safe_name }, stored_ext)
}

/// A picture's last step before it is stored, given its bytes and its type (`picture`): the
/// first bytes must be that type's own (a file named `.png` that is not a PNG is refused), then
/// its embedded metadata is stripped. A phone photo carries GPS coordinates and a camera serial;
/// nobody consents to publishing their home address with a garden picture. The strip is
/// lossless (whole-segment removal, no re-encode) and fails open to the original bytes on any
/// parse trouble (relay/core/strip_metadata.rs).
///
/// BLOCKING: it reads and copies the whole picture (up to `PICTURE_MAX_BYTES`), so a route calls
/// it inside `spawn_blocking`, never on the async executor.
pub(crate) fn finish_picture(picture: &str, data: Vec<u8>) -> Result<Vec<u8>, Refused> {
    let magic_ok = match picture {
        "image/png" => data.len() >= 4 && &data[..4] == b"\x89PNG",
        "image/jpeg" => data.len() >= 3 && &data[..3] == b"\xFF\xD8\xFF",
        "image/gif" => data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a"),
        "image/webp" => data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
        _ => false,
    };
    if !magic_ok {
        return Err((StatusCode::BAD_REQUEST, "File content does not match claimed image type.".to_string()));
    }
    let original = data.len();
    let stripped = crate::relay::core::strip_metadata::strip_image_metadata(picture, &data);
    if stripped.len() != original {
        tracing::info!(
            "Upload metadata strip: {} shed {} bytes of embedded metadata",
            picture,
            original.saturating_sub(stripped.len())
        );
    }
    Ok(stripped)
}

/// What is said when a video's boxes could not be read all the way through, so the place where
/// it was filmed could not be removed from it with certainty.
pub(crate) const VIDEO_UNREADABLE: &str = "This video could not be read all the way through, so where it was filmed could not be removed from it, and it was not stored. Save it again from your phone or video editor and send that copy.";

/// A video's hidden metadata removed before it is stored (BUG-190, 2026-10-11): phone cameras
/// write where a recording was made into an MP4's metadata (Apple's location key, the `©xyz`
/// atom, the 3GPP `loci` box, XMP), so a phone video published as it arrived told everyone where
/// it was filmed. The strip (relay/core/strip_video_metadata.rs) turns those boxes into zeroed
/// `free` boxes of the same size, in place: no re-encode, nothing moves, so the video plays as
/// before and a file of many GB costs only a walk over its box headers.
///
/// Every upload that is not encrypted comes here (a picture goes to [`finish_picture`]); a file
/// whose first bytes are not an MP4-family file is only looked at, never opened for writing.
/// Refused, and not stored, when the walk had to stop early on a box size that made no sense:
/// the boxes behind it could still hold the location, so storing it would publish a file we
/// could not clean (safe by default; a damaged file is rarely a playable one). Not caught: a
/// location recorded inside the video stream itself (GoPro, DJI, some dashcams), which only a
/// re-encode or dropping that track removes; see the module's notes.
///
/// BLOCKING (file reads and writes), so a route calls it in `spawn_blocking`.
pub(crate) fn finish_video(path: &std::path::Path) -> Result<(), Refused> {
    let report = crate::relay::core::strip_video_metadata::strip_video_metadata_in_place(path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to check the video's hidden information: {e}")))?;
    if !report.stopped_early.is_empty() {
        tracing::info!("Upload video strip: refused a file whose boxes stop early ({})", report.stopped_early.join("; "));
        return Err((StatusCode::UNPROCESSABLE_ENTITY, VIDEO_UNREADABLE.to_string()));
    }
    if report.changed() {
        tracing::info!("Upload video strip: cleared {} ({} bytes)", report.paths().join(", "), report.bytes_zeroed);
    }
    Ok(())
}

/// The answer to a stored upload, from either route: `{url, filename, size, type}`. The apps
/// publish a Media upload with `POST /api/media` from it, whichever way the file came.
pub(crate) fn upload_answer(stored_name: &str, size: u64, file_type: &str) -> serde_json::Value {
    serde_json::json!({ "url": format!("/uploads/{stored_name}"), "filename": stored_name, "size": size, "type": file_type })
}
