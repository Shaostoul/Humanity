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
//! - [`finish_picture_as`]: a picture's magic bytes checked and its hidden metadata stripped,
//!   unless the person chose to keep it (B4);
//! - [`finish_file_as`]: where a video was filmed removed from it, in place (BUG-190), and a
//!   HEIC or AVIF photo's Exif and XMP zeroed in place (2026-10-11; refused only when it cannot
//!   be read through, R7), unless the person chose to keep everything (B4);
//! - [`FileLimit`], [`allowance_at_start`], [`allowance_for_piece`] and [`count_upload`]: the
//!   size limits (the role's, a private file's, a Media file's) and the day's and the hour's
//!   allowances (B1 to B3, 2026-10-11);
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
    // HEIC and AVIF photos: stored as plain files, their Exif and XMP zeroed (finish_file_as).
    "image/heic", "image/heif", "image/avif",
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
    "heic", "heif", "avif",
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
    // `application/octet-stream` is what a client sends when it does not know: such a file is
    // judged by its extension alone (R7, 2026-10-11). It used to count as an allowed type on its
    // own, so an iPhone's .heic photo sent that way passed with any extension and skipped every
    // strip, keeping where it was taken.
    let type_ok = ALLOWED_TYPES.contains(&content_type) && content_type != "application/octet-stream";
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

/// The declared types of a HEIC or AVIF photo.
const HEIF_TYPES: &[&str] = &["image/heic", "image/heif", "image/avif"];

/// The extensions of a HEIC or AVIF photo.
const HEIF_EXTENSIONS: &[&str] = &["heic", "heif", "avif"];

