use regex::Regex;
use std::sync::LazyLock;

// 1. ISO 8601 & RFC 3339 Timestamps
static RE_ISO8601: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?x) \b \d{4}-\d{2}-\d{2}[T\s]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})? \b").unwrap()
});

// 2. UNIX Epoch (Heuristic for years 2020-2038)
static RE_EPOCH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?x) \b 1[6-9]\d{8}(?:\.\d{3,6})? \b").unwrap()
});

// 3. UUIDv4
static RE_UUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b").unwrap()
});

// 4. Hexadecimal Pointers & Memory Addresses
static RE_HEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b0x[0-9a-f]+\b").unwrap()
});

// 5. Volatile Temp File Paths
static RE_TEMP_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?x) (?:/tmp/|/var/folders/[a-zA-Z0-9_/]+/T/)[a-zA-Z0-9_.-]+").unwrap()
});

/// Normalizes volatile parts of tool outputs (timestamps, UUIDs, memory addresses)
/// to compute a structural intent hash for loop detection.
pub fn normalize_volatile_data(content: &str) -> String {
    let mut normalized = content.to_string();
    normalized = RE_ISO8601.replace_all(&normalized, "[TIME]").to_string();
    normalized = RE_EPOCH.replace_all(&normalized, "[EPOCH]").to_string();
    normalized = RE_UUID.replace_all(&normalized, "[UUID]").to_string();
    normalized = RE_HEX.replace_all(&normalized, "[HEX]").to_string();
    normalized = RE_TEMP_PATH.replace_all(&normalized, "[TEMP_PATH]").to_string();
    normalized
}

/// Generates a blake3 structural intent hash from normalized data
pub fn compute_structural_hash(normalized_content: &str) -> String {
    blake3::hash(normalized_content.as_bytes()).to_hex().to_string()
}
