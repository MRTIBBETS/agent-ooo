use agent_ooo_core::detox::sanitizer::{
    has_ansi_candidate, has_secret_candidate, sanitize_content, scan_transcript_secrets,
};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn test_fast_path_candidate_predicates_and_near_misses() {
    // Empty and short (< 20 bytes) inputs never trigger secret candidate detection.
    assert!(!has_secret_candidate(b""));
    assert!(!has_secret_candidate(b"short"));
    assert!(!has_secret_candidate(b"sk-short"));
    assert!(!has_secret_candidate(b"Use Bearer tokens"));
    assert!(!has_secret_candidate(b"github_"));
    assert!(!has_secret_candidate(b"AKIA123"));
    assert!(!has_secret_candidate(b"AKIA123456789012345")); // 19 bytes
    assert!(!has_secret_candidate(b"eyJshort"));
    assert!(!has_secret_candidate(b"high"));

    // Clean ASCII prose >= 20 bytes stays negative.
    let clean = "Reviewed cargo test output across workspace crates with zero warnings.";
    assert!(!has_secret_candidate(clean.as_bytes()));
    assert!(!has_ansi_candidate(clean.as_bytes()));
    assert_eq!(sanitize_content(clean), (clean.to_string(), 0));

    // Near-misses padded >= 20 bytes must still yield zero redactions.
    let near_misses = [
        "Prefix keys with sk-short before rotating.",
        "Use Bearer tokens in Authorization headers for testing.",
        "Check github_handle and github_pat_short validation rules.",
        "Legacy identifier AKIA12345678901 is shorter than 20 chars.",
        "Header eyJshort is not a valid three-segment JWT token.",
        "high_water_mark_threshold_exceeded_without_secret",
        "-----BEGIN CUSTOM BLOCK----- without private key header",
    ];
    for sample in near_misses {
        assert!(
            !has_ansi_candidate(sample.as_bytes()),
            "unexpected ANSI candidate in: {sample}"
        );
        let (cleaned, count) = sanitize_content(sample);
        assert_eq!(count, 0, "unexpected redaction in: {sample}");
        assert_eq!(cleaned, sample);
    }
}

#[test]
fn test_fast_path_equivalence_across_all_secret_families_and_ansi() {
    let secret_samples = [
        (
            "anthropic",
            "token=sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx",
            "sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx",
        ),
        (
            "openai_classic",
            "token=sk-1234567890abcdef1234567890abcdef",
            "sk-1234567890abcdef1234567890abcdef",
        ),
        (
            "openai_proj",
            "token=sk-proj-1234567890abcdef1234567890abcdef",
            "sk-proj-1234567890abcdef1234567890abcdef",
        ),
        (
            "google_aiza",
            "gcp=AIzaSyA1234567890abcdefghijklmnopqrstuv",
            "AIzaSyA1234567890abcdefghijklmnopqrstuv",
        ),
        (
            "google_aq",
            "vertex=AQ.AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
            "AQ.AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
        ),
        (
            "github_ghp",
            "gh=ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
        ),
        (
            "github_pat",
            "gh=github_pat_1234567890abcdef1234567890",
            "github_pat_1234567890abcdef1234567890",
        ),
        (
            "aws_akia",
            "aws=AKIAABCDEFGHIJKLMNOP",
            "AKIAABCDEFGHIJKLMNOP",
        ),
        (
            "jwt",
            "jwt=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
        ),
        (
            "bearer",
            "Authorization: Bearer abcDEF1234567890ghijKLMN",
            "abcDEF1234567890ghijKLMN",
        ),
        (
            "pem",
            "-----BEGIN PRIVATE KEY-----",
            "-----BEGIN PRIVATE KEY-----",
        ),
    ];

    for (label, line, raw_fragment) in secret_samples {
        assert!(
            has_secret_candidate(line.as_bytes()),
            "{label} should trigger has_secret_candidate"
        );
        assert!(!has_ansi_candidate(line.as_bytes()));
        let (cleaned, count) = sanitize_content(line);
        assert_eq!(count, 1, "{label} should redact once: {cleaned}");
        assert!(
            !cleaned.contains(raw_fragment),
            "{label} leaked raw fragment in: {cleaned}"
        );
        assert!(cleaned.contains("[REDACTED_SECRET]"));
    }

    // ANSI CSI and OSC sequences without secrets
    let ansi_only =
        "\x1B[2K\x1B[1;32mpass\x1B[0m \x1B]8;;https://example.com\x1B\\link\x1B]8;;\x07";
    assert!(!has_secret_candidate(ansi_only.as_bytes()));
    assert!(has_ansi_candidate(ansi_only.as_bytes()));
    let (cleaned_ansi, ansi_redactions) = sanitize_content(ansi_only);
    assert_eq!(ansi_redactions, 0);
    assert_eq!(cleaned_ansi, "pass link");

    // Combined ANSI + secret
    let combined = "\x1B[31mError:\x1B[0m key=AKIAABCDEFGHIJKLMNOP";
    assert!(has_secret_candidate(combined.as_bytes()));
    assert!(has_ansi_candidate(combined.as_bytes()));
    let (cleaned_combined, combined_redactions) = sanitize_content(combined);
    assert_eq!(combined_redactions, 1);
    assert_eq!(cleaned_combined, "Error: key=[REDACTED_SECRET]");
}

