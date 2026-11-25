use agent_ooo_core::discharge::canary::CanaryStatus;
use agent_ooo_core::discharge::gate::{
    evaluate_pre_push_gate, parse_pre_push_ranges, CanaryGateReport, GateMode, GateOutcome,
};
use clap::Subcommand;
use colored::Colorize;
use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const PRE_PUSH_MARKER: &str = "# Agent OOO Git Pre-Push Canary Gate";
const LEGACY_PRE_PUSH_MARKER: &str = "# Agent OOO Git Pre-Push Canary Hook";
const MAX_HOOK_STDIN_BYTES: u64 = 65_536;

#[derive(Subcommand)]
pub enum HookAction {
    /// Install the Agent OOO pre-push canary gate in .git/hooks/pre-push.
    Install {
        /// Report canary failures and secret leaks as warnings without blocking git push
        #[arg(long)]
        warn_only: bool,
        /// Overwrite an existing .git/hooks/pre-push script
        #[arg(short, long)]
        force: bool,
        /// Target workspace directory containing .git (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Remove the Agent OOO pre-push hook from .git/hooks/pre-push.
    Uninstall {
        /// Remove .git/hooks/pre-push even if it was not created by Agent OOO
        #[arg(short, long)]
        force: bool,
        /// Target workspace directory containing .git (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Inspect pre-push hook installation status and mode.
    Status {
        /// Target workspace directory containing .git (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Execute the pre-push canary and secret hygiene gate.
    Check {
        /// Report findings as warnings instead of blocking with a non-zero exit code
        #[arg(long)]
        warn_only: bool,
        /// Explicit git commit range or SHA to scan (e.g. origin/main..HEAD)
        #[arg(long)]
        range: Option<String>,
        /// Read git pre-push ref updates from stdin (up to 64 KB)
        #[arg(long)]
        stdin: bool,
        /// Target workspace directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookStatus {
    pub git_present: bool,
    pub hook_path: PathBuf,
    pub installed: bool,
    pub managed_by_ooo: bool,
    pub warn_only: bool,
    pub executable: bool,
    pub checkpoint_present: bool,
}

fn resolve_base_dir(path: Option<&str>) -> anyhow::Result<PathBuf> {
    match path {
        Some(p) => Ok(PathBuf::from(p)),
        None => Ok(env::current_dir()?),
    }
}

/// Builds the POSIX/bash git `pre-push` hook script invoking `ooo hook check`.
pub fn build_pre_push_script(warn_only: bool) -> String {
    let extra_flag = if warn_only { " --warn-only" } else { "" };
    format!(
        "#!/usr/bin/env bash\n\
         {PRE_PUSH_MARKER}\n\
         # Verifies checkpoint integrity, workspace diff canaries, and secret hygiene prior to push.\n\
         \n\
         set -euo pipefail\n\
         \n\
         if command -v ooo >/dev/null 2>&1; then\n\
             ooo hook check{extra_flag} --stdin\n\
         elif command -v agent-ooo >/dev/null 2>&1; then\n\
             agent-ooo hook check{extra_flag} --stdin\n\
         else\n\
             exit 0\n\
         fi\n"
    )
}

/// Installs `.git/hooks/pre-push` under `base_dir` with executable permissions on Unix.
pub fn install_pre_push_hook(
    base_dir: &Path,
    warn_only: bool,
    force: bool,
) -> anyhow::Result<PathBuf> {
    let git_dir = base_dir.join(".git");
    if !git_dir.is_dir() {
        anyhow::bail!(
            "No .git directory found at {}. Run inside a git repository.",
            base_dir.display()
        );
    }

    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;
    let pre_push_path = hooks_dir.join("pre-push");

    if pre_push_path.exists() && !force {
        anyhow::bail!(
            "Git pre-push hook already exists at {}. Pass --force to overwrite.",
            pre_push_path.display()
        );
    }

    let hook_script = build_pre_push_script(warn_only);
    std::fs::write(&pre_push_path, hook_script)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&pre_push_path)?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&pre_push_path, perms)?;
    }

    Ok(pre_push_path)
}

/// Removes `.git/hooks/pre-push` if managed by Agent OOO or if `force` is passed.
pub fn uninstall_pre_push_hook(base_dir: &Path, force: bool) -> anyhow::Result<PathBuf> {
    let pre_push_path = base_dir.join(".git").join("hooks").join("pre-push");
    if !pre_push_path.exists() {
        anyhow::bail!("No git pre-push hook found at {}.", pre_push_path.display());
    }

    let content = std::fs::read_to_string(&pre_push_path).unwrap_or_default();
    let is_managed = content.contains(PRE_PUSH_MARKER) || content.contains(LEGACY_PRE_PUSH_MARKER);

    if !is_managed && !force {
        anyhow::bail!(
            "Hook at {} was not created by Agent OOO. Pass --force to remove it.",
            pre_push_path.display()
        );
    }

    std::fs::remove_file(&pre_push_path)?;
    Ok(pre_push_path)
}

/// Inspects the installation and configuration status of `.git/hooks/pre-push`.
pub fn status_pre_push_hook(base_dir: &Path) -> anyhow::Result<HookStatus> {
    let git_dir = base_dir.join(".git");
    let git_present = git_dir.is_dir();
    let hook_path = git_dir.join("hooks").join("pre-push");
    let installed = hook_path.exists();

    let (managed_by_ooo, warn_only, executable) = if installed {
        let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
        let managed = content.contains(PRE_PUSH_MARKER) || content.contains(LEGACY_PRE_PUSH_MARKER);
        let warn = managed && content.contains("--warn-only");

        #[cfg(unix)]
        let exec = {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(&hook_path)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        };
        #[cfg(not(unix))]
        let exec = true;

        (managed, warn, exec)
    } else {
        (false, false, false)
    };

    let ooo_dir = base_dir.join(".agent-ooo");
    let checkpoint_present =
        ooo_dir.join("checkpoint.capnp").exists() && ooo_dir.join("checkpoint.json").exists();

    Ok(HookStatus {
        git_present,
        hook_path,
        installed,
        managed_by_ooo,
        warn_only,
        executable,
        checkpoint_present,
    })
}

fn read_bounded_stdin_ranges() -> anyhow::Result<Vec<String>> {
    let stdin = std::io::stdin();
    let mut limited = stdin.lock().take(MAX_HOOK_STDIN_BYTES);
    let mut raw = Vec::new();
    limited.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    Ok(parse_pre_push_ranges(&text))
}

/// Runs the pre-push canary gate against `base_dir` and prints a concise summary.
pub fn run_hook_check(
    base_dir: &Path,
    warn_only: bool,
    range: Option<&str>,
    use_stdin: bool,
) -> anyhow::Result<CanaryGateReport> {
    let mut push_ranges = if use_stdin {
        read_bounded_stdin_ranges()?
    } else {
        Vec::new()
    };

    if let Some(explicit_range) = range {
        let trimmed = explicit_range.trim();
        if !trimmed.is_empty() {
            push_ranges.push(trimmed.to_string());
        }
    }

    run_hook_check_with_ranges(base_dir, warn_only, &push_ranges)
}

fn run_hook_check_with_ranges(
    base_dir: &Path,
    warn_only: bool,
    push_ranges: &[String],
) -> anyhow::Result<CanaryGateReport> {
    let mode = if warn_only {
        GateMode::WarnOnly
    } else {
        GateMode::Block
    };
    let report = evaluate_pre_push_gate(base_dir, mode, push_ranges)?;

    let badge = match report.outcome {
        GateOutcome::Pass => "[CANARY GATE] PASS".green().bold(),
        GateOutcome::Warn => "[CANARY GATE] WARN".yellow().bold(),
        GateOutcome::Block => "[CANARY GATE] BLOCK".red().bold(),
    };

    println!(
        "{} ({} file(s) scanned in {:.2} ms)",
        badge, report.files_scanned, report.elapsed_ms
    );

    if let Some(ref canary_report) = report.canary_report {
        let non_passing: Vec<_> = canary_report
            .findings
            .iter()
            .filter(|f| f.status != CanaryStatus::Pass)
            .collect();
        if non_passing.is_empty() {
            println!("  {} Checkpoint canaries: all passed", "[PASS]".green());
        } else {
            for finding in non_passing {
                let f_badge = match finding.status {
                    CanaryStatus::Pass => "[PASS]".green(),
                    CanaryStatus::Warn => "[WARN]".yellow(),
                    CanaryStatus::Fail => "[FAIL]".red(),
                };
                println!("  {} {}: {}", f_badge, finding.code.bold(), finding.message);
            }
        }
    }

    if !report.secret_leaks.is_empty() {
        println!(
            "  {} Detected {} secret leak(s) (masked previews only):",
            "[SECRET]".red().bold(),
            report.secret_leaks.len()
        );
        for leak in &report.secret_leaks {
            println!(
                "    * {}:{} [{}] {}",
                leak.source, leak.line, leak.secret_type, leak.masked_preview
            );
        }
    }

    if report.outcome == GateOutcome::Block {
        anyhow::bail!(
            "Pre-push canary gate blocked push (resolve findings or run with --warn-only)."
        );
    }

    Ok(report)
}

/// Dispatches `ooo hook <subcommand>`.
pub fn handle_hook_command(action: &HookAction) -> anyhow::Result<()> {
    match action {
        HookAction::Install {
            warn_only,
            force,
            path,
        } => {
            let base_dir = resolve_base_dir(path.as_deref())?;
            let hook_path = install_pre_push_hook(&base_dir, *warn_only, *force)?;
            let mode_label = if *warn_only { "warn-only" } else { "block" };
            println!(
                "{} Git pre-push canary gate installed ({}) at: {}",
                "[HOOK]".bold().cyan(),
                mode_label,
                hook_path.display().to_string().bold()
            );
        }
        HookAction::Uninstall { force, path } => {
            let base_dir = resolve_base_dir(path.as_deref())?;
            let hook_path = uninstall_pre_push_hook(&base_dir, *force)?;
            println!(
                "{} Git pre-push hook removed from: {}",
                "[HOOK]".bold().cyan(),
                hook_path.display()
            );
        }
        HookAction::Status { path } => {
            let base_dir = resolve_base_dir(path.as_deref())?;
            let st = status_pre_push_hook(&base_dir)?;
            println!("{} Pre-push hook status:", "[HOOK]".bold().cyan());
            if !st.git_present {
                println!("  Git Repository: {}", "not found (.git missing)".yellow());
                return Ok(());
            }
            println!("  Hook Path: {}", st.hook_path.display());
            println!(
                "  Installed: {}",
                if st.installed {
                    "yes".green()
                } else {
                    "no".yellow()
                }
            );
            if st.installed {
                println!(
                    "  Managed by Agent OOO: {}",
                    if st.managed_by_ooo {
                        "yes".green()
                    } else {
                        "no (custom hook)".yellow()
                    }
                );
                if st.managed_by_ooo {
                    println!(
                        "  Gate Mode: {}",
                        if st.warn_only {
                            "warn-only".yellow()
                        } else {
                            "block".green()
                        }
                    );
                }
                println!(
                    "  Executable: {}",
                    if st.executable {
                        "yes".green()
                    } else {
                        "no".red()
                    }
                );
            }
            println!(
                "  Checkpoint Pair (.agent-ooo/): {}",
                if st.checkpoint_present {
                    "present".green()
                } else {
                    "none".normal()
                }
            );
        }
        HookAction::Check {
            warn_only,
            range,
            stdin,
            path,
        } => {
            let base_dir = resolve_base_dir(path.as_deref())?;
            let _ = run_hook_check(&base_dir, *warn_only, range.as_deref(), *stdin)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn run_git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git should execute");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn test_build_pre_push_script_modes() {
        let block_script = build_pre_push_script(false);
        assert!(block_script.contains(PRE_PUSH_MARKER));
        assert!(block_script.contains("ooo hook check --stdin"));
        assert!(block_script.contains("agent-ooo hook check --stdin"));
        assert!(!block_script.contains("--warn-only"));
        assert!(block_script.contains("exit 0"));

        let warn_script = build_pre_push_script(true);
        assert!(warn_script.contains(PRE_PUSH_MARKER));
        assert!(warn_script.contains("ooo hook check --warn-only --stdin"));
        assert!(warn_script.contains("agent-ooo hook check --warn-only --stdin"));
    }

    #[test]
    fn test_install_status_uninstall_lifecycle() {
        let dir = TempDir::new().unwrap();

        // Missing .git fails install
        assert!(install_pre_push_hook(dir.path(), false, false).is_err());

        std::fs::create_dir_all(dir.path().join(".git")).unwrap();

        let status_empty = status_pre_push_hook(dir.path()).unwrap();
        assert!(status_empty.git_present);
        assert!(!status_empty.installed);

        let installed = install_pre_push_hook(dir.path(), false, false).unwrap();
        assert!(installed.exists());

        // Refuses overwrite without --force
        assert!(install_pre_push_hook(dir.path(), true, false).is_err());

        // Overwrite with --force switches to --warn-only
        install_pre_push_hook(dir.path(), true, true).unwrap();
        let status_warn = status_pre_push_hook(dir.path()).unwrap();
        assert!(status_warn.installed);
        assert!(status_warn.managed_by_ooo);
        assert!(status_warn.warn_only);
        assert!(status_warn.executable);

        // Uninstall removes managed hook
        uninstall_pre_push_hook(dir.path(), false).unwrap();
        assert!(!installed.exists());

        // Custom hook requires --force to uninstall
        std::fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        std::fs::write(&installed, "#!/bin/sh\necho custom\n").unwrap();
        assert!(uninstall_pre_push_hook(dir.path(), false).is_err());
        uninstall_pre_push_hook(dir.path(), true).unwrap();
        assert!(!installed.exists());
    }

    #[test]
    fn test_run_hook_check_blocks_and_warns() {
        let dir = TempDir::new().unwrap();
        run_git(dir.path(), &["init"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(dir.path().join("README.md"), "# Clean\n").unwrap();
        run_git(dir.path(), &["add", "README.md"]);
        run_git(dir.path(), &["commit", "-m", "init"]);

        // Clean check passes
        let pass = run_hook_check(dir.path(), false, None, false).unwrap();
        assert_eq!(pass.outcome, GateOutcome::Pass);

        // Untracked secret blocks by default and warns with warn_only
        let dummy_aws = concat!("AKIA", "ABCDEFGHIJKLMNOP");
        std::fs::write(dir.path().join(".env"), format!("AWS={dummy_aws}\n")).unwrap();

        let blocked = run_hook_check(dir.path(), false, None, false);
        assert!(blocked.is_err());

        let warned = run_hook_check(dir.path(), true, None, false).unwrap();
        assert_eq!(warned.outcome, GateOutcome::Warn);
        assert_eq!(warned.secret_leaks.len(), 1);
    }
}
