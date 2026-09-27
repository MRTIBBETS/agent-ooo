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

/// Streams a JSONL transcript file and extracts basic metrics without loading the whole file into memory.
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
            // Count roles (e.g., source: "USER", "MODEL", "SYSTEM" or role: "user", "assistant")
            let role = value.get("source")
                .or_else(|| value.get("role"))
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

            // Naive token estimation: words / 0.75
            if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
                let estimated_tokens = (content.len() as f64 / 4.0) as usize;
                metrics.total_input_tokens += estimated_tokens;
            }
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
}
