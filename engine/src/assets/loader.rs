//! Asset loader — type identification and data format parsing.
//!
//! Supports CSV, TOML, RON, and raw byte loading.
//! All parse functions are pure (no I/O) for cross-platform use.

use serde::de::DeserializeOwned;

/// Supported asset types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssetType {
    Mesh,    // .glb, .gltf
    Texture, // .png, .jpg, .ktx2
    Shader,  // .wgsl
    Audio,   // .ogg, .wav
    Data,    // .ron, .csv, .toml, .json
}

/// Identify asset type from a file extension string.
pub fn asset_type_from_ext(ext: &str) -> AssetType {
    match ext {
        "glb" | "gltf" => AssetType::Mesh,
        "png" | "jpg" | "ktx2" => AssetType::Texture,
        "wgsl" => AssetType::Shader,
        "ogg" | "wav" => AssetType::Audio,
        _ => AssetType::Data,
    }
}

/// Load raw asset bytes from disk and identify type (native only).
#[cfg(feature = "native")]
pub fn load_asset_bytes(path: &std::path::Path) -> Result<(AssetType, Vec<u8>), std::io::Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let asset_type = asset_type_from_ext(ext);
    let bytes = std::fs::read(path)?;
    Ok((asset_type, bytes))
}

/// Parse a CSV file into a Vec of deserialized records.
/// Skips comment lines (starting with #). Flexible: handles headers automatically.
pub fn parse_csv<T: DeserializeOwned>(data: &[u8]) -> Result<Vec<T>, String> {
    // Filter out comment lines before parsing
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 error: {e}"))?;
    let filtered: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(filtered.as_bytes());

    let mut records = Vec::new();
    for result in reader.deserialize() {
        match result {
            Ok(record) => records.push(record),
            Err(e) => {
                log::warn!("CSV parse warning (skipping row): {e}");
            }
        }
    }
    Ok(records)
}

/// Parse a TOML string into a deserialized struct.
pub fn parse_toml<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 error: {e}"))?;
    toml::from_str(text).map_err(|e| format!("TOML parse error: {e}"))
}

/// Parse a RON string into a deserialized struct.
pub fn parse_ron<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 error: {e}"))?;
    ron::from_str(text).map_err(|e| format!("RON parse error: {e}"))
}

/// Parse a JSON string into a deserialized struct.
pub fn parse_json<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|e| format!("JSON parse error: {e}"))
}
