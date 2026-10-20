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

/// Pushes every `*.jsonl` file directly inside `dir`.
fn scan_jsonl_dir(candidates: &mut Vec<SessionCandidate>, dir: &Path, agent_type: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            push_candidate(candidates, path, agent_type);
        }
    }
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
            "Could not auto-discover any agent transcripts in ~/.gemini, ~/.claude, or ~/.cursor"
                .to_string()
        })
}

/// Resolves the transcript path, either from explicit user input or via interactive auto-discovery.
pub fn resolve_transcript_path(explicit_path: Option<&str>) -> Result<PathBuf, String> {
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
            "Could not auto-discover any agent transcripts in ~/.gemini, ~/.claude, or ~/.cursor"
                .to_string(),
        );
    }

    if candidates.len() == 1 {
        return Ok(candidates[0].path.clone());
    }

    // Non-interactive callers (scripts, CI, other agents) get the most recent session
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprintln!(
            "No terminal detected. Using most recent session: {}",
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

        options.push(format!(
            "{:<15} (last active: {})",
            candidate.agent_type, time_str
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
}
