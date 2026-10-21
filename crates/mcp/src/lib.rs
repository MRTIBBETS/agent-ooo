use agent_ooo_core::checkin::{discovery, parser, triage};
use agent_ooo_core::detox::{pipeline as detox_pipeline, sanitizer};
use agent_ooo_core::discharge::pipeline as discharge_pipeline;
use agent_ooo_core::reset::{pipeline as reset_pipeline, serializer};
use serde_json::{json, Value};
use std::env;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::{Instant, UNIX_EPOCH};

const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26"];

/// Processes a stream of newline-delimited JSON-RPC 2.0 requests from `reader`
/// and writes newline-delimited JSON-RPC 2.0 responses to `writer`.
pub fn serve_stdio<R: BufRead, W: Write>(reader: R, mut writer: W) -> anyhow::Result<()> {
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_jsonrpc_message(trimmed) {
            let serialized = serde_json::to_string(&response)?;
            writeln!(writer, "{serialized}")?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Handles a single JSON-RPC 2.0 message string.
/// Returns `None` for notifications (messages without an `id`).
pub fn handle_jsonrpc_message(raw: &str) -> Option<Value> {
    let parsed: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            return Some(jsonrpc_error(
                Value::Null,
                -32700,
                &format!("Parse error: {e}"),
            ));
        }
    };

    // Ignore notifications (requests without an "id" field)
    let id = parsed.get("id")?.clone();

    let Some(method) = parsed.get("method").and_then(|m| m.as_str()) else {
        return Some(jsonrpc_error(id, -32600, "Invalid Request: missing method"));
    };

    let params = parsed.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_PROTOCOL_VERSION);
            let protocol_version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                DEFAULT_PROTOCOL_VERSION
            };

            Some(jsonrpc_result(
                id,
                json!({
                    "protocolVersion": protocol_version,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "agent-ooo-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ))
        }
        "ping" => Some(jsonrpc_result(id, json!({}))),
        "tools/list" => Some(jsonrpc_result(id, json!({ "tools": tool_definitions() }))),
        "resources/list" => Some(jsonrpc_result(id, json!({ "resources": [] }))),
        "prompts/list" => Some(jsonrpc_result(id, json!({ "prompts": [] }))),
        "tools/call" => Some(handle_tool_call(id, &params)),
        _ => Some(jsonrpc_error(
            id,
            -32601,
            &format!("Method not found: {method}"),
        )),
    }
}

fn jsonrpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn jsonrpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "spa",
            "description": "Run the full 4-step retreat (Checkin, Detox, Reset, Discharge) on a session transcript and write a Cap'n Proto checkpoint and spa report to workspace_dir/.agent-ooo/.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "transcript": {
                        "type": "string",
                        "description": "Optional path to a specific transcript file (.jsonl). Auto-discovers the most recent session if omitted."
                    },
                    "workspace_dir": {
                        "type": "string",
                        "description": "Target directory where .agent-ooo/ checkpoints and spa_report.md will be written. Defaults to the current working directory."
                    }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "checkin",
            "description": "Step 1: Parse transcripts and diagnose context rot, or list discovered sessions across Antigravity, Antigravity IDE, Claude Code, and Cursor.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "transcript": {
                        "type": "string",
                        "description": "Optional path to a specific transcript file (.jsonl). Auto-discovers the most recent session if omitted."
                    },
                    "list_sessions": {
                        "type": "boolean",
                        "description": "When true, lists discovered transcripts ordered from most recent to oldest instead of triaging a single session."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of sessions to return when list_sessions is true (default: 10)."
                    }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "detox",
            "description": "Step 2: Strip ANSI noise, scan and mask leaked secrets (Anthropic, OpenAI, Google, GitHub, AWS, JWT, Bearer, PEM), and offload massive payloads (>512KB). Prefers sibling transcript_full.jsonl for Antigravity sessions and returns masked secret previews.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "transcript": {
                        "type": "string",
                        "description": "Optional path to a specific transcript file (.jsonl) or session directory. If omitted, uses the most recently modified session."
                    },
                    "all_sessions": {
                        "type": "boolean",
                        "description": "When true, scans all discovered sessions across Antigravity, Antigravity IDE, Claude Code, and Cursor for leaked secrets."
                    },
                    "workspace_dir": {
                        "type": "string",
                        "description": "Directory for .agent-ooo/spillover when running on a single session. Defaults to the current working directory."
                    }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "reset",
            "description": "Step 3: Extract git environment registers and write zero-copy Cap'n Proto (.agent-ooo/checkpoint.capnp) and JSON (.agent-ooo/checkpoint.json) checkpoints.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "transcript": {
                        "type": "string",
                        "description": "Optional path to a specific transcript file (.jsonl). Auto-discovers the most recent session if omitted."
                    },
                    "workspace_dir": {
                        "type": "string",
                        "description": "Target directory where .agent-ooo/ checkpoints will be written. Defaults to the current working directory."
                    }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "discharge",
            "description": "Step 4: Verify checkpoint integrity and inspect .agent-ooo/checkpoint.capnp and checkpoint.json, returning session ID, timestamp, git environment registers, and arena counts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace_dir": {
                        "type": "string",
                        "description": "Directory containing .agent-ooo/ (or the .agent-ooo directory itself). Defaults to the current working directory."
                    }
                },
                "additionalProperties": false
            }
        }
    ])
}

