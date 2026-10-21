use crate::detox::arena::MemoryArena;
use crate::reset::loop_normalizer::normalize_volatile_data;
use crate::schema::TrustOrigin;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::sync::LazyLock;

const MAX_PENDING_CALLS: usize = 256;
const MAX_EVENTS: usize = 256;
const MAX_CONSTRAINTS: usize = 64;
const MAX_CONSTRAINT_CHARS: usize = 280;
const MAX_NORMALIZE_BYTES: usize = 16 * 1024;
const LOOP_WINDOW_EVENTS: usize = 6;

static EXIT_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:command exited with code|exit (?:code|status):?)\s*(-?\d+)")
        .expect("valid exit code regex")
});

static CONSTRAINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:never|do\s+not|don't|must\s+not|always|under\s+no\s+circumstances|stop\s+doing)\b",
    )
    .expect("valid constraint regex")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolEventRecord {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args_hash: [u8; 32],
    pub exit_code: i32,
    pub stdout_hash: [u8; 32],
    pub timestamp: u64,
    pub origin: TrustOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRecord {
    pub constraint: String,
    pub invalidation_rule: String,
}

#[derive(Debug, Clone, Default)]
pub struct ResetAnalysis {
    pub events: Vec<ToolEventRecord>,
    pub constraints: Vec<ConstraintRecord>,
    pub severed_loops_count: usize,
    pub pruned_turns_count: usize,
}

#[derive(Debug, Clone)]
struct PendingToolCall {
    tool_call_id: String,
    tool_name: String,
    args_hash: [u8; 32],
    timestamp: u64,
    call_turn_idx: usize,
    inline_result: Option<(i32, [u8; 32])>,
}

#[derive(Debug, Clone)]
struct TrackedEvent {
    record: ToolEventRecord,
    turn_indices: Vec<usize>,
}

fn bounded_utf8_prefix(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn hash_normalized(text: &str) -> [u8; 32] {
    let prefix = bounded_utf8_prefix(text, MAX_NORMALIZE_BYTES);
    let normalized = normalize_volatile_data(prefix);
    *blake3::hash(normalized.as_bytes()).as_bytes()
}

fn value_to_compact_str(val: Option<&Value>) -> String {
    match val {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn extract_tool_result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => {
            let mut out = String::new();
            for item in items {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(t);
                } else if let Some(c) = item.get("content").and_then(|v| v.as_str()) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(c);
                }
            }
            out
        }
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn extract_exit_code(obj: &Value, output_text: &str) -> i32 {
    if let Some(code) = obj
        .get("exit_code")
        .or_else(|| obj.get("exitCode"))
        .or_else(|| obj.get("returncode"))
        .and_then(|v| v.as_i64())
    {
        return code as i32;
    }

    if let Some(caps) = EXIT_CODE_RE.captures(output_text) {
        if let Some(m) = caps.get(1) {
            if let Ok(parsed) = m.as_str().parse::<i32>() {
                return parsed;
            }
        }
    }

    let is_error = obj
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || obj
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("ERROR"))
            .unwrap_or(false)
        || output_text.contains("Encountered error")
        || output_text.starts_with("Error:");

    if is_error {
        1
    } else {
        0
    }
}

fn extract_timestamp(turn: &Value, fallback: u64) -> u64 {
    if let Some(ts) = turn.get("timestamp").and_then(|v| v.as_u64()) {
        return ts;
    }
    if let Some(step) = turn.get("step_index").and_then(|v| v.as_u64()) {
        return step;
    }
    fallback
}

fn push_pending(pending: &mut VecDeque<PendingToolCall>, call: PendingToolCall) {
    if pending.len() >= MAX_PENDING_CALLS {
        pending.pop_front();
    }
    pending.push_back(call);
}

fn pop_matching_pending(
    pending: &mut VecDeque<PendingToolCall>,
    call_id: Option<&str>,
    tool_name: Option<&str>,
) -> Option<PendingToolCall> {
    if let Some(id) = call_id.filter(|s| !s.is_empty()) {
        if let Some(pos) = pending.iter().position(|p| p.tool_call_id == id) {
            return pending.remove(pos);
        }
    }
    if let Some(name) = tool_name.filter(|s| !s.is_empty()) {
        if let Some(pos) = pending.iter().position(|p| p.tool_name == name) {
            return pending.remove(pos);
        }
    }
    pending.pop_front()
}

fn flush_inline_pending(
    pending: &mut VecDeque<PendingToolCall>,
    tracked_events: &mut Vec<TrackedEvent>,
) {
    let mut remaining = VecDeque::with_capacity(pending.len());
    for item in pending.drain(..) {
        if let Some((exit_code, stdout_hash)) = item.inline_result {
            tracked_events.push(TrackedEvent {
                record: ToolEventRecord {
                    tool_call_id: item.tool_call_id,
                    tool_name: item.tool_name,
                    args_hash: item.args_hash,
                    exit_code,
                    stdout_hash,
                    timestamp: item.timestamp,
                    origin: TrustOrigin::Tool,
                },
                turn_indices: vec![item.call_turn_idx],
            });
        } else {
            remaining.push_back(item);
        }
    }
    *pending = remaining;
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

fn extract_constraints_from_text(
    text: &str,
    seen_hashes: &mut HashSet<blake3::Hash>,
    constraints: &mut Vec<ConstraintRecord>,
) {
    for line in text.lines() {
        for raw_sentence in line.split(['.', '!', '?', ';']) {
            if constraints.len() >= MAX_CONSTRAINTS {
                return;
            }
            let cleaned = raw_sentence
                .trim()
                .trim_start_matches(['-', '*', '>', '#'])
                .trim();
            if cleaned.len() < 8 {
                continue;
            }
            if CONSTRAINT_RE.is_match(cleaned) {
                let bounded = truncate_chars(cleaned, MAX_CONSTRAINT_CHARS);
                let key_hash = blake3::hash(bounded.to_ascii_lowercase().as_bytes());
                if seen_hashes.insert(key_hash) {
                    constraints.push(ConstraintRecord {
                        constraint: bounded,
                        invalidation_rule: "user_override".to_string(),
                    });
                }
            }
        }
    }
}

fn extract_user_constraints(
    turn: &Value,
    seen_hashes: &mut HashSet<blake3::Hash>,
    constraints: &mut Vec<ConstraintRecord>,
) {
    if constraints.len() >= MAX_CONSTRAINTS {
        return;
    }
    let message = turn.get("message").filter(|m| m.is_object());
    let role = turn
        .get("source")
        .or_else(|| turn.get("role"))
        .or_else(|| message.and_then(|m| m.get("role")))
        .or_else(|| turn.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_uppercase();

    let is_user = role == "USER" || role == "USER_EXPLICIT" || role == "USER_INPUT";
    if !is_user {
        return;
    }

    if let Some(content_str) = turn.get("content").and_then(|v| v.as_str()) {
        extract_constraints_from_text(content_str, seen_hashes, constraints);
    }

    if let Some(msg_content) = message.and_then(|m| m.get("content")) {
        match msg_content {
            Value::String(s) => extract_constraints_from_text(s, seen_hashes, constraints),
            Value::Array(blocks) => {
                for block in blocks {
                    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if block_type == "tool_result" {
                        continue;
                    }
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        extract_constraints_from_text(text, seen_hashes, constraints);
                    }
                }
            }
            _ => {}
        }
    }
}

fn is_loop_match(a: &ToolEventRecord, b: &ToolEventRecord) -> bool {
    if a.tool_name != b.tool_name {
        return false;
    }
    a.args_hash == b.args_hash
        || (a.exit_code != 0 && b.exit_code != 0 && a.stdout_hash == b.stdout_hash)
}

/// Analyzes the turns retained in `MemoryArena`, extracts correlated `ToolEventRecord`s
/// and `ConstraintRecord`s, detects recursive intent-hash loops within a 6-event sliding
/// window, and prunes duplicate intermediate loop turns from `arena`.
pub fn analyze_and_sever_arena(arena: &mut MemoryArena) -> ResetAnalysis {
    let mut seen_constraint_hashes: HashSet<blake3::Hash> = HashSet::new();
    let mut constraints: Vec<ConstraintRecord> = Vec::new();

    for item in arena.gen0() {
        extract_user_constraints(item, &mut seen_constraint_hashes, &mut constraints);
    }

    let mut pending_calls: VecDeque<PendingToolCall> = VecDeque::new();
    let mut tracked_events: Vec<TrackedEvent> = Vec::new();

    for (turn_idx, turn) in arena.transient().iter().enumerate() {
        if turn.get("isSidechain").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }

        extract_user_constraints(turn, &mut seen_constraint_hashes, &mut constraints);

        let ts = extract_timestamp(turn, turn_idx as u64);
        let message = turn.get("message").filter(|m| m.is_object());

        // 1. Claude Code nested `message.content[]` blocks (`tool_use` and `tool_result`)
        if let Some(blocks) = message
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            for (block_idx, block) in blocks.iter().enumerate() {
                match block.get("type").and_then(|v| v.as_str()) {
                    Some("tool_use") => {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("cc-call-{turn_idx}-{block_idx}"));
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown_tool")
                            .to_string();
                        let args_str = value_to_compact_str(block.get("input"));
                        push_pending(
                            &mut pending_calls,
                            PendingToolCall {
                                tool_call_id: id,
                                tool_name: name,
                                args_hash: hash_normalized(&args_str),
                                timestamp: ts,
                                call_turn_idx: turn_idx,
                                inline_result: None,
                            },
                        );
                    }
                    Some("tool_result") => {
                        let ref_id = block.get("tool_use_id").and_then(|v| v.as_str());
                        let out_text = extract_tool_result_text(block.get("content"));
                        let exit_code = extract_exit_code(block, &out_text);
                        let stdout_hash = hash_normalized(&out_text);

                        if let Some(matched) =
                            pop_matching_pending(&mut pending_calls, ref_id, None)
                        {
                            let mut indices = vec![matched.call_turn_idx];
                            if matched.call_turn_idx != turn_idx {
                                indices.push(turn_idx);
                            }
                            tracked_events.push(TrackedEvent {
                                record: ToolEventRecord {
                                    tool_call_id: matched.tool_call_id,
                                    tool_name: matched.tool_name,
                                    args_hash: matched.args_hash,
                                    exit_code,
                                    stdout_hash,
                                    timestamp: ts,
                                    origin: TrustOrigin::Tool,
                                },
                                turn_indices: indices,
                            });
                        } else {
                            tracked_events.push(TrackedEvent {
                                record: ToolEventRecord {
                                    tool_call_id: ref_id
                                        .unwrap_or("unmatched_tool_result")
                                        .to_string(),
                                    tool_name: "unknown_tool".to_string(),
                                    args_hash: [0u8; 32],
                                    exit_code,
                                    stdout_hash,
                                    timestamp: ts,
                                    origin: TrustOrigin::Tool,
                                },
                                turn_indices: vec![turn_idx],
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        // 2. Antigravity `tool_calls`
        let has_tool_responses = turn
            .get("tool_responses")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        if let Some(tool_calls) = turn.get("tool_calls").and_then(|v| v.as_array()) {
            flush_inline_pending(&mut pending_calls, &mut tracked_events);

            let inline_result = if !has_tool_responses
                && (turn.get("content").is_some() || turn.get("status").is_some())
            {
                let out_str = turn.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let code = extract_exit_code(turn, out_str);
                Some((code, hash_normalized(out_str)))
            } else {
                None
            };

            for (call_idx, tc) in tool_calls.iter().enumerate() {
                let id = tc
                    .get("id")
                    .or_else(|| tc.get("call_id"))
                    .or_else(|| tc.get("tool_call_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("ag-call-{turn_idx}-{call_idx}"));

                let name = tc
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        tc.get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str())
                    })
                    .unwrap_or("unknown_tool")
                    .to_string();

                let args_val = tc
                    .get("arguments")
                    .or_else(|| tc.get("args"))
                    .or_else(|| tc.get("input"))
                    .or_else(|| tc.get("function").and_then(|f| f.get("arguments")));
                let args_str = value_to_compact_str(args_val);

                push_pending(
                    &mut pending_calls,
                    PendingToolCall {
                        tool_call_id: id,
                        tool_name: name,
                        args_hash: hash_normalized(&args_str),
                        timestamp: ts,
                        call_turn_idx: turn_idx,
                        inline_result,
                    },
                );
            }
        }

        // 3. Antigravity `tool_responses`
        if let Some(tool_responses) = turn.get("tool_responses").and_then(|v| v.as_array()) {
            for (resp_idx, resp) in tool_responses.iter().enumerate() {
                let ref_id = resp
                    .get("tool_call_id")
                    .or_else(|| resp.get("id"))
                    .or_else(|| resp.get("call_id"))
                    .and_then(|v| v.as_str());
                let resp_name = resp.get("name").and_then(|v| v.as_str());
                let out_str = resp
                    .get("output")
                    .or_else(|| resp.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let exit_code = extract_exit_code(resp, out_str);
                let stdout_hash = hash_normalized(out_str);

                if let Some(matched) = pop_matching_pending(&mut pending_calls, ref_id, resp_name) {
                    let mut indices = vec![matched.call_turn_idx];
                    if matched.call_turn_idx != turn_idx {
                        indices.push(turn_idx);
                    }
                    tracked_events.push(TrackedEvent {
                        record: ToolEventRecord {
                            tool_call_id: matched.tool_call_id,
                            tool_name: matched.tool_name,
                            args_hash: matched.args_hash,
                            exit_code,
                            stdout_hash,
                            timestamp: ts,
                            origin: TrustOrigin::Tool,
                        },
                        turn_indices: indices,
                    });
                } else {
                    tracked_events.push(TrackedEvent {
                        record: ToolEventRecord {
                            tool_call_id: ref_id
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| format!("ag-resp-{turn_idx}-{resp_idx}")),
                            tool_name: resp_name.unwrap_or("unknown_tool").to_string(),
                            args_hash: [0u8; 32],
                            exit_code,
                            stdout_hash,
                            timestamp: ts,
                            origin: TrustOrigin::Tool,
                        },
                        turn_indices: vec![turn_idx],
                    });
                }
            }
        }
    }

    flush_inline_pending(&mut pending_calls, &mut tracked_events);

    // Detect recursive loops within a sliding window of `LOOP_WINDOW_EVENTS` (6) events
    let mut visited = vec![false; tracked_events.len()];
    let mut pruned_event_flags = vec![false; tracked_events.len()];
    let mut severed_loops_count = 0usize;

    for i in 0..tracked_events.len() {
        if visited[i] {
            continue;
        }
        let mut chain = vec![i];
        let mut last_idx = i;

        for j in (i + 1)..tracked_events.len() {
            if j - last_idx > LOOP_WINDOW_EVENTS {
                break;
            }
            if !visited[j]
                && is_loop_match(&tracked_events[last_idx].record, &tracked_events[j].record)
            {
                chain.push(j);
                last_idx = j;
            }
        }

        let is_failing_chain = chain.len() >= 2 && tracked_events[i].record.exit_code != 0;
        let is_repeated_call_chain = chain.len() >= 3;

        if is_failing_chain || is_repeated_call_chain {
            for &idx in &chain {
                visited[idx] = true;
            }
            severed_loops_count += 1;

            // Keep the first attempt (chain[0]) and final resolution/failure (chain[last]),
            // pruning intermediate duplicate attempts.
            if chain.len() >= 3 {
                for &mid_idx in &chain[1..chain.len() - 1] {
                    pruned_event_flags[mid_idx] = true;
                }
            }
        }
    }

    let mut kept_turn_indices: HashSet<usize> = HashSet::new();
    for (idx, ev) in tracked_events.iter().enumerate() {
        if !pruned_event_flags[idx] {
            for &t_idx in &ev.turn_indices {
                kept_turn_indices.insert(t_idx);
            }
        }
    }

    let mut prune_turn_indices: HashSet<usize> = HashSet::new();
    for (idx, ev) in tracked_events.iter().enumerate() {
        if pruned_event_flags[idx] {
            for &t_idx in &ev.turn_indices {
                if !kept_turn_indices.contains(&t_idx) {
                    prune_turn_indices.insert(t_idx);
                }
            }
        }
    }

    let pruned_turns_count = arena.sever_loops(&prune_turn_indices);

    let mut events: Vec<ToolEventRecord> = tracked_events
        .into_iter()
        .enumerate()
        .filter_map(|(idx, ev)| (!pruned_event_flags[idx]).then_some(ev.record))
        .collect();

    if events.len() > MAX_EVENTS {
        let drop_count = events.len() - MAX_EVENTS;
        events.drain(0..drop_count);
    }

    ResetAnalysis {
        events,
        constraints,
        severed_loops_count,
        pruned_turns_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn correlates_claude_code_and_antigravity_tool_events() {
        let mut arena = MemoryArena::new(20);
        // Claude Code tool_use followed by tool_result
        arena.push_transient(json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_01",
                        "name": "Bash",
                        "input": {"command": "cargo test"}
                    }
                ]
            }
        }));
        arena.push_transient(json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_01",
                        "is_error": false,
                        "content": "test result: ok"
                    }
                ]
            }
        }));

        // Antigravity tool_calls + tool_responses
        arena.push_transient(json!({
            "source": "MODEL",
            "tool_calls": [
                {
                    "id": "call_99",
                    "name": "run_command",
                    "arguments": {"CommandLine": "false"}
                }
            ],
            "tool_responses": [
                {
                    "id": "call_99",
                    "name": "run_command",
                    "output": "Command exited with code 2"
                }
            ]
        }));

        let analysis = analyze_and_sever_arena(&mut arena);
        assert_eq!(analysis.events.len(), 2);
        assert_eq!(analysis.events[0].tool_call_id, "toolu_01");
        assert_eq!(analysis.events[0].tool_name, "Bash");
        assert_eq!(analysis.events[0].exit_code, 0);

        assert_eq!(analysis.events[1].tool_call_id, "call_99");
        assert_eq!(analysis.events[1].tool_name, "run_command");
        assert_eq!(analysis.events[1].exit_code, 2);
    }

    #[test]
    fn detects_and_severs_3x_failing_loop_with_volatile_data() {
        let mut arena = MemoryArena::new(20);

        // 3 consecutive failing tool calls whose outputs only differ by timestamps, UUIDs, and temp paths
        let fixtures = [
            (
                "t1",
                "2026-10-20T10:00:01Z",
                "550e8400-e29b-41d4-a716-446655440000",
                "/tmp/build_a1.log",
            ),
            (
                "t2",
                "2026-10-20T10:00:05Z",
                "123e4567-e89b-42d3-a456-426614174000",
                "/tmp/build_b2.log",
            ),
            (
                "t3",
                "2026-10-20T10:00:09Z",
                "987e6543-e21b-43d2-b654-426614174999",
                "/tmp/build_c3.log",
            ),
        ];

        for (id, ts, uuid, tmp) in fixtures {
            arena.push_transient(json!({
                "type": "assistant",
                "message": {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": id,
                        "name": "Bash",
                        "input": {"command": format!("cargo build --log {tmp}")}
                    }]
                }
            }));
            arena.push_transient(json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": id,
                        "is_error": true,
                        "content": format!("[{ts}] Error in request {uuid}: failed writing {tmp}")
                    }]
                }
            }));
        }

        assert_eq!(arena.transient().len(), 6);
        let analysis = analyze_and_sever_arena(&mut arena);

        assert_eq!(analysis.severed_loops_count, 1);
        // Middle attempt (2 turns: assistant tool_use + user tool_result) is pruned,
        // leaving first attempt (2 turns) and final failure (2 turns) = 4 turns.
        assert_eq!(analysis.pruned_turns_count, 2);
        assert_eq!(arena.transient().len(), 4);
        assert_eq!(analysis.events.len(), 2);
        assert_eq!(analysis.events[0].tool_call_id, "t1");
        assert_eq!(analysis.events[1].tool_call_id, "t3");
    }

    #[test]
    fn extracts_user_negative_constraints_and_ignores_tool_results() {
        let mut arena = MemoryArena::new(20);
        arena.push_transient(json!({
            "source": "USER",
            "content": "Hello there. Never use unwrap() in core. Always use streaming buffers! Do not modify crates/cli."
        }));
        // Duplicate constraint in a later turn should be deduplicated
        arena.push_transient(json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {"type": "text", "text": "never use unwrap() in core."},
                    {"type": "tool_result", "tool_use_id": "t1", "content": "Never match inside tool_result"}
                ]
            }
        }));

        let analysis = analyze_and_sever_arena(&mut arena);
        let texts: Vec<&str> = analysis
            .constraints
            .iter()
            .map(|c| c.constraint.as_str())
            .collect();
        assert_eq!(
            texts,
            vec![
                "Never use unwrap() in core",
                "Always use streaming buffers",
                "Do not modify crates/cli"
            ]
        );
        assert!(analysis
            .constraints
            .iter()
            .all(|c| c.invalidation_rule == "user_override"));
    }
}
