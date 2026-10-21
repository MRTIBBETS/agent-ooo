pub mod banner;

use agent_ooo_core::checkin::{discovery, parser, triage};
use agent_ooo_core::detox::pipeline as detox_pipeline;
use agent_ooo_core::discharge::pipeline as discharge_pipeline;
use agent_ooo_core::reset::pipeline as reset_pipeline;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::env;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "agent-ooo")]
#[command(bin_name = "agent-ooo")]
#[command(version)]
#[command(about = "Spa retreats for your AI. Reset, refresh, relax.", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the full 4-step retreat (Checkin, Detox, Reset, Discharge).
    #[command(display_order = 1)]
    Spa {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 1: Parse transcripts and diagnose context rot.
    #[command(display_order = 2)]
    Checkin {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 2: Strip ANSI noise, mask secrets, and offload massive payloads.
    #[command(display_order = 3)]
    Detox {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 3: Sever error loops and write zero-copy checkpoints.
    #[command(display_order = 4)]
    Reset {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 4: Verify health and generate the Spa Report.
    #[command(display_order = 5)]
    Discharge {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Stream a secret-scrubbed session transcript to stdout or a file.
    #[command(display_order = 6)]
    Export {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
        /// Export format: jsonl, normalized, or handoff
        #[arg(short, long, default_value = "normalized")]
        format: String,
        /// Optional cap on the number of recent transient turns exported (System Gen 0 turns are always retained)
        #[arg(short, long)]
        max_turns: Option<usize>,
        /// Optional output file path. Writes directly to stdout when omitted.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Background watchdog monitoring active sessions.
    #[command(display_order = 7)]
    Watch {
        /// Polling interval in seconds
        #[arg(short, long, default_value_t = 10)]
        interval: u64,
    },
    /// Update agent-ooo to the latest version.
    #[command(display_order = 8)]
    Update,
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let quiet_stdout = matches!(&cli.command, Some(Commands::Export { output: None, .. }));
    if !quiet_stdout {
        banner::print_banner();
    }

    match &cli.command {
        Some(Commands::Checkin { transcript }) => {
            println!(
                "{} Admitting session for checkin triage...",
                "[CHECKIN]".bold().green()
            );
            let path = discovery::resolve_transcript_path(transcript.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            let metrics = parser::parse_transcript(&path)?;
            let receipt = triage::calculate_triage(&path, metrics);
            println!("Session ID: {}", receipt.session_id);
            println!(
                "Guest Condition: {} | Rot Index: {:.2}/10.0",
                receipt.condition.as_str().bold(),
                receipt.degradation_score
            );
            println!(
                "Harness: {} ({}) | Turns: {} | Tool Calls: {} ({} failed)",
                receipt.vitals.agent_harness,
                receipt.vitals.model_id,
                receipt.vitals.total_turns,
                receipt.vitals.total_tool_calls,
                receipt.vitals.failed_tool_calls
            );
        }
        Some(Commands::Detox { transcript }) => {
            println!(
                "{} Syntactic cleanse initiated...",
                "[DETOX]".bold().yellow()
            );
            let path = discovery::resolve_transcript_path(transcript.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            let current_dir = env::current_dir()?;
            let (_, receipt) = detox_pipeline::execute_detox(&path, &current_dir)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("Sanitized Bytes: {}", receipt.sanitized_bytes);
        }
        Some(Commands::Reset { transcript }) => {
            println!(
                "{} Structural state alignment initiated...",
                "[RESET]".bold().blue()
            );
            let path = discovery::resolve_transcript_path(transcript.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            let current_dir = env::current_dir()?;
            let (mut arena, detox_receipt) = detox_pipeline::execute_detox(&path, &current_dir)
                .map_err(|e| anyhow::anyhow!(e))?;
            let session_id = discovery::session_id_for(&path);
            let receipt = reset_pipeline::execute_reset(
                &session_id,
                &current_dir,
                &mut arena,
                &detox_receipt,
            )?;
            println!(
                "Severed Loops: {}, Constraints: {}, Tool Events: {}",
                receipt.severed_loops_count, receipt.constraints_count, receipt.event_stream_count
            );
            println!("Checkpoint size: {} bytes", receipt.checkpoint_capnp_bytes);
        }
        Some(Commands::Discharge { transcript }) => {
            println!(
                "{} Clinical verification initiated...",
                "[DISCHARGE]".bold().magenta()
            );
            let target_dir = match transcript.as_deref() {
                Some(p) => std::path::PathBuf::from(p),
                None => env::current_dir()?,
            };
            let inspection = agent_ooo_core::reset::serializer::inspect_checkpoint(&target_dir)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("Session ID: {}", inspection.session_id);
            println!(
                "Checkpoint Integrity: {}",
                if inspection.integrity_passed {
                    "PASS".green()
                } else {
                    "FAIL".red()
                }
            );
            println!(
                "Cap'n Proto: {} bytes | JSON: {} bytes | Events: {} | Constraints: {}",
                inspection.checkpoint_capnp_bytes,
                inspection.checkpoint_json_bytes,
                inspection.event_stream_count,
                inspection.constraints.len()
            );
            for finding in &inspection.canary_report.findings {
                let badge = match finding.status {
                    agent_ooo_core::discharge::canary::CanaryStatus::Pass => "[PASS]".green(),
                    agent_ooo_core::discharge::canary::CanaryStatus::Warn => "[WARN]".yellow(),
                    agent_ooo_core::discharge::canary::CanaryStatus::Fail => "[FAIL]".red(),
                };
                println!("  {} {}: {}", badge, finding.code.bold(), finding.message);
            }
        }
        Some(Commands::Export {
            transcript,
            format,
            max_turns,
            output,
        }) => {
            let export_format = agent_ooo_core::detox::exporter::ExportFormat::parse_label(format)
                .map_err(|e| anyhow::anyhow!(e))?;
            let current_dir = env::current_dir()?;

            if let Some(out_path) = output {
                println!(
                    "{} Streaming sanitized transcript export...",
                    "[EXPORT]".bold().cyan()
                );
                let path = discovery::resolve_transcript_path(transcript.as_deref())
                    .map_err(|e| anyhow::anyhow!(e))?;
                let mut out_file = std::fs::File::create(out_path)?;
                let receipt = agent_ooo_core::detox::exporter::export_transcript(
                    &path,
                    &current_dir,
                    export_format,
                    *max_turns,
                    &mut out_file,
                )?;
                println!(
                    "Exported {} lines ({} bytes) to {} [masked {} secret(s), spilled {} payload(s)]",
                    receipt.lines_exported,
                    receipt.bytes_written,
                    out_path,
                    receipt.redactions_count,
                    receipt.spilled_payloads_count
                );
            } else {
                let path = discovery::resolve_transcript_non_interactive(transcript.as_deref())
                    .map_err(|e| anyhow::anyhow!(e))?;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                if let Err(err) = agent_ooo_core::detox::exporter::export_transcript(
                    &path,
                    &current_dir,
                    export_format,
                    *max_turns,
                    &mut handle,
                ) {
                    if let agent_ooo_core::OooError::Io(ref io_err) = err {
                        if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                            return Ok(());
                        }
                    }
                    return Err(err.into());
                }
            }
        }
        Some(Commands::Spa { transcript }) => {
            println!(
                "{} Starting The Spa Package (Steps 1 -> 4)...",
                "[SPA PACKAGE]".bold().cyan()
            );

            let start_time = Instant::now();
            let path = discovery::resolve_transcript_path(transcript.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("{} Discovered transcript: {}", "->".cyan(), path.display());

            let current_dir = env::current_dir()?;

            // 1. Checkin
            println!("\n{} Step 1: Checkin & Triage...", "=>".green());
            let metrics = parser::parse_transcript(&path)?;
            let triage_receipt = triage::calculate_triage(&path, metrics);
            println!(
                "   Condition: {}, Rot Index: {:.2}/10.0, Tokens: {}",
                triage_receipt.condition.as_str(),
                triage_receipt.degradation_score,
                triage_receipt.total_input_tokens
            );

            // 2. Detox
            println!("\n{} Step 2: Syntactic Cleanse (Detox)...", "=>".yellow());
            let (mut arena, detox_receipt) = detox_pipeline::execute_detox(&path, &current_dir)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!(
                "   Masked {} secrets, saved {} bytes.",
                detox_receipt.redactions_count,
                detox_receipt
                    .raw_bytes
                    .saturating_sub(detox_receipt.sanitized_bytes)
            );

            // 3. Reset
            println!("\n{} Step 3: Structural Alignment (Reset)...", "=>".blue());
            let session_id = discovery::session_id_for(&path);
            let reset_receipt = reset_pipeline::execute_reset(
                &session_id,
                &current_dir,
                &mut arena,
                &detox_receipt,
            )?;
            println!(
                "   Severed {} loops, captured {} constraints. Zero-copy state locked.",
                reset_receipt.severed_loops_count, reset_receipt.constraints_count
            );

            // 4. Discharge
            println!("\n{} Step 4: Verification & Discharge...", "=>".magenta());
            // Measured before the report is written so spa_report.md and the terminal agree.
            let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            let cert = discharge_pipeline::execute_discharge(
                &current_dir,
                &triage_receipt,
                &detox_receipt,
                &reset_receipt,
                elapsed_ms,
            )?;

            println!(
                "\n{}",
                "============================================="
                    .bold()
                    .cyan()
            );
            println!("{} Spa Retreat Complete!", "★".yellow());
            println!("* Guest Condition: {}", cert.status.as_str().bold());
            println!("* Execution Time: {:.2} ms", cert.execution_time_ms);
            println!(
                "* Checkpoint Integrity: {}",
                if cert.canaries_passed {
                    "PASS".green()
                } else {
                    "FAIL".red()
                }
            );
            let behavioral_label = match cert.canary_report.behavioral_status {
                agent_ooo_core::discharge::canary::CanaryStatus::Pass => "PASS".green(),
                agent_ooo_core::discharge::canary::CanaryStatus::Warn => "WARN".yellow(),
                agent_ooo_core::discharge::canary::CanaryStatus::Fail => "FAIL".red(),
            };
            println!("* Behavioral Canaries: {}", behavioral_label);
            println!(
                "* Token Reduction (Detox): {:.2}%",
                cert.token_reduction_percentage
            );
            println!("* Estimated Savings: ${:.4}", cert.net_savings_usd);
            println!("* Developer Hours Saved: {:.1}", cert.developer_hours_saved);
            println!("* Spa Report written to: {}", cert.spa_report_path);
            println!(
                "{}",
                "============================================="
                    .bold()
                    .cyan()
            );
        }
        Some(Commands::Watch { interval }) => {
            println!(
                "{} Background watchdog daemon active (interval: {}s)",
                "[WATCH]".bold().green(),
                interval
            );
        }
        Some(Commands::Update) => {
            println!("{} Checking for updates...", "[UPDATE]".bold().cyan());
            match self_update::backends::github::Update::configure()
                .repo_owner("mrtibbets")
                .repo_name("agent-ooo")
                .bin_name("agent-ooo")
                .show_download_progress(true)
                .current_version(env!("CARGO_PKG_VERSION"))
                .build()
                .and_then(|update| update.update())
            {
                Ok(status) => {
                    if status.is_updated() {
                        println!(
                            "{} Successfully updated to v{}",
                            "★".yellow(),
                            status.version()
                        );
                    } else {
                        println!("{} Already up to date (v{})", "★".green(), status.version());
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to update: {}", "[ERROR]".bold().red(), e);
                }
            }
        }
        None => {
            println!(
                "Run '{}' or '{}' to begin.",
                "agent-ooo checkin".bold(),
                "agent-ooo spa".bold()
            );
        }
    }

    Ok(())
}
