use dialoguer::{theme::ColorfulTheme, Select};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SessionCandidate {
    pub path: PathBuf,
    pub modified: std::time::SystemTime,
    pub agent_type: String,
    pub size_bytes: u64,
}

fn push_candidate(candidates: &mut Vec<SessionCandidate>, path: PathBuf, agent_type: &str) {
    if let Ok(metadata) = fs::metadata(&path) {
        if metadata.is_file() {
            candidates.push(SessionCandidate {
                path,
                modified: metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                agent_type: agent_type.to_string(),
                size_bytes: metadata.len(),
            });
        }
    }
}

/// Scans `<brain_dir>/<session>/` for a transcript. Prefers the current
/// `.system_generated/logs/transcript.jsonl` layout and falls back to the legacy flat layout.
fn scan_antigravity_brain(
    candidates: &mut Vec<SessionCandidate>,
    brain_dir: &Path,
    agent_type: &str,
) {
    let Ok(entries) = fs::read_dir(brain_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let current = entry
            .path()
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");
        let legacy = entry.path().join("transcript.jsonl");
        if current.is_file() {
            push_candidate(candidates, current, agent_type);
        } else if legacy.is_file() {
            push_candidate(candidates, legacy, agent_type);
        }
    }
}

/// Recursively scans `dir` up to `max_depth` for `*.jsonl` files (capped at 512 files per tree).
/// `max_depth = 1` checks files directly in `dir`.
pub fn scan_jsonl_tree(
    candidates: &mut Vec<SessionCandidate>,
    dir: &Path,
    agent_type: &str,
    max_depth: usize,
) {
    const MAX_TREE_CANDIDATES: usize = 512;
    if max_depth == 0 || candidates.len() >= MAX_TREE_CANDIDATES * 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten().take(MAX_TREE_CANDIDATES) {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_file() {
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                push_candidate(candidates, path, agent_type);
            }
        } else if file_type.is_dir() && max_depth > 1 {
            let dir_name = entry.file_name();
            if dir_name == "node_modules" || dir_name == ".git" || dir_name == "target" {
                continue;
            }
            scan_jsonl_tree(candidates, &path, agent_type, max_depth - 1);
        }
    }
}

/// Pushes every `*.jsonl` file directly inside `dir`.
fn scan_jsonl_dir(candidates: &mut Vec<SessionCandidate>, dir: &Path, agent_type: &str) {
    scan_jsonl_tree(candidates, dir, agent_type, 1);
}

/// Finds the most recently modified transcript files across known agent directories.
pub fn discover_recent_transcripts() -> Vec<SessionCandidate> {
    let home = env::var("HOME").unwrap_or_else(|_| String::from("~"));
    let home = Path::new(&home);
    let mut candidates = Vec::new();

    // 1. Antigravity: ~/.gemini/antigravity/brain/<id>/.system_generated/logs/transcript.jsonl
    scan_antigravity_brain(
        &mut candidates,
        &home.join(".gemini").join("antigravity").join("brain"),
        "Antigravity",
    );

    // 2. Antigravity IDE: ~/.gemini/antigravity-ide/brain/<id>/.system_generated/logs/transcript.jsonl
    scan_antigravity_brain(
        &mut candidates,
        &home.join(".gemini").join("antigravity-ide").join("brain"),
        "Antigravity IDE",
    );

    // 3. Claude Code: ~/.claude/projects/<project>/<session>.jsonl (legacy: ~/.claude/logs/*.jsonl)
    if let Ok(projects) = fs::read_dir(home.join(".claude").join("projects")) {
        for project in projects.flatten() {
            if project.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                scan_jsonl_dir(&mut candidates, &project.path(), "Claude Code");
            }
        }
    }
    scan_jsonl_dir(
        &mut candidates,
        &home.join(".claude").join("logs"),
        "Claude Code",
    );

    // 4. Cursor: ~/.cursor/sessions/*.jsonl (unverified layout)
    scan_jsonl_dir(
        &mut candidates,
        &home.join(".cursor").join("sessions"),
        "Cursor",
    );

    // 5. Codex CLI: ~/.codex/sessions/**/*.jsonl (depth up to 4)
    scan_jsonl_tree(
        &mut candidates,
        &home.join(".codex").join("sessions"),
        "Codex CLI",
        4,
    );

    // 6. Windsurf: ~/.codeium/windsurf and ~/.windsurf/sessions (*.jsonl up to depth 3)
    scan_jsonl_tree(
        &mut candidates,
        &home.join(".codeium").join("windsurf"),
        "Windsurf",
        3,
    );
    scan_jsonl_tree(
        &mut candidates,
        &home.join(".windsurf").join("sessions"),
        "Windsurf",
        3,
    );

    // 7. Cline: ~/.cline and ~/.roo-code (*.jsonl up to depth 3)
    scan_jsonl_tree(&mut candidates, &home.join(".cline"), "Cline", 3);
    scan_jsonl_tree(&mut candidates, &home.join(".roo-code"), "Cline", 3);

    // Sort by most recent first
    candidates.sort_by_key(|c| std::cmp::Reverse(c.modified));
    candidates
}

