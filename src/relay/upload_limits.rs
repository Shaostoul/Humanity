//! Upload limits and streaming (2026-10-10), out of api.rs: the per-role limit, the route's body
//! limit, the up-front refusal on a declared size, and streaming a file to a part file that is
//! refused the moment it passes the limit. The handler itself is api.rs `upload_file`.

use axum::http::StatusCode;

/// Absolute ceiling on one upload, whatever a role allows (v0.482): it only stops a misconfigured
/// role from asking for something absurd. The real limit is the uploader's role's
/// `max_upload_mb` (Server Settings > Roles), and the server-wide `max_total_upload_mb` keeps the
/// disk from filling.
pub const UPLOAD_HARD_CEILING: u64 = 1024 * 1024 * 1024; // 1 GB
/// What a multipart form adds around the file itself (boundaries, headers, the file name), allowed
/// on top of a role's limit when the request's declared size is checked.
pub const UPLOAD_FORM_OVERHEAD: u64 = 64 * 1024;
/// The body limit on the upload route (src/relay/mod.rs): the ceiling plus the form's overhead.
pub const UPLOAD_BODY_LIMIT: usize = (UPLOAD_HARD_CEILING + UPLOAD_FORM_OVERHEAD) as usize;
/// Where an upload is streamed while it arrives, apart from data/uploads (which is served to the
/// public): a part file is never reachable by its address.
pub(crate) const UPLOAD_PART_DIR: &str = "data/uploads-part";

/// The most `role_def`'s holder may upload in one file, in bytes.
pub fn role_upload_limit(role_def: &crate::relay::storage::RoleDef) -> u64 {
    (role_def.max_upload_mb.max(1) as u64).saturating_mul(1024 * 1024).min(UPLOAD_HARD_CEILING)
}

/// An upload on its way to disk: an image's bytes, or a part file to move into place.
pub(crate) enum UploadBody {
    Bytes(axum::body::Bytes),
    Part(std::path::PathBuf),
}

impl UploadBody {
    pub(crate) fn len_hint(&self) -> usize {
        match self {
            UploadBody::Bytes(b) => b.len(),
            UploadBody::Part(p) => std::fs::metadata(p).map(|m| m.len() as usize).unwrap_or(0),
        }
    }
    /// Put it at `path`: write the bytes, or move the part file there.
    pub(crate) fn put(&self, path: &std::path::Path) -> std::io::Result<()> {
        match self {
            UploadBody::Bytes(b) => std::fs::write(path, b),
            UploadBody::Part(p) => std::fs::rename(p, path).or_else(|_| {
                // A part directory on another disk: copy, then remove the part.
                std::fs::copy(p, path)?;
                std::fs::remove_file(p)
            }),
        }
    }
    /// Not stored after all: a part file is removed.
    pub(crate) fn discard(&self) {
        if let UploadBody::Part(p) = self {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// A file streamed to `UPLOAD_PART_DIR`, and its length.
pub(crate) struct PartFile {
    pub(crate) path: std::path::PathBuf,
    pub(crate) len: usize,
}

/// A request whose declared size (`Content-Length`) is already over `max` plus the form's overhead:
/// refused before a byte of it is read.
pub(crate) fn declared_too_large(headers: &axum::http::HeaderMap, max: u64) -> Option<(StatusCode, String)> {
    let declared = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())?;
    (declared > max.saturating_add(UPLOAD_FORM_OVERHEAD)).then(|| upload_too_large(declared, max))
}

/// Stream one form field to a part file in `dir`, refusing it (and removing the part) the moment it
/// passes `max` bytes.
pub(crate) async fn stream_to_part(field: &mut axum::extract::multipart::Field<'_>, max: usize, dir: &std::path::Path) -> Result<PartFile, (StatusCode, String)> {
    use tokio::io::AsyncWriteExt;
    tokio::fs::create_dir_all(dir).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create upload dir: {e}")))?;
    let mut name = [0u8; 12];
    getrandom::getrandom(&mut name).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to name the upload: {e}")))?;
    let path = dir.join(format!("{}.part", hex::encode(name)));
    let mut file = tokio::fs::File::create(&path).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start the upload: {e}")))?;
    let mut len = 0usize;
    let outcome: Result<(), (StatusCode, String)> = async {
        while let Some(chunk) = field.chunk().await.map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to read file: {e}")))? {
            len += chunk.len();
            if len > max {
                return Err(upload_too_large(len as u64, max as u64));
            }
            file.write_all(&chunk).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {e}")))?;
        }
        file.flush().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {e}")))
    }
    .await;
    drop(file);
    if let Err(e) = outcome {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(e);
    }
    Ok(PartFile { path, len })
}

/// The refusal for a file over the uploader's limit, in words a person can act on.
pub(crate) fn upload_too_large(len: u64, max: u64) -> (StatusCode, String) {
    (StatusCode::PAYLOAD_TOO_LARGE, format!(
        "File too large ({:.1} MB; your limit on this server is {} MB. An admin can raise it in Server Settings > Roles).",
        len as f64 / (1024.0 * 1024.0),
        max / (1024 * 1024)
    ))
}


#[cfg(test)]
#[path = "api_upload_tests.rs"]
mod upload_tests;
