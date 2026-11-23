pub mod banner;
pub mod card;

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
    /// Automatically select the most recent session without interactive prompting
    #[arg(short = 'y', long = "latest", global = true)]
    pub latest: bool,

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
        /// Render a compact, screenshot-ready ANSI terminal vitals card
        #[arg(long)]
        card: bool,
    },
    /// Step 2: Strip ANSI noise, mask secrets, and offload massive payloads.
    #[command(display_order = 3)]
    Detox {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
        /// Scan all discovered sessions across all local agent harnesses for leaked credentials
        #[arg(short, long)]
        all: bool,
        /// Safely scrub leaked secrets directly from source transcript files in place
        #[arg(long)]
        redact_in_place: bool,
        /// Create and verify a .bak copy before replacing a modified transcript in place (requires --redact-in-place)
        #[arg(long)]
        backup: bool,
    },
    /// Step 3: Sever error loops and write zero-copy checkpoints.
    #[command(display_order = 4)]
    Reset {
        /// Optional path to a specific transcript file (.jsonl)
        transcript: Option<String>,
    },
    /// Step 4: Verify health, inspect checkpoints, or recall a cold-storage CAS payload.
    #[command(display_order = 5)]
    Discharge {
        /// Optional path to a workspace directory containing .agent-ooo/
        transcript: Option<String>,
        /// Optional CAS spillover handle (<blake3> or <blake3>.bin) to recall from .agent-ooo/spillover/
        #[arg(long)]
        recall: Option<String>,
        /// Byte offset when recalling a spilled payload
        #[arg(long, default_value_t = 0)]
        offset: u64,
        /// Maximum bytes to read when recalling a spilled payload (clamped to 32 KB)
        #[arg(long, default_value_t = 8192)]
        max_bytes: usize,
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
    /// Update agent-ooo to the latest version.
    #[command(display_order = 7)]
    Update,
    /// Step 8: Initialize turnkey agent skill bundle and git lifecycle hooks.
    #[command(display_order = 8, alias = "init-hooks")]
    InitSkill {
        /// Destination directory for the skill bundle (defaults to ./skills/agent-ooo)
        #[arg(short, long)]
        target: Option<String>,
        /// Target harness for instructions (antigravity, claude, cursor, codex, git, all)
        #[arg(short = 'H', long, default_value = "all")]
        harness: String,
        /// Install or generate git lifecycle hook scripts (e.g. .git/hooks/pre-push)
        #[arg(long)]
        hooks: bool,
        /// Overwrite existing files
        #[arg(short, long)]
        force: bool,
        /// Print generated skill markdown directly to stdout
        #[arg(long)]
        stdout: bool,
    },
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let quiet_stdout = matches!(&cli.command, Some(Commands::Export { output: None, .. }))
        || matches!(&cli.command, Some(Commands::InitSkill { stdout: true, .. }))
        || matches!(&cli.command, Some(Commands::Checkin { card: true, .. }));
    if !quiet_stdout {
        banner::print_banner();
    }

    match &cli.command {
        Some(Commands::Checkin { transcript, card }) => {
            if !*card {
                println!(
                    "{} Admitting session for checkin triage...",
                    "[CHECKIN]".bold().green()
                );
            }
            let path =
                discovery::resolve_transcript_path_with_mode(transcript.as_deref(), cli.latest)
                    .map_err(|e| anyhow::anyhow!(e))?;
            let metrics = parser::parse_transcript(&path)?;
            let receipt = triage::calculate_triage(&path, metrics);
            if *card {
                let scan = agent_ooo_core::detox::sanitizer::scan_transcript_secrets(&path)?;
                println!("{}", card::render_vitals_card(&receipt, &scan));
            } else {
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
                if receipt.vitals.api_input_tokens > 0
                    || receipt.vitals.api_cache_read_tokens > 0
                    || receipt.vitals.api_output_tokens > 0
                {
                    println!(
                        "API Ledger: {} in | {} cache_read | {} out (~${:.4} est. session spend)",
                        receipt.vitals.api_input_tokens,
                        receipt.vitals.api_cache_read_tokens,
                        receipt.vitals.api_output_tokens,
                        receipt.vitals.estimated_session_cost_usd
                    );
                }
            }
        }
        Some(Commands::Detox {
            transcript,
            all,
            redact_in_place,
            backup,
        }) => {
            validate_detox_flags(*redact_in_place, *backup)?;
            println!(
                "{} Syntactic cleanse initiated...",
                "[DETOX]".bold().yellow()
            );
            if *redact_in_place {
                handle_detox_in_place(transcript.as_deref(), cli.latest, *all, *backup)?;
                return Ok(());
            }
            if *all {
                let candidates = discovery::discover_recent_transcripts();
                let mut sessions_with_secrets = 0usize;
                let mut total_occurrences = 0usize;
                let mut reports = Vec::new();

                let mut skipped_sessions = 0usize;
                for candidate in &candidates {
                    match agent_ooo_core::detox::sanitizer::scan_transcript_secrets(&candidate.path)
                    {
                        Ok(report) => {
                            if report.distinct_secrets > 0 {
                                sessions_with_secrets += 1;
                                total_occurrences += report.total_occurrences;
                                reports.push((candidate, report));
                            }
                        }
                        Err(_) => {
                            skipped_sessions += 1;
                        }
                    }
                }

                if skipped_sessions > 0 {
                    println!(
                        "Sessions Scanned: {} ({} unreadable skipped) | Sessions With Secrets: {} | Total Occurrences: {}",
                        candidates.len().saturating_sub(skipped_sessions),
                        skipped_sessions,
                        sessions_with_secrets,
                        total_occurrences
                    );
                } else {
                    println!(
                        "Sessions Scanned: {} | Sessions With Secrets: {} | Total Occurrences: {}",
                        candidates.len(),
                        sessions_with_secrets,
                        total_occurrences
                    );
                }

                if sessions_with_secrets == 0 {
                    println!(
                        "{}",
                        "Clean bill of health: No leaked secrets detected across any discovered sessions."
                            .green()
                            .bold()
                    );
                } else {
                    for (candidate, report) in &reports {
                        let session_id = discovery::session_id_for(&candidate.path);
                        println!(
                            "\n[{}] {} ({} distinct, {} total)",
                            candidate.agent_type.bold(),
                            session_id,
                            report.distinct_secrets,
                            report.total_occurrences
                        );
                        for finding in &report.findings {
                            println!(
                                "  * {} [{}]: {} occurrence(s) (lines {}..{})",
                                finding.secret_type.bold(),
                                finding.masked_preview,
                                finding.occurrences,
                                finding.first_line,
                                finding.last_line
                            );
                        }
                    }
                }
                return Ok(());
            }

            let path =
                discovery::resolve_transcript_path_with_mode(transcript.as_deref(), cli.latest)
                    .map_err(|e| anyhow::anyhow!(e))?;
            let current_dir = env::current_dir()?;
            let (_, receipt) = detox_pipeline::execute_detox(&path, &current_dir)
                .map_err(|e| anyhow::anyhow!(e))?;
            let scan = agent_ooo_core::detox::sanitizer::scan_transcript_secrets(&path)?;
            println!(
                "Raw Bytes: {} | Sanitized Bytes: {} | Spilled Payloads: {}",
                receipt.raw_bytes, receipt.sanitized_bytes, receipt.spilled_payloads_count
            );
            println!(
                "Secrets Masked: {} total across {} distinct credential(s)",
                scan.total_occurrences, scan.distinct_secrets
            );
            for finding in &scan.findings {
                println!(
                    "  * {} [{}]: {} occurrence(s) (lines {}..{})",
                    finding.secret_type.bold(),
                    finding.masked_preview,
                    finding.occurrences,
                    finding.first_line,
                    finding.last_line
                );
            }
        }
        Some(Commands::Reset { transcript }) => {
            println!(
                "{} Structural state alignment initiated...",
                "[RESET]".bold().blue()
            );
            let path =
                discovery::resolve_transcript_path_with_mode(transcript.as_deref(), cli.latest)
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
        Some(Commands::Discharge {
            transcript,
            recall,
            offset,
            max_bytes,
        }) => {
            let target_dir = match transcript.as_deref() {
                Some(p) => std::path::PathBuf::from(p),
                None => env::current_dir()?,
            };
            if let Some(handle) = recall {
                println!(
                    "{} Recalling cold-storage CAS payload: {}",
                    "[DISCHARGE]".bold().magenta(),
                    handle
                );
                let rec = agent_ooo_core::detox::spillover::recall_spilled_payload(
                    &target_dir,
                    handle,
                    *offset,
                    *max_bytes,
                )?;
                println!(
                    "Handle: {} | Offset: {}..{} of {} bytes (has_more: {})\n",
                    rec.handle,
                    rec.offset,
                    rec.offset + rec.bytes_read as u64,
                    rec.total_bytes,
                    rec.has_more
                );
                println!("{}", rec.content);
                return Ok(());
            }
            println!(
                "{} Clinical verification initiated...",
                "[DISCHARGE]".bold().magenta()
            );
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
                let path =
                    discovery::resolve_transcript_path_with_mode(transcript.as_deref(), cli.latest)
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
            let path =
                discovery::resolve_transcript_path_with_mode(transcript.as_deref(), cli.latest)
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
                "* State Footprint Reduction: {:.2}%",
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
        Some(Commands::InitSkill {
            target,
            harness,
            hooks,
            force,
            stdout,
        }) => {
            handle_init_skill(target.as_deref(), harness, *hooks, *force, *stdout)?;
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

fn validate_detox_flags(redact_in_place: bool, backup: bool) -> anyhow::Result<()> {
    if backup && !redact_in_place {
        anyhow::bail!("--backup requires --redact-in-place");
    }
    Ok(())
}

fn print_in_place_receipt(receipt: &agent_ooo_core::detox::scrub::InPlaceRedactionReceipt) {
    if receipt.modified {
        println!(
            "Target: {} | Status: {} | Lines Modified: {}/{} | Redactions: {} | Bytes: {} -> {}",
            receipt.target_path,
            "modified".yellow().bold(),
            receipt.lines_modified,
            receipt.lines_scanned,
            receipt.redactions_count,
            receipt.bytes_before,
            receipt.bytes_after
        );
        if let Some(ref bp) = receipt.backup_path {
            println!(
                "  Backup: {} (verified: {})",
                bp,
                if receipt.backup_verified {
                    "yes".green()
                } else {
                    "no".red()
                }
            );
        }
        println!(
            "  BLAKE3: {} -> {}",
            receipt.original_blake3, receipt.final_blake3
        );
    } else {
        println!(
            "Target: {} | Status: {} | Lines Scanned: {} | Bytes: {} | BLAKE3: {}",
            receipt.target_path,
            "clean (unchanged)".green(),
            receipt.lines_scanned,
            receipt.bytes_before,
            receipt.original_blake3
        );
    }
}

fn handle_detox_in_place(
    transcript: Option<&str>,
    latest: bool,
    all: bool,
    backup: bool,
) -> anyhow::Result<()> {
    if all {
        let candidates = discovery::discover_recent_transcripts();
        let mut seen = std::collections::HashSet::new();
        let mut targets = Vec::new();
        for candidate in &candidates {
            for target in agent_ooo_core::detox::scrub::resolve_in_place_targets(&candidate.path) {
                if seen.insert(target.clone()) {
                    targets.push(target);
                }
            }
        }

        let mut scanned = 0usize;
        let mut modified = 0usize;
        let mut total_redactions = 0usize;
        let mut backups_verified = 0usize;
        let mut failed = 0usize;

        for target in &targets {
            match agent_ooo_core::detox::scrub::redact_transcript_in_place(target, backup) {
                Ok(receipt) => {
                    scanned += 1;
                    if receipt.modified {
                        modified += 1;
                        total_redactions += receipt.redactions_count;
                        if receipt.backup_verified {
                            backups_verified += 1;
                        }
                        print_in_place_receipt(&receipt);
                    }
                }
                Err(err) => {
                    failed += 1;
                    eprintln!("  {} {}: {}", "[ERROR]".red().bold(), target.display(), err);
                }
            }
        }

        println!(
            "Targets Scanned: {} | Files Modified: {} | Total Redactions: {} | Backups Verified: {} | Errors: {}",
            scanned, modified, total_redactions, backups_verified, failed
        );
        if modified == 0 && failed == 0 {
            println!(
                "{}",
                "Clean bill of health: No leaked secrets required in-place redaction."
                    .green()
                    .bold()
            );
        }
        if failed > 0 {
            anyhow::bail!("{} target(s) failed during in-place redaction.", failed);
        }
        return Ok(());
    }

    let path = discovery::resolve_transcript_path_with_mode(transcript, latest)
        .map_err(|e| anyhow::anyhow!(e))?;
    let targets = agent_ooo_core::detox::scrub::resolve_in_place_targets(&path);
    let mut seen = std::collections::HashSet::new();
    for target in targets {
        if !seen.insert(target.clone()) {
            continue;
        }
        let receipt = agent_ooo_core::detox::scrub::redact_transcript_in_place(&target, backup)
            .map_err(|e| anyhow::anyhow!(e))?;
        print_in_place_receipt(&receipt);
    }
    Ok(())
}

pub const SKILL_TEMPLATE: &str = include_str!("../../../skills/agent-ooo/SKILL.md");

pub fn handle_init_skill(
    target: Option<&str>,
    harness: &str,
    hooks: bool,
    force: bool,
    stdout: bool,
) -> anyhow::Result<()> {
    let current_dir = env::current_dir()?;
    handle_init_skill_with_base(target, harness, hooks, force, stdout, &current_dir)
}

pub fn handle_init_skill_with_base(
    target: Option<&str>,
    harness: &str,
    hooks: bool,
    force: bool,
    stdout: bool,
    base_dir: &std::path::Path,
) -> anyhow::Result<()> {
    if stdout {
        print!("{}", SKILL_TEMPLATE);
        return Ok(());
    }

    println!(
        "{} Initializing Agent OOO skill bundle...",
        "[SKILL]".bold().cyan()
    );

    let raw_target = target.unwrap_or("./skills/agent-ooo");
    let target_dir = if std::path::Path::new(raw_target).is_absolute() {
        std::path::PathBuf::from(raw_target)
    } else {
        base_dir.join(raw_target)
    };
    std::fs::create_dir_all(&target_dir)?;

    let skill_path = target_dir.join("SKILL.md");
    if skill_path.exists() && !force {
        anyhow::bail!(
            "Skill file already exists at {}. Pass --force to overwrite.",
            skill_path.display()
        );
    }
    std::fs::write(&skill_path, SKILL_TEMPLATE)?;

    println!(
        "  {} Skill template written to: {}",
        "✓".green(),
        skill_path.display().to_string().bold()
    );

    let lower_harness = harness.to_ascii_lowercase();
    match lower_harness.as_str() {
        "antigravity" => {
            println!(
                "  * Antigravity: Discovered at {} or ~/.gemini/antigravity/skills/agent-ooo",
                skill_path.display()
            );
        }
        "claude" => {
            println!(
                "  * Claude Code: Reference via /skill or include in CLAUDE.md / .claude/skills/"
            );
        }
        "cursor" => {
            println!("  * Cursor: Include in .cursor/rules/agent-ooo.mdc or project prompts");
        }
        "codex" => {
            println!("  * OpenAI Codex: Reference in CODEX.md or project system prompt");
        }
        "git" => {
            println!("  * Git: Canary gate configured for .git/hooks/pre-push");
        }
        _ => {
            println!("  * Ready for Antigravity, Claude Code, Cursor, Codex CLI, and Git lifecycle hooks.");
        }
    }

    let should_install_hooks = hooks || lower_harness == "git";
    if should_install_hooks {
        let git_dir = base_dir.join(".git");
        if git_dir.exists() {
            let hooks_dir = git_dir.join("hooks");
            std::fs::create_dir_all(&hooks_dir)?;
            let pre_push_path = hooks_dir.join("pre-push");

            if pre_push_path.exists() && !force {
                anyhow::bail!(
                    "Git pre-push hook already exists at {}. Pass --force to overwrite.",
                    pre_push_path.display()
                );
            }

            let hook_script = r#"#!/usr/bin/env bash
# Agent OOO Git Pre-Push Canary Hook
# Verifies checkpoint integrity, unresolved tool failures, and unverified late edits.

set -euo pipefail

if command -v ooo >/dev/null 2>&1; then
    echo "[agent-ooo] Running discharge canary gate prior to push..."
    ooo discharge
elif command -v agent-ooo >/dev/null 2>&1; then
    echo "[agent-ooo] Running discharge canary gate prior to push..."
    agent-ooo discharge
else
    exit 0
fi
"#;
            std::fs::write(&pre_push_path, hook_script)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata(&pre_push_path) {
                    let mut perms = metadata.permissions();
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(&pre_push_path, perms);
                }
            }

            println!(
                "  {} Git pre-push hook installed to: {}",
                "✓".green(),
                pre_push_path.display().to_string().bold()
            );
        } else {
            println!(
                "  {} No .git directory found at {}; skipped git lifecycle hook.",
                "!".yellow(),
                base_dir.display()
            );
        }
    }

    println!(
        "\n{} Verify installation with: '{}' or '{}'",
        "★".yellow(),
        "ooo checkin".bold(),
        "ooo discharge".bold()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_checkin_card_flag_parsing() {
        let cli_default = Cli::try_parse_from(["ooo", "checkin"]).expect("parse checkin");
        match cli_default.command {
            Some(Commands::Checkin { transcript, card }) => {
                assert_eq!(transcript, None);
                assert!(!card);
            }
            _ => panic!("Expected Commands::Checkin"),
        }

        let cli_card =
            Cli::try_parse_from(["ooo", "checkin", "--card"]).expect("parse checkin --card");
        match cli_card.command {
            Some(Commands::Checkin { transcript, card }) => {
                assert_eq!(transcript, None);
                assert!(card);
            }
            _ => panic!("Expected Commands::Checkin"),
        }

        let cli_card_path = Cli::try_parse_from(["ooo", "checkin", "session.jsonl", "--card"])
            .expect("parse checkin path --card");
        match cli_card_path.command {
            Some(Commands::Checkin { transcript, card }) => {
                assert_eq!(transcript, Some("session.jsonl".to_string()));
                assert!(card);
            }
            _ => panic!("Expected Commands::Checkin"),
        }
    }

    #[test]
    fn test_cli_detox_all_flag_parsing() {
        let args = ["agent-ooo", "detox", "--all"];
        let cli = Cli::try_parse_from(args).expect("parse detox --all");
        match cli.command {
            Some(Commands::Detox {
                transcript,
                all,
                redact_in_place,
                backup,
            }) => {
                assert!(all);
                assert_eq!(transcript, None);
                assert!(!redact_in_place);
                assert!(!backup);
            }
            _ => panic!("Expected Commands::Detox"),
        }

        let args_short = ["agent-ooo", "detox", "-a"];
        let cli_short = Cli::try_parse_from(args_short).expect("parse detox -a");
        match cli_short.command {
            Some(Commands::Detox {
                transcript,
                all,
                redact_in_place,
                backup,
            }) => {
                assert!(all);
                assert_eq!(transcript, None);
                assert!(!redact_in_place);
                assert!(!backup);
            }
            _ => panic!("Expected Commands::Detox"),
        }

        let args_path = ["agent-ooo", "detox", "custom.jsonl"];
        let cli_path = Cli::try_parse_from(args_path).expect("parse detox path");
        match cli_path.command {
            Some(Commands::Detox {
                transcript,
                all,
                redact_in_place,
                backup,
            }) => {
                assert!(!all);
                assert_eq!(transcript, Some("custom.jsonl".to_string()));
                assert!(!redact_in_place);
                assert!(!backup);
            }
            _ => panic!("Expected Commands::Detox"),
        }
    }

    #[test]
    fn test_cli_detox_redact_in_place_flag_parsing() {
        let cli_in_place = Cli::try_parse_from(["ooo", "detox", "--redact-in-place"])
            .expect("parse detox --redact-in-place");
        match cli_in_place.command {
            Some(Commands::Detox {
                transcript,
                all,
                redact_in_place,
                backup,
            }) => {
                assert_eq!(transcript, None);
                assert!(!all);
                assert!(redact_in_place);
                assert!(!backup);
            }
            _ => panic!("Expected Commands::Detox"),
        }

        let cli_backup = Cli::try_parse_from([
            "ooo",
            "detox",
            "--redact-in-place",
            "--backup",
            "session.jsonl",
        ])
        .expect("parse detox --redact-in-place --backup");
        match cli_backup.command {
            Some(Commands::Detox {
                transcript,
                all,
                redact_in_place,
                backup,
            }) => {
                assert_eq!(transcript, Some("session.jsonl".to_string()));
                assert!(!all);
                assert!(redact_in_place);
                assert!(backup);
            }
            _ => panic!("Expected Commands::Detox"),
        }

        let cli_all_in_place =
            Cli::try_parse_from(["ooo", "detox", "--all", "--redact-in-place", "--backup"])
                .expect("parse detox --all --redact-in-place --backup");
        match cli_all_in_place.command {
            Some(Commands::Detox {
                transcript,
                all,
                redact_in_place,
                backup,
            }) => {
                assert_eq!(transcript, None);
                assert!(all);
                assert!(redact_in_place);
                assert!(backup);
            }
            _ => panic!("Expected Commands::Detox"),
        }

        assert!(validate_detox_flags(false, false).is_ok());
        assert!(validate_detox_flags(true, false).is_ok());
        assert!(validate_detox_flags(true, true).is_ok());
        let err = validate_detox_flags(false, true).unwrap_err();
        assert!(err
            .to_string()
            .contains("--backup requires --redact-in-place"));
    }

    #[test]
    fn test_init_skill_parsing_and_execution() {
        let args = [
            "agent-ooo",
            "init-skill",
            "--target",
            "custom-skill",
            "-H",
            "claude",
            "--force",
        ];
        let cli = Cli::try_parse_from(args).expect("parse init-skill");
        match cli.command {
            Some(Commands::InitSkill {
                target,
                harness,
                hooks,
                force,
                stdout,
            }) => {
                assert_eq!(target, Some("custom-skill".to_string()));
                assert_eq!(harness, "claude");
                assert!(!hooks);
                assert!(force);
                assert!(!stdout);
            }
            _ => panic!("Expected Commands::InitSkill"),
        }

        // Test alias init-hooks
        let args_alias = ["ooo", "init-hooks", "--hooks"];
        let cli_alias = Cli::try_parse_from(args_alias).expect("parse init-hooks alias");
        match cli_alias.command {
            Some(Commands::InitSkill { hooks, .. }) => {
                assert!(hooks);
            }
            _ => panic!("Expected Commands::InitSkill via alias"),
        }

        // Test execution with temp directory
        let temp_dir = tempfile::tempdir().unwrap();
        let target_str = temp_dir.path().join("skills/agent-ooo");
        let target_path_str = target_str.to_string_lossy().to_string();

        // 1. Initial generation
        handle_init_skill_with_base(
            Some(&target_path_str),
            "antigravity",
            false,
            false,
            false,
            temp_dir.path(),
        )
        .expect("write skill to temp dir");

        let generated_skill = target_str.join("SKILL.md");
        assert!(generated_skill.exists());
        let content = std::fs::read_to_string(&generated_skill).unwrap();
        assert!(content.contains("name: agent-ooo"));
        assert!(content.contains("## 1. Operational triggers and treatment menu"));

        // 2. Overwrite without force fails
        let err = handle_init_skill_with_base(
            Some(&target_path_str),
            "antigravity",
            false,
            false,
            false,
            temp_dir.path(),
        );
        assert!(err.is_err());

        // 3. Overwrite with force succeeds
        handle_init_skill_with_base(
            Some(&target_path_str),
            "antigravity",
            false,
            true,
            false,
            temp_dir.path(),
        )
        .expect("overwrite with force");

        // 4. Test git hook generation when .git is present
        let git_dir = temp_dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();

        handle_init_skill_with_base(
            Some(&target_path_str),
            "all",
            true,
            true,
            false,
            temp_dir.path(),
        )
        .expect("write hooks");

        let pre_push = git_dir.join("hooks").join("pre-push");
        assert!(pre_push.exists());
        let hook_content = std::fs::read_to_string(&pre_push).unwrap();
        assert!(hook_content.contains("ooo discharge"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&pre_push).unwrap().permissions();
            assert_ne!(perms.mode() & 0o111, 0, "hook must be executable");
        }
    }
}