fn handle_tool_call(id: Value, params: &Value) -> Value {
    let Some(name) = params.get("name").and_then(|n| n.as_str()) else {
        return jsonrpc_error(id, -32602, "Invalid params: missing tool name");
    };
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let execution = match name {
        "spa" => execute_spa(&args),
        "checkin" => execute_checkin(&args),
        "detox" => execute_detox(&args),
        "reset" => execute_reset(&args),
        "discharge" => execute_discharge(&args),
        _ => {
            return jsonrpc_error(id, -32602, &format!("Unknown tool: {name}"));
        }
    };

    match execution {
        Ok(payload) => {
            let text =
                serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
            jsonrpc_result(
                id,
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": text
                        }
                    ],
                    "isError": false
                }),
            )
        }
        Err(err) => jsonrpc_result(
            id,
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("{err:#}")
                    }
                ],
                "isError": true
            }),
        ),
    }
}

fn execute_checkin(args: &Value) -> anyhow::Result<Value> {
    let list_sessions = args
        .get("list_sessions")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if list_sessions {
        let limit =
            (args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize).clamp(1, 200);
        let candidates = discovery::discover_recent_transcripts();
        let sessions: Vec<Value> = candidates
            .into_iter()
            .take(limit)
            .map(|c| {
                let session_id = discovery::session_id_for(&c.path);
                let scan_target = discovery::resolve_scan_target(&c.path);
                let modified_epoch_secs = c
                    .modified
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                json!({
                    "session_id": session_id,
                    "agent_type": c.agent_type,
                    "path": c.path.to_string_lossy(),
                    "scan_target_path": scan_target.to_string_lossy(),
                    "modified_epoch_secs": modified_epoch_secs,
                    "size_bytes": c.size_bytes,
                })
            })
            .collect();

        return Ok(json!({
            "count": sessions.len(),
            "sessions": sessions
        }));
    }

    let explicit_transcript = args.get("transcript").and_then(|v| v.as_str());
    let path = discovery::resolve_transcript_non_interactive(explicit_transcript)
        .map_err(|e| anyhow::anyhow!(e))?;
    let metrics = parser::parse_transcript(&path)?;
    let receipt = triage::calculate_triage(&path, metrics);
    Ok(serde_json::to_value(receipt)?)
}

fn execute_detox(args: &Value) -> anyhow::Result<Value> {
    let all_sessions = args
        .get("all_sessions")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if all_sessions {
        let candidates = discovery::discover_recent_transcripts();
        let mut reports = Vec::new();
        let mut sessions_with_secrets = 0usize;
        let mut total_occurrences = 0usize;

        for candidate in &candidates {
            let report = sanitizer::scan_transcript_secrets(&candidate.path).map_err(|e| {
                anyhow::anyhow!("Failed scanning {}: {e}", candidate.path.display())
            })?;
            if report.distinct_secrets > 0 {
                sessions_with_secrets += 1;
                total_occurrences += report.total_occurrences;
            }
            reports.push(report);
        }

        return Ok(serde_json::to_value(json!({
            "sessions_scanned": candidates.len(),
            "sessions_with_secrets": sessions_with_secrets,
            "total_occurrences": total_occurrences,
            "reports": reports,
        }))?);
    }

    let explicit_transcript = args.get("transcript").and_then(|v| v.as_str());
    let raw_path = discovery::resolve_transcript_non_interactive(explicit_transcript)
        .map_err(|e| anyhow::anyhow!(e))?;
    let scan_target = discovery::resolve_scan_target(&raw_path);
    let workspace_dir = match args.get("workspace_dir").and_then(|v| v.as_str()) {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };

    let (_, receipt) = detox_pipeline::execute_detox(&scan_target, &workspace_dir)
        .map_err(|e| anyhow::anyhow!(e))?;
    let secrets = sanitizer::scan_transcript_secrets(&raw_path)?;

    Ok(json!({
        "receipt": receipt,
        "secrets": secrets,
    }))
}

fn execute_reset(args: &Value) -> anyhow::Result<Value> {
    let explicit_transcript = args.get("transcript").and_then(|v| v.as_str());
    let path = discovery::resolve_transcript_non_interactive(explicit_transcript)
        .map_err(|e| anyhow::anyhow!(e))?;
    let workspace_dir = match args.get("workspace_dir").and_then(|v| v.as_str()) {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };

    let (mut arena, detox_receipt) =
        detox_pipeline::execute_detox(&path, &workspace_dir).map_err(|e| anyhow::anyhow!(e))?;
    let session_id = discovery::session_id_for(&path);
    let receipt =
        reset_pipeline::execute_reset(&session_id, &workspace_dir, &mut arena, &detox_receipt)?;

    Ok(json!({
        "session_id": session_id,
        "receipt": receipt,
    }))
}

