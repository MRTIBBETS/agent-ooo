use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::LazyLock;

// Using regex to strip ANSI codes: CSI sequences (colors, cursor moves, clears)
// and OSC sequences (window titles, hyperlinks).
static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1B\[[0-9;?]*[A-Za-z]|\x1B\][^\x07\x1B]*(?:\x07|\x1B\\)").unwrap()
});

// Common secret patterns. Each requires a realistic token body so prose
// mentioning "Bearer tokens" or "sk-" prefixes is not redacted.
static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\bsk-ant-[A-Za-z0-9_\-]{20,}",        // Anthropic
        r"|\bsk-(?:proj-)?[A-Za-z0-9_\-]{20,}", // OpenAI
        r"|AIza[0-9A-Za-z_\-]{35}",             // Google / GCP API key
        r"|gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,}", // GitHub
        r"|AKIA[0-9A-Z]{16}",                   // AWS access key ID
        r"|eyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}", // JWT
        r"|(?i:bearer)\s+[A-Za-z0-9\-._~+/]{20,}=*", // Bearer token
        r"|-----BEGIN [A-Z ]*PRIVATE KEY-----", // PEM private key header
    ))
    .unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretFinding {
    pub secret_type: String,
    pub masked_preview: String,
    pub occurrences: usize,
    pub first_line: usize,
    pub last_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptScanReport {
    pub session_id: String,
    pub scanned_path: String,
    pub lines_scanned: usize,
    pub total_occurrences: usize,
    pub distinct_secrets: usize,
    pub findings: Vec<SecretFinding>,
}

/// Decodes a base64url (or standard base64) byte slice without external dependencies.
fn decode_base64url(input: &[u8]) -> Option<Vec<u8>> {
    if input.len() > 4096 {
        return None;
    }
    let mut out = Vec::with_capacity((input.len() * 3) / 4);
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;

    for &b in input {
        if b == b'=' {
            break;
        }
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

/// Extracts the `role` claim from a JWT payload segment if present and safe.
fn extract_jwt_role(jwt: &str) -> Option<String> {
    let mut parts = jwt.split('.');
    let _header = parts.next()?;
    let payload_b64 = parts.next()?;
    let decoded = decode_base64url(payload_b64.as_bytes())?;
    let val: Value = serde_json::from_slice(&decoded).ok()?;
    let role = val.get("role")?.as_str()?;
    if !role.is_empty()
        && role.len() <= 64
        && role
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        Some(role.to_string())
    } else {
        None
    }
}

/// Classifies a matched secret string into a canonical secret type identifier.
/// For JWTs carrying a `role` claim (such as Supabase `service_role`), appends the role.
pub fn classify_secret(matched: &str) -> String {
    if matched.starts_with("sk-ant-") {
        "anthropic_api_key".to_string()
    } else if matched.starts_with("sk-") {
        "openai_api_key".to_string()
    } else if matched.starts_with("AIza") {
        "google_api_key".to_string()
    } else if matched.starts_with("gh") || matched.starts_with("github_pat_") {
        "github_token".to_string()
    } else if matched.starts_with("AKIA") {
        "aws_access_key".to_string()
    } else if matched.starts_with("eyJ") {
        match extract_jwt_role(matched) {
            Some(role) => format!("jwt (role: {role})"),
            None => "jwt".to_string(),
        }
    } else if matched.starts_with("-----BEGIN ") {
        "pem_private_key".to_string()
    } else {
        "bearer_token".to_string()
    }
}

/// Extracts the underlying token material (stripping any leading `Bearer ` prefix).
fn normalize_secret_token(matched: &str) -> &str {
    if matched.len() > 7 && matched[..6].eq_ignore_ascii_case("bearer") {
        matched[6..].trim_start()
    } else {
        matched
    }
}

/// Produces a redacted preview (`first 7 chars...last 4 chars`) so raw credentials are never echoed.
pub fn mask_secret_preview(matched: &str) -> String {
    if matched.starts_with("-----BEGIN ") {
        return matched.to_string();
    }
    let token = normalize_secret_token(matched);
    let char_count = token.chars().count();
    if char_count > 14 {
        let prefix: String = token.chars().take(7).collect();
        let mut suffix_chars: Vec<char> = token.chars().rev().take(4).collect();
        suffix_chars.reverse();
        let suffix: String = suffix_chars.into_iter().collect();
        format!("{prefix}...{suffix}")
    } else {
        "[REDACTED_SECRET]".to_string()
    }
}

fn scan_str_into(
    text: &str,
    line_num: usize,
    index_by_hash: &mut HashMap<blake3::Hash, usize>,
    findings: &mut Vec<SecretFinding>,
    total_occurrences: &mut usize,
) {
    for mat in SECRET_RE.find_iter(text) {
        let raw = mat.as_str();
        let token = normalize_secret_token(raw);
        // Hash normalized token so raw secret material is never retained in memory.
        let hash = blake3::hash(token.as_bytes());
        *total_occurrences += 1;

        if let Some(&idx) = index_by_hash.get(&hash) {
            findings[idx].occurrences += 1;
            findings[idx].last_line = line_num;
        } else {
            let idx = findings.len();
            index_by_hash.insert(hash, idx);
            findings.push(SecretFinding {
                secret_type: classify_secret(raw),
                masked_preview: mask_secret_preview(raw),
                occurrences: 1,
                first_line: line_num,
                last_line: line_num,
            });
        }
    }
}

fn scan_json_value(
    value: &Value,
    line_num: usize,
    index_by_hash: &mut HashMap<blake3::Hash, usize>,
    findings: &mut Vec<SecretFinding>,
    total_occurrences: &mut usize,
) {
    match value {
        Value::String(s) => scan_str_into(s, line_num, index_by_hash, findings, total_occurrences),
        Value::Array(items) => {
            for item in items {
                scan_json_value(item, line_num, index_by_hash, findings, total_occurrences);
            }
        }
        Value::Object(map) => {
            for val in map.values() {
                scan_json_value(val, line_num, index_by_hash, findings, total_occurrences);
            }
        }
        _ => {}
    }
}

/// Streams a transcript file in constant memory and returns deduplicated, masked secret findings.
/// Automatically prefers a sibling `transcript_full.jsonl` when `path` is `transcript.jsonl`.
pub fn scan_transcript_secrets(path: &Path) -> crate::Result<TranscriptScanReport> {
    let target_path = crate::checkin::discovery::resolve_scan_target(path);
    let session_id = crate::checkin::discovery::session_id_for(&target_path);

    let file = File::open(&target_path)?;
    let reader = BufReader::new(file);

    let mut index_by_hash: HashMap<blake3::Hash, usize> = HashMap::new();
    let mut findings: Vec<SecretFinding> = Vec::new();
    let mut lines_scanned = 0;
    let mut total_occurrences = 0;

    for (idx, line_res) in reader.lines().enumerate() {
        let line = line_res?;
        if line.trim().is_empty() {
            continue;
        }
        let line_num = idx + 1;
        lines_scanned += 1;

        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            scan_json_value(
                &value,
                line_num,
                &mut index_by_hash,
                &mut findings,
                &mut total_occurrences,
            );
        } else {
            scan_str_into(
                &line,
                line_num,
                &mut index_by_hash,
                &mut findings,
                &mut total_occurrences,
            );
        }
    }

    Ok(TranscriptScanReport {
        session_id,
        scanned_path: target_path.to_string_lossy().to_string(),
        lines_scanned,
        total_occurrences,
        distinct_secrets: findings.len(),
        findings,
    })
}

/// Sanitizes a string by stripping ANSI escape codes and masking detected secrets.
pub fn sanitize_content(content: &str) -> (String, usize) {
    let mut redactions = 0;

    // First, mask secrets
    let masked = SECRET_RE.replace_all(content, |_: &regex::Captures| {
        redactions += 1;
        "[REDACTED_SECRET]"
    });

    // Second, strip ANSI escape codes
    let cleaned = ANSI_RE.replace_all(&masked, "");

    (cleaned.into_owned(), redactions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_ansi() {
        let input = "Hello \x1B[31mWorld\x1B[0m!";
        let (output, _) = sanitize_content(input);
        assert_eq!(output, "Hello World!");
    }

    #[test]
    fn test_sanitize_secrets() {
        let input = "My key is sk-1234567890abcdef1234567890abcdef1234567890abcdef and Bearer abcDEF1234567890ghijKLMN";
        let (output, count) = sanitize_content(input);
        assert!(output.contains("[REDACTED_SECRET]"));
        assert_eq!(count, 2);
    }

    #[test]
    fn test_sanitize_cloud_keys() {
        let input = concat!(
            "anthropic=sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx ",
            "gcp=AIzaSyA1234567890abcdefghijklmnopqrstuv ",
            "gh=ghp_abcdefghijklmnopqrstuvwxyz0123456789 ",
            "aws=AKIAABCDEFGHIJKLMNOP ",
            "jwt=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
        );
        let (output, count) = sanitize_content(input);
        assert_eq!(count, 5, "output was: {output}");
        assert!(!output.contains("AKIA"));
    }

    #[test]
    fn test_prose_is_not_redacted() {
        let input = "Use Bearer tokens for auth. The bearer token. Prefix keys with sk- and rotate them. Run task-before-startup-sequence-hook, see #ask-multiple-agents-together.";
        let (output, count) = sanitize_content(input);
        assert_eq!(count, 0);
        assert_eq!(output, input);
    }

    #[test]
    fn test_sanitize_extended_ansi() {
        let input = "\x1B[2K\x1B[1;32mok\x1B[0m \x1B]0;title\x07done";
        let (output, _) = sanitize_content(input);
        assert_eq!(output, "ok done");
    }

    #[test]
    fn test_scan_transcript_secrets_deduplicates_and_masks() {
        use std::io::Write;
        let dir = tempfile::TempDir::new().unwrap();
        let compact = dir.path().join("transcript.jsonl");
        let full = dir.path().join("transcript_full.jsonl");

        // Write truncated compact transcript with no secrets
        std::fs::write(&compact, r#"{"source":"USER","content":"truncated"}"#).unwrap();

        // Write full transcript with duplicate Anthropic keys and an AWS key
        let mut f = File::create(&full).unwrap();
        writeln!(
            f,
            r#"{{"source":"USER","content":"key1=sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"source":"MODEL","tool_calls":[{{"name":"run","args":"export K=sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx && aws=AKIAABCDEFGHIJKLMNOP"}}]}}"#
        )
        .unwrap();

        // Passing `compact` should automatically scan sibling `transcript_full.jsonl`
        let report = scan_transcript_secrets(&compact).unwrap();
        assert!(report.scanned_path.ends_with("transcript_full.jsonl"));
        assert_eq!(report.lines_scanned, 2);
        assert_eq!(report.total_occurrences, 3);
        assert_eq!(report.distinct_secrets, 2);

        assert_eq!(report.findings[0].secret_type, "anthropic_api_key");
        assert_eq!(report.findings[0].masked_preview, "sk-ant-...UvWx");
        assert_eq!(report.findings[0].occurrences, 2);
        assert_eq!(report.findings[0].first_line, 1);
        assert_eq!(report.findings[0].last_line, 2);

        assert_eq!(report.findings[1].secret_type, "aws_access_key");
        assert_eq!(report.findings[1].masked_preview, "AKIAABC...MNOP");
        assert_eq!(report.findings[1].occurrences, 1);
    }

    #[test]
    fn test_jwt_role_extraction() {
        // Payload is {"role":"service_role","iss":"supabase"} -> eyJyb2xlIjoic2VydmljZV9yb2xlIiwiaXNzIjoic3VwYWJhc2UifQ
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJyb2xlIjoic2VydmljZV9yb2xlIiwiaXNzIjoic3VwYWJhc2UifQ.signature12345";
        assert_eq!(classify_secret(jwt), "jwt (role: service_role)");
    }
}
