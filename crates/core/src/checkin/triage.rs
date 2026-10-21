use crate::checkin::parser::TranscriptMetrics;
use crate::checkin::{AgentVitals, ToolUsageStat, TriageReceipt, WellnessTier};
use std::path::Path;

/// Analyzes the parsed metrics to detect context rot, cognitive loops, and guest vitals.
pub fn calculate_triage(transcript_path: &Path, metrics: TranscriptMetrics) -> TriageReceipt {
    let path_str = transcript_path.to_string_lossy().to_string();

    let session_id = crate::checkin::discovery::session_id_for(transcript_path);
    let agent_harness = crate::checkin::discovery::agent_harness_for(transcript_path);
    let model_id = metrics
        .model_id
        .unwrap_or_else(|| "unspecified".to_string());

    // 1. Detect repetitive tool call sequences (Loop Lock)
    let mut max_repetition = 0;
    let mut current_repetition = 0;
    let mut last_tool = String::new();

    for tool in &metrics.tool_calls {
        if tool == &last_tool {
            current_repetition += 1;
            if current_repetition > max_repetition {
                max_repetition = current_repetition;
            }
        } else {
            last_tool = tool.clone();
            current_repetition = 1;
            if max_repetition == 0 {
                max_repetition = 1;
            }
        }
    }

    let loop_lock_detected = max_repetition >= 3;

    // 2. Compute tool error rate and top 3 tools
    let total_tool_calls = metrics.total_tool_calls_count.max(metrics.tool_calls.len());
    let failed_tool_calls = metrics.failed_tool_calls.min(total_tool_calls);
    let tool_error_rate_pct = if total_tool_calls > 0 {
        (failed_tool_calls as f64 / total_tool_calls as f64) * 100.0
    } else {
        0.0
    };

    let mut top_tools: Vec<ToolUsageStat> = metrics
        .tool_frequency
        .into_iter()
        .map(|(tool_name, count)| ToolUsageStat { tool_name, count })
        .collect();
    top_tools.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.tool_name.cmp(&b.tool_name))
    });
    top_tools.truncate(3);

    // 3. Thinking balance and token pace
    let total_reasoning_and_output = metrics.thinking_bytes + metrics.output_bytes;
    let thinking_ratio_pct = if total_reasoning_and_output > 0 {
        (metrics.thinking_bytes as f64 / total_reasoning_and_output as f64) * 100.0
    } else {
        0.0
    };
    let avg_tokens_per_turn = metrics
        .total_input_tokens
        .checked_div(metrics.total_turns)
        .unwrap_or(0);

    // 4. Multi-factor degradation score (Rot Index, 0.0 to 10.0)
    let token_load = (metrics.total_input_tokens as f64 / 100_000.0).min(5.0);
    let repetition_penalty = if max_repetition >= 5 {
        3.0
    } else if max_repetition >= 3 {
        2.0
    } else if max_repetition == 2 {
        0.5
    } else {
        0.0
    };
    let friction_penalty = (tool_error_rate_pct / 25.0).min(2.0);
    let degradation_score = (token_load + repetition_penalty + friction_penalty).min(10.0);

    let condition = if degradation_score <= 1.5 {
        WellnessTier::Elite
    } else if degradation_score <= 4.0 {
        WellnessTier::Fit
    } else if degradation_score <= 6.5 {
        WellnessTier::Fair
    } else {
        WellnessTier::Fatigued
    };

    let vitals = AgentVitals {
        agent_harness,
        model_id,
        total_turns: metrics.total_turns,
        user_turns: metrics.user_turns,
        assistant_turns: metrics.assistant_turns,
        total_tool_calls,
        failed_tool_calls,
        tool_error_rate_pct,
        top_tools,
        thinking_ratio_pct,
        avg_tokens_per_turn,
    };

    TriageReceipt {
        session_id,
        transcript_path: path_str,
        total_input_tokens: metrics.total_input_tokens,
        loop_lock_detected,
        repetition_count: max_repetition,
        degradation_score,
        condition,
        vitals,
    }
}