fn execute_discharge(args: &Value) -> anyhow::Result<Value> {
    let dir = match args.get("workspace_dir").and_then(|v| v.as_str()) {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };
    let inspection = serializer::inspect_checkpoint(&dir)?;
    Ok(serde_json::to_value(inspection)?)
}

fn execute_spa(args: &Value) -> anyhow::Result<Value> {
    let start_time = Instant::now();
    let explicit_transcript = args.get("transcript").and_then(|v| v.as_str());
    let path = discovery::resolve_transcript_non_interactive(explicit_transcript)
        .map_err(|e| anyhow::anyhow!(e))?;

    let workspace_dir = match args.get("workspace_dir").and_then(|v| v.as_str()) {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };

    let metrics = parser::parse_transcript(&path)?;
    let triage_receipt = triage::calculate_triage(&path, metrics);
    let (mut arena, detox_receipt) =
        detox_pipeline::execute_detox(&path, &workspace_dir).map_err(|e| anyhow::anyhow!(e))?;
    let secrets = sanitizer::scan_transcript_secrets(&path)?;
    let session_id = discovery::session_id_for(&path);
    let reset_receipt =
        reset_pipeline::execute_reset(&session_id, &workspace_dir, &mut arena, &detox_receipt)?;
    let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    let discharge_cert = discharge_pipeline::execute_discharge(
        &workspace_dir,
        &triage_receipt,
        &detox_receipt,
        &reset_receipt,
        elapsed_ms,
    )?;

    Ok(json!({
        "triage": triage_receipt,
        "detox": detox_receipt,
        "secrets": secrets,
        "reset": reset_receipt,
        "discharge": discharge_cert,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn initialize_and_tools_list_match_cli_commands() {
        let init_resp = handle_jsonrpc_message(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        )
        .unwrap();
        assert_eq!(init_resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "agent-ooo-mcp");

        // Notifications return None
        assert!(handle_jsonrpc_message(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        )
        .is_none());

        let list_resp =
            handle_jsonrpc_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let tools = list_resp["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, vec!["spa", "checkin", "detox", "reset", "discharge"]);
    }

    #[test]
    fn end_to_end_detox_spa_and_discharge_tools() {
        let dir = TempDir::new().unwrap();
        let transcript = dir.path().join("transcript.jsonl");
        let full = dir.path().join("transcript_full.jsonl");

        std::fs::write(&transcript, r#"{"source":"USER","content":"hello"}"#).unwrap();
        let mut f = std::fs::File::create(&full).unwrap();
        writeln!(
            f,
            r#"{{"source":"USER","content":"leaked sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx"}}"#
        )
        .unwrap();

        // 1. Call detox on transcript.jsonl (should scan sibling transcript_full.jsonl)
        let detox_req = json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "detox",
                "arguments": {
                    "transcript": transcript.to_string_lossy(),
                    "workspace_dir": dir.path().to_string_lossy()
                }
            }
        });
        let detox_resp = handle_jsonrpc_message(&detox_req.to_string()).unwrap();
        assert_eq!(detox_resp["result"]["isError"], false);
        let detox_text = detox_resp["result"]["content"][0]["text"].as_str().unwrap();
        let detox_json: Value = serde_json::from_str(detox_text).unwrap();
        assert_eq!(detox_json["secrets"]["distinct_secrets"], 1);
        assert_eq!(
            detox_json["secrets"]["findings"][0]["secret_type"],
            "anthropic_api_key"
        );
        assert_eq!(
            detox_json["secrets"]["findings"][0]["masked_preview"],
            "sk-ant-...UvWx"
        );

        // 2. Call spa to generate checkpoints in dir
        let spa_req = json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "spa",
                "arguments": {
                    "transcript": full.to_string_lossy(),
                    "workspace_dir": dir.path().to_string_lossy()
                }
            }
        });
        let spa_resp = handle_jsonrpc_message(&spa_req.to_string()).unwrap();
        assert_eq!(spa_resp["result"]["isError"], false);

        // 3. Call discharge on dir to inspect checkpoint
        let discharge_req = json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tools/call",
            "params": {
                "name": "discharge",
                "arguments": {
                    "workspace_dir": dir.path().to_string_lossy()
                }
            }
        });
        let discharge_resp = handle_jsonrpc_message(&discharge_req.to_string()).unwrap();
        assert_eq!(discharge_resp["result"]["isError"], false);
        let discharge_text = discharge_resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let discharge_json: Value = serde_json::from_str(discharge_text).unwrap();
        assert_eq!(discharge_json["integrity_passed"], true);
        assert!(discharge_json["checkpoint_capnp_bytes"].as_u64().unwrap() > 0);
    }
}
