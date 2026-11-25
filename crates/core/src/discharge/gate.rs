use crate::detox::sanitizer::scan_line_for_secrets;
use crate::discharge::canary::{self, CanaryFinding, CanaryReport, CanaryStatus};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

const MAX_DIFF_LINES: usize = 50_000;
const MAX_ARTIFACT_LINES: usize = 50_000;
const MAX_PUSH_RANGES: usize = 16;
const MAX_UNTRACKED_FILES: usize = 256;
const MAX_UNTRACKED_FILE_BYTES: u64 = 1_048_576;
const BINARY_SNIFF_BYTES: usize = 512;
const MAX_SECRET_LEAKS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateMode {
    Block,
    WarnOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateOutcome {
    Pass,
    Warn,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretLeakLocation {
    pub source: String,
    pub line: usize,
    pub secret_type: String,
    pub masked_preview: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanaryGateReport {
    pub outcome: GateOutcome,
    pub mode: GateMode,
    pub canary_report: Option<CanaryReport>,
    pub secret_leaks: Vec<SecretLeakLocation>,
    pub files_scanned: usize,
    pub elapsed_ms: f64,
}

fn is_all_zero_sha(sha: &str) -> bool {
    !sha.is_empty() && sha.bytes().all(|b| b == b'0')
}

/// Parses git `pre-push` hook stdin lines (`<local_ref> <local_sha> <remote_ref> <remote_sha>`)
/// into commit ranges or single commit SHAs suitable for diff scanning.
pub fn parse_pre_push_ranges(stdin_text: &str) -> Vec<String> {
    let mut ranges = Vec::new();
    for line in stdin_text.lines() {
        let mut tokens = line.split_whitespace();
        let Some(_local_ref) = tokens.next() else {
            continue;
        };
        let Some(local_sha) = tokens.next() else {
            continue;
        };
        let Some(_remote_ref) = tokens.next() else {
            continue;
        };
        let Some(remote_sha) = tokens.next() else {
            continue;
        };

        if is_all_zero_sha(local_sha) {
            continue;
        }

        if is_all_zero_sha(remote_sha) {
            ranges.push(local_sha.to_string());
        } else {
            ranges.push(format!("{remote_sha}..{local_sha}"));
        }
    }
    ranges
}

fn parse_hunk_new_start(line: &str) -> Option<usize> {
    let after_at = line.strip_prefix("@@ ")?;
    let plus_idx = after_at.find('+')?;
    let rest = &after_at[plus_idx + 1..];
    let digit_len = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if digit_len == 0 {
        return None;
    }
    rest[..digit_len].parse::<usize>().ok()
}

fn push_line_secrets(
    source: &str,
    line_num: usize,
    text: &str,
    secret_leaks: &mut Vec<SecretLeakLocation>,
) {
    if secret_leaks.len() >= MAX_SECRET_LEAKS {
        return;
    }
    for (secret_type, masked_preview) in scan_line_for_secrets(text) {
        if secret_leaks.len() >= MAX_SECRET_LEAKS {
            break;
        }
        secret_leaks.push(SecretLeakLocation {
            source: source.to_string(),
            line: line_num,
            secret_type,
            masked_preview,
        });
    }
}

fn scan_text_reader<R: BufRead>(
    reader: &mut R,
    source: &str,
    line_buf: &mut String,
    secret_leaks: &mut Vec<SecretLeakLocation>,
    max_lines: usize,
) {
    let mut line_num = 0usize;
    while line_num < max_lines && secret_leaks.len() < MAX_SECRET_LEAKS {
        line_buf.clear();
        match reader.read_line(line_buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                line_num += 1;
                let trimmed = line_buf.trim_end_matches(&['\r', '\n'][..]);
                push_line_secrets(source, line_num, trimmed, secret_leaks);
            }
        }
    }
}

fn git_ref_resolves(workspace_dir: &Path, rev: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", rev])
        .current_dir(workspace_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn stream_git_diff_command(
    workspace_dir: &Path,
    args: &[&str],
    line_buf: &mut String,
    secret_leaks: &mut Vec<SecretLeakLocation>,
    diff_lines_remaining: &mut usize,
    files_scanned: &mut usize,
) {
    if *diff_lines_remaining == 0 || secret_leaks.len() >= MAX_SECRET_LEAKS {
        return;
    }

    let Ok(mut child) = Command::new("git")
        .args(args)
        .current_dir(workspace_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.wait();
        return;
    };

    let mut reader = BufReader::new(stdout);
    let mut current_source = String::from("<git-diff>");
    let mut current_line = 0usize;

    while *diff_lines_remaining > 0 && secret_leaks.len() < MAX_SECRET_LEAKS {
        line_buf.clear();
        match reader.read_line(line_buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                *diff_lines_remaining -= 1;
                let line = line_buf.trim_end_matches(&['\r', '\n'][..]);

                if let Some(path) = line.strip_prefix("+++ b/") {
                    current_source.clear();
                    current_source.push_str(path);
                    *files_scanned += 1;
                    continue;
                }
                if line.starts_with("+++ ") {
                    continue;
                }
                if let Some(new_start) = parse_hunk_new_start(line) {
                    current_line = new_start;
                    continue;
                }
                if let Some(added) = line.strip_prefix('+') {
                    let line_num = current_line.max(1);
                    current_line = line_num.saturating_add(1);
                    push_line_secrets(&current_source, line_num, added, secret_leaks);
                }
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

fn should_skip_untracked_rel(rel: &str) -> bool {
    rel.is_empty()
        || rel == ".git"
        || rel == "target"
        || rel == ".agent-ooo"
        || rel.starts_with(".git/")
        || rel.starts_with("target/")
        || rel.starts_with(".agent-ooo/")
}

fn scan_untracked_files(
    workspace_dir: &Path,
    line_buf: &mut String,
    secret_leaks: &mut Vec<SecretLeakLocation>,
    files_scanned: &mut usize,
) {
    if secret_leaks.len() >= MAX_SECRET_LEAKS {
        return;
    }

    let Ok(mut child) = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(workspace_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.wait();
        return;
    };

    let mut path_reader = BufReader::new(stdout);
    let mut path_buf: Vec<u8> = Vec::with_capacity(256);
    let mut untracked_seen = 0usize;

    while untracked_seen < MAX_UNTRACKED_FILES && secret_leaks.len() < MAX_SECRET_LEAKS {
        path_buf.clear();
        match path_reader.read_until(b'\0', &mut path_buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if path_buf.last() == Some(&b'\0') {
                    path_buf.pop();
                }
                let Ok(rel) = std::str::from_utf8(&path_buf) else {
                    continue;
                };
                if should_skip_untracked_rel(rel) {
                    continue;
                }
                untracked_seen += 1;

                let full_path = workspace_dir.join(rel);
                let Ok(meta) = std::fs::symlink_metadata(&full_path) else {
                    continue;
                };
                if !meta.is_file() || meta.len() > MAX_UNTRACKED_FILE_BYTES {
                    continue;
                }

                let Ok(file) = File::open(&full_path) else {
                    continue;
                };
                let mut reader = BufReader::new(file);
                let Ok(peek) = reader.fill_buf() else {
                    continue;
                };
                let check_len = peek.len().min(BINARY_SNIFF_BYTES);
                if peek[..check_len].contains(&0) {
                    continue;
                }

                *files_scanned += 1;
                scan_text_reader(&mut reader, rel, line_buf, secret_leaks, MAX_ARTIFACT_LINES);
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

/// Evaluates the pre-push canary and secret hygiene gate for `workspace_dir`.
pub fn evaluate_pre_push_gate(
    workspace_dir: &Path,
    mode: GateMode,
    push_ranges: &[String],
) -> Result<CanaryGateReport, crate::OooError> {
    let start = Instant::now();
    let agent_ooo_dir = workspace_dir.join(".agent-ooo");

    let has_capnp = agent_ooo_dir.join("checkpoint.capnp").exists();
    let has_json = agent_ooo_dir.join("checkpoint.json").exists();

    let canary_report = match (has_capnp, has_json) {
        (true, true) => Some(canary::evaluate_canaries(workspace_dir, &agent_ooo_dir)?),
        (true, false) | (false, true) => Some(CanaryReport {
            structural_integrity: false,
            behavioral_status: CanaryStatus::Fail,
            findings: vec![CanaryFinding {
                code: "checkpoint_structural_integrity".to_string(),
                status: CanaryStatus::Fail,
                message: "Incomplete checkpoint pair in .agent-ooo/ (missing capnp or json)."
                    .to_string(),
            }],
        }),
        (false, false) => None,
    };

    let mut line_buf = String::with_capacity(512);
    let mut secret_leaks: Vec<SecretLeakLocation> = Vec::new();
    let mut files_scanned = 0usize;

    for rel in [
        ".agent-ooo/checkpoint.json",
        ".agent-ooo/spa_report.md",
        ".agent-ooo/spa_report.json",
    ] {
        if secret_leaks.len() >= MAX_SECRET_LEAKS {
            break;
        }
        let path = workspace_dir.join(rel);
        if path.is_file() {
            if let Ok(file) = File::open(&path) {
                files_scanned += 1;
                let mut reader = BufReader::new(file);
                scan_text_reader(
                    &mut reader,
                    rel,
                    &mut line_buf,
                    &mut secret_leaks,
                    MAX_ARTIFACT_LINES,
                );
            }
        }
    }

    if workspace_dir.join(".git").exists() && secret_leaks.len() < MAX_SECRET_LEAKS {
        let mut diff_lines_remaining = MAX_DIFF_LINES;

        if !push_ranges.is_empty() {
            for range in push_ranges.iter().take(MAX_PUSH_RANGES) {
                if secret_leaks.len() >= MAX_SECRET_LEAKS || diff_lines_remaining == 0 {
                    break;
                }
                let trimmed = range.trim();
                if trimmed.is_empty() || trimmed.starts_with('-') {
                    continue;
                }
                if trimmed.contains("..") {
                    stream_git_diff_command(
                        workspace_dir,
                        &["diff", "-U0", trimmed],
                        &mut line_buf,
                        &mut secret_leaks,
                        &mut diff_lines_remaining,
                        &mut files_scanned,
                    );
                } else {
                    stream_git_diff_command(
                        workspace_dir,
                        &["show", "--format=", "-U0", trimmed],
                        &mut line_buf,
                        &mut secret_leaks,
                        &mut diff_lines_remaining,
                        &mut files_scanned,
                    );
                }
            }
        } else if git_ref_resolves(workspace_dir, "@{u}") {
            stream_git_diff_command(
                workspace_dir,
                &["diff", "-U0", "@{u}..HEAD"],
                &mut line_buf,
                &mut secret_leaks,
                &mut diff_lines_remaining,
                &mut files_scanned,
            );
        }

        if git_ref_resolves(workspace_dir, "HEAD") {
            stream_git_diff_command(
                workspace_dir,
                &["diff", "-U0", "HEAD"],
                &mut line_buf,
                &mut secret_leaks,
                &mut diff_lines_remaining,
                &mut files_scanned,
            );
        } else {
            stream_git_diff_command(
                workspace_dir,
                &["diff", "-U0", "--cached"],
                &mut line_buf,
                &mut secret_leaks,
                &mut diff_lines_remaining,
                &mut files_scanned,
            );
        }

        scan_untracked_files(
            workspace_dir,
            &mut line_buf,
            &mut secret_leaks,
            &mut files_scanned,
        );
    }

    let has_fail_canary = canary_report
        .as_ref()
        .is_some_and(|r| !r.structural_integrity || r.behavioral_status == CanaryStatus::Fail);
    let has_warn_canary = canary_report
        .as_ref()
        .is_some_and(|r| r.behavioral_status == CanaryStatus::Warn);
    let has_secrets = !secret_leaks.is_empty();

    let outcome = if has_fail_canary || has_secrets {
        match mode {
            GateMode::Block => GateOutcome::Block,
            GateMode::WarnOnly => GateOutcome::Warn,
        }
    } else if has_warn_canary {
        GateOutcome::Warn
    } else {
        GateOutcome::Pass
    };

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(CanaryGateReport {
        outcome,
        mode,
        canary_report,
        secret_leaks,
        files_scanned,
        elapsed_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn run_git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command should execute");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn init_git_repo(dir: &Path) {
        run_git(dir, &["init"]);
        run_git(dir, &["config", "user.email", "test@example.com"]);
        run_git(dir, &["config", "user.name", "Test"]);
    }

    #[test]
    fn test_parse_pre_push_ranges_handles_updates_creates_deletes() {
        let zeros = "0".repeat(40);
        let sha_a = "a".repeat(40);
        let sha_b = "b".repeat(40);
        let stdin = format!(
            "refs/heads/main {sha_b} refs/heads/main {sha_a}\n\
             refs/heads/feat {sha_b} refs/heads/feat {zeros}\n\
             refs/heads/old {zeros} refs/heads/old {sha_a}\n\
             malformed line\n\
             \n"
        );

        let ranges = parse_pre_push_ranges(&stdin);
        assert_eq!(ranges, vec![format!("{sha_a}..{sha_b}"), sha_b]);
    }

    #[test]
    fn test_clean_workspace_passes() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());
        std::fs::write(dir.path().join("README.md"), "# Clean repo\n").unwrap();
        run_git(dir.path(), &["add", "README.md"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        let report = evaluate_pre_push_gate(dir.path(), GateMode::Block, &[]).unwrap();
        assert_eq!(report.outcome, GateOutcome::Pass);
        assert!(report.canary_report.is_none());
        assert!(report.secret_leaks.is_empty());
    }

    #[test]
    fn test_corrupt_agent_ooo_blocks_or_warns() {
        let dir = TempDir::new().unwrap();
        let ooo_dir = dir.path().join(".agent-ooo");
        std::fs::create_dir_all(&ooo_dir).unwrap();
        std::fs::write(ooo_dir.join("checkpoint.json"), r#"{"session_id":"s1"}"#).unwrap();

        let blocked = evaluate_pre_push_gate(dir.path(), GateMode::Block, &[]).unwrap();
        assert_eq!(blocked.outcome, GateOutcome::Block);
        assert!(blocked
            .canary_report
            .as_ref()
            .is_some_and(|r| !r.structural_integrity && r.behavioral_status == CanaryStatus::Fail));

        let warned = evaluate_pre_push_gate(dir.path(), GateMode::WarnOnly, &[]).unwrap();
        assert_eq!(warned.outcome, GateOutcome::Warn);

        // Also test corrupt capnp file when both files exist.
        std::fs::write(ooo_dir.join("checkpoint.capnp"), b"not-valid-capnp").unwrap();
        let corrupt_blocked = evaluate_pre_push_gate(dir.path(), GateMode::Block, &[]).unwrap();
        assert_eq!(corrupt_blocked.outcome, GateOutcome::Block);
        assert!(corrupt_blocked
            .canary_report
            .as_ref()
            .is_some_and(|r| !r.structural_integrity));
    }

    #[test]
    fn test_untracked_workspace_secret_blocks_and_masks_preview() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());
        std::fs::write(dir.path().join("clean.txt"), "ok\n").unwrap();
        run_git(dir.path(), &["add", "clean.txt"]);
        run_git(dir.path(), &["commit", "-m", "init"]);

        let dummy_aws = concat!("AKIA", "ABCDEFGHIJKLMNOP");
        std::fs::write(
            dir.path().join(".env.local"),
            format!("AWS_ACCESS_KEY_ID={dummy_aws}\n"),
        )
        .unwrap();
        // Also write a binary file containing NUL bytes to ensure binary skip works.
        let mut blob = b"\x00\x01\x02".to_vec();
        blob.extend_from_slice(dummy_aws.as_bytes());
        std::fs::write(dir.path().join("blob.bin"), blob).unwrap();

        let blocked = evaluate_pre_push_gate(dir.path(), GateMode::Block, &[]).unwrap();
        assert_eq!(blocked.outcome, GateOutcome::Block);
        assert_eq!(blocked.secret_leaks.len(), 1);
        let leak = &blocked.secret_leaks[0];
        assert_eq!(leak.source, ".env.local");
        assert_eq!(leak.line, 1);
        assert_eq!(leak.secret_type, "aws_access_key");
        assert_eq!(leak.masked_preview, "AKIAABC...MNOP");
        assert_ne!(leak.masked_preview, dummy_aws);

        let warned = evaluate_pre_push_gate(dir.path(), GateMode::WarnOnly, &[]).unwrap();
        assert_eq!(warned.outcome, GateOutcome::Warn);
        assert_eq!(warned.secret_leaks.len(), 1);
    }

    #[test]
    fn test_unpushed_git_commit_range_detects_secret() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());

        std::fs::write(dir.path().join("app.rs"), "pub fn init() {}\n").unwrap();
        run_git(dir.path(), &["add", "app.rs"]);
        run_git(dir.path(), &["commit", "-m", "base"]);
        let base_sha = run_git(dir.path(), &["rev-parse", "HEAD"]);

        let dummy_ant = concat!("sk-ant-", "api03-AbCdEfGhIjKlMnOpQrStUvWx");
        std::fs::write(
            dir.path().join("app.rs"),
            format!("pub fn init() {{ let _k = \"{dummy_ant}\"; }}\n"),
        )
        .unwrap();
        run_git(dir.path(), &["add", "app.rs"]);
        run_git(dir.path(), &["commit", "-m", "oops committed secret"]);
        let head_sha = run_git(dir.path(), &["rev-parse", "HEAD"]);

        let range = format!("{base_sha}..{head_sha}");
        let report = evaluate_pre_push_gate(dir.path(), GateMode::Block, &[range]).unwrap();
        assert_eq!(report.outcome, GateOutcome::Block);
        assert_eq!(report.secret_leaks.len(), 1);
        assert_eq!(report.secret_leaks[0].source, "app.rs");
        assert_eq!(report.secret_leaks[0].line, 1);
        assert_eq!(report.secret_leaks[0].secret_type, "anthropic_api_key");
        assert_eq!(report.secret_leaks[0].masked_preview, "sk-ant-...UvWx");

        // Also verify single-SHA git show branch-create range handling.
        let report_show =
            evaluate_pre_push_gate(dir.path(), GateMode::WarnOnly, &[head_sha]).unwrap();
        assert_eq!(report_show.outcome, GateOutcome::Warn);
        assert_eq!(report_show.secret_leaks.len(), 1);
    }
}
