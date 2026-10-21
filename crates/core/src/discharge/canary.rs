use crate::schema::session_checkpoint;
use capnp::message::ReaderOptions;
use capnp::serialize_packed;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanaryStatus {
    Pass,
    Warn,
    Fail,
}

impl CanaryStatus {
    pub fn as_label(&self) -> &'static str {
        match self {
            CanaryStatus::Pass => "PASS",
            CanaryStatus::Warn => "WARN",
            CanaryStatus::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanaryFinding {
    pub code: String,
    pub status: CanaryStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanaryReport {
    pub structural_integrity: bool,
    pub behavioral_status: CanaryStatus,
    pub findings: Vec<CanaryFinding>,
}

/// Verifies checkpoint integrity: both checkpoint files exist, the Cap'n Proto file is
/// non-empty, and the JSON checkpoint parses with a non-empty session ID.
pub fn verify_behavioral_canary(checkpoint_dir: &Path) -> Result<bool, String> {
    let capnp_len = std::fs::metadata(checkpoint_dir.join("checkpoint.capnp"))
        .map(|m| m.len())
        .map_err(|e| format!("checkpoint.capnp missing: {e}"))?;
    if capnp_len == 0 {
        return Ok(false);
    }

    let json_str = std::fs::read_to_string(checkpoint_dir.join("checkpoint.json"))
        .map_err(|e| format!("checkpoint.json missing: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let has_session = json
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    Ok(has_session)
}

fn is_mutating_file_tool(name: &str) -> bool {
    matches!(
        name,
        "edit_file"
            | "write_file"
            | "Edit"
            | "Write"
            | "MultiEdit"
            | "apply_patch"
            | "create_file"
            | "replace_file_content"
            | "multi_replace_file_content"
    )
}

fn is_verification_tool(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        name,
        "Bash" | "run_command" | "bash" | "execute_command" | "shell"
    ) || lower.contains("cargo test")
        || lower.contains("cargo check")
        || lower.contains("pytest")
        || lower.contains("npm test")
        || lower.contains("pnpm test")
        || lower.contains("go test")
        || lower.contains("tsc")
        || lower.contains("test")
}

fn evaluate_git_diff_canary(workspace_dir: &Path) -> CanaryFinding {
    let child = Command::new("git")
        .args(["diff", "-U0", "HEAD"])
        .current_dir(workspace_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .or_else(|_| {
            Command::new("git")
                .args(["diff", "-U0"])
                .current_dir(workspace_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
        });

    let Ok(mut child) = child else {
        return CanaryFinding {
            code: "stub_or_conflict_injection".to_string(),
            status: CanaryStatus::Pass,
            message: "Git binary unavailable; skipped diff scan.".to_string(),
        };
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.wait();
        return CanaryFinding {
            code: "stub_or_conflict_injection".to_string(),
            status: CanaryStatus::Pass,
            message: "No git diff stdout stream available.".to_string(),
        };
    };

    let reader = BufReader::new(stdout);
    let mut conflict_count = 0usize;
    let mut stub_count = 0usize;
    let mut in_test_file = false;

    for line_res in reader.lines().take(50_000) {
        let Ok(line) = line_res else { break };
        if let Some(path) = line.strip_prefix("+++ b/") {
            let lower = path.to_ascii_lowercase();
            in_test_file = lower.contains("/tests/") || lower.ends_with("_test.rs");
            continue;
        }
        if line.starts_with("+++") || !line.starts_with('+') {
            continue;
        }
        let added = &line[1..];
        let trimmed = added.trim_start();

        if trimmed.starts_with("<<<<<<<")
            || trimmed.starts_with("=======")
            || trimmed.starts_with(">>>>>>>")
        {
            conflict_count += 1;
        }

        if !in_test_file
            && !trimmed.starts_with("//")
            && (trimmed.contains("todo!(")
                || trimmed.contains("unimplemented!(")
                || trimmed.contains("FIXME:")
                || trimmed.contains("XXX:"))
        {
            stub_count += 1;
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    if conflict_count > 0 {
        CanaryFinding {
            code: "stub_or_conflict_injection".to_string(),
            status: CanaryStatus::Fail,
            message: format!(
                "Detected {conflict_count} unresolved git merge conflict markers in workspace diff."
            ),
        }
    } else if stub_count > 0 {
        CanaryFinding {
            code: "stub_or_conflict_injection".to_string(),
            status: CanaryStatus::Warn,
            message: format!(
                "Detected {stub_count} newly added stub or todo markers in workspace diff."
            ),
        }
    } else {
        CanaryFinding {
            code: "stub_or_conflict_injection".to_string(),
            status: CanaryStatus::Pass,
            message: "No git conflict markers or stub macros detected in workspace diff."
                .to_string(),
        }
    }
}

/// Evaluates the 4 deterministic clinical and behavioral canaries against `checkpoint_dir`
/// and `workspace_dir`.
pub fn evaluate_canaries(
    workspace_dir: &Path,
    checkpoint_dir: &Path,
) -> Result<CanaryReport, crate::OooError> {
    let mut findings = Vec::with_capacity(4);

    let capnp_path = checkpoint_dir.join("checkpoint.capnp");
    let json_path = checkpoint_dir.join("checkpoint.json");

    let capnp_meta = std::fs::metadata(&capnp_path)?;
    if capnp_meta.len() == 0 {
        findings.push(CanaryFinding {
            code: "checkpoint_structural_integrity".to_string(),
            status: CanaryStatus::Fail,
            message: "checkpoint.capnp is empty at 0 bytes.".to_string(),
        });
        return Ok(CanaryReport {
            structural_integrity: false,
            behavioral_status: CanaryStatus::Fail,
            findings,
        });
    }

    let json_str = std::fs::read_to_string(&json_path)?;
    let json_val: serde_json::Value = serde_json::from_str(&json_str)?;
    let json_session_id = json_val
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let capnp_file = File::open(&capnp_path)?;
    let mut reader = BufReader::new(capnp_file);
    let message_reader = match serialize_packed::read_message(&mut reader, ReaderOptions::new()) {
        Ok(m) => m,
        Err(e) => {
            findings.push(CanaryFinding {
                code: "checkpoint_structural_integrity".to_string(),
                status: CanaryStatus::Fail,
                message: format!("Failed to decode packed Cap'n Proto checkpoint: {e}"),
            });
            return Ok(CanaryReport {
                structural_integrity: false,
                behavioral_status: CanaryStatus::Fail,
                findings,
            });
        }
    };

    let root = message_reader.get_root::<session_checkpoint::Reader>()?;
    let capnp_session_id = root
        .get_session_id()?
        .to_str()
        .map_err(|e| crate::OooError::General(format!("Invalid UTF-8 in session_id: {e}")))?;

    let event_stream = root.get_event_stream()?;
    let mut hashes_valid = true;
    let bounded_len = (event_stream.len() as usize).min(4096);
    let mut decoded_events: Vec<(String, i32)> = Vec::with_capacity(bounded_len);

    for ev in event_stream.iter().take(4096) {
        let tool_name = ev
            .get_tool_name()?
            .to_str()
            .map_err(|e| crate::OooError::General(format!("Invalid UTF-8 in tool_name: {e}")))?
            .to_string();
        if ev.get_args_hash()?.len() != 32 || ev.get_stdout_hash()?.len() != 32 {
            hashes_valid = false;
        }
        decoded_events.push((tool_name, ev.get_exit_code()));
    }

    let structural_integrity =
        !capnp_session_id.is_empty() && capnp_session_id == json_session_id && hashes_valid;

    if structural_integrity {
        findings.push(CanaryFinding {
            code: "checkpoint_structural_integrity".to_string(),
            status: CanaryStatus::Pass,
            message: format!(
                "Cap'n Proto and JSON checkpoints verified for session '{capnp_session_id}'."
            ),
        });
    } else {
        findings.push(CanaryFinding {
            code: "checkpoint_structural_integrity".to_string(),
            status: CanaryStatus::Fail,
            message:
                "Session ID mismatch or malformed digest slice between Cap'n Proto and JSON checkpoints."
                    .to_string(),
        });
    }

    // Canary 2: unresolved_tool_failure
    let last_overall_failure = decoded_events
        .last()
        .filter(|(_, code)| *code != 0)
        .cloned();
    let last_verify_failure = decoded_events
        .iter()
        .rev()
        .find(|(name, _)| is_verification_tool(name))
        .filter(|(_, code)| *code != 0)
        .cloned();

    if let Some((tool_name, code)) = last_verify_failure.or(last_overall_failure) {
        findings.push(CanaryFinding {
            code: "unresolved_tool_failure".to_string(),
            status: CanaryStatus::Warn,
            message: format!(
                "Session ended with an unresolved failure in '{tool_name}' with exit code {code}."
            ),
        });
    } else {
        findings.push(CanaryFinding {
            code: "unresolved_tool_failure".to_string(),
            status: CanaryStatus::Pass,
            message: "No unresolved build or tool failure at session end.".to_string(),
        });
    }

    // Canary 3: unverified_late_edit
    let last_edit_idx = decoded_events
        .iter()
        .rposition(|(name, _)| is_mutating_file_tool(name));
    let last_verify_idx = decoded_events
        .iter()
        .rposition(|(name, _)| is_verification_tool(name));

    let has_modified_files = root.get_registers()?.iter().any(|r| {
        r.get_key().ok().and_then(|k| k.to_str().ok()) == Some("modified_files")
            && r.get_value()
                .ok()
                .and_then(|v| v.to_str().ok())
                .map(|s| !s.is_empty())
                .unwrap_or(false)
    });

    let unverified_edit = match (last_edit_idx, last_verify_idx) {
        (Some(edit_i), Some(verify_i)) => edit_i > verify_i,
        (Some(_), None) => has_modified_files,
        _ => false,
    };

    if unverified_edit {
        let edit_tool = last_edit_idx
            .and_then(|i| decoded_events.get(i))
            .map(|(n, _)| n.as_str())
            .unwrap_or("edit_file");
        findings.push(CanaryFinding {
            code: "unverified_late_edit".to_string(),
            status: CanaryStatus::Warn,
            message: format!(
                "Mutating tool '{edit_tool}' executed after the last verification run without re-running tests."
            ),
        });
    } else {
        findings.push(CanaryFinding {
            code: "unverified_late_edit".to_string(),
            status: CanaryStatus::Pass,
            message: "No unverified file mutations detected after the last verification run."
                .to_string(),
        });
    }

    // Canary 4: stub_or_conflict_injection
    findings.push(evaluate_git_diff_canary(workspace_dir));

    let behavioral_status = if findings.iter().any(|f| f.status == CanaryStatus::Fail) {
        CanaryStatus::Fail
    } else if findings.iter().any(|f| f.status == CanaryStatus::Warn) {
        CanaryStatus::Warn
    } else {
        CanaryStatus::Pass
    };

    Ok(CanaryReport {
        structural_integrity,
        behavioral_status,
        findings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detox::arena::MemoryArena;
    use crate::detox::DetoxReceipt;
    use crate::reset::analyzer::{ResetAnalysis, ToolEventRecord};
    use crate::reset::registers::EnvironmentRegisters;
    use crate::reset::serializer::write_checkpoints;
    use crate::schema::TrustOrigin;
    use tempfile::TempDir;

    fn write_test_checkpoint(
        dir: &Path,
        events: Vec<ToolEventRecord>,
        modified_files: Vec<String>,
    ) {
        let regs = EnvironmentRegisters {
            git_branch: "main".to_string(),
            git_commit: "1234567".to_string(),
            working_directory: dir.to_string_lossy().to_string(),
            modified_files,
        };
        let arena = MemoryArena::new(10);
        let analysis = ResetAnalysis {
            events,
            constraints: Vec::new(),
            severed_loops_count: 0,
            pruned_turns_count: 0,
        };
        let detox = DetoxReceipt::default();
        write_checkpoints(dir, "sess-canary", &regs, &arena, &analysis, &detox).unwrap();
    }

    #[test]
    fn fails_when_checkpoint_missing() {
        let dir = TempDir::new().unwrap();
        assert!(verify_behavioral_canary(dir.path()).is_err());
    }

    #[test]
    fn passes_with_valid_checkpoint() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("checkpoint.capnp"), b"\x00\x01").unwrap();
        std::fs::write(
            dir.path().join("checkpoint.json"),
            r#"{"session_id":"abc"}"#,
        )
        .unwrap();
        assert_eq!(verify_behavioral_canary(dir.path()), Ok(true));
    }

    #[test]
    fn evaluate_canaries_passes_on_clean_checkpoint() {
        let dir = TempDir::new().unwrap();
        let events = vec![
            ToolEventRecord {
                tool_call_id: "c1".to_string(),
                tool_name: "write_file".to_string(),
                args_hash: [1u8; 32],
                exit_code: 0,
                stdout_hash: [2u8; 32],
                timestamp: 1,
                origin: TrustOrigin::Tool,
            },
            ToolEventRecord {
                tool_call_id: "c2".to_string(),
                tool_name: "cargo test".to_string(),
                args_hash: [3u8; 32],
                exit_code: 0,
                stdout_hash: [4u8; 32],
                timestamp: 2,
                origin: TrustOrigin::Tool,
            },
        ];
        write_test_checkpoint(dir.path(), events, vec!["src/lib.rs".to_string()]);

        let report = evaluate_canaries(dir.path(), &dir.path().join(".agent-ooo")).unwrap();
        assert!(report.structural_integrity);
        assert_eq!(report.behavioral_status, CanaryStatus::Pass);
        assert!(report
            .findings
            .iter()
            .all(|f| f.status == CanaryStatus::Pass));
    }

    #[test]
    fn evaluate_canaries_warns_on_unresolved_tool_failure() {
        let dir = TempDir::new().unwrap();
        let events = vec![ToolEventRecord {
            tool_call_id: "c1".to_string(),
            tool_name: "cargo test".to_string(),
            args_hash: [1u8; 32],
            exit_code: 101,
            stdout_hash: [2u8; 32],
            timestamp: 1,
            origin: TrustOrigin::Tool,
        }];
        write_test_checkpoint(dir.path(), events, Vec::new());

        let report = evaluate_canaries(dir.path(), &dir.path().join(".agent-ooo")).unwrap();
        assert!(report.structural_integrity);
        assert_eq!(report.behavioral_status, CanaryStatus::Warn);
        assert!(report
            .findings
            .iter()
            .any(|f| { f.code == "unresolved_tool_failure" && f.status == CanaryStatus::Warn }));
    }

    #[test]
    fn evaluate_canaries_warns_on_unverified_late_edit() {
        let dir = TempDir::new().unwrap();
        let events = vec![
            ToolEventRecord {
                tool_call_id: "c1".to_string(),
                tool_name: "run_command".to_string(),
                args_hash: [1u8; 32],
                exit_code: 0,
                stdout_hash: [2u8; 32],
                timestamp: 1,
                origin: TrustOrigin::Tool,
            },
            ToolEventRecord {
                tool_call_id: "c2".to_string(),
                tool_name: "write_file".to_string(),
                args_hash: [3u8; 32],
                exit_code: 0,
                stdout_hash: [4u8; 32],
                timestamp: 2,
                origin: TrustOrigin::Tool,
            },
        ];
        write_test_checkpoint(dir.path(), events, vec!["src/main.rs".to_string()]);

        let report = evaluate_canaries(dir.path(), &dir.path().join(".agent-ooo")).unwrap();
        assert!(report.structural_integrity);
        assert_eq!(report.behavioral_status, CanaryStatus::Warn);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "unverified_late_edit" && f.status == CanaryStatus::Warn));
    }

    #[test]
    fn evaluate_canaries_warns_on_added_todo_stub_in_git_diff() {
        let dir = TempDir::new().unwrap();
        let src_file = dir.path().join("lib.rs");
        std::fs::write(&src_file, "pub fn clean() -> u32 { 1 }\n").unwrap();

        let _ = Command::new("git")
            .arg("init")
            .current_dir(dir.path())
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["add", "lib.rs"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Inject uncommitted todo!() stub
        std::fs::write(
            &src_file,
            "pub fn clean() -> u32 { todo!(\"implement later\") }\n",
        )
        .unwrap();

        write_test_checkpoint(dir.path(), Vec::new(), vec!["lib.rs".to_string()]);

        let report = evaluate_canaries(dir.path(), &dir.path().join(".agent-ooo")).unwrap();
        assert_eq!(report.behavioral_status, CanaryStatus::Warn);
        assert!(report
            .findings
            .iter()
            .any(|f| { f.code == "stub_or_conflict_injection" && f.status == CanaryStatus::Warn }));
    }
}
