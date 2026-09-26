pub mod banner;

use clap::{Parser, Subcommand};
use colored::Colorize;

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
    /// Step 1: Intake, transcript discovery & diagnostic triage
    Checkin {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 2: Syntactic cleanse (strip ANSI, collapse DOM, mask secrets)
    Detox {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 3: State compaction, loop severance & checkpointing
    Reset {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 4: Canary oracle verification & bill of health
    Discharge {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// The Spa Package: Run Steps 1 through 4 end-to-end
    Spa {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Background watchdog monitoring active sessions
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
            println!(
                "{} Admitting session for checkin triage: {}",
                "[CHECKIN]".bold().green(),
                transcript.as_deref().unwrap_or("auto-discovery")
            );
        }
        Some(Commands::Detox { transcript }) => {
            println!(
                "{} Syntactic cleanse initiated: {}",
                "[DETOX]".bold().yellow(),
                transcript.as_deref().unwrap_or("auto-discovery")
            );
        }
        Some(Commands::Reset { transcript }) => {
            println!(
                "{} Structural state alignment initiated: {}",
                "[RESET]".bold().blue(),
                transcript.as_deref().unwrap_or("auto-discovery")
            );
        }
        Some(Commands::Discharge { transcript }) => {
            println!(
                "{} Clinical verification initiated: {}",
                "[DISCHARGE]".bold().magenta(),
                transcript.as_deref().unwrap_or("auto-discovery")
            );
        }
        Some(Commands::Spa { transcript }) => {
            println!(
                "{} Starting The Spa Package (Steps 1 -> 4): {}",
                "[SPA PACKAGE]".bold().cyan(),
                transcript.as_deref().unwrap_or("auto-discovery")
            );
        }
        Some(Commands::Watch { interval }) => {
            println!(
                "{} Background watchdog daemon active (interval: {}s)",
                "[WATCH]".bold().green(),
                interval
            );
        }
        None => {
            println!(
                "Run '{}' or '{}' to begin.",
                "agent-ooo checkin".bold(),
                "agent-ooo spa".bold()
            );
            println!("Use '{}' to see all available commands.", "agent-ooo --help".bold());
        }
    }

    Ok(())
}
