use blake3::Hasher;
use std::fs;
use std::path::Path;

const SPILLOVER_THRESHOLD_BYTES: usize = 512 * 1024; // 512 KB

/// Evaluates content size. If it exceeds 512KB, writes to disk and returns a stub
/// along with the spilled `<blake3>.bin` handle. Otherwise returns the content unaltered.
pub fn manage_spillover_with_handle(
    content: &str,
    base_dir: &Path,
) -> Result<(String, Option<String>), std::io::Error> {
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
        let filepath = spillover_dir.join(&filename);

        fs::write(filepath, bytes)?;

        let stub = format!("[Detox Spillover: blake3:{}, bytes: {}]", hash_hex, size);
        Ok((stub, Some(filename)))
    } else {
        Ok((content.to_string(), None))
    }
}

/// Evaluates content size. If it exceeds 512KB, writes to disk and returns a stub.
/// Otherwise returns the content unaltered.
pub fn manage_spillover(content: &str, base_dir: &Path) -> Result<(String, bool), std::io::Error> {
    let (out, handle) = manage_spillover_with_handle(content, base_dir)?;
    Ok((out, handle.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Spillover was the only detox path with no test coverage.
    #[test]
    fn large_payload_is_spilled_and_recoverable() {
        let dir = TempDir::new().unwrap();
        let payload = "x".repeat(SPILLOVER_THRESHOLD_BYTES + 1);

        let (stub, spilled) = manage_spillover(&payload, dir.path()).unwrap();
        assert!(spilled);
        assert!(stub.starts_with("[Detox Spillover: blake3:"));
        assert!(stub.len() < 200);

        // The original bytes must be recoverable from content-addressed storage.
        let hash = blake3::hash(payload.as_bytes()).to_hex();
        let stored = fs::read(
            dir.path()
                .join(".agent-ooo/spillover")
                .join(format!("{hash}.bin")),
        )
        .unwrap();
        assert_eq!(stored, payload.as_bytes());
    }

    #[test]
    fn payload_at_threshold_is_untouched() {
        let dir = TempDir::new().unwrap();
        let payload = "x".repeat(SPILLOVER_THRESHOLD_BYTES);

        let (out, spilled) = manage_spillover(&payload, dir.path()).unwrap();
        assert!(!spilled);
        assert_eq!(out, payload);
        assert!(!dir.path().join(".agent-ooo").exists());
    }
}
