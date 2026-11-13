use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

const SPILLOVER_THRESHOLD_BYTES: usize = 512 * 1024; // 512 KB
const MAX_RECALL_SLICE_BYTES: usize = 32 * 1024; // 32 KB bounded recall window

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallReceipt {
    pub handle: String,
    pub total_bytes: u64,
    pub offset: u64,
    pub bytes_read: usize,
    pub has_more: bool,
    pub content: String,
}

fn sanitize_handle_filename(raw_handle: &str) -> Result<String, crate::OooError> {
    let trimmed = raw_handle
        .trim()
        .strip_prefix("blake3:")
        .unwrap_or_else(|| raw_handle.trim());
    let hex_part = trimmed.strip_suffix(".bin").unwrap_or(trimmed);
    if hex_part.is_empty()
        || hex_part.len() > 128
        || !hex_part.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(crate::OooError::General(format!(
            "Invalid CAS recall handle '{raw_handle}': expected a blake3 hex digest."
        )));
    }
    Ok(format!("{hex_part}.bin"))
}

/// Retrieves a bounded UTF-8 slice from `.agent-ooo/spillover/<blake3>.bin`
/// without loading the full multi-megabyte payload into memory.
pub fn recall_spilled_payload(
    base_dir: &Path,
    handle: &str,
    offset: u64,
    max_bytes: usize,
) -> Result<RecallReceipt, crate::OooError> {
    use std::io::{Seek, SeekFrom};

    let filename = sanitize_handle_filename(handle)?;
    let spillover_path = if base_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == ".agent-ooo")
        .unwrap_or(false)
    {
        base_dir.join("spillover").join(&filename)
    } else {
        base_dir
            .join(".agent-ooo")
            .join("spillover")
            .join(&filename)
    };

    let mut file = File::open(&spillover_path)?;
    let total_bytes = file.metadata()?.len();
    let clamped_offset = offset.min(total_bytes);
    file.seek(SeekFrom::Start(clamped_offset))?;

    let read_cap = max_bytes.clamp(1, MAX_RECALL_SLICE_BYTES);
    let mut buf = Vec::with_capacity(read_cap);
    file.take(read_cap as u64).read_to_end(&mut buf)?;

    let bytes_read = buf.len();
    let has_more = clamped_offset + (bytes_read as u64) < total_bytes;
    let raw_content = String::from_utf8_lossy(&buf);
    let (clean_content, _) = crate::detox::sanitizer::sanitize_content(&raw_content);

    Ok(RecallReceipt {
        handle: filename,
        total_bytes,
        offset: clamped_offset,
        bytes_read,
        has_more,
        content: clean_content,
    })
}

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

    #[test]
    fn recall_spilled_payload_reads_bounded_slices_and_rejects_traversal() {
        let dir = TempDir::new().unwrap();
        let payload = format!("HEADER:{}:FOOTER", "a".repeat(SPILLOVER_THRESHOLD_BYTES));
        let (_, handle) = manage_spillover_with_handle(&payload, dir.path()).unwrap();
        let handle_str = handle.unwrap();

        let recalled = recall_spilled_payload(dir.path(), &handle_str, 0, 16).unwrap();
        assert_eq!(recalled.bytes_read, 16);
        assert!(recalled.has_more);
        assert!(recalled.content.starts_with("HEADER:aaaaaaaaa"));

        // Reject path traversal handles
        assert!(recall_spilled_payload(dir.path(), "../checkpoint.json", 0, 64).is_err());
    }
}
