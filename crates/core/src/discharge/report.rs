use crate::checkin::TriageReceipt;
use crate::detox::DetoxReceipt;
use crate::reset::ResetReceipt;
use crate::discharge::DischargeCertificate;

use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn generate_spa_report(
    base_dir: &Path,
    triage: &TriageReceipt,
    detox: &DetoxReceipt,
    reset: &ResetReceipt,
    canaries_passed: bool,
    execution_time_ms: f64,
) -> Result<DischargeCertificate, crate::OooError> {
    let agent_ooo_dir = base_dir.join(".agent-ooo");
    std::fs::create_dir_all(&agent_ooo_dir)?;

    let tokens_before = triage.total_input_tokens as f64;
    let tokens_after = tokens_before - (detox.evicted_tokens as f64) - (detox.sanitized_bytes as f64 / 4.0);
    
    let token_reduction_percentage = if tokens_before > 0.0 {
        ((tokens_before - tokens_after.max(0.0)) / tokens_before) * 100.0
    } else {
        0.0
    };

    // Heuristics: Every 5 loops severed saves ~0.5 dev hours.
    // Every 100k tokens reduced saves ~$0.50.
    let developer_hours_saved = (reset.severed_loops_count as f64) * 0.1;
    let net_savings_usd = ((tokens_before - tokens_after.max(0.0)) / 100_000.0) * 0.5;

    let cert = DischargeCertificate {
        canaries_passed,
        token_reduction_percentage,
        developer_hours_saved,
        net_savings_usd,
        spa_report_path: agent_ooo_dir.join("spa_report.md").to_string_lossy().to_string(),
        execution_time_ms,
    };

    let md_content = format!(
        r#"---
title: Agent OOO Spa Report
purpose: Clinical verification and footprint reduction summary
session: {session_id}
status: {status}
---

# Agent OOO Spa Report

**Session ID:** `{session_id}`
**Canary Verification:** {canary_status}

## 1. Syntactic Cleanse (Detox)
* **Raw Footprint:** {raw} bytes
* **Sanitized Footprint:** {sanitized} bytes
* **Secrets Masked:** {secrets}
* **Large Payloads Spilled to Disk:** {spilled}

## 2. Structural Alignment (Reset)
* **Error Loops Severed:** {loops}
* **Environment Registers Extracted:** {registers}
* **Zero-copy Cap'n Proto Size:** {capnp} bytes

## 3. Economic Impact
* **Token Reduction:** {reduction:.2}%
* **Net Cost Savings:** ${savings:.4}
* **Developer Triage Time Saved:** {hours:.1} hours
* **Execution Telemetry:** {telemetry:.2} ms

> "Reset, refresh, relax."
"#,
        session_id = triage.session_id,
        status = if canaries_passed { "DISCHARGED_HEALTHY" } else { "QUARANTINED" },
        canary_status = if canaries_passed { "PASS" } else { "FAIL" },
        raw = detox.raw_bytes,
        sanitized = detox.sanitized_bytes,
        secrets = detox.redactions_count,
        spilled = detox.spilled_payloads_count,
        loops = reset.severed_loops_count,
        registers = reset.environment_registers_count,
        capnp = reset.checkpoint_capnp_bytes,
        reduction = token_reduction_percentage,
        savings = net_savings_usd,
        hours = developer_hours_saved,
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
