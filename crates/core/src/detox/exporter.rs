use crate::checkin::discovery;
use crate::detox::sanitizer::sanitize_content;
use crate::detox::spillover::manage_spillover_with_handle;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const MAX_PINNED_GEN0: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// Preserves the original JSONL schema line-by-line, with ANSI stripped,
    /// secrets masked, sidechains skipped, and >512KB payloads replaced with CAS stubs.
    Jsonl,
    /// Normalized, lightweight turn-by-turn JSONL (`{"role": "user"|"assistant"|"tool", "content": "...", "tool_name": "..."}`)
    /// suitable for external LLM review tools (`--history-loader`) and context handoffs.
    Normalized,
    /// Compact human/LLM-readable Markdown handoff summary combining `.agent-ooo/checkpoint.json`
    /// registers, negative constraints, and the retained Generation 0 + transient turns.
    HandoffMarkdown,
}

impl ExportFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportFormat::Jsonl => "jsonl",
            ExportFormat::Normalized => "normalized",
            ExportFormat::HandoffMarkdown => "handoff",
        }
    }

    pub fn parse_label(label: &str) -> Result<Self, crate::OooError> {
        match label.trim().to_ascii_lowercase().as_str() {
            "jsonl" | "raw" => Ok(ExportFormat::Jsonl),
            "normalized" | "norm" => Ok(ExportFormat::Normalized),
            "handoff" | "markdown" | "md" | "handoff_markdown" => Ok(ExportFormat::HandoffMarkdown),
            other => Err(crate::OooError::General(format!(
                "Unsupported export format '{other}'. Expected 'jsonl', 'normalized', or 'handoff'."
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportReceipt {
    pub session_id: String,
    pub format: String,
    pub lines_exported: usize,
    pub redactions_count: usize,
    pub spilled_payloads_count: usize,
    pub bytes_written: usize,
}

fn is_structural_key(key: &str) -> bool {
    matches!(
        key,
        "id" | "tool_use_id"
            | "uuid"
            | "parentUuid"
            | "sessionId"
            | "type"
            | "role"
            | "source"
            | "status"
            | "created_at"
            | "timestamp"
    )
}

/// Recursively strips ANSI codes, masks leaked credentials, and offloads payloads >512 KB
/// across all non-structural string fields in `value`.
fn sanitize_value_in_place(
    value: &mut Value,
    workspace_dir: &Path,
    redactions_count: &mut usize,
    spilled_payloads_count: &mut usize,
) {
    match value {
        Value::String(s) => {
            let (clean, red) = sanitize_content(s);
            *redactions_count += red;
            let final_str = match manage_spillover_with_handle(&clean, workspace_dir) {
                Ok((spilled_str, Some(_))) => {
                    *spilled_payloads_count += 1;
                    spilled_str
                }
                Ok((unchanged, None)) => unchanged,
                Err(_) => clean,
            };
            *s = final_str;
        }
        Value::Array(items) => {
            for item in items {
                sanitize_value_in_place(
                    item,
                    workspace_dir,
                    redactions_count,
                    spilled_payloads_count,
                );
            }
        }
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if is_structural_key(k) {
                    continue;
                }
                sanitize_value_in_place(v, workspace_dir, redactions_count, spilled_payloads_count);
            }
        }
        _ => {}
    }
}

fn is_system_turn(value: &Value) -> bool {
    let raw_role = value
        .get("source")
        .or_else(|| value.get("role"))
        .or_else(|| value.get("message").and_then(|m| m.get("role")))
        .or_else(|| value.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    matches!(
        raw_role.to_ascii_uppercase().as_str(),
        "SYSTEM" | "SYSTEM_PROMPT"
    )
}

#[derive(Debug, Clone)]
struct NormalizedTurn {
    role: String,
    content: String,
    tool_name: Option<String>,
}

fn extract_normalized_turns(value: &Value) -> Vec<NormalizedTurn> {
    let mut out = Vec::new();

    // 1. Claude Code schema: `message.content` array or string
    if let Some(msg) = value.get("message") {
        let base_role = msg
            .get("role")
            .or_else(|| value.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("user")
            .to_ascii_lowercase();

        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
            if !text.is_empty() {
                out.push(NormalizedTurn {
                    role: base_role,
                    content: text.to_string(),
                    tool_name: None,
                });
            }
            return out;
        }

        if let Some(blocks) = msg.get("content").and_then(|c| c.as_array()) {
            let mut text_parts: Vec<String> = Vec::new();
            let mut first_tool_use: Option<String> = None;

            for block in blocks {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                            if !t.is_empty() {
                                text_parts.push(t.to_string());
                            }
                        }
                    }
                    "thinking" => {
                        if let Some(t) = block.get("thinking").and_then(|v| v.as_str()) {
                            if !t.is_empty() {
                                text_parts.push(t.to_string());
                            }
                        }
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool")
                            .to_string();
                        if first_tool_use.is_none() {
                            first_tool_use = Some(name.clone());
                        }
                        if let Some(input) = block.get("input") {
                            text_parts.push(format!("[tool_use: {name}] {input}"));
                        }
                    }
                    "tool_result" => {
                        let content_str = match block.get("content") {
                            Some(Value::String(s)) => s.clone(),
                            Some(other) => other.to_string(),
                            None => String::new(),
                        };
                        let tool_id = block
                            .get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        out.push(NormalizedTurn {
                            role: "tool".to_string(),
                            content: content_str,
                            tool_name: tool_id,
                        });
                    }
                    _ => {}
                }
            }

            if !text_parts.is_empty() {
                out.push(NormalizedTurn {
                    role: base_role,
                    content: text_parts.join("\n"),
                    tool_name: first_tool_use,
                });
            }
            return out;
        }
    }

    // 2. Antigravity / generic JSONL schema
    let raw_source = value
        .get("source")
        .or_else(|| value.get("role"))
        .or_else(|| value.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("user");

    let role = match raw_source.to_ascii_uppercase().as_str() {
        "SYSTEM" | "SYSTEM_PROMPT" => "system",
        "USER" | "USER_EXPLICIT" | "USER_INPUT" => "user",
        "MODEL" | "ASSISTANT" | "PLANNER_RESPONSE" => "assistant",
        "TOOL" | "TOOL_RESPONSE" => "tool",
        _ => "user",
    }
    .to_string();

    let mut content_parts: Vec<String> = Vec::new();
    if let Some(c) = value.get("content").and_then(|v| v.as_str()) {
        if !c.is_empty() {
            content_parts.push(c.to_string());
        }
    }
    if let Some(t) = value.get("thinking").and_then(|v| v.as_str()) {
        if !t.is_empty() {
            content_parts.push(t.to_string());
        }
    }

    let mut primary_tool: Option<String> = None;
    if let Some(calls) = value.get("tool_calls").and_then(|v| v.as_array()) {
        for call in calls {
            let name = call
                .get("name")
                .or_else(|| call.get("ToolName"))
                .and_then(|v| v.as_str())
                .unwrap_or("tool")
                .to_string();
            if primary_tool.is_none() {
                primary_tool = Some(name.clone());
            }
            if let Some(args) = call.get("arguments").or_else(|| call.get("args")) {
                content_parts.push(format!("[tool_call: {name}] {args}"));
            }
        }
    }

    if !content_parts.is_empty() || primary_tool.is_some() {
        out.push(NormalizedTurn {
            role: role.clone(),
            content: content_parts.join("\n"),
            tool_name: primary_tool,
        });
    }

    if let Some(responses) = value.get("tool_responses").and_then(|v| v.as_array()) {
        for resp in responses {
            let name = resp
                .get("name")
                .or_else(|| resp.get("tool_name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let output_str = resp
                .get("output")
                .or_else(|| resp.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            out.push(NormalizedTurn {
                role: "tool".to_string(),
                content: output_str,
                tool_name: name,
            });
        }
    }

    out
}

fn write_bytes_counted<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<usize, crate::OooError> {
    writer.write_all(bytes)?;
    Ok(bytes.len())
}

fn write_turn_jsonl<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(usize, usize), crate::OooError> {
    let serialized = serde_json::to_string(value)?;
    let mut written = write_bytes_counted(writer, serialized.as_bytes())?;
    written += write_bytes_counted(writer, b"\n")?;
    Ok((1, written))
}

fn write_turn_normalized<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(usize, usize), crate::OooError> {
    let turns = extract_normalized_turns(value);
    let mut lines_count = 0usize;
    let mut bytes_count = 0usize;

    for turn in turns {
        let obj = json!({
            "role": turn.role,
            "content": turn.content,
            "tool_name": turn.tool_name.unwrap_or_default(),
        });
        let line = serde_json::to_string(&obj)?;
        bytes_count += write_bytes_counted(writer, line.as_bytes())?;
        bytes_count += write_bytes_counted(writer, b"\n")?;
        lines_count += 1;
    }

    Ok((lines_count, bytes_count))
}

fn write_handoff_header<W: Write>(
    session_id: &str,
    workspace_dir: &Path,
    writer: &mut W,
) -> Result<usize, crate::OooError> {
    let spa_json_path = workspace_dir.join(".agent-ooo").join("spa_report.json");
    let status = std::fs::read_to_string(&spa_json_path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| {
            v.get("status")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "Fit".to_string());

    let mut header = format!(
        "---\ntitle: Agent OOO Spa Report\npurpose: Refreshed session handoff and workspace state\nsession: {session_id}\nstatus: {status}\n---\n\n# Agent OOO Spa Report\n\n**Session ID:** `{session_id}`\n"
    );

    let checkpoint_json_path = workspace_dir.join(".agent-ooo").join("checkpoint.json");
    if let Ok(raw) = std::fs::read_to_string(&checkpoint_json_path) {
        if let Ok(cp) = serde_json::from_str::<Value>(&raw) {
            if let Some(regs) = cp.get("registers") {
                header.push_str("\n## 1. Reset: workspace registers\n\n");
                let branch = regs
                    .get("git_branch")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let commit = regs
                    .get("git_commit")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let cwd = regs
                    .get("working_directory")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                header.push_str(&format!(
                    "* **Git branch:** `{branch}` at commit `{commit}`\n"
                ));
                header.push_str(&format!("* **Working directory:** `{cwd}`\n"));
                if let Some(files) = regs.get("modified_files").and_then(|v| v.as_array()) {
                    let list: Vec<&str> = files.iter().filter_map(|f| f.as_str()).collect();
                    if !list.is_empty() {
                        header.push_str(&format!("* **Modified files:** `{}`\n", list.join(", ")));
                    }
                }
            }

            if let Some(constraints) = cp.get("constraints").and_then(|v| v.as_array()) {
                let rules: Vec<&str> = constraints.iter().filter_map(|c| c.as_str()).collect();
                if !rules.is_empty() {
                    header.push_str("\n## 2. Reset: pinned user rules\n\n");
                    for rule in rules {
                        let (clean_rule, _) = sanitize_content(rule);
                        let clean_rule = clean_rule.replace(['(', ')'], "");
                        header.push_str(&format!("* {clean_rule}\n"));
                    }
                }
            }
        }
    }

    header.push_str("\n## 3. Refreshed session history\n\n");
    write_bytes_counted(writer, header.as_bytes())
}

fn write_turn_handoff<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(usize, usize), crate::OooError> {
    let turns = extract_normalized_turns(value);
    let mut count = 0usize;
    let mut bytes = 0usize;

    for turn in turns {
        let heading = match turn.tool_name.as_deref() {
            Some(tool) if !tool.is_empty() => {
                format!("### {} via `{}`\n\n", turn.role.to_ascii_uppercase(), tool)
            }
            _ => format!("### {}\n\n", turn.role.to_ascii_uppercase()),
        };
        bytes += write_bytes_counted(writer, heading.as_bytes())?;
        bytes += write_bytes_counted(writer, turn.content.as_bytes())?;
        bytes += write_bytes_counted(writer, b"\n\n")?;
        count += 1;
    }

    Ok((count, bytes))
}

fn write_turn_by_format<W: Write>(
    value: &Value,
    format: ExportFormat,
    writer: &mut W,
) -> Result<(usize, usize), crate::OooError> {
    match format {
        ExportFormat::Jsonl => write_turn_jsonl(value, writer),
        ExportFormat::Normalized => write_turn_normalized(value, writer),
        ExportFormat::HandoffMarkdown => write_turn_handoff(value, writer),
    }
}

/// Streams and sanitizes `transcript_path` in constant memory (or a bounded ring buffer
/// when `max_turns` is `Some(n)`) and writes the scrubbed output to `writer`.
pub fn export_transcript<W: Write>(
    transcript_path: &Path,
    workspace_dir: &Path,
    format: ExportFormat,
    max_turns: Option<usize>,
    writer: &mut W,
) -> Result<ExportReceipt, crate::OooError> {
    let session_id = discovery::session_id_for(transcript_path);
    let scan_target = discovery::resolve_scan_target(transcript_path);

    let file = File::open(&scan_target)?;
    let reader = BufReader::new(file);

    let mut redactions_count = 0usize;
    let mut spilled_payloads_count = 0usize;
    let mut lines_exported = 0usize;
    let mut bytes_written = 0usize;

    if let Some(capacity) = max_turns {
        let mut gen0_turns: Vec<Value> = Vec::new();
        let mut transient_ring: VecDeque<Value> = VecDeque::with_capacity(capacity.clamp(1, 2048));

        for line_res in reader.lines() {
            let line = line_res?;
            let Ok(mut value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if value.get("isSidechain").and_then(|v| v.as_bool()) == Some(true) {
                continue;
            }

            sanitize_value_in_place(
                &mut value,
                workspace_dir,
                &mut redactions_count,
                &mut spilled_payloads_count,
            );

            if is_system_turn(&value) {
                if gen0_turns.len() < MAX_PINNED_GEN0 {
                    gen0_turns.push(value);
                }
            } else if capacity > 0 {
                if transient_ring.len() >= capacity {
                    transient_ring.pop_front();
                }
                transient_ring.push_back(value);
            }
        }

        if format == ExportFormat::HandoffMarkdown {
            bytes_written += write_handoff_header(&session_id, workspace_dir, writer)?;
        }

        for turn in gen0_turns.iter().chain(transient_ring.iter()) {
            let (l, b) = write_turn_by_format(turn, format, writer)?;
            lines_exported += l;
            bytes_written += b;
        }
    } else {
        if format == ExportFormat::HandoffMarkdown {
            bytes_written += write_handoff_header(&session_id, workspace_dir, writer)?;
        }

        for line_res in reader.lines() {
            let line = line_res?;
            let Ok(mut value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if value.get("isSidechain").and_then(|v| v.as_bool()) == Some(true) {
                continue;
            }

            sanitize_value_in_place(
                &mut value,
                workspace_dir,
                &mut redactions_count,
                &mut spilled_payloads_count,
            );

            let (l, b) = write_turn_by_format(&value, format, writer)?;
            lines_exported += l;
            bytes_written += b;
        }
    }

    writer.flush()?;

    Ok(ExportReceipt {
        session_id,
        format: format.as_str().to_string(),
        lines_exported,
        redactions_count,
        spilled_payloads_count,
        bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn export_jsonl_scrubs_secrets_ansi_sidechains_and_spills_large_payloads() {
        let mut file = NamedTempFile::new().unwrap();
        // 1. Sidechain entry with a secret (must be skipped completely)
        writeln!(
            file,
            r#"{{"isSidechain":true,"source":"USER","content":"AKIAIOSFODNN7EXAMPLE"}}"#
        )
        .unwrap();
        // 2. Main session user turn with AWS key and ANSI escape sequence
        writeln!(
            file,
            r#"{{"source":"USER","content":"\u001b[31mKey is AKIAABCDEFGHIJKLMNOP\u001b[0m"}}"#
        )
        .unwrap();
        // 3. Oversized 600KB payload
        let big_payload = "x".repeat(600 * 1024);
        writeln!(file, r#"{{"source":"MODEL","content":"{big_payload}"}}"#).unwrap();

        let dir = TempDir::new().unwrap();
        let mut out_buf: Vec<u8> = Vec::new();

        let receipt = export_transcript(
            file.path(),
            dir.path(),
            ExportFormat::Jsonl,
            None,
            &mut out_buf,
        )
        .unwrap();

        let exported_str = String::from_utf8(out_buf).unwrap();
        assert_eq!(receipt.lines_exported, 2);
        assert_eq!(receipt.redactions_count, 1);
        assert_eq!(receipt.spilled_payloads_count, 1);
        assert!(receipt.bytes_written > 0);
        assert!(exported_str.contains("[REDACTED_SECRET]"));
        assert!(!exported_str.contains("AKIAABCDEFGHIJKLMNOP"));
        assert!(!exported_str.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!exported_str.contains("\x1b[31m"));
        assert!(exported_str.contains("[Detox Spillover: blake3:"));
    }

    #[test]
    fn export_normalized_and_handoff_support_antigravity_and_claude_code() {
        let mut file = NamedTempFile::new().unwrap();
        // System turn
        writeln!(
            file,
            r#"{{"source":"SYSTEM","content":"Never use unwrap in library code."}}"#
        )
        .unwrap();
        // Antigravity turn
        writeln!(file, r#"{{"source":"USER","content":"Fix the parser"}}"#).unwrap();
        // Claude Code assistant + tool_use turn
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Running cargo test"}},{{"type":"tool_use","id":"toolu_1","name":"Bash","input":{{"command":"cargo test"}}}}]}}}}"#
        )
        .unwrap();
        // Claude Code tool_result turn
        writeln!(
            file,
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"test result: ok"}}]}}}}"#
        )
        .unwrap();

        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".agent-ooo")).unwrap();
        std::fs::write(
            dir.path().join(".agent-ooo/checkpoint.json"),
            r#"{"session_id":"sess-1","constraints":["Never use unwrap in library code."],"registers":{"git_branch":"main","git_commit":"abc1234","working_directory":"/workspace","modified_files":["src/lib.rs"]}}"#,
        )
        .unwrap();

        // Test Normalized with max_turns = Some(2) (retains system turn + last 2 transient turns)
        let mut norm_buf: Vec<u8> = Vec::new();
        let norm_receipt = export_transcript(
            file.path(),
            dir.path(),
            ExportFormat::Normalized,
            Some(2),
            &mut norm_buf,
        )
        .unwrap();
        let norm_str = String::from_utf8(norm_buf).unwrap();
        assert_eq!(norm_receipt.lines_exported, 3);
        assert!(norm_str.contains(r#""role":"system""#));
        assert!(!norm_str.contains("Fix the parser")); // evicted by max_turns=2
        assert!(norm_str.contains(r#""tool_name":"Bash""#));
        assert!(norm_str.contains(r#""role":"tool""#));

        // Test HandoffMarkdown
        let mut md_buf: Vec<u8> = Vec::new();
        let md_receipt = export_transcript(
            file.path(),
            dir.path(),
            ExportFormat::HandoffMarkdown,
            None,
            &mut md_buf,
        )
        .unwrap();
        let md_str = String::from_utf8(md_buf).unwrap();
        assert_eq!(md_receipt.lines_exported, 4);
        assert!(md_str.contains("title: Agent OOO Spa Report"));
        assert!(md_str.contains("# Agent OOO Spa Report"));
        assert!(md_str.contains("* **Git branch:** `main` at commit `abc1234`"));
        assert!(md_str.contains("## 2. Reset: pinned user rules"));
        assert!(md_str.contains("* Never use unwrap in library code."));
        assert!(md_str.contains("### ASSISTANT via `Bash`"));
        assert!(!md_str.contains('('));
        assert!(!md_str.contains(')'));
        assert!(!md_str.contains('\u{2014}'));
    }
}