/// Derives a stable session identifier from a transcript path.
/// Antigravity transcripts live under `<session-id>/.system_generated/logs/`, so the
/// session directory is used. Other transcripts use the file stem (Claude Code names files by session UUID).
pub fn session_id_for(path: &Path) -> String {
    let components: Vec<_> = path.components().collect();
    if let Some(idx) = components
        .iter()
        .position(|c| c.as_os_str() == ".system_generated")
    {
        if idx > 0 {
            return components[idx - 1]
                .as_os_str()
                .to_string_lossy()
                .to_string();
        }
    }
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "transcript.jsonl" || n == "transcript_full.jsonl")
        .unwrap_or(false)
    {
        if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
            return parent.to_string_lossy().to_string();
        }
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown_session".to_string())
}

/// Identifies the originating agent harness from a transcript path.
pub fn agent_harness_for(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.contains("antigravity-ide") {
        "Antigravity IDE".to_string()
    } else if s.contains("antigravity") || s.contains(".system_generated") {
        "Antigravity".to_string()
    } else if s.contains(".claude") {
        "Claude Code".to_string()
    } else if s.contains(".cursor") {
        "Cursor".to_string()
    } else if s.contains(".codex") {
        "Codex CLI".to_string()
    } else if s.contains("windsurf") || s.contains(".codeium") {
        "Windsurf".to_string()
    } else if s.contains(".cline") || s.contains(".roo-code") {
        "Cline".to_string()
    } else {
        "Generic JSONL".to_string()
    }
}

/// Prefers the untruncated sibling `transcript_full.jsonl` when `path` points to `transcript.jsonl`
/// and `transcript_full.jsonl` exists in the same directory. Also resolves session directories
/// directly to their nested transcript file.
pub fn resolve_scan_target(path: &Path) -> PathBuf {
    if path.is_dir() {
        let candidates = [
            path.join(".system_generated")
                .join("logs")
                .join("transcript_full.jsonl"),
            path.join(".system_generated")
                .join("logs")
                .join("transcript.jsonl"),
            path.join("transcript_full.jsonl"),
            path.join("transcript.jsonl"),
        ];
        for candidate in candidates {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    if path.file_name().and_then(|n| n.to_str()) == Some("transcript.jsonl") {
        let full_path = path.with_file_name("transcript_full.jsonl");
        if full_path.is_file() {
            return full_path;
        }
    }
    path.to_path_buf()
}

/// Resolves the transcript path without terminal interaction or stdout/stderr writes.
/// Selects `explicit_path` if provided, or the most recent discovered transcript.
pub fn resolve_transcript_non_interactive(explicit_path: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path_str) = explicit_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            return Ok(path);
        } else {
            return Err(format!("Explicit transcript path not found: {}", path_str));
        }
    }

    let candidates = discover_recent_transcripts();
    candidates
        .into_iter()
        .next()
        .map(|c| c.path)
        .ok_or_else(|| {
            "Could not auto-discover any agent transcripts in ~/.gemini, ~/.claude, ~/.codex, or ~/.cursor"
                .to_string()
        })
}

