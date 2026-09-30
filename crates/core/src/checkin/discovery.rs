use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use dialoguer::{Select, theme::ColorfulTheme};

#[derive(Debug, Clone)]
struct SessionCandidate {
    path: PathBuf,
    modified: std::time::SystemTime,
    agent_type: String,
}

/// Finds the most recently modified transcript files across known agent directories.
fn discover_recent_transcripts() -> Vec<SessionCandidate> {
    let home = env::var("HOME").unwrap_or_else(|_| String::from("~"));
    let mut candidates = Vec::new();

    // 1. Antigravity: ~/.gemini/antigravity/brain/*/transcript.jsonl
    let antigravity_base = Path::new(&home).join(".gemini").join("antigravity").join("brain");
    if let Ok(entries) = fs::read_dir(&antigravity_base) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let transcript_path = entry.path().join("transcript.jsonl");
                if let Ok(metadata) = fs::metadata(&transcript_path) {
                    candidates.push(SessionCandidate {
                        path: transcript_path,
                        modified: metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                        agent_type: "Antigravity".to_string(),
                    });
                }
            }
        }
    }

    // 2. Claude Code: ~/.claude/logs/*.jsonl
    let claude_base = Path::new(&home).join(".claude").join("logs");
    if let Ok(entries) = fs::read_dir(&claude_base) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Ok(metadata) = fs::metadata(entry.path()) {
                    candidates.push(SessionCandidate {
                        path: entry.path(),
                        modified: metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                        agent_type: "Claude Code".to_string(),
                    });
                }
            }
        }
    }

    // 3. Cursor: ~/.cursor/sessions/*.jsonl
    let cursor_base = Path::new(&home).join(".cursor").join("sessions");
    if let Ok(entries) = fs::read_dir(&cursor_base) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Ok(metadata) = fs::metadata(entry.path()) {
                    candidates.push(SessionCandidate {
                        path: entry.path(),
                        modified: metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                        agent_type: "Cursor".to_string(),
                    });
                }
            }
        }
    }

    // Sort by most recent first
    candidates.sort_by(|a, b| b.modified.cmp(&a.modified));
    candidates
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
        return Err("Could not auto-discover any agent transcripts in ~/.gemini, ~/.claude, or ~/.cursor".to_string());
    }

    if candidates.len() == 1 {
        return Ok(candidates[0].path.clone());
    }

    // Interactive prompt for top 5 candidates
    let display_count = candidates.len().min(5);
    let mut options = Vec::new();

    let now = std::time::SystemTime::now();

    for candidate in candidates.iter().take(display_count) {
        let elapsed = now.duration_since(candidate.modified).unwrap_or(std::time::Duration::from_secs(0));
        let mins = elapsed.as_secs() / 60;
        let time_str = if mins == 0 { "just now".to_string() } else if mins < 60 { format!("{} mins ago", mins) } else { format!("{} hrs ago", mins / 60) };
        
        options.push(format!("{:<15} (last active: {})", candidate.agent_type, time_str));
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
