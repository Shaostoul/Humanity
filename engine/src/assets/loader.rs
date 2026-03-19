//! Asset loader — type-specific loading for GLB, images, shaders, data files.

/// Supported asset types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssetType {
    Mesh,    // .glb
    Texture, // .png, .ktx2
    Shader,  // .wgsl
    Audio,   // .ogg
    Data,    // .ron, .csv, .toml
}

/// Loads a raw asset from disk and identifies its type.
pub fn load_asset_bytes(path: &std::path::Path) -> Result<(AssetType, Vec<u8>), std::io::Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let asset_type = match ext {
        "glb" | "gltf" => AssetType::Mesh,
        "png" | "jpg" | "ktx2" => AssetType::Texture,
        "wgsl" => AssetType::Shader,
        "ogg" | "wav" => AssetType::Audio,
        _ => AssetType::Data,
    };
    let bytes = std::fs::read(path)?;
    Ok((asset_type, bytes))
}