fn is_interactive_terminal() -> bool {
    let term_ok = env::var("TERM")
        .map(|t| !t.is_empty() && !t.eq_ignore_ascii_case("dumb"))
        .unwrap_or(false);
    std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
        && term_ok
        && env::var_os("CI").is_none()
        && env::var_os("AGENT_OOO_NON_INTERACTIVE").is_none()
        && env::var_os("ANTIGRAVITY_AGENT").is_none()
        && env::var_os("ANTIGRAVITY_CLI_ALIAS").is_none()
        && env::var_os("CLAUDECODE").is_none()
}

/// Resolves the transcript path, either from explicit user input or via auto-discovery.
/// When `latest` is true (or when running in a non-interactive/agent environment), selects
/// the most recently modified transcript without prompting.
pub fn resolve_transcript_path_with_mode(
    explicit_path: Option<&str>,
    latest: bool,
) -> Result<PathBuf, String> {
    if let Some(path_str) = explicit_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            return Ok(path);
        } else {
            return Err(format!("Explicit transcript path not found: {}", path_str));
        }
    }

    let candidates = discover_recent_transcripts();

    if candidates.is_empty() {
        return Err(
            "Could not auto-discover any agent transcripts in ~/.gemini, ~/.claude, ~/.codex, or ~/.cursor"
                .to_string(),
        );
    }

    if candidates.len() == 1 || latest {
        return Ok(candidates[0].path.clone());
    }

    // Non-interactive callers (scripts, CI, other agents) get the most recent session
    if !is_interactive_terminal() {
        eprintln!(
            "Non-interactive session detected. Using most recent session: {}",
            candidates[0].path.display()
        );
        return Ok(candidates[0].path.clone());
    }

    // Interactive prompt for top 5 candidates
    let display_count = candidates.len().min(5);
    let mut options = Vec::new();

    let now = std::time::SystemTime::now();

    for candidate in candidates.iter().take(display_count) {
        let elapsed = now
            .duration_since(candidate.modified)
            .unwrap_or(std::time::Duration::from_secs(0));
        let mins = elapsed.as_secs() / 60;
        let time_str = if mins == 0 {
            "just now".to_string()
        } else if mins < 60 {
            format!("{} mins ago", mins)
        } else {
            format!("{} hrs ago", mins / 60)
        };

        let sess_id = session_id_for(&candidate.path);
        let short_id: String = sess_id.chars().take(12).collect();
        let kb = candidate.size_bytes / 1024;
        let size_str = if kb >= 1024 {
            format!("{:.1} MB", kb as f64 / 1024.0)
        } else {
            format!("{} KB", kb.max(1))
        };

        options.push(format!(
            "{:<15} [{:<12}] {:>7} (last active: {})",
            candidate.agent_type, short_id, size_str, time_str
        ));
    }

    println!();
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Which agent session would you like to check into the spa?")
        .default(0)
        .items(&options)
        .interact()
        .map_err(|e| e.to_string())?;

    Ok(candidates[selection].path.clone())
}