impl FileKind {
    /// The picture type the file is (upload_limits.rs `picture_type`), or None: never for an
    /// encrypted file, and never for a HEIC or AVIF photo, which is kept as a plain file (the
    /// apps do not show those inline) and cleaned by [`finish_file_as`] instead.
    pub(crate) fn picture(&self, content_type: &str, encrypted: bool) -> Option<&'static str> {
        if encrypted || self.heif_photo(content_type) {
            None
        } else {
            super::upload_limits::picture_type(content_type, &self.ext)
        }
    }

    /// True when the upload says it is a HEIC or AVIF photo: by its declared type, or by its
    /// extension when the declared type says nothing else (`application/octet-stream`, or a
    /// type not on the allowed list). A `.heic` declared `image/jpeg` is judged as a JPEG, and
    /// refused when its first bytes are not a JPEG's.
    pub(crate) fn heif_photo(&self, content_type: &str) -> bool {
        if HEIF_TYPES.contains(&content_type) {
            return true;
        }
        let declared_other = ALLOWED_TYPES.contains(&content_type) && content_type != "application/octet-stream";
        !declared_other && HEIF_EXTENSIONS.contains(&self.ext.as_str())
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
            "image/heic" => "heic",
            "image/heif" => "heif",
            "image/avif" => "avif",
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

/// The kind the upload's answer reports (`type`): what the apps show it as. A HEIC or AVIF
/// photo is a "document", a plain file to download, like any other file the apps do not show
/// inline (both apps show PNG, JPEG, GIF and WebP inline, nothing else), whatever image type it
/// came with.
pub(crate) fn file_type(content_type: &str, stored_ext: &str, encrypted: bool) -> &'static str {
    let ext = stored_ext;
    if encrypted {
        "encrypted"
    } else if HEIF_EXTENSIONS.contains(&ext) {
        "document"
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
    picture_is_what_it_says(picture, &data)?;
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

/// A picture's first bytes must be its type's own: a file named `.png` that is not a PNG is
/// refused. Checked whether or not the person keeps the picture's hidden information (B4).
pub(crate) fn picture_is_what_it_says(picture: &str, data: &[u8]) -> Result<(), Refused> {
    let magic_ok = match picture {
        "image/png" => data.len() >= 4 && &data[..4] == b"\x89PNG",
        "image/jpeg" => data.len() >= 3 && &data[..3] == b"\xFF\xD8\xFF",
        "image/gif" => data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a"),
        "image/webp" => data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
        _ => false,
    };
    if magic_ok {
        Ok(())
    } else {
        Err((StatusCode::BAD_REQUEST, "File content does not match claimed image type.".to_string()))
    }
}

/// What is said when a video's boxes could not be read all the way through, so the place where
/// it was filmed could not be removed from it with certainty.
pub(crate) const VIDEO_UNREADABLE: &str = "This video could not be read all the way through, so where it was filmed could not be removed from it, and it was not stored. Save it again from your phone or video editor and send that copy.";

/// The answer to a stored upload, from either route: `{url, filename, size, type}`. The apps
/// publish a Media upload with `POST /api/media` from it, whichever way the file came. For a Media
/// upload the `url` is where the file will be once it is published as public: until then it
/// waits in a folder nothing serves (R6).
pub(crate) fn upload_answer(stored_name: &str, size: u64, file_type: &str) -> serde_json::Value {
    serde_json::json!({ "url": format!("/uploads/{stored_name}"), "filename": stored_name, "size": size, "type": file_type })
}

// ── The hidden information a person chose to keep (B4, 2026-10-11) ──

/// What is said for a HEIC or AVIF photo whose item tables could not be read all the way through
/// (R7, 2026-10-11), so its Exif and XMP could not be cleaned with certainty. A well-formed one is
/// cleaned in place and stored (relay/core/strip_heif_metadata.rs). The words predate that strip
/// and are kept as they were: for the photos still refused, they say what to do.
pub(crate) const HEIF_REFUSED: &str = "HEIC and AVIF photos cannot be cleaned of where they were taken yet. Save it as a JPEG and send that.";

/// A picture's last step, honouring the person's choice to keep its hidden information
/// (`keep_metadata`, B4): the check that it is what it says always runs; the strip only when they
/// did not choose to keep it. BLOCKING, like [`finish_picture`].
pub(crate) fn finish_picture_as(picture: &str, data: Vec<u8>, keep_metadata: bool) -> Result<Vec<u8>, Refused> {
    if keep_metadata {
        picture_is_what_it_says(picture, &data)?;
        Ok(data)
    } else {
        finish_picture(picture, data)
    }
}

/// Anything that is neither a picture nor encrypted, before it is stored: a video's hidden
/// metadata removed (BUG-190, 2026-10-11), and a HEIC or AVIF photo's too (2026-10-11).
///
/// Phone cameras write where a recording was made into an MP4's metadata (Apple's location key,
/// the `©xyz` atom, the 3GPP `loci` box, XMP), so a phone video published as it arrived told
/// everyone where it was filmed. The strip (relay/core/strip_video_metadata.rs) turns those boxes
/// into zeroed `free` boxes of the same size, in place: no re-encode, nothing moves, so the video
/// plays as before and a file of many GB costs only a walk over its box headers. A file whose
/// first bytes are not an MP4-family file is only looked at, never opened for writing.
///
/// A HEIC or AVIF photo is in the same box family with no movie in it: its location is in an
/// `Exif` item (and sometimes an XMP one) inside the top-level `meta` box, beside the picture's
/// own items. The video strip clears a top-level XMP box in it and leaves `meta` alone; then the
/// HEIF strip (relay/core/strip_heif_metadata.rs) overwrites the Exif and XMP items' bytes with
/// zeros where they stand, in place, the file's size unchanged and every other item untouched.
///
/// Refused, and not stored: a video whose walk had to stop early on a box size that made no sense
/// (422 [`VIDEO_UNREADABLE`]: the boxes behind it could still hold the location; a damaged file
/// is rarely a playable one), and so is a file in the MP4 family with no movie and no HEIF brand,
/// a video cut off before its movie box; a HEIC or AVIF photo whose item tables could not be read
/// all the way through, or one of whose Exif or XMP items could not be found with certainty (422
/// [`HEIF_REFUSED`]). A read or write failure is a 500, which a resumable upload keeps its session
/// for (R3). Not caught: a location recorded inside the video stream itself (GoPro, DJI, some
/// dashcams), which only a re-encode or dropping that track removes.
///
/// With `keep_metadata` (B4) none of this runs and the file is taken as it is: the person chose
/// to keep everything, so there is nothing to check, a file the strips could not read through
/// included. BLOCKING (file reads and writes), so a route calls it in `spawn_blocking`.
pub(crate) fn finish_file_as(path: &std::path::Path, keep_metadata: bool) -> Result<(), Refused> {
    use crate::relay::core::{strip_heif_metadata as heif, strip_video_metadata as video};
    if keep_metadata {
        return Ok(());
    }
    let failed = |e: std::io::Error| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to check the file's hidden information: {e}"));
    let report = video::strip_video_metadata_in_place(path).map_err(failed)?;
    if report.kind == video::VideoKind::Mp4Family && !report.found_moov {
        // No movie in it: a HEIC or AVIF photo, or a video cut off before its movie box.
        let still = heif::strip_heif_metadata_in_place(path).map_err(failed)?;
        if still.kind == heif::HeifKind::Still {
            if !still.complete() || !report.stopped_early.is_empty() {
                let why = [report.stopped_early.as_slice(), still.stopped_early.as_slice()].concat();
                tracing::info!("Upload HEIF strip: refused a photo it could not read through ({})", why.join("; "));
                return Err((StatusCode::UNPROCESSABLE_ENTITY, HEIF_REFUSED.to_string()));
            }
            if report.changed() || still.changed() {
                let items: Vec<String> = still.items_zeroed.iter().map(|(id, kind, n)| format!("item {id} ({kind}, {n} bytes)")).collect();
                tracing::info!("Upload HEIF strip: cleared {}", [report.paths().join(", "), items.join(", ")].join(" ").trim());
            }
            return Ok(());
        }
        // Not a HEIF photo: a video whose movie box never arrived. It would not play, and where
        // it was filmed cannot be ruled out, since the movie box is where that is kept.
        tracing::info!("Upload video strip: refused a video with no movie box ({})", report.stopped_early.join("; "));
        return Err((StatusCode::UNPROCESSABLE_ENTITY, VIDEO_UNREADABLE.to_string()));
    }
    if !report.stopped_early.is_empty() {
        tracing::info!("Upload video strip: refused a file whose boxes stop early ({})", report.stopped_early.join("; "));
        return Err((StatusCode::UNPROCESSABLE_ENTITY, VIDEO_UNREADABLE.to_string()));
    }
    if report.changed() {
        tracing::info!("Upload video strip: cleared {} ({} bytes)", report.paths().join(", "), report.bytes_zeroed);
    }
    Ok(())
}

// ── The size limits a file is held to (B1, B2, B3; 2026-10-11) ──

const MIB: u64 = 1024 * 1024;

/// What the encryption of a private file adds to it (AES-GCM's 16-byte tag, with room to spare):
/// allowed on top of the role's private limit, so a file of exactly that size still goes.
pub(crate) const PRIVATE_ENVELOPE_SLACK: u64 = 4 * 1024;

/// The role's private file limit in MB, as the peer list says it (`private_limit_mb`): its
/// `max_private_file_mb`, at least 1, at most one request's 1 GB ceiling.
pub(crate) fn private_limit_mb(role: &RoleDef) -> u64 {
    (role.max_private_file_mb.max(1) as u64).min(super::upload_limits::UPLOAD_HARD_CEILING / MIB)
}

/// The most the role's holder may keep in files from private conversations at once, in bytes, or
/// None for no limit (0, Admin's default; B5).
pub(crate) fn private_held_limit(role: &RoleDef) -> Option<u64> {
    (role.max_private_held_mb > 0).then(|| role.max_private_held_mb as u64 * MIB)
}

/// The role's largest Media file in bytes, or None when it has none (0: up to the person's Media
/// share, B1).
pub(crate) fn media_file_limit(role: &RoleDef) -> Option<u64> {
    (role.max_media_file_mb > 0).then(|| role.max_media_file_mb as u64 * MIB)
}

/// The refusal for a Media file over the role's own limit (B1).
pub(crate) fn media_file_too_large(role: &RoleDef) -> Refused {
    (StatusCode::PAYLOAD_TOO_LARGE, format!("Media files are limited to {} MB for your role on this server.", role.max_media_file_mb))
}

/// The most one file may be in a single request, and what is said when it is over.
#[derive(Debug, Clone)]
pub(crate) struct FileLimit {
    pub bytes: u64,
    /// The words for this limit; None for the role's own `max_upload_mb`, whose words name the
    /// file's size too (upload_limits.rs `upload_too_large`).
    refusal: Option<Refused>,
}

impl FileLimit {
    /// The limit for a single request by `role` to `shelf`: an encrypted upload is held to the
    /// role's private limit instead of `max_upload_mb` (B2), a Media upload to `max_upload_mb`
    /// and, when the role has one, its Media file limit (B1); always within the 1 GB ceiling.
    pub(crate) fn for_request(role: &RoleDef, shelf: i64, encrypted: bool) -> FileLimit {
        if encrypted {
            let mb = private_limit_mb(role);
            return FileLimit {
                bytes: (mb * MIB + PRIVATE_ENVELOPE_SLACK).min(super::upload_limits::UPLOAD_HARD_CEILING),
                refusal: Some((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!("Files in private conversations are limited to {mb} MB for your role on this server."),
                )),
            };
        }
        let general = FileLimit { bytes: super::upload_limits::role_upload_limit(role), refusal: None };
        match media_file_limit(role) {
            Some(bytes) if shelf == crate::relay::storage::media::MEDIA_SHELF => {
                general.tighter(FileLimit { bytes, refusal: Some(media_file_too_large(role)) })
            }
            _ => general,
        }
    }

    /// The smaller of the two limits, with its own words.
    pub(crate) fn tighter(self, other: FileLimit) -> FileLimit {
        if other.bytes < self.bytes {
            other
        } else {
            self
        }
    }

    /// What is said for a file of `len` bytes that is over this limit.
    pub(crate) fn too_large(&self, len: u64) -> Refused {
        self.refusal.clone().unwrap_or_else(|| super::upload_limits::upload_too_large(len, self.bytes))
    }

    /// A request whose declared size is already over this limit (with the form's own bytes
    /// allowed on top): refused before a byte of it is read.
    pub(crate) fn declared_too_large(&self, headers: &axum::http::HeaderMap) -> Option<Refused> {
        super::upload_limits::declared_too_large(headers, self.bytes).map(|role_words| self.refusal.clone().unwrap_or(role_words))
    }

    /// The streaming refusal (upload_limits.rs `stream_to_part`, which knows only the role's
    /// words) said in this limit's words; any other refusal as it is.
    pub(crate) fn reword(&self, e: Refused) -> Refused {
        match &self.refusal {
            Some(own) if e.0 == StatusCode::PAYLOAD_TOO_LARGE => own.clone(),
            _ => e,
        }
    }
}

// ── The day's and the hour's allowance (B3, 2026-10-11) ──

/// "in 5 hours 12 minutes", "in 12 minutes", "in less than a minute": how long until `then_ms`.
pub(crate) fn in_words(now_ms: i64, then_ms: i64) -> String {
    let minutes = ((then_ms - now_ms).max(0) + 59_999) / 60_000;
    let (h, m) = (minutes / 60, minutes % 60);
    let plural = |n: i64, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
    match (h, m) {
        (0, 0) => "in less than a minute".to_string(),
        (0, m) => format!("in {}", plural(m, "minute")),
        (h, 0) => format!("in {}", plural(h, "hour")),
        (h, m) => format!("in {} {}", plural(h, "hour"), plural(m, "minute")),
    }
}

/// An upload refused for the day's or the hour's allowance: the words, which allowance, and when
/// it resets (Unix ms), so an app can wait and carry on (a resumable upload answers these in JSON).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OverAllowance {
    pub words: String,
    pub span: crate::relay::storage::upload_counts::Span,
    pub resets_at: i64,
}

impl OverAllowance {
    /// As every other upload refusal: 429 and the words.
    pub(crate) fn refused(&self) -> Refused {
        (StatusCode::TOO_MANY_REQUESTS, self.words.clone())
    }

    /// As JSON: `{error, allowance: "day"|"hour", resets_at}`.
    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::json!({ "error": self.words, "allowance": self.span.word(), "resets_at": self.resets_at })
    }
}

