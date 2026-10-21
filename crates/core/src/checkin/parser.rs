use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

const MAX_TRACKED_TOOLS: usize = 64;
const MAX_STORED_TOOL_CALLS: usize = 4096;

#[derive(Debug, Default)]
pub struct TranscriptMetrics {
    pub total_turns: usize,
    pub role_distribution: HashMap<String, usize>,
    pub tool_calls: Vec<String>,
    pub total_tool_calls_count: usize,
    pub total_input_tokens: usize, // Estimation based on content length or actual field if present
    pub model_id: Option<String>,
    pub user_turns: usize,
    pub assistant_turns: usize,
    pub failed_tool_calls: usize,
    pub tool_frequency: HashMap<String, usize>,
    pub thinking_bytes: usize,
    pub output_bytes: usize,
}

fn record_tool_call(metrics: &mut TranscriptMetrics, tool_name: &str) {
    metrics.total_tool_calls_count += 1;
    if metrics.tool_calls.len() < MAX_STORED_TOOL_CALLS {
        metrics.tool_calls.push(tool_name.to_string());
    }
    if let Some(count) = metrics.tool_frequency.get_mut(tool_name) {
        *count += 1;
    } else if metrics.tool_frequency.len() < MAX_TRACKED_TOOLS {
        metrics.tool_frequency.insert(tool_name.to_string(), 1);
    }
}

fn output_has_nonzero_exit_code(text: &str) -> bool {
    let marker = "Command exited with code ";
    let Some(idx) = text.find(marker) else {
        return false;
    };
    let rest = &text[idx + marker.len()..];
    let digits: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    if digits.is_empty() {
        return false;
    }
    digits.parse::<i32>().map(|code| code != 0).unwrap_or(false)
}

fn is_failed_tool_block(obj: &Value, top_status_error: bool) -> bool {
    if top_status_error {
        return true;
    }
    if obj.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    if obj
        .get("status")
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("error"))
        .unwrap_or(false)
    {
        return true;
    }
    if let Some(code) = obj
        .get("exit_code")
        .or_else(|| obj.get("returncode"))
        .or_else(|| obj.get("exitCode"))
        .and_then(|v| v.as_i64())
    {
        if code != 0 {
            return true;
        }
    }
    let text = obj
        .get("output")
        .or_else(|| obj.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    output_has_nonzero_exit_code(text)
}

