//! Asset loader — type identification and raw byte loading.
//!
//! The data files' parsers (CSV, TOML, RON, JSON) lived here until the crate split's rung 0
//! (2026-10-31) moved them down to the core as `crate::data_parse`, so the simulation and the
//! ship stop reaching into the renderer's half to read a data file. What stays is the asset
//! manager's own business: which kind of asset a file is, and reading its bytes.

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
