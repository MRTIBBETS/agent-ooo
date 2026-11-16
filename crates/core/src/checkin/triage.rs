use crate::checkin::parser::TranscriptMetrics;
use crate::checkin::{AgentVitals, ToolUsageStat, TriageReceipt, WellnessTier};
use std::path::Path;

pub(crate) fn estimate_api_cost_usd(
    model_id: &str,
    input_tokens: u64,
    cache_read_tokens: u64,
    output_tokens: u64,
) -> f64 {
    let lower = model_id.to_ascii_lowercase();
    // Per-1M token reference rates in USD: (uncached_input, cache_read, output)
    let (in_rate, cache_rate, out_rate) = if lower.contains("flash")
        || lower.contains("lite")
        || lower.contains("haiku")
        || lower.contains("4o-mini")
        || lower.contains("o1-mini")
        || lower.contains("o3-mini")
    {
        (0.30, 0.03, 2.50)
    } else if lower.contains("opus") || (lower.contains("o1") && !lower.contains("mini")) {
        (15.00, 1.50, 75.00)
    } else if lower.contains("sonnet")
        || lower.contains("argon")
        || (lower.contains("gpt-4o") && !lower.contains("mini"))
    {
        (3.00, 0.30, 15.00)
    } else {
        // High-reasoning tier default (Carbon or Pro)
        (2.50, 0.25, 15.00)
    };

    (input_tokens as f64 / 1_000_000.0) * in_rate
        + (cache_read_tokens as f64 / 1_000_000.0) * cache_rate
        + (output_tokens as f64 / 1_000_000.0) * out_rate
}

/// Analyzes the parsed metrics to detect context rot, cognitive loops, and guest vitals.
pub fn calculate_triage(transcript_path: &Path, metrics: TranscriptMetrics) -> TriageReceipt {
    let path_str = transcript_path.to_string_lossy().to_string();

    let session_id = crate::checkin::discovery::session_id_for(transcript_path);
    let agent_harness = crate::checkin::discovery::agent_harness_for(transcript_path);
    let model_id = metrics
        .model_id
        .unwrap_or_else(|| "unspecified".to_string());

    // 1. Detect repetitive tool+argument intent sequences (Loop Lock).
    // Parallel reads of different files are normal; identical tool+argument calls
    // repeated consecutively indicate a cognitive loop lock.
    let mut max_repetition = 0;
    let mut current_repetition = 0;
    let mut last_sig: Option<u64> = None;

    if !metrics.tool_call_signatures.is_empty() {
        for &sig in &metrics.tool_call_signatures {
            if last_sig == Some(sig) {
                current_repetition += 1;
                if current_repetition > max_repetition {
                    max_repetition = current_repetition;
                }
            } else {
                last_sig = Some(sig);
                current_repetition = 1;
                if max_repetition == 0 {
                    max_repetition = 1;
                }
            }
        }
    } else {
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

    let estimated_session_cost_usd = estimate_api_cost_usd(
        &model_id,
        metrics.api_input_tokens,
        metrics.api_cache_read_tokens,
        metrics.api_output_tokens,
    );

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
        api_input_tokens: metrics.api_input_tokens,
        api_cache_read_tokens: metrics.api_cache_read_tokens,
        api_output_tokens: metrics.api_output_tokens,
        estimated_session_cost_usd,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_api_cost_usd_tiers() {
        // 1. Flash / Lite / Haiku / Mini models: (0.30, 0.03, 2.50)
        // 1M uncached in ($0.30) + 1M cached in ($0.03) + 1M out ($2.50) = $2.83
        let cost_flash = estimate_api_cost_usd("gemini-2.5-flash", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_flash - 2.83).abs() < 1e-4);

        let cost_haiku = estimate_api_cost_usd("claude-3-haiku", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_haiku - 2.83).abs() < 1e-4);

        let cost_4o_mini = estimate_api_cost_usd("gpt-4o-mini", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_4o_mini - 2.83).abs() < 1e-4);

        let cost_o1_mini = estimate_api_cost_usd("o1-mini", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_o1_mini - 2.83).abs() < 1e-4);

        let cost_o3_mini = estimate_api_cost_usd("o3-mini", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_o3_mini - 2.83).abs() < 1e-4);

        // 2. Opus / o1 (excluding mini): (15.00, 1.50, 75.00)
        // 1M uncached ($15.00) + 1M cached ($1.50) + 1M out ($75.00) = $91.50
        let cost_opus = estimate_api_cost_usd("claude-3-opus", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_opus - 91.50).abs() < 1e-4);

        let cost_o1 = estimate_api_cost_usd("o1-preview", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_o1 - 91.50).abs() < 1e-4);

        // 3. Sonnet / Argon / gpt-4o (excluding mini): (3.00, 0.30, 15.00)
        // 1M uncached ($3.00) + 1M cached ($0.30) + 1M out ($15.00) = $18.30
        let cost_sonnet =
            estimate_api_cost_usd("claude-3-5-sonnet", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_sonnet - 18.30).abs() < 1e-4);

        let cost_argon = estimate_api_cost_usd("Gemini 4 Argon", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_argon - 18.30).abs() < 1e-4);

        let cost_gpt4o = estimate_api_cost_usd("gpt-4o", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_gpt4o - 18.30).abs() < 1e-4);

        // 4. Default / Pro / Carbon: (2.50, 0.25, 15.00)
        // 1M uncached ($2.50) + 1M cached ($0.25) + 1M out ($15.00) = $17.75
        let cost_pro = estimate_api_cost_usd("gemini-2.5-pro", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_pro - 17.75).abs() < 1e-4);

        let cost_default = estimate_api_cost_usd("unspecified", 1_000_000, 1_000_000, 1_000_000);
        assert!((cost_default - 17.75).abs() < 1e-4);
    }
}