/// What a role allows a day in bytes, None for no limit (0).
fn day_allowance(role: &RoleDef) -> Option<u64> {
    (role.max_upload_mb_per_day > 0).then(|| role.max_upload_mb_per_day as u64 * MIB)
}

/// The words for an upload past the day's allowance: the limit, what was used, and when it resets.
fn over_the_day(role: &RoleDef, used: i64, now_ms: i64, adding: &str) -> OverAllowance {
    use crate::relay::storage::upload_counts::Span;
    let resets_at = Span::Day.resets_at(now_ms);
    OverAllowance {
        words: format!(
            "Your role may upload {} MB a day on this server, and you have uploaded {} today{adding}. The allowance resets at midnight UTC, {}.",
            role.max_upload_mb_per_day,
            crate::relay::storage::media::mb(used),
            in_words(now_ms, resets_at)
        ),
        span: Span::Day,
        resets_at,
    }
}

/// Before an upload starts: refused when the role's hour allowance of uploads is used up, or its
/// day allowance of bytes is (nothing left today). Otherwise, how many bytes are left today (None:
/// no limit). Both upload routes call it first, before a byte is read.
pub(crate) fn allowance_at_start(db: &Storage, key: &str, role: &RoleDef, now_ms: i64) -> Result<Option<u64>, OverAllowance> {
    use crate::relay::storage::upload_counts::Span;
    if role.max_uploads_per_hour > 0 {
        let started = db.upload_count(key, Span::Hour, now_ms);
        if started >= role.max_uploads_per_hour {
            let resets_at = Span::Hour.resets_at(now_ms);
            return Err(OverAllowance {
                words: format!(
                    "Your role may start {} uploads an hour on this server, and you have started {started} this hour. More can start at the top of the hour, {}.",
                    role.max_uploads_per_hour,
                    in_words(now_ms, resets_at)
                ),
                span: Span::Hour,
                resets_at,
            });
        }
    }
    let Some(allowed) = day_allowance(role) else { return Ok(None) };
    let used = db.upload_count(key, Span::Day, now_ms);
    if used as u64 >= allowed {
        return Err(over_the_day(role, used, now_ms, ""));
    }
    Ok(Some(allowed - used as u64))
}