/// Resolves the transcript path, either from explicit user input or via interactive auto-discovery.
pub fn resolve_transcript_path(explicit_path: Option<&str>) -> Result<PathBuf, String> {
    resolve_transcript_path_with_mode(explicit_path, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_antigravity_system_generated() {
        let p = Path::new(
            "/h/.gemini/antigravity/brain/abc-123/.system_generated/logs/transcript.jsonl",
        );
        assert_eq!(session_id_for(p), "abc-123");
    }

    #[test]
    fn session_id_antigravity_legacy() {
        let p = Path::new("/h/.gemini/antigravity/brain/abc-123/transcript.jsonl");
        assert_eq!(session_id_for(p), "abc-123");
    }

    #[test]
    fn session_id_claude_code() {
        let p = Path::new("/h/.claude/projects/-Users-me/882c47e8-b7ab.jsonl");
        assert_eq!(session_id_for(p), "882c47e8-b7ab");
    }

    #[test]
    fn resolve_scan_target_prefers_sibling_transcript_full() {
        let dir = tempfile::TempDir::new().unwrap();
        let compact = dir.path().join("transcript.jsonl");
        let full = dir.path().join("transcript_full.jsonl");
        fs::write(&compact, "{}\n").unwrap();
        assert_eq!(resolve_scan_target(&compact), compact);

        fs::write(&full, "{}\n").unwrap();
        assert_eq!(resolve_scan_target(&compact), full);
    }

    #[test]
    fn test_scan_jsonl_tree_nested() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let nested = dir.path().join("2026").join("11").join("13");
        fs::create_dir_all(&nested).expect("create nested dirs");

        let file1 = nested.join("rollout-abc.jsonl");
        fs::write(&file1, "{}\n").expect("write file");

        let flat = dir.path().join("flat-session.jsonl");
        fs::write(&flat, "{}\n").expect("write file");

        // file at depth 5: dir -> a (1) -> b (2) -> c (3) -> d (4) -> deep.jsonl (5)
        let too_deep = dir.path().join("a").join("b").join("c").join("d");
        fs::create_dir_all(&too_deep).expect("create too deep");
        let file_deep = too_deep.join("deep.jsonl");
        fs::write(&file_deep, "{}\n").expect("write deep file");

        let mut candidates = Vec::new();
        scan_jsonl_tree(&mut candidates, dir.path(), "Codex CLI", 4);

        let paths: Vec<_> = candidates.iter().map(|c| c.path.clone()).collect();
        assert!(paths.contains(&file1));
        assert!(paths.contains(&flat));
        assert!(!paths.contains(&file_deep));
    }

    #[test]
    fn test_agent_harness_for() {
        assert_eq!(
            agent_harness_for(Path::new(
                "/home/user/.codex/sessions/2026/11/13/session.jsonl"
            )),
            "Codex CLI"
        );
        assert_eq!(
            agent_harness_for(Path::new(
                "/home/user/.codeium/windsurf/cascade/session.jsonl"
            )),
            "Windsurf"
        );
        assert_eq!(
            agent_harness_for(Path::new("/home/user/.windsurf/sessions/session.jsonl")),
            "Windsurf"
        );
        assert_eq!(
            agent_harness_for(Path::new("/home/user/.cline/tasks/session.jsonl")),
            "Cline"
        );
        assert_eq!(
            agent_harness_for(Path::new("/home/user/.roo-code/session.jsonl")),
            "Cline"
        );
        assert_eq!(
            agent_harness_for(Path::new("/home/user/.claude/projects/proj/session.jsonl")),
            "Claude Code"
        );
        assert_eq!(
            agent_harness_for(Path::new("/home/user/.cursor/sessions/session.jsonl")),
            "Cursor"
        );
        assert_eq!(
            agent_harness_for(Path::new(
                "/home/user/.gemini/antigravity/brain/session/transcript.jsonl"
            )),
            "Antigravity"
        );
        assert_eq!(
            agent_harness_for(Path::new(
                "/home/user/.gemini/antigravity-ide/brain/session/transcript.jsonl"
            )),
            "Antigravity IDE"
        );
    }

    #[test]
    fn test_session_id_for_codex_and_windsurf() {
        let codex = Path::new("/home/user/.codex/sessions/2026/11/13/rollout-abc.jsonl");
        assert_eq!(session_id_for(codex), "rollout-abc");

        let windsurf = Path::new("/home/user/.windsurf/sessions/cascade-session-123.jsonl");
        assert_eq!(session_id_for(windsurf), "cascade-session-123");

        let windsurf_transcript =
            Path::new("/home/user/.codeium/windsurf/cascade/session-456/transcript.jsonl");
        assert_eq!(session_id_for(windsurf_transcript), "session-456");
    }
}
