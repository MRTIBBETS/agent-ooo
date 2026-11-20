use agent_ooo_core::checkin::{TriageReceipt, WellnessTier};
use agent_ooo_core::detox::sanitizer::TranscriptScanReport;
use colored::{ColoredString, Colorize};

const INNER_WIDTH: usize = 68;
const CONTENT_WIDTH: usize = INNER_WIDTH - 2;
const LABEL_WIDTH: usize = 15;
const VALUE_WIDTH: usize = CONTENT_WIDTH - LABEL_WIDTH;

#[derive(Clone, Copy)]
enum SegmentStyle {
    Plain,
    Bold,
    Dimmed,
    Tier(WellnessTier),
    GreenBold,
    RedBold,
}

fn apply_style(text: &str, style: SegmentStyle) -> ColoredString {
    match style {
        SegmentStyle::Plain => text.normal(),
        SegmentStyle::Bold => text.bold(),
        SegmentStyle::Dimmed => text.dimmed(),
        SegmentStyle::Tier(tier) => match tier {
            WellnessTier::Elite => text.bright_green().bold(),
            WellnessTier::Fit => text.cyan().bold(),
            WellnessTier::Fair => text.yellow().bold(),
            WellnessTier::Fatigued => text.red().bold(),
        },
        SegmentStyle::GreenBold => text.green().bold(),
        SegmentStyle::RedBold => text.red().bold(),
    }
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut chars: Vec<char> = Vec::with_capacity(max_chars.min(128));
    let mut overflowed = false;

    for c in s.chars() {
        let normalized = match c {
            '\n' | '\r' | '\t' => ' ',
            _ if c.is_control() => continue,
            _ => c,
        };
        if chars.len() < max_chars {
            chars.push(normalized);
        } else {
            overflowed = true;
            break;
        }
    }

    if !overflowed {
        return chars.into_iter().collect();
    }

    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    chars.truncate(max_chars - 3);
    let mut out: String = chars.into_iter().collect();
    out.push_str("...");
    out
}

fn format_scaled_unit(value: f64, suffix: &str) -> String {
    let formatted = format!("{value:.1}");
    if let Some(trimmed) = formatted.strip_suffix(".0") {
        format!("{trimmed}{suffix}")
    } else {
        format!("{formatted}{suffix}")
    }
}

pub fn format_compact_tokens(tokens: u64) -> String {
    if tokens < 1_000 {
        tokens.to_string()
    } else if tokens < 999_950 {
        format_scaled_unit(tokens as f64 / 1_000.0, "K")
    } else if tokens < 999_950_000 {
        format_scaled_unit(tokens as f64 / 1_000_000.0, "M")
    } else {
        format_scaled_unit(tokens as f64 / 1_000_000_000.0, "B")
    }
}

fn render_row(segments: &[(String, SegmentStyle)]) -> String {
    let mut line = String::with_capacity(INNER_WIDTH + 64);
    line.push_str(&"│".dimmed().to_string());
    line.push(' ');

    let mut remaining = CONTENT_WIDTH;
    for (text, style) in segments {
        if remaining == 0 {
            break;
        }
        let fitted = truncate_chars(text, remaining);
        let len = fitted.chars().count();
        remaining = remaining.saturating_sub(len);
        line.push_str(&apply_style(&fitted, *style).to_string());
    }

    for _ in 0..remaining {
        line.push(' ');
    }

    line.push(' ');
    line.push_str(&"│".dimmed().to_string());
    line
}

fn make_label(name: &str) -> (String, SegmentStyle) {
    (
        format!("{name:<width$}", width = LABEL_WIDTH),
        SegmentStyle::Dimmed,
    )
}

