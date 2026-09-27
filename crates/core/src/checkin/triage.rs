use crate::checkin::parser::TranscriptMetrics;
use crate::checkin::TriageReceipt;
use std::path::Path;

/// Analyzes the parsed metrics to detect context rot and cognitive loops.
pub fn calculate_triage(
    transcript_path: &Path,
    metrics: TranscriptMetrics,
) -> TriageReceipt {
    let path_str = transcript_path.to_string_lossy().to_string();
    
    // Extract a session ID from the path (e.g., the parent directory name or filename)
    let session_id = transcript_path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown_session".to_string());

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
        }
    }

    let loop_lock_detected = max_repetition >= 3;

    // 2. Compute degradation score (Rot Index)
    // Heuristic: High token count + many turns + repetition = high degradation
    let base_score = (metrics.total_input_tokens as f64) / 100_000.0;
    let penalty = if loop_lock_detected { 2.0 } else { 1.0 };
    let degradation_score = (base_score * penalty).min(10.0); // Cap at 10.0

    TriageReceipt {
        session_id,
        transcript_path: path_str,
        total_input_tokens: metrics.total_input_tokens,
        loop_lock_detected,
        repetition_count: max_repetition,
        degradation_score,
    }
}
