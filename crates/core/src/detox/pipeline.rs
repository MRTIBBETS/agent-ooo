use super::arena::MemoryArena;
use super::sanitizer::sanitize_content;
use super::spillover::manage_spillover;
use super::DetoxReceipt;

use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Executes the detox pipeline on a given transcript path.
pub fn execute_detox(
    transcript_path: &Path,
    workspace_dir: &Path,
) -> Result<(MemoryArena, DetoxReceipt), String> {
    let file = File::open(transcript_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    // Initialize the generational arena (let's say we hold up to 100 turns in transient before evicting)
    let mut arena = MemoryArena::new(100);
    
    let mut receipt = DetoxReceipt {
        raw_bytes: 0,
        sanitized_bytes: 0,
        evicted_tokens: 0,
        spilled_payloads_count: 0,
        redactions_count: 0,
    };

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        receipt.raw_bytes += line.len();

        if let Ok(mut value) = serde_json::from_str::<Value>(&line) {
            // Check if it's a system prompt to pin it
            let source = value
                .get("source")
                .or_else(|| value.get("role"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_uppercase();

            let is_system = source == "SYSTEM" || source == "SYSTEM_PROMPT";

            // Sanitize content
            if let Some(content_val) = value.get_mut("content") {
                if let Some(content_str) = content_val.as_str() {
                    let (sanitized_str, red_count) = sanitize_content(content_str);
                    receipt.redactions_count += red_count;

                    // Check for spillover (large tool outputs)
                    match manage_spillover(&sanitized_str, workspace_dir) {
                        Ok((final_str, spilled)) => {
                            if spilled {
                                receipt.spilled_payloads_count += 1;
                            }
                            *content_val = Value::String(final_str);
                        }
                        Err(e) => {
                            eprintln!("Warning: Spillover failed - {}", e);
                            *content_val = Value::String(sanitized_str);
                        }
                    }
                }
            }

            // Also check tool call payloads if they are embedded differently
            if let Some(tool_responses) = value.get_mut("tool_responses").and_then(|v| v.as_array_mut()) {
                for resp in tool_responses {
                    if let Some(output_val) = resp.get_mut("output") {
                        if let Some(output_str) = output_val.as_str() {
                            let (sanitized_str, red_count) = sanitize_content(output_str);
                            receipt.redactions_count += red_count;

                            if let Ok((final_str, spilled)) = manage_spillover(&sanitized_str, workspace_dir) {
                                if spilled { receipt.spilled_payloads_count += 1; }
                                *output_val = Value::String(final_str);
                            }
                        }
                    }
                }
            }

            // Now serialize to calculate new size
            let new_line = serde_json::to_string(&value).unwrap_or_default();
            receipt.sanitized_bytes += new_line.len();

            // Load into arena
            if is_system {
                arena.pin_gen0(value);
            } else {
                if let Some(evicted) = arena.push_transient(value) {
                    let evicted_str = serde_json::to_string(&evicted).unwrap_or_default();
                    receipt.evicted_tokens += evicted_str.len() / 4; // Naive token estimation
                }
            }
        }
    }

    Ok((arena, receipt))
}
