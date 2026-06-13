use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageReport {
    pub file_name: String,
    pub byte_len: u64,
    pub sha256: String,
    pub stores_firmware_bytes: bool,
}

pub fn inspect_image(path: impl AsRef<Path>) -> Result<ImageReport> {
    let path = path.as_ref();
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read firmware image {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    Ok(ImageReport {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string(),
        byte_len: bytes.len() as u64,
        sha256,
        stores_firmware_bytes: false,
    })
}
