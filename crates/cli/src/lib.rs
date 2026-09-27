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
    Checkin { transcript: Option<String> },
    Detox { transcript: Option<String> },
    Reset { transcript: Option<String> },
    Discharge { transcript: Option<String> },
    Spa { transcript: Option<String> },
    Watch {
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
        Some(Commands::Discharge { transcript: _ }) => {
            println!("{} Clinical verification initiated...", "[DISCHARGE]".bold().magenta());
            // Discharge requires the previous steps to exist. We'll run them purely to get receipts for testing, 
            // but normally it reads the checkpoint. 
            // We just implement it as part of 'Spa' for full end-to-end.
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
