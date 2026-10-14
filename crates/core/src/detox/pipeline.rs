use super::arena::MemoryArena;
use super::sanitizer::sanitize_content;
use super::spillover::manage_spillover;
use super::DetoxReceipt;

use serde_json::Value;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Recursively sanitizes Claude Code style content: a plain string, or an array of
/// blocks whose `text`, `thinking`, or nested `content` fields carry strings.
/// Returns (redactions, net change in string length).
fn sanitize_nested(value: &mut Value) -> (usize, i64) {
    match value {
        Value::String(s) => {
            let (clean, red) = sanitize_content(s);
            let d = clean.len() as i64 - s.len() as i64;
            *s = clean;
            (red, d)
        }
        Value::Array(items) => items.iter_mut().fold((0, 0), |(r, d), item| {
            let (r2, d2) = sanitize_nested(item);
            (r + r2, d + d2)
        }),
        Value::Object(map) => {
            let mut totals = (0, 0);
            for key in ["text", "thinking", "content"] {
                if let Some(field) = map.get_mut(key) {
                    let (r, d) = sanitize_nested(field);
                    totals.0 += r;
                    totals.1 += d;
                }
            }
            totals
        }
        _ => (0, 0),
    }
}

/// Executes the detox pipeline on a given transcript path.
pub fn execute_detox(
    transcript_path: &Path,
    workspace_dir: &Path,
) -> Result<(MemoryArena, DetoxReceipt), String> {
    let file = File::open(transcript_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    // Initialize the generational arena (let's say we hold up to 100 turns in transient before evicting)
    let mut arena = MemoryArena::new(100);
    // Sanitized byte size of each transient item, in the same order as the arena ring buffer
    let mut transient_sizes: VecDeque<usize> = VecDeque::with_capacity(101);
    
    let mut receipt = DetoxReceipt {
        raw_bytes: 0,
        sanitized_bytes: 0,
        evicted_tokens: 0,
        spilled_payloads_count: 0,
        redactions_count: 0,
    };

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;

        if let Ok(mut value) = serde_json::from_str::<Value>(&line) {
            // Count raw bytes only for parsed lines so raw and sanitized totals are comparable
            receipt.raw_bytes += line.len();
            // Check if it's a system prompt to pin it
            let source = value
                .get("source")
                .or_else(|| value.get("role"))
                .or_else(|| value.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_uppercase();

            let is_system = source == "SYSTEM" || source == "SYSTEM_PROMPT";

            // Net change in decoded string length for this line (negative = bytes removed)
            let mut delta: i64 = 0;

            // Sanitize content
            if let Some(content_val) = value.get_mut("content") {
                if let Some(content_str) = content_val.as_str() {
                    let original_len = content_str.len() as i64;
                    let (sanitized_str, red_count) = sanitize_content(content_str);
                    receipt.redactions_count += red_count;

                    // Check for spillover (large tool outputs)
                    let final_str = match manage_spillover(&sanitized_str, workspace_dir) {
                        Ok((final_str, spilled)) => {
                            if spilled {
                                receipt.spilled_payloads_count += 1;
                            }
                            final_str
                        }
                        Err(e) => {
                            eprintln!("Warning: Spillover failed - {}", e);
                            sanitized_str
                        }
                    };
                    delta += final_str.len() as i64 - original_len;
                    *content_val = Value::String(final_str);
                }
            }

            // Also check tool call payloads if they are embedded differently
            if let Some(tool_responses) = value.get_mut("tool_responses").and_then(|v| v.as_array_mut()) {
                for resp in tool_responses {
                    if let Some(output_val) = resp.get_mut("output") {
                        if let Some(output_str) = output_val.as_str() {
                            let original_len = output_str.len() as i64;
                            let (sanitized_str, red_count) = sanitize_content(output_str);
                            receipt.redactions_count += red_count;

                            if let Ok((final_str, spilled)) = manage_spillover(&sanitized_str, workspace_dir) {
                                if spilled { receipt.spilled_payloads_count += 1; }
                                delta += final_str.len() as i64 - original_len;
                                *output_val = Value::String(final_str);
                            }
                        }
                    }
                }
            }

            // Claude Code nests text under message.content (string or array of blocks)
            if let Some(message_content) = value.get_mut("message").and_then(|m| m.get_mut("content")) {
                let (red_count, d) = sanitize_nested(message_content);
                receipt.redactions_count += red_count;
                delta += d;
            }

            // Sanitized size = raw line size adjusted by the real change in field lengths.
            // Re-serializing would silently unescape sequences like \u003c and inflate savings.
            let line_sanitized = (line.len() as i64 + delta).max(0) as usize;
            receipt.sanitized_bytes += line_sanitized;

            // Load into arena
            if is_system {
                arena.pin_gen0(value);
            } else {
                transient_sizes.push_back(line_sanitized);
                if arena.push_transient(value).is_some() {
                    let evicted_size = transient_sizes.pop_front().unwrap_or(0);
                    receipt.evicted_tokens += evicted_size / 4; // Naive token estimation
                }
            }
        }
    }

    Ok((arena, receipt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn escaped_characters_are_not_counted_as_savings() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"source":"MODEL","content":"\u003cdiv\u003e plain html \u003c/div\u003e"}}"#).unwrap();
        let dir = TempDir::new().unwrap();

        let (_, receipt) = execute_detox(file.path(), dir.path()).unwrap();
        assert_eq!(receipt.raw_bytes, receipt.sanitized_bytes);
        assert_eq!(receipt.redactions_count, 0);
    }

    #[test]
    fn claude_code_content_is_sanitized() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","content":"\u001b[31mkey=AKIAABCDEFGHIJKLMNOP\u001b[0m"}}]}}}}"#).unwrap();
        writeln!(file, "not json").unwrap();
        let dir = TempDir::new().unwrap();

        let (_, receipt) = execute_detox(file.path(), dir.path()).unwrap();
        assert_eq!(receipt.redactions_count, 1);
        // Unparseable lines are excluded from both totals
        assert!(receipt.raw_bytes > 0);
        assert!(receipt.sanitized_bytes < receipt.raw_bytes + "[REDACTED_SECRET]".len());
    }
}