/// Sums the length of all text carried by a Claude Code style content value
/// (a plain string, or an array of `text` / `thinking` / `tool_result` blocks).
fn content_text_len(content: &Value) -> usize {
    match content {
        Value::String(s) => s.len(),
        Value::Array(blocks) => blocks
            .iter()
            .map(|b| {
                let text = b
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                let thinking = b
                    .get("thinking")
                    .and_then(|v| v.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                let nested = b.get("content").map(content_text_len).unwrap_or(0);
                let input = b.get("input").map(|v| v.to_string().len()).unwrap_or(0);
                text + thinking + nested + input
            })
            .sum(),
        _ => 0,
    }
}

/// Streams a JSONL transcript file and extracts basic metrics without loading the whole file into memory.
/// Supports the Antigravity flat schema (`source`, `content`, `tool_calls`) and the
/// Claude Code nested schema (`type`, `message.role`, `message.content[]`).
pub fn parse_transcript<P: AsRef<Path>>(path: P) -> io::Result<TranscriptMetrics> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut metrics = TranscriptMetrics::default();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if value.get("isSidechain").and_then(|v| v.as_bool()) == Some(true) {
                continue;
            }
            metrics.total_turns += 1;
            let message = value.get("message").filter(|m| m.is_object());

            if metrics.model_id.is_none() {
                if let Some(model_str) = message
                    .and_then(|m| m.get("model"))
                    .or_else(|| value.get("model"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    metrics.model_id = Some(model_str.to_string());
                }
            }

            // Count roles (e.g., source: "USER", "MODEL", "SYSTEM" or role: "user", "assistant")
            let role = value
                .get("source")
                .or_else(|| value.get("role"))
                .or_else(|| message.and_then(|m| m.get("role")))
                .or_else(|| value.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_uppercase();

            let is_user_role = matches!(role.as_str(), "USER" | "USER_EXPLICIT" | "USER_INPUT");
            let is_assistant_role =
                matches!(role.as_str(), "MODEL" | "ASSISTANT" | "PLANNER_RESPONSE");

            if is_user_role {
                metrics.user_turns += 1;
            } else if is_assistant_role {
                metrics.assistant_turns += 1;
            }

            *metrics.role_distribution.entry(role).or_insert(0) += 1;

            // Track top-level thinking and assistant output bytes (Antigravity schema)
            let top_thinking_len = value
                .get("thinking")
                .and_then(|v| v.as_str())
                .map(|s| s.len())
                .unwrap_or(0);
            metrics.thinking_bytes += top_thinking_len;

            if is_assistant_role {
                if let Some(c) = value.get("content").and_then(|v| v.as_str()) {
                    metrics.output_bytes += c.len();
                }
            }

            // Extract tool call signatures
            // Check Antigravity format `tool_calls`
            if let Some(tool_calls) = value.get("tool_calls").and_then(|v| v.as_array()) {
                for tool_call in tool_calls {
                    if let Some(tool_name) = tool_call
                        .get("name")
                        .or_else(|| tool_call.get("ToolName"))
                        .and_then(|v| v.as_str())
                    {
                        record_tool_call(&mut metrics, tool_name);
                    } else if let Some(function) =
                        tool_call.get("function").and_then(|v| v.as_object())
                    {
                        if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
                            record_tool_call(&mut metrics, name);
                        }
                    }
                    if let Some(args) = tool_call.get("arguments").or_else(|| tool_call.get("args"))
                    {
                        metrics.output_bytes += args.to_string().len();
                    }
                }
            }

            let top_status_error = value
                .get("status")
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case("error"))
                .unwrap_or(false);

            if let Some(tool_responses) = value.get("tool_responses").and_then(|v| v.as_array()) {
                for resp in tool_responses {
                    if is_failed_tool_block(resp, top_status_error) {
                        metrics.failed_tool_calls += 1;
                    }
                }
            }

            // Check Claude Code format `message.content`
            if let Some(msg_content) = message.and_then(|m| m.get("content")) {
                if let Some(s) = msg_content.as_str() {
                    if is_assistant_role {
                        metrics.output_bytes += s.len();
                    }
                } else if let Some(blocks) = msg_content.as_array() {
                    for block in blocks {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match block_type {
                            "tool_use" => {
                                if let Some(name) = block.get("name").and_then(|v| v.as_str()) {
                                    record_tool_call(&mut metrics, name);
                                }
                                if let Some(input) = block.get("input") {
                                    metrics.output_bytes += input.to_string().len();
                                }
                            }
                            "tool_result" => {
                                if is_failed_tool_block(block, false) {
                                    metrics.failed_tool_calls += 1;
                                }
                            }
                            "thinking" => {
                                if let Some(t) = block.get("thinking").and_then(|v| v.as_str()) {
                                    metrics.thinking_bytes += t.len();
                                }
                            }
                            "text" if is_assistant_role => {
                                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                                    metrics.output_bytes += t.len();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Naive token estimation: characters / 4
            let content_len = match value.get("content").and_then(|v| v.as_str()) {
                Some(content) => content.len() + top_thinking_len,
                None => {
                    message
                        .and_then(|m| m.get("content"))
                        .map(content_text_len)
                        .unwrap_or(0)
                        + top_thinking_len
                }
            };
            metrics.total_input_tokens += content_len / 4;
        } else {
            metrics.total_turns += 1;
        }
    }

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_transcript() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"source": "USER", "content": "Hello"}}"#).unwrap();
        writeln!(
            file,
            r#"{{"source": "MODEL", "tool_calls": [{{"name": "test_tool"}}]}}"#
        )
        .unwrap();

        let metrics = parse_transcript(file.path()).unwrap();
        assert_eq!(metrics.total_turns, 2);
        assert_eq!(metrics.tool_calls.len(), 1);
        assert_eq!(metrics.tool_calls[0], "test_tool");
    }

    #[test]
    fn test_parse_claude_code_transcript() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"type":"file-history-snapshot","snapshot":{{}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"user","message":{{"role":"user","content":"list the files please"}}}}"#
        )
        .unwrap();
        writeln!(file, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Sure."}},{{"type":"tool_use","id":"t1","name":"Bash","input":{{"command":"ls"}}}}]}}}}"#).unwrap();
        writeln!(file, r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t1","content":"a.txt b.txt c.txt d.txt"}}]}}}}"#).unwrap();

        let metrics = parse_transcript(file.path()).unwrap();
        assert_eq!(metrics.total_turns, 4);
        assert_eq!(metrics.tool_calls, vec!["Bash".to_string()]);
        assert_eq!(metrics.role_distribution.get("USER"), Some(&2));
        assert_eq!(metrics.role_distribution.get("ASSISTANT"), Some(&1));
        assert!(metrics.total_input_tokens > 0);
    }
}
