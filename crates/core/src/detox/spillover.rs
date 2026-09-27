use std::fs;
use std::path::Path;
use blake3::Hasher;

const SPILLOVER_THRESHOLD_BYTES: usize = 512 * 1024; // 512 KB

/// Evaluates content size. If it exceeds 512KB, writes to disk and returns a stub.
/// Otherwise returns the content unaltered.
pub fn manage_spillover(content: &str, base_dir: &Path) -> Result<(String, bool), std::io::Error> {
    let bytes = content.as_bytes();
    let size = bytes.len();

    if size > SPILLOVER_THRESHOLD_BYTES {
        let mut hasher = Hasher::new();
        hasher.update(bytes);
        let hash = hasher.finalize();
        let hash_hex = hash.to_hex();

        let spillover_dir = base_dir.join(".agent-ooo").join("spillover");
        fs::create_dir_all(&spillover_dir)?;

        let filename = format!("{}.bin", hash_hex);
        let filepath = spillover_dir.join(filename);

        fs::write(filepath, bytes)?;

        let stub = format!("[Detox Spillover: blake3:{}, bytes: {}]", hash_hex, size);
        Ok((stub, true))
    } else {
        Ok((content.to_string(), false))
    }
}