/// Renders a compact, screenshot-ready ANSI terminal vitals card for a session.
pub fn render_vitals_card(receipt: &TriageReceipt, scan: &TranscriptScanReport) -> String {
    let top_border = format!("╭{}╮", "─".repeat(INNER_WIDTH))
        .dimmed()
        .to_string();
    let mid_border = format!("├{}┤", "─".repeat(INNER_WIDTH))
        .dimmed()
        .to_string();
    let bot_border = format!("╰{}╯", "─".repeat(INNER_WIDTH))
        .dimmed()
        .to_string();

    let title = "AGENT OOO // VITALS CARD".to_string();
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let header_gap_len =
        CONTENT_WIDTH.saturating_sub(title.chars().count() + version.chars().count());
    let header_row = render_row(&[
        (title, SegmentStyle::Bold),
        (" ".repeat(header_gap_len), SegmentStyle::Plain),
        (version, SegmentStyle::Dimmed),
    ]);

    let session_row = render_row(&[
        make_label("Session"),
        (
            truncate_chars(&receipt.session_id, VALUE_WIDTH),
            SegmentStyle::Plain,
        ),
    ]);

    let loop_status = if receipt.loop_lock_detected {
        ("DETECTED".to_string(), SegmentStyle::RedBold)
    } else {
        ("Clean".to_string(), SegmentStyle::GreenBold)
    };
    let condition_row = render_row(&[
        make_label("Condition"),
        (
            format!("{:<10}", receipt.condition.as_str()),
            SegmentStyle::Tier(receipt.condition),
        ),
        ("| ".to_string(), SegmentStyle::Dimmed),
        ("Loop Lock: ".to_string(), SegmentStyle::Dimmed),
        loop_status,
    ]);

    let clamped_score = if receipt.degradation_score.is_finite() {
        receipt.degradation_score.clamp(0.0, 10.0)
    } else {
        0.0
    };
    let filled_cells = (clamped_score.round() as usize).min(10);
    let bar = format!(
        "[{}{}]",
        "█".repeat(filled_cells),
        "░".repeat(10 - filled_cells)
    );
    let score_text = format!(" {:.2}/10.0", clamped_score);
    let rot_row = render_row(&[
        make_label("Rot Index"),
        (bar, SegmentStyle::Tier(receipt.condition)),
        (score_text, SegmentStyle::Tier(receipt.condition)),
    ]);

    let harness_model = truncate_chars(
        &format!(
            "{} / {}",
            receipt.vitals.agent_harness, receipt.vitals.model_id
        ),
        VALUE_WIDTH,
    );
    let harness_row = render_row(&[
        make_label("Harness/Model"),
        (harness_model, SegmentStyle::Plain),
    ]);

    let activity_text = truncate_chars(
        &format!(
            "{} turns | {} calls ({} err)",
            receipt.vitals.total_turns,
            receipt.vitals.total_tool_calls,
            receipt.vitals.failed_tool_calls
        ),
        VALUE_WIDTH,
    );
    let activity_row = render_row(&[make_label("Activity"), (activity_text, SegmentStyle::Plain)]);

    let top_tools_raw = if receipt.vitals.top_tools.is_empty() {
        "none".to_string()
    } else {
        receipt
            .vitals
            .top_tools
            .iter()
            .take(3)
            .map(|t| format!("{} ({})", t.tool_name, t.count))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let top_tools_row = render_row(&[
        make_label("Top Tools"),
        (
            truncate_chars(&top_tools_raw, VALUE_WIDTH),
            SegmentStyle::Plain,
        ),
    ]);

    let in_tok = format_compact_tokens(receipt.vitals.api_input_tokens);
    let cache_tok = format_compact_tokens(receipt.vitals.api_cache_read_tokens);
    let out_tok = format_compact_tokens(receipt.vitals.api_output_tokens);
    let cost_str = if receipt.vitals.estimated_session_cost_usd <= 0.0 {
        "$0.0000".to_string()
    } else {
        format!("~${:.4}", receipt.vitals.estimated_session_cost_usd)
    };
    let spend_raw = format!("{in_tok} in / {cache_tok} cache / {out_tok} out ({cost_str})");
    let spend_row = render_row(&[
        make_label("API Spend"),
        (truncate_chars(&spend_raw, VALUE_WIDTH), SegmentStyle::Plain),
    ]);

    let (secrets_text, secrets_style) = if scan.total_occurrences == 0 {
        ("0 leaked (clean)".to_string(), SegmentStyle::GreenBold)
    } else {
        (
            truncate_chars(
                &format!(
                    "{} leaked ({} distinct)",
                    scan.total_occurrences, scan.distinct_secrets
                ),
                VALUE_WIDTH,
            ),
            SegmentStyle::RedBold,
        )
    };
    let secrets_row = render_row(&[make_label("Secrets Masked"), (secrets_text, secrets_style)]);

    let footer_row = render_row(&[(
        "ooo checkin --card | github.com/mrtibbets/agent-ooo".to_string(),
        SegmentStyle::Dimmed,
    )]);

    [
        top_border,
        header_row,
        session_row,
        mid_border.clone(),
        condition_row,
        rot_row,
        harness_row,
        activity_row,
        top_tools_row,
        spend_row,
        secrets_row,
        mid_border,
        footer_row,
        bot_border,
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_ooo_core::checkin::{AgentVitals, ToolUsageStat};

    fn strip_ansi(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for esc in chars.by_ref() {
                    if esc.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    fn assert_card_invariants(rendered: &str) -> String {
        assert!(!rendered.contains('\u{2014}'), "em dash found in card");
        let plain = strip_ansi(rendered);
        assert!(!plain.contains('\u{2014}'), "em dash found in plain card");
        let expected_cols = INNER_WIDTH + 2;
        for (idx, line) in plain.lines().enumerate() {
            assert_eq!(
                line.chars().count(),
                expected_cols,
                "line {idx} width mismatch: {line:?}"
            );
        }
        plain
    }

    #[test]
    fn test_format_compact_tokens() {
        assert_eq!(format_compact_tokens(0), "0");
        assert_eq!(format_compact_tokens(999), "999");
        assert_eq!(format_compact_tokens(1_000), "1K");
        assert_eq!(format_compact_tokens(1_540), "1.5K");
        assert_eq!(format_compact_tokens(999_999), "1M");
        assert_eq!(format_compact_tokens(2_340_000), "2.3M");
    }

    #[test]
    fn test_render_vitals_card_elite_clean() {
        let receipt = TriageReceipt {
            session_id: "session-elite-103bff8e".to_string(),
            transcript_path: "/tmp/transcript.jsonl".to_string(),
            total_input_tokens: 18_000,
            loop_lock_detected: false,
            repetition_count: 1,
            degradation_score: 0.85,
            condition: WellnessTier::Elite,
            vitals: AgentVitals {
                agent_harness: "Antigravity".to_string(),
                model_id: "Gemini 3.8 Flash".to_string(),
                total_turns: 24,
                user_turns: 10,
                assistant_turns: 14,
                total_tool_calls: 27,
                failed_tool_calls: 0,
                tool_error_rate_pct: 0.0,
                top_tools: vec![
                    ToolUsageStat {
                        tool_name: "run_command".to_string(),
                        count: 14,
                    },
                    ToolUsageStat {
                        tool_name: "read_file".to_string(),
                        count: 9,
                    },
                    ToolUsageStat {
                        tool_name: "edit_file".to_string(),
                        count: 4,
                    },
                ],
                thinking_ratio_pct: 25.0,
                avg_tokens_per_turn: 750,
                api_input_tokens: 12_400,
                api_cache_read_tokens: 48_200,
                api_output_tokens: 3_100,
                estimated_session_cost_usd: 0.0184,
            },
        };
        let scan = TranscriptScanReport {
            session_id: receipt.session_id.clone(),
            scanned_path: receipt.transcript_path.clone(),
            lines_scanned: 48,
            total_occurrences: 0,
            distinct_secrets: 0,
            findings: Vec::new(),
        };

        let rendered = render_vitals_card(&receipt, &scan);
        let plain = assert_card_invariants(&rendered);

        assert!(plain.contains("AGENT OOO // VITALS CARD"));
        assert!(plain.contains("session-elite-103bff8e"));
        assert!(plain.contains("Elite"));
        assert!(plain.contains("Loop Lock: Clean"));
        assert!(plain.contains("[█░░░░░░░░░] 0.85/10.0"));
        assert!(plain.contains("Antigravity / Gemini 3.8 Flash"));
        assert!(plain.contains("24 turns | 27 calls (0 err)"));
        assert!(plain.contains("run_command (14), read_file (9), edit_file (4)"));
        assert!(plain.contains("12.4K in / 48.2K cache / 3.1K out (~$0.0184)"));
        assert!(plain.contains("0 leaked (clean)"));
    }

    #[test]
    fn test_render_vitals_card_fatigued_long_fields_and_secrets() {
        let receipt = TriageReceipt {
            session_id:
                "very-long-session-identifier-that-exceeds-the-card-value-column-width-easily-1234567890"
                    .to_string(),
            transcript_path: "/tmp/fatigued.jsonl".to_string(),
            total_input_tokens: 500_000,
            loop_lock_detected: true,
            repetition_count: 6,
            degradation_score: 8.75,
            condition: WellnessTier::Fatigued,
            vitals: AgentVitals {
                agent_harness: "ClaudeCodeEnterprise".to_string(),
                model_id: "claude-opus-4-5-20261120-extended-thinking-preview-variant".to_string(),
                total_turns: 180,
                user_turns: 60,
                assistant_turns: 120,
                total_tool_calls: 95,
                failed_tool_calls: 19,
                tool_error_rate_pct: 20.0,
                top_tools: vec![
                    ToolUsageStat {
                        tool_name: "super_verbose_workspace_scanner_tool".to_string(),
                        count: 42,
                    },
                    ToolUsageStat {
                        tool_name: "another_long_tool_name_here".to_string(),
                        count: 31,
                    },
                ],
                thinking_ratio_pct: 40.0,
                avg_tokens_per_turn: 2_777,
                api_input_tokens: 0,
                api_cache_read_tokens: 0,
                api_output_tokens: 0,
                estimated_session_cost_usd: 0.0,
            },
        };
        let scan = TranscriptScanReport {
            session_id: receipt.session_id.clone(),
            scanned_path: receipt.transcript_path.clone(),
            lines_scanned: 300,
            total_occurrences: 5,
            distinct_secrets: 2,
            findings: Vec::new(),
        };

        let rendered = render_vitals_card(&receipt, &scan);
        let plain = assert_card_invariants(&rendered);

        assert!(plain.contains("Fatigued"));
        assert!(plain.contains("Loop Lock: DETECTED"));
        assert!(plain.contains("[█████████░] 8.75/10.0"));
        assert!(plain.contains("..."));
        assert!(plain.contains("180 turns | 95 calls (19 err)"));
        assert!(plain.contains("0 in / 0 cache / 0 out ($0.0000)"));
        assert!(plain.contains("5 leaked (2 distinct)"));
    }
}
