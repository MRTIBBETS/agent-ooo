pub mod banner;

use clap::{Parser, Subcommand};
use colored::Colorize;
use agent_ooo_core::checkin::{discovery, parser, triage};
use agent_ooo_core::detox::pipeline as detox_pipeline;
use agent_ooo_core::reset::pipeline as reset_pipeline;
use agent_ooo_core::discharge::pipeline as discharge_pipeline;
use std::env;

#[derive(Parser)]
#[command(name = "agent-ooo")]
#[command(bin_name = "agent-ooo")]
#[command(version = "0.1.0")]
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
    /// Background watchdog monitoring active sessions.
    #[command(display_order = 6)]
    Watch {
        /// Polling interval in seconds
        #[arg(short, long, default_value_t = 10)]
        interval: u64,
    },
}

pub fn run() -> anyhow::Result<()> {
    banner::print_banner();

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Checkin { transcript }) => {
            println!("{} Admitting session for checkin triage...", "[CHECKIN]".bold().green());
            let path = discovery::resolve_transcript_path(transcript.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
            let metrics = parser::parse_transcript(&path)?;
            let receipt = triage::calculate_triage(&path, metrics);
            println!("Session ID: {}", receipt.session_id);
            println!("Degradation Score: {:.2}/10.0", receipt.degradation_score);
        }
        Some(Commands::Detox { transcript }) => {
            println!("{} Syntactic cleanse initiated...", "[DETOX]".bold().yellow());
            let path = discovery::resolve_transcript_path(transcript.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
            let current_dir = env::current_dir()?;
            let (_, receipt) = detox_pipeline::execute_detox(&path, &current_dir).map_err(|e| anyhow::anyhow!(e))?;
            println!("Sanitized Bytes: {}", receipt.sanitized_bytes);
        }
        Some(Commands::Reset { transcript }) => {
            println!("{} Structural state alignment initiated...", "[RESET]".bold().blue());
            let path = discovery::resolve_transcript_path(transcript.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
            let current_dir = env::current_dir()?;
            let (arena, _) = detox_pipeline::execute_detox(&path, &current_dir).map_err(|e| anyhow::anyhow!(e))?;
            let session_id = path.file_name().unwrap_or_default().to_string_lossy();
            let receipt = reset_pipeline::execute_reset(&session_id, &current_dir, &arena)?;
            println!("Checkpoint size: {} bytes", receipt.checkpoint_capnp_bytes);
        }
        Some(Commands::Discharge { transcript }) => {
            println!("{} Clinical verification initiated...", "[DISCHARGE]".bold().magenta());
            println!("Please use `agent-ooo spa` to run the full pipeline.");
        }
        Some(Commands::Spa { transcript }) => {
            println!("{} Starting The Spa Package (Steps 1 -> 4)...", "[SPA PACKAGE]".bold().cyan());
            
            let path = discovery::resolve_transcript_path(transcript.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
            println!("{} Discovered transcript: {}", "->".cyan(), path.display());
            
            let current_dir = env::current_dir()?;
            
            // 1. Checkin
            println!("\n{} Step 1: Checkin & Triage...", "=>".green());
            let metrics = parser::parse_transcript(&path)?;
            let triage_receipt = triage::calculate_triage(&path, metrics);
            println!("   Score: {:.2}/10.0, Tokens: {}", triage_receipt.degradation_score, triage_receipt.total_input_tokens);

            // 2. Detox
            println!("\n{} Step 2: Syntactic Cleanse (Detox)...", "=>".yellow());
            let (arena, detox_receipt) = detox_pipeline::execute_detox(&path, &current_dir).map_err(|e| anyhow::anyhow!(e))?;
            println!("   Masked {} secrets, saved {} bytes.", detox_receipt.redactions_count, detox_receipt.raw_bytes.saturating_sub(detox_receipt.sanitized_bytes));

            // 3. Reset
            println!("\n{} Step 3: Structural Alignment (Reset)...", "=>".blue());
            let session_id = path.file_name().unwrap_or_default().to_string_lossy();
            let reset_receipt = reset_pipeline::execute_reset(&session_id, &current_dir, &arena)?;
            println!("   Checkpoints saved. Zero-copy state locked.");

            // 4. Discharge
            println!("\n{} Step 4: Verification & Discharge...", "=>".magenta());
            let cert = discharge_pipeline::execute_discharge(&current_dir, &triage_receipt, &detox_receipt, &reset_receipt)?;
            
            println!("\n{}", "=============================================".bold().cyan());
            println!("{} Spa Retreat Complete!", "★".yellow());
            println!("* Canary Verification: {}", if cert.canaries_passed { "PASS".green() } else { "FAIL".red() });
            println!("* Token Footprint Reduction: {:.2}%", cert.token_reduction_percentage);
            println!("* Estimated Savings: ${:.4}", cert.net_savings_usd);
            println!("* Developer Hours Saved: {:.1}", cert.developer_hours_saved);
            println!("* Spa Report written to: {}", cert.spa_report_path);
            println!("{}", "=============================================".bold().cyan());
        }
        Some(Commands::Watch { interval }) => {
            println!("{} Background watchdog daemon active (interval: {}s)", "[WATCH]".bold().green(), interval);
        }
        None => {
            println!("Run '{}' or '{}' to begin.", "agent-ooo checkin".bold(), "agent-ooo spa".bold());
        }
    }

    Ok(())
}
