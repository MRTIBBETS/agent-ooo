use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Default)]
pub struct TranscriptMetrics {
    pub total_turns: usize,
    pub role_distribution: HashMap<String, usize>,
    pub tool_calls: Vec<String>,
    pub total_input_tokens: usize, // Estimation based on content length or actual field if present
}

/// Sums the length of all text carried by a Claude Code style content value
/// (a plain string, or an array of `text` / `thinking` / `tool_result` blocks).
fn content_text_len(content: &Value) -> usize {
    match content {
        Value::String(s) => s.len(),
        Value::Array(blocks) => blocks
            .iter()
            .map(|b| {
                let text = b.get("text").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0);
                let thinking = b.get("thinking").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0);
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

        metrics.total_turns += 1;

        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            let message = value.get("message").filter(|m| m.is_object());

            // Count roles (e.g., source: "USER", "MODEL", "SYSTEM" or role: "user", "assistant")
            let role = value.get("source")
                .or_else(|| value.get("role"))
                .or_else(|| message.and_then(|m| m.get("role")))
                .or_else(|| value.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_uppercase();

            *metrics.role_distribution.entry(role).or_insert(0) += 1;

            // Extract tool call signatures
            // Check Antigravity format `tool_calls`
            if let Some(tool_calls) = value.get("tool_calls").and_then(|v| v.as_array()) {
                for tool_call in tool_calls {
                    if let Some(tool_name) = tool_call.get("name").and_then(|v| v.as_str()) {
                        metrics.tool_calls.push(tool_name.to_string());
                    } else if let Some(function) = tool_call.get("function").and_then(|v| v.as_object()) {
                        if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
                            metrics.tool_calls.push(name.to_string());
                        }
                    }
                }
            }

            // Check Claude Code format `message.content[] { type: "tool_use", name }`
            if let Some(blocks) = message.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
                for block in blocks {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        if let Some(name) = block.get("name").and_then(|v| v.as_str()) {
                            metrics.tool_calls.push(name.to_string());
                        }
                    }
                }
            }

            // Naive token estimation: characters / 4
            let content_len = match value.get("content").and_then(|v| v.as_str()) {
                Some(content) => content.len(),
                None => message.and_then(|m| m.get("content")).map(content_text_len).unwrap_or(0),
            };
            metrics.total_input_tokens += content_len / 4;
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
        writeln!(file, r#"{{"source": "MODEL", "tool_calls": [{{"name": "test_tool"}}]}}"#).unwrap();

        let metrics = parse_transcript(file.path()).unwrap();
        assert_eq!(metrics.total_turns, 2);
        assert_eq!(metrics.tool_calls.len(), 1);
        assert_eq!(metrics.tool_calls[0], "test_tool");
    }

    #[test]
    fn test_parse_claude_code_transcript() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"type":"file-history-snapshot","snapshot":{{}}}}"#).unwrap();
        writeln!(file, r#"{{"type":"user","message":{{"role":"user","content":"list the files please"}}}}"#).unwrap();
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