#[test]
fn test_scan_transcript_catches_json_unicode_escaped_secret_and_tracks_line_numbers() {
    let dir = TempDir::new().expect("create temp dir");
    let transcript = dir.path().join("transcript.jsonl");

    // Raw line uses \u0073 ('s') so the raw byte slice has no "sk-" prefix,
    // exercising the contains_json_unicode_escape fallback before JSON parsing.
    let escaped_line =
        r#"{"source":"MODEL","content":"escaped=\u0073k-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx"}"#;
    assert!(
        !has_secret_candidate(escaped_line.as_bytes()),
        "raw escaped line should not match has_secret_candidate directly"
    );

    {
        let mut f = File::create(&transcript).expect("create transcript");
        writeln!(f, r#"{{"source":"USER","content":"clean turn one"}}"#).expect("write line 1");
        writeln!(f).expect("write blank line 2");
        writeln!(f, "{escaped_line}").expect("write escaped line 3");
        writeln!(
            f,
            r#"{{"source":"MODEL","content":"plain=AKIAABCDEFGHIJKLMNOP"}}"#
        )
        .expect("write line 4");
    }

    let report = scan_transcript_secrets(&transcript).expect("scan transcript");
    assert_eq!(report.lines_scanned, 3);
    assert_eq!(report.total_occurrences, 2);
    assert_eq!(report.distinct_secrets, 2);

    assert_eq!(report.findings[0].secret_type, "anthropic_api_key");
    assert_eq!(report.findings[0].masked_preview, "sk-ant-...UvWx");
    assert_eq!(report.findings[0].first_line, 3);
    assert_eq!(report.findings[0].last_line, 3);

    assert_eq!(report.findings[1].secret_type, "aws_access_key");
    assert_eq!(report.findings[1].first_line, 4);
    assert_eq!(report.findings[1].last_line, 4);
}

#[test]
fn benchmark_sanitizer_clean_and_mixed_throughput() {
    const CLEAN_TURNS: usize = 10_000;
    const MIXED_TURNS: usize = 10_000;

    let dir = TempDir::new().expect("create temp dir");
    let clean_path = dir.path().join("clean_4mb.jsonl");
    let mixed_path = dir.path().join("mixed_4mb.jsonl");

    let mut clean_lines = Vec::with_capacity(CLEAN_TURNS);
    let mut clean_bytes: usize = 0;
    {
        let file = File::create(&clean_path).expect("create clean jsonl");
        let mut writer = BufWriter::new(file);
        for i in 0..CLEAN_TURNS {
            let line = format!(
                r#"{{"step_index":{i},"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","content":"Inspected module chunk {i:05} and verified bounded streaming invariants across workspace crates without allocating full transcript buffers in memory during routine maintenance.","tool_calls":[{{"name":"read_file","args":{{"path":"crates/core/src/detox/sanitizer.rs","start":1,"end":160,"turn":{i}}}}}]}}"#
            );
            clean_bytes += line.len() + 1;
            writeln!(writer, "{line}").expect("write clean line");
            clean_lines.push(line);
        }
        writer.flush().expect("flush clean writer");
    }

    assert!(
        clean_bytes >= 3_500_000,
        "clean fixture should be ~4 MB, got {clean_bytes} bytes"
    );

    // 1. Clean scan_transcript_secrets throughput
    let start = Instant::now();
    let clean_report = scan_transcript_secrets(&clean_path).expect("scan clean transcript");
    let clean_scan_elapsed = start.elapsed();
    let clean_scan_mb_s =
        (clean_bytes as f64 / (1024.0 * 1024.0)) / clean_scan_elapsed.as_secs_f64().max(1e-9);

    assert_eq!(clean_report.lines_scanned, CLEAN_TURNS);
    assert_eq!(clean_report.total_occurrences, 0);
    assert_eq!(clean_report.distinct_secrets, 0);

    // 2. Clean sanitize_content throughput across all 10,000 lines
    let start = Instant::now();
    let mut clean_redactions = 0usize;
    let mut clean_sanitized_bytes = 0usize;
    for line in &clean_lines {
        let (out, count) = sanitize_content(line);
        clean_redactions += count;
        clean_sanitized_bytes += out.len();
    }
    let clean_sanitize_elapsed = start.elapsed();
    let clean_sanitize_mb_s =
        (clean_bytes as f64 / (1024.0 * 1024.0)) / clean_sanitize_elapsed.as_secs_f64().max(1e-9);
    assert_eq!(clean_redactions, 0);
    assert_eq!(clean_sanitized_bytes, clean_bytes - CLEAN_TURNS);

    // 3. Mixed dataset: periodic ANSI + secrets (every 50th turn -> 200 dirty lines, 2 secrets each = 400 occurrences, 4 distinct secrets)
    let mut mixed_sanitize_inputs = Vec::with_capacity(MIXED_TURNS);
    let mut mixed_jsonl_bytes: usize = 0;
    let mut mixed_sanitize_bytes: usize = 0;
    let mut expected_dirty = 0usize;
    let secret_pairs = [
        (
            "sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx",
            "AKIAABCDEFGHIJKLMNOP",
        ),
        (
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "AIzaSyA1234567890abcdefghijklmnopqrstuv",
        ),
    ];
    {
        let file = File::create(&mixed_path).expect("create mixed jsonl");
        let mut writer = BufWriter::new(file);
        for i in 0..MIXED_TURNS {
            let (jsonl_line, sanitize_input) = if i % 50 == 0 {
                expected_dirty += 1;
                let (s1, s2) = secret_pairs[(i / 50) % secret_pairs.len()];
                (
                    format!(
                        "{{\"step_index\":{i},\"source\":\"MODEL\",\"type\":\"GENERIC\",\"status\":\"DONE\",\"content\":\"\\u001b[31mWARN\\u001b[0m leaked {s1} and {s2} in diagnostic output chunk {i:05}\"}}"
                    ),
                    format!(
                        "\x1B[31mWARN\x1B[0m leaked {s1} and {s2} in diagnostic output chunk {i:05} with surrounding log context and tool metadata"
                    ),
                )
            } else {
                let clean = format!(
                    r#"{{"step_index":{i},"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","content":"Completed routine step {i:05} with clean status output and deterministic state verification across workspace targets."}}"#
                );
                (clean.clone(), clean)
            };
            mixed_jsonl_bytes += jsonl_line.len() + 1;
            mixed_sanitize_bytes += sanitize_input.len();
            writeln!(writer, "{jsonl_line}").expect("write mixed line");
            mixed_sanitize_inputs.push(sanitize_input);
        }
        writer.flush().expect("flush mixed writer");
    }

    let start = Instant::now();
    let mixed_report = scan_transcript_secrets(&mixed_path).expect("scan mixed transcript");
    let mixed_scan_elapsed = start.elapsed();
    let mixed_scan_mb_s =
        (mixed_jsonl_bytes as f64 / (1024.0 * 1024.0)) / mixed_scan_elapsed.as_secs_f64().max(1e-9);

    assert_eq!(mixed_report.lines_scanned, MIXED_TURNS);
    assert_eq!(mixed_report.total_occurrences, expected_dirty * 2);
    assert_eq!(mixed_report.distinct_secrets, 4);

    let start = Instant::now();
    let mut mixed_redactions = 0usize;
    for (idx, input) in mixed_sanitize_inputs.iter().enumerate() {
        let (out, count) = sanitize_content(input);
        mixed_redactions += count;
        if idx % 50 == 0 {
            assert_eq!(count, 2);
            assert!(out.starts_with("WARN leaked [REDACTED_SECRET] and [REDACTED_SECRET]"));
        }
        assert!(!out.contains("sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx"));
        assert!(!out.contains("AKIAABCDEFGHIJKLMNOP"));
        assert!(!out.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
        assert!(!out.contains("AIzaSyA1234567890abcdefghijklmnopqrstuv"));
        assert!(!out.contains('\x1B'));
    }
    let mixed_sanitize_elapsed = start.elapsed();
    let mixed_sanitize_mb_s = (mixed_sanitize_bytes as f64 / (1024.0 * 1024.0))
        / mixed_sanitize_elapsed.as_secs_f64().max(1e-9);

    assert_eq!(mixed_redactions, expected_dirty * 2);

    println!(
        "sanitizer throughput (clean {:.2} MB): scan_transcript_secrets={:.2} MB/s ({:?}), sanitize_content={:.2} MB/s ({:?})",
        clean_bytes as f64 / (1024.0 * 1024.0),
        clean_scan_mb_s,
        clean_scan_elapsed,
        clean_sanitize_mb_s,
        clean_sanitize_elapsed,
    );
    println!(
        "sanitizer throughput (mixed scan {:.2} MB / sanitize {:.2} MB): scan_transcript_secrets={:.2} MB/s ({:?}), sanitize_content={:.2} MB/s ({:?})",
        mixed_jsonl_bytes as f64 / (1024.0 * 1024.0),
        mixed_sanitize_bytes as f64 / (1024.0 * 1024.0),
        mixed_scan_mb_s,
        mixed_scan_elapsed,
        mixed_sanitize_mb_s,
        mixed_sanitize_elapsed,
    );

    #[cfg(not(debug_assertions))]
    {
        assert!(
            clean_scan_mb_s >= 20.0,
            "expected release clean scan_transcript_secrets >= 20.0 MB/s, got {clean_scan_mb_s:.2} MB/s"
        );
        assert!(
            clean_sanitize_mb_s >= 20.0,
            "expected release clean sanitize_content >= 20.0 MB/s, got {clean_sanitize_mb_s:.2} MB/s"
        );
    }
}
