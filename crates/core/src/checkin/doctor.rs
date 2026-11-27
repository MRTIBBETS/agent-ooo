use crate::checkin::discovery::{self, SessionCandidate};
use crate::checkin::parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const SUPPORTED_HARNESSES: [&str; 7] = [
    "Antigravity",
    "Antigravity IDE",
    "Claude Code",
    "Cursor",
    "Codex CLI",
    "Windsurf",
    "Cline",
];

pub const DEFAULT_MAX_DOCTOR_SESSIONS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DoctorSeverity {
    Pass,
    Warn,
    Fail,
}

impl DoctorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            DoctorSeverity::Pass => "PASS",
            DoctorSeverity::Warn => "WARN",
            DoctorSeverity::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFinding {
    pub code: String,
    pub severity: DoctorSeverity,
    pub message: String,
    pub sample_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDoctorResult {
    pub session_id: String,
    pub agent_harness: String,
    pub transcript_path: String,
    pub status: DoctorSeverity,
    pub total_lines: usize,
    pub valid_json_lines: usize,
    pub malformed_json_lines: usize,
    pub recognized_turns: usize,
    pub unknown_role_turns: usize,
    pub user_turns: usize,
    pub assistant_turns: usize,
    pub total_tool_calls: usize,
    pub api_usage_turns: usize,
    pub estimated_tokens: usize,
    pub model_detected: bool,
    pub findings: Vec<DoctorFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessDoctorSummary {
    pub agent_harness: String,
    pub sessions_checked: usize,
    pub pass_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
    pub status: DoctorSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub overall_status: DoctorSeverity,
    pub sessions_checked: usize,
    pub pass_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
    pub harnesses_discovered: Vec<String>,
    pub harnesses_missing: Vec<String>,
    pub harness_summaries: Vec<HarnessDoctorSummary>,
    pub sessions: Vec<SessionDoctorResult>,
}

/// Diagnoses a single transcript file for schema drift, malformed JSONL, missing telemetry,
/// or zero-tool-call anomalies.
pub fn diagnose_transcript(path: &Path) -> SessionDoctorResult {
    let session_id = discovery::session_id_for(path);
    let agent_harness = discovery::agent_harness_for(path);
    let transcript_path = path.to_string_lossy().to_string();

    let (metrics, diag) = match parser::parse_transcript_with_diagnostics(path) {
        Ok(res) => res,
        Err(err) => {
            return SessionDoctorResult {
                session_id,
                agent_harness,
                transcript_path,
                status: DoctorSeverity::Fail,
                total_lines: 0,
                valid_json_lines: 0,
                malformed_json_lines: 0,
                recognized_turns: 0,
                unknown_role_turns: 0,
                user_turns: 0,
                assistant_turns: 0,
                total_tool_calls: 0,
                api_usage_turns: 0,
                estimated_tokens: 0,
                model_detected: false,
                findings: vec![DoctorFinding {
                    code: "unreadable_file".to_string(),
                    severity: DoctorSeverity::Fail,
                    message: format!("Failed to read transcript: {}", err),
                    sample_keys: Vec::new(),
                }],
            };
        }
    };

    let total_tool_calls = metrics.total_tool_calls_count;
    let recognized_turns = diag.recognized_role_turns;
    let model_detected = metrics.model_id.is_some();

    let mut findings = Vec::new();

    if diag.total_non_empty_lines == 0 {
        findings.push(DoctorFinding {
            code: "empty_transcript".to_string(),
            severity: DoctorSeverity::Fail,
            message: "Transcript contains no non-empty lines.".to_string(),
            sample_keys: Vec::new(),
        });
    }

    if diag.malformed_json_lines > 0 {
        let severity = if diag.valid_json_lines == 0
            || diag.malformed_json_lines * 2 > diag.total_non_empty_lines
        {
            DoctorSeverity::Fail
        } else {
            DoctorSeverity::Warn
        };
        findings.push(DoctorFinding {
            code: "malformed_json_lines".to_string(),
            severity,
            message: format!(
                "{} of {} non-empty lines failed JSON parsing.",
                diag.malformed_json_lines, diag.total_non_empty_lines
            ),
            sample_keys: Vec::new(),
        });
    }

    if diag.valid_json_lines > 0
        && ((metrics.user_turns + metrics.assistant_turns == 0 && total_tool_calls == 0)
            || (diag.unknown_role_turns > 0 && diag.unknown_role_turns == metrics.total_turns))
    {
        let sample_keys: Vec<String> = diag
            .unknown_top_level_keys
            .iter()
            .take(12)
            .cloned()
            .collect();
        findings.push(DoctorFinding {
            code: "unrecognized_schema".to_string(),
            severity: DoctorSeverity::Fail,
            message: format!(
                "Parsed {} valid JSON line(s) but detected 0 user/assistant turns and 0 tool calls (likely harness schema drift).",
                diag.valid_json_lines
            ),
            sample_keys,
        });
    } else if diag.unknown_role_turns > 0 {
        let sample_keys: Vec<String> = diag
            .unknown_top_level_keys
            .iter()
            .take(12)
            .cloned()
            .collect();
        findings.push(DoctorFinding {
            code: "partial_unrecognized_roles".to_string(),
            severity: DoctorSeverity::Warn,
            message: format!(
                "{} of {} turn(s) had unrecognized roles.",
                diag.unknown_role_turns, metrics.total_turns
            ),
            sample_keys,
        });
    }

    if (metrics.assistant_turns >= 2 || metrics.total_turns >= 4) && total_tool_calls == 0 {
        let message = if diag.suspected_tool_keys.is_empty() {
            format!(
                "Session has {} assistant turn(s) ({} total turns) with 0 matched tool calls.",
                metrics.assistant_turns, metrics.total_turns
            )
        } else {
            format!(
                "Session has {} assistant turn(s) ({} total turns) with 0 matched tool calls; suspected unrecognized tool keys: {}.",
                metrics.assistant_turns,
                metrics.total_turns,
                diag.suspected_tool_keys.join(", ")
            )
        };
        findings.push(DoctorFinding {
            code: "zero_tool_calls".to_string(),
            severity: DoctorSeverity::Warn,
            message,
            sample_keys: diag.suspected_tool_keys.clone(),
        });
    }

    if metrics.assistant_turns > 0 && metrics.api_usage_turns == 0 {
        if metrics.total_input_tokens == 0 {
            findings.push(DoctorFinding {
                code: "missing_token_telemetry".to_string(),
                severity: DoctorSeverity::Fail,
                message: format!(
                    "Detected {} assistant turn(s) with 0 API usage turns and 0 estimable tokens.",
                    metrics.assistant_turns
                ),
                sample_keys: Vec::new(),
            });
        } else {
            findings.push(DoctorFinding {
                code: "missing_api_token_telemetry".to_string(),
                severity: DoctorSeverity::Warn,
                message: format!(
                    "Detected {} assistant turn(s) without native API usage fields; falling back to char/4 estimation ({} tokens).",
                    metrics.assistant_turns, metrics.total_input_tokens
                ),
                sample_keys: Vec::new(),
            });
        }
    }

    if metrics.assistant_turns > 0 && !model_detected {
        findings.push(DoctorFinding {
            code: "missing_model_telemetry".to_string(),
            severity: DoctorSeverity::Warn,
            message: format!(
                "Detected {} assistant turn(s) without an identifiable model ID.",
                metrics.assistant_turns
            ),
            sample_keys: Vec::new(),
        });
    }

    let status = findings
        .iter()
        .map(|f| f.severity)
        .max()
        .unwrap_or(DoctorSeverity::Pass);

    SessionDoctorResult {
        session_id,
        agent_harness,
        transcript_path,
        status,
        total_lines: diag.total_non_empty_lines,
        valid_json_lines: diag.valid_json_lines,
        malformed_json_lines: diag.malformed_json_lines,
        recognized_turns,
        unknown_role_turns: diag.unknown_role_turns,
        user_turns: metrics.user_turns,
        assistant_turns: metrics.assistant_turns,
        total_tool_calls,
        api_usage_turns: metrics.api_usage_turns,
        estimated_tokens: metrics.total_input_tokens,
        model_detected,
        findings,
    }
}

/// Diagnoses a single transcript file and wraps it in a `DoctorReport`.
pub fn diagnose_single_to_report(path: &Path) -> DoctorReport {
    let result = diagnose_transcript(path);
    aggregate_results(vec![result])
}

/// Diagnoses a bounded slice of `SessionCandidate` entries and aggregates harness coverage.
pub fn diagnose_candidates(candidates: &[SessionCandidate], max_sessions: usize) -> DoctorReport {
    let mut results = Vec::with_capacity(candidates.len().min(max_sessions));
    for candidate in candidates.iter().take(max_sessions) {
        let mut result = diagnose_transcript(&candidate.path);
        if !candidate.agent_type.is_empty() {
            result.agent_harness = candidate.agent_type.clone();
        }
        results.push(result);
    }
    aggregate_results(results)
}

/// Discovers transcripts across all supported local harnesses and runs the Schema Drift Doctor.
pub fn diagnose_discovered_transcripts() -> DoctorReport {
    let candidates = discovery::discover_recent_transcripts();
    diagnose_candidates(&candidates, DEFAULT_MAX_DOCTOR_SESSIONS)
}

fn aggregate_results(sessions: Vec<SessionDoctorResult>) -> DoctorReport {
    let mut pass_count = 0usize;
    let mut warn_count = 0usize;
    let mut fail_count = 0usize;

    let mut per_harness: HashMap<String, (usize, usize, usize)> = HashMap::new();

    for session in &sessions {
        match session.status {
            DoctorSeverity::Pass => pass_count += 1,
            DoctorSeverity::Warn => warn_count += 1,
            DoctorSeverity::Fail => fail_count += 1,
        }
        let counts = per_harness
            .entry(session.agent_harness.clone())
            .or_insert((0, 0, 0));
        match session.status {
            DoctorSeverity::Pass => counts.0 += 1,
            DoctorSeverity::Warn => counts.1 += 1,
            DoctorSeverity::Fail => counts.2 += 1,
        }
    }

    let overall_status = if fail_count > 0 {
        DoctorSeverity::Fail
    } else if warn_count > 0 {
        DoctorSeverity::Warn
    } else {
        DoctorSeverity::Pass
    };

    let mut harnesses_discovered = Vec::new();
    let mut harnesses_missing = Vec::new();
    let mut harness_summaries = Vec::new();

    for &harness in &SUPPORTED_HARNESSES {
        if let Some((p, w, f)) = per_harness.remove(harness) {
            harnesses_discovered.push(harness.to_string());
            let status = if f > 0 {
                DoctorSeverity::Fail
            } else if w > 0 {
                DoctorSeverity::Warn
            } else {
                DoctorSeverity::Pass
            };
            harness_summaries.push(HarnessDoctorSummary {
                agent_harness: harness.to_string(),
                sessions_checked: p + w + f,
                pass_count: p,
                warn_count: w,
                fail_count: f,
                status,
            });
        } else {
            harnesses_missing.push(harness.to_string());
        }
    }

    let mut extra_harnesses: Vec<(String, (usize, usize, usize))> =
        per_harness.into_iter().collect();
    extra_harnesses.sort_by(|a, b| a.0.cmp(&b.0));
    for (harness, (p, w, f)) in extra_harnesses {
        harnesses_discovered.push(harness.clone());
        let status = if f > 0 {
            DoctorSeverity::Fail
        } else if w > 0 {
            DoctorSeverity::Warn
        } else {
            DoctorSeverity::Pass
        };
        harness_summaries.push(HarnessDoctorSummary {
            agent_harness: harness,
            sessions_checked: p + w + f,
            pass_count: p,
            warn_count: w,
            fail_count: f,
            status,
        });
    }

    DoctorReport {
        overall_status,
        sessions_checked: sessions.len(),
        pass_count,
        warn_count,
        fail_count,
        harnesses_discovered,
        harnesses_missing,
        harness_summaries,
        sessions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::SystemTime;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn healthy_antigravity_and_claude_code_transcripts_pass() {
        let mut ag_file = NamedTempFile::new().unwrap();
        writeln!(
            ag_file,
            r#"{{"source":"USER_EXPLICIT","type":"USER_INPUT","content":"List files"}}"#
        )
        .unwrap();
        writeln!(
            ag_file,
            r#"{{"source":"MODEL","type":"PLANNER_RESPONSE","model":"gemini-2.5-pro","input_tokens":120,"output_tokens":40,"tool_calls":[{{"name":"run_command","args":{{"CommandLine":"ls"}}}}]}}"#
        )
        .unwrap();

        let ag_res = diagnose_transcript(ag_file.path());
        assert_eq!(ag_res.status, DoctorSeverity::Pass);
        assert!(ag_res.findings.is_empty());
        assert_eq!(ag_res.recognized_turns, 2);
        assert_eq!(ag_res.total_tool_calls, 1);
        assert_eq!(ag_res.api_usage_turns, 1);
        assert!(ag_res.model_detected);

        let mut cc_file = NamedTempFile::new().unwrap();
        writeln!(
            cc_file,
            r#"{{"type":"user","message":{{"role":"user","content":"Check tests"}}}}"#
        )
        .unwrap();
        writeln!(
            cc_file,
            r#"{{"type":"assistant","message":{{"role":"assistant","model":"claude-3-5-sonnet","usage":{{"input_tokens":200,"output_tokens":50}},"content":[{{"type":"tool_use","id":"t1","name":"Bash","input":{{"command":"cargo test"}}}}]}}}}"#
        )
        .unwrap();

        let cc_res = diagnose_transcript(cc_file.path());
        assert_eq!(cc_res.status, DoctorSeverity::Pass);
        assert!(cc_res.findings.is_empty());
        assert_eq!(cc_res.total_tool_calls, 1);
        assert_eq!(cc_res.api_usage_turns, 1);
        assert!(cc_res.model_detected);
    }

    #[test]
    fn unrecognized_jsonl_schema_fails_with_sample_keys() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"event_kind":"msg","speaker":"human","payload":{{"body":"hi"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"event_kind":"msg","speaker":"bot","payload":{{"body":"hello"}}}}"#
        )
        .unwrap();

        let report = diagnose_single_to_report(file.path());
        assert_eq!(report.overall_status, DoctorSeverity::Fail);
        assert_eq!(report.fail_count, 1);
        assert_eq!(report.sessions[0].recognized_turns, 0);
        let finding = report.sessions[0]
            .findings
            .iter()
            .find(|f| f.code == "unrecognized_schema")
            .expect("expected unrecognized_schema finding");
        assert_eq!(finding.severity, DoctorSeverity::Fail);
        assert!(finding.sample_keys.contains(&"event_kind".to_string()));
        assert!(finding.sample_keys.contains(&"speaker".to_string()));
        assert!(finding.sample_keys.contains(&"payload".to_string()));
    }

    #[test]
    fn zero_tool_call_anomaly_flags_suspected_tool_keys() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"source":"USER","content":"Inspect workspace"}}"#).unwrap();
        writeln!(
            file,
            r#"{{"source":"MODEL","model":"gpt-4o","input_tokens":50,"output_tokens":20,"function_call":{{"name":"read_file","arguments":"{{}}"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"source":"MODEL","model":"gpt-4o","input_tokens":60,"output_tokens":25,"toolUse":{{"name":"grep"}}}}"#
        )
        .unwrap();

        let res = diagnose_transcript(file.path());
        assert_eq!(res.status, DoctorSeverity::Warn);
        let finding = res
            .findings
            .iter()
            .find(|f| f.code == "zero_tool_calls")
            .expect("expected zero_tool_calls finding");
        assert_eq!(finding.severity, DoctorSeverity::Warn);
        assert!(finding.message.contains("suspected unrecognized tool keys"));
        assert!(finding.sample_keys.contains(&"function_call".to_string()));
        assert!(finding.sample_keys.contains(&"toolUse".to_string()));
    }

    #[test]
    fn missing_token_telemetry_warns_or_fails() {
        // Fallback char/4 estimation exists -> WARN missing_api_token_telemetry
        let mut warn_file = NamedTempFile::new().unwrap();
        writeln!(warn_file, r#"{{"source":"USER","content":"Hello there"}}"#).unwrap();
        writeln!(
            warn_file,
            r#"{{"source":"MODEL","model":"gemini-2.5-flash","content":"Running a tool now.","tool_calls":[{{"name":"ls","args":{{}}}}]}}"#
        )
        .unwrap();

        let warn_res = diagnose_transcript(warn_file.path());
        assert_eq!(warn_res.status, DoctorSeverity::Warn);
        assert!(
            warn_res
                .findings
                .iter()
                .any(|f| f.code == "missing_api_token_telemetry"
                    && f.severity == DoctorSeverity::Warn)
        );

        // Assistant turn with neither API usage nor content text -> FAIL missing_token_telemetry
        let mut fail_file = NamedTempFile::new().unwrap();
        writeln!(
            fail_file,
            r#"{{"source":"MODEL","model":"gemini-2.5-flash","tool_calls":[{{"name":"ls","args":{{}}}}]}}"#
        )
        .unwrap();

        let fail_res = diagnose_transcript(fail_file.path());
        assert_eq!(fail_res.status, DoctorSeverity::Fail);
        assert!(fail_res
            .findings
            .iter()
            .any(|f| f.code == "missing_token_telemetry" && f.severity == DoctorSeverity::Fail));
    }

    #[test]
    fn malformed_jsonl_warns_and_fails_by_ratio() {
        // Majority valid -> WARN
        let mut warn_file = NamedTempFile::new().unwrap();
        writeln!(warn_file, r#"{{"source":"USER","content":"Hi"}}"#).unwrap();
        writeln!(
            warn_file,
            r#"{{"source":"MODEL","model":"gemini-2.5-pro","input_tokens":10,"output_tokens":10,"tool_calls":[{{"name":"ls","args":{{}}}}]}}"#
        )
        .unwrap();
        writeln!(warn_file, "{{corrupt line").unwrap();

        let warn_res = diagnose_transcript(warn_file.path());
        assert_eq!(warn_res.status, DoctorSeverity::Warn);
        assert!(warn_res
            .findings
            .iter()
            .any(|f| f.code == "malformed_json_lines" && f.severity == DoctorSeverity::Warn));

        // Majority malformed -> FAIL
        let mut fail_file = NamedTempFile::new().unwrap();
        writeln!(fail_file, "{{bad 1").unwrap();
        writeln!(fail_file, "{{bad 2").unwrap();
        writeln!(fail_file, r#"{{"source":"USER","content":"Hi"}}"#).unwrap();

        let fail_res = diagnose_transcript(fail_file.path());
        assert_eq!(fail_res.status, DoctorSeverity::Fail);
        assert!(fail_res
            .findings
            .iter()
            .any(|f| f.code == "malformed_json_lines" && f.severity == DoctorSeverity::Fail));
    }

    #[test]
    fn multi_harness_diagnose_candidates_rollup() {
        let dir = TempDir::new().unwrap();

        let ag_path = dir.path().join("ag.jsonl");
        std::fs::write(
            &ag_path,
            concat!(
                r#"{"source":"USER","content":"Go"}"#,
                "\n",
                r#"{"source":"MODEL","model":"gemini-2.5-pro","input_tokens":10,"output_tokens":5,"tool_calls":[{"name":"ls","args":{}}]}"#,
                "\n"
            ),
        )
        .unwrap();

        let cc_path = dir.path().join("cc.jsonl");
        std::fs::write(
            &cc_path,
            concat!(
                r#"{"type":"user","message":{"role":"user","content":"Go"}}"#,
                "\n",
                r#"{"type":"assistant","message":{"role":"assistant","model":"claude-3-5-sonnet","content":[{"type":"text","text":"Only text"}]}}"#,
                "\n"
            ),
        )
        .unwrap();

        let cursor_path = dir.path().join("cursor.jsonl");
        std::fs::write(&cursor_path, r#"{"unknown_key":123}"#).unwrap();

        let candidates = vec![
            SessionCandidate {
                path: ag_path,
                modified: SystemTime::now(),
                agent_type: "Antigravity".to_string(),
                size_bytes: 120,
            },
            SessionCandidate {
                path: cc_path,
                modified: SystemTime::now(),
                agent_type: "Claude Code".to_string(),
                size_bytes: 120,
            },
            SessionCandidate {
                path: cursor_path,
                modified: SystemTime::now(),
                agent_type: "Cursor".to_string(),
                size_bytes: 20,
            },
        ];

        let report = diagnose_candidates(&candidates, 200);
        assert_eq!(report.sessions_checked, 3);
        assert_eq!(report.pass_count, 1);
        assert_eq!(report.warn_count, 1);
        assert_eq!(report.fail_count, 1);
        assert_eq!(report.overall_status, DoctorSeverity::Fail);

        assert_eq!(
            report.harnesses_discovered,
            vec![
                "Antigravity".to_string(),
                "Claude Code".to_string(),
                "Cursor".to_string()
            ]
        );
        assert_eq!(
            report.harnesses_missing,
            vec![
                "Antigravity IDE".to_string(),
                "Codex CLI".to_string(),
                "Windsurf".to_string(),
                "Cline".to_string()
            ]
        );
        assert_eq!(report.harness_summaries.len(), 3);
        assert_eq!(report.harness_summaries[0].status, DoctorSeverity::Pass);
        assert_eq!(report.harness_summaries[1].status, DoctorSeverity::Warn);
        assert_eq!(report.harness_summaries[2].status, DoctorSeverity::Fail);
    }
}
