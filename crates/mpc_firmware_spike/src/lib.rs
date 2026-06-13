use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
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
    let file = File::open(path)
        .with_context(|| format!("failed to open firmware image {}", path.display()))?;
    let byte_len = file
        .metadata()
        .with_context(|| format!("failed to read firmware image metadata {}", path.display()))?
        .len();

    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read firmware image {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

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
        byte_len,
        sha256,
        stores_firmware_bytes: false,
    })
}