/// A single request's limit from what is left of the day's allowance (`left`, None for no limit):
/// over it, the day's words.
pub(crate) fn day_limit(db: &Storage, key: &str, role: &RoleDef, now_ms: i64, left: Option<u64>) -> FileLimit {
    match left {
        None => FileLimit { bytes: u64::MAX, refusal: None },
        Some(left) => {
            let used = db.upload_count(key, crate::relay::storage::upload_counts::Span::Day, now_ms);
            FileLimit { bytes: left, refusal: Some(over_the_day(role, used, now_ms, ", and this file would pass that").refused()) }
        }
    }
}

/// A resumable upload's next piece of `len` bytes: refused when it would take the person past the
/// day's allowance (what has arrived is kept, and the rest can come after the reset).
pub(crate) fn allowance_for_piece(db: &Storage, key: &str, role: &RoleDef, now_ms: i64, len: u64) -> Result<(), OverAllowance> {
    let Some(allowed) = day_allowance(role) else { return Ok(()) };
    let used = db.upload_count(key, crate::relay::storage::upload_counts::Span::Day, now_ms);
    if used as u64 + len > allowed {
        return Err(over_the_day(role, used, now_ms, ". What has arrived of this file is kept: send the rest after the reset"));
    }
    Ok(())
}

/// Count an upload against the allowances: `bytes` against the day, and one upload started
/// against the hour when `started`. A count that cannot be written is logged: the upload itself
/// already happened.
pub(crate) fn count_upload(db: &Storage, key: &str, now_ms: i64, bytes: u64, started: bool) {
    use crate::relay::storage::upload_counts::Span;
    if let Err(e) = db.add_upload_count(key, Span::Day, now_ms, bytes as i64) {
        tracing::error!("Upload allowance: could not count {bytes} bytes ({e})");
    }
    if started {
        if let Err(e) = db.add_upload_count(key, Span::Hour, now_ms, 1) {
            tracing::error!("Upload allowance: could not count an upload ({e})");
        }
    }
}

#[cfg(test)]
#[path = "upload_rules_tests.rs"]
mod tests;
