use crate::checkin::{TriageReceipt, WellnessTier};
use crate::detox::DetoxReceipt;
use crate::discharge::canary::{CanaryReport, CanaryStatus};
use crate::discharge::DischargeCertificate;
use crate::reset::ResetReceipt;

use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn generate_spa_report(
    base_dir: &Path,
    triage: &TriageReceipt,
    detox: &DetoxReceipt,
    reset: &ResetReceipt,
    canaries_passed: bool,
    canary_report: CanaryReport,
    execution_time_ms: f64,
) -> Result<DischargeCertificate, crate::OooError> {
    let agent_ooo_dir = base_dir.join(".agent-ooo");
    std::fs::create_dir_all(&agent_ooo_dir)?;

    // Measure before and after in the same unit (bytes of parsed transcript lines / 4).
    // Only syntactic cleanse (redaction, ANSI, spillover) counts as realized savings.
    // Arena compaction is reported as a preview: the checkpoint does not yet persist
    // the retained turns, so evicted tokens are not a real reduction today.
    let tokens_before = detox.raw_bytes as f64 / 4.0;
    let tokens_after = detox.sanitized_bytes as f64 / 4.0;
    let tokens_reduced = (tokens_before - tokens_after).max(0.0);
    let compaction_preview_pct = if tokens_before > 0.0 {
        (detox.evicted_tokens as f64 / tokens_before * 100.0).min(100.0)
    } else {
        0.0
    };

    let token_reduction_percentage = if tokens_before > 0.0 {
        (tokens_reduced / tokens_before) * 100.0
    } else {
        0.0
    };

    // Heuristics: each severed loop saves ~0.1 dev hours (6 minutes).
    // Assumed blended input price: $5.00 per 1M tokens ($0.50 per 100k).
    let developer_hours_saved = (reset.severed_loops_count as f64) * 0.1;
    let net_savings_usd = (tokens_reduced / 100_000.0) * 0.5;

    let status = if !canaries_passed || canary_report.behavioral_status == CanaryStatus::Fail {
        WellnessTier::Fatigued
    } else if canary_report.behavioral_status == CanaryStatus::Warn {
        WellnessTier::Fair
    } else if triage.condition == WellnessTier::Elite
        && detox.redactions_count == 0
        && reset.severed_loops_count == 0
    {
        WellnessTier::Elite
    } else {
        WellnessTier::Fit
    };

    let mut canary_lines = String::new();
    for finding in &canary_report.findings {
        let status_label = match finding.status {
            CanaryStatus::Pass => "PASS",
            CanaryStatus::Warn => "WARN",
            CanaryStatus::Fail => "FAIL",
        };
        let sanitized_msg = finding.message.replace(['(', ')'], "");
        canary_lines.push_str(&format!(
            "* **{status_label} `{}`:** {}\n",
            finding.code, sanitized_msg
        ));
    }

    let behavioral_summary = match canary_report.behavioral_status {
        CanaryStatus::Pass => "PASS",
        CanaryStatus::Warn => "WARN",
        CanaryStatus::Fail => "FAIL",
    };

    let top_tools_formatted = if triage.vitals.top_tools.is_empty() {
        "none recorded".to_string()
    } else {
        triage
            .vitals
            .top_tools
            .iter()
            .map(|t| format!("`{}` at {} calls", t.tool_name, t.count))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let thinking_pct = triage.vitals.thinking_ratio_pct.clamp(0.0, 100.0);
    let output_pct = if triage.vitals.total_turns > 0
        && (thinking_pct > 0.0 || triage.vitals.assistant_turns > 0)
    {
        (100.0 - thinking_pct).max(0.0)
    } else {
        0.0
    };

    let cert = DischargeCertificate {
        status,
        checkin_condition: triage.condition,
        rot_index: triage.degradation_score,
        vitals: triage.vitals.clone(),
        canaries_passed,
        canary_report,
        token_reduction_percentage,
        developer_hours_saved,
        net_savings_usd,
        spa_report_path: agent_ooo_dir
            .join("spa_report.md")
            .to_string_lossy()
            .to_string(),
        execution_time_ms,
    };

    let md_content = format!(
        r#"---
title: Agent OOO Spa Report
purpose: Guest wellness summary and footprint reduction receipt
session: {session_id}
status: {status}
---

# Agent OOO Spa Report

**Session ID:** `{session_id}`
**Guest condition:** {status}, arrived {checkin_condition} with a Rot Index of {rot_index:.2} out of 10.0
**Checkpoint integrity:** {canary_status}
**Behavioral canary status:** {behavioral_summary}
**Treatment time:** {telemetry:.2} ms, measured locally on your machine

## 1. Checkin: guest profile and vitals
* **Agent harness:** {harness} running `{model}`
* **Session turns:** {total_turns} total turns across {user_turns} user turns and {assistant_turns} assistant turns
* **Tool cadence and friction:** {total_tools} tool calls with {failed_tools} failures, a {error_rate:.1}% error rate, and a max streak of {max_rep}
* **Most visited tools:** {top_tools_formatted}
* **Thinking balance:** {thinking_pct:.1}% internal reasoning vs. {output_pct:.1}% direct output
* **Token pace:** {avg_tokens} average tokens per turn across {total_tokens} total input tokens

## 2. Detox: deep-tissue cleanse
* **Raw footprint:** {raw} bytes
* **Sanitized footprint:** {sanitized} bytes
* **Secrets masked:** {secrets}
* **Large payloads spilled to disk:** {spilled} payloads across {handles} cold-storage recall handles

## 3. Reset: structural alignment
* **Error loops severed:** {loops}
* **Tool events correlated:** {events}
* **User rules pinned:** {constraints}
* **Environment registers extracted:** {registers}
* **Zero-copy Cap'n Proto size:** {capnp} bytes

## 4. Discharge: exit exam
{canary_lines}
## 5. Economic impact
* **Token reduction from Detox:** {reduction:.2}%
* **Net cost savings:** ${savings:.4} at an estimated $5.00 per 1M input tokens
* **Developer triage time saved:** {hours:.1} hours
* **Arena compaction preview:** {evicted} tokens, or {preview:.2}% of the session, sit in turns older than the last 100. Not yet persisted to the checkpoint, so not counted as savings.

> "Reset, refresh, relax."
"#,
        session_id = triage.session_id,
        status = status.as_str(),
        checkin_condition = triage.condition.as_str(),
        rot_index = triage.degradation_score,
        canary_status = if canaries_passed { "PASS" } else { "FAIL" },
        behavioral_summary = behavioral_summary,
        harness = triage.vitals.agent_harness,
        model = triage.vitals.model_id,
        total_turns = triage.vitals.total_turns,
        user_turns = triage.vitals.user_turns,
        assistant_turns = triage.vitals.assistant_turns,
        total_tools = triage.vitals.total_tool_calls,
        failed_tools = triage.vitals.failed_tool_calls,
        error_rate = triage.vitals.tool_error_rate_pct,
        max_rep = triage.repetition_count,
        top_tools_formatted = top_tools_formatted,
        thinking_pct = thinking_pct,
        output_pct = output_pct,
        avg_tokens = triage.vitals.avg_tokens_per_turn,
        total_tokens = triage.total_input_tokens,
        raw = detox.raw_bytes,
        sanitized = detox.sanitized_bytes,
        secrets = detox.redactions_count,
        spilled = detox.spilled_payloads_count,
        handles = detox.recall_handles.len(),
        loops = reset.severed_loops_count,
        events = reset.event_stream_count,
        constraints = reset.constraints_count,
        registers = reset.environment_registers_count,
        capnp = reset.checkpoint_capnp_bytes,
        canary_lines = canary_lines,
        reduction = token_reduction_percentage,
        savings = net_savings_usd,
        hours = developer_hours_saved,
        evicted = detox.evicted_tokens,
        preview = compaction_preview_pct,
        telemetry = execution_time_ms,
    );

    let md_path = agent_ooo_dir.join("spa_report.md");
    let mut file = File::create(&md_path)?;
    file.write_all(md_content.as_bytes())?;

    let json_path = agent_ooo_dir.join("spa_report.json");
    let json_str = serde_json::to_string_pretty(&cert)?;
    std::fs::write(&json_path, json_str)?;

    Ok(cert)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkin::{AgentVitals, ToolUsageStat};
    use crate::discharge::canary::CanaryFinding;
    use tempfile::TempDir;

    #[test]
    fn spa_report_obeys_title_wellness_tier_sentence_case_and_zero_parentheses_rules() {
        let dir = TempDir::new().unwrap();
        let triage = TriageReceipt {
            session_id: "sess-wellness-1".to_string(),
            transcript_path: "/tmp/transcript.jsonl".to_string(),
            total_input_tokens: 48_000,
            loop_lock_detected: false,
            repetition_count: 1,
            degradation_score: 0.48,
            condition: WellnessTier::Elite,
            vitals: AgentVitals {
                agent_harness: "Claude Code".to_string(),
                model_id: "claude-opus-4-6".to_string(),
                total_turns: 20,
                user_turns: 8,
                assistant_turns: 12,
                total_tool_calls: 15,
                failed_tool_calls: 0,
                tool_error_rate_pct: 0.0,
                top_tools: vec![
                    ToolUsageStat {
                        tool_name: "Bash".to_string(),
                        count: 10,
                    },
                    ToolUsageStat {
                        tool_name: "Edit".to_string(),
                        count: 5,
                    },
                ],
                thinking_ratio_pct: 35.0,
                avg_tokens_per_turn: 2400,
            },
        };
        let detox = DetoxReceipt {
            raw_bytes: 200_000,
            sanitized_bytes: 192_000,
            evicted_tokens: 1_200,
            evicted_bytes: 4_800,
            spilled_payloads_count: 1,
            redactions_count: 1,
            recall_handles: vec!["abc123.bin".to_string()],
        };
        let reset = ResetReceipt {
            severed_loops_count: 1,
            environment_registers_count: 4,
            checkpoint_capnp_bytes: 512,
            checkpoint_json_bytes: 640,
            event_stream_count: 15,
            constraints_count: 2,
        };
        let canary_report = CanaryReport {
            structural_integrity: true,
            behavioral_status: CanaryStatus::Pass,
            findings: vec![
                CanaryFinding {
                    code: "checkpoint_structural_integrity".to_string(),
                    status: CanaryStatus::Pass,
                    message:
                        "Cap'n Proto and JSON checkpoints verified for session 'sess-wellness-1'."
                            .to_string(),
                },
                CanaryFinding {
                    code: "unresolved_tool_failure".to_string(),
                    status: CanaryStatus::Pass,
                    message: "No unresolved build or tool failure at session end.".to_string(),
                },
            ],
        };

        let cert = generate_spa_report(
            dir.path(),
            &triage,
            &detox,
            &reset,
            true,
            canary_report,
            3.42,
        )
        .unwrap();

        assert_eq!(cert.status, WellnessTier::Fit);
        assert_eq!(cert.checkin_condition, WellnessTier::Elite);

        let report_markdown =
            std::fs::read_to_string(dir.path().join(".agent-ooo/spa_report.md")).unwrap();

        assert!(report_markdown.contains("title: Agent OOO Spa Report"));
        assert!(report_markdown.contains("# Agent OOO Spa Report"));
        assert!(report_markdown.contains("status: Fit"));
        assert!(report_markdown.contains("## 1. Checkin: guest profile and vitals"));
        assert!(report_markdown.contains("## 2. Detox: deep-tissue cleanse"));
        assert!(report_markdown.contains("## 3. Reset: structural alignment"));
        assert!(report_markdown.contains("## 4. Discharge: exit exam"));
        assert!(report_markdown.contains("## 5. Economic impact"));
        assert!(report_markdown.contains("`Bash` at 10 calls, `Edit` at 5 calls"));
        assert!(!report_markdown.contains('('));
        assert!(!report_markdown.contains(')'));
        assert!(!report_markdown.contains('\u{2014}'));
    }
}
