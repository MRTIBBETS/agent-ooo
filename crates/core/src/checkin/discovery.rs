use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Finds the most recently modified transcript file across known agent directories.
pub fn discover_latest_transcript() -> Option<PathBuf> {
    let home = env::var("HOME").unwrap_or_else(|_| String::from("~"));
    let mut candidates = Vec::new();

    // 1. Antigravity: ~/.gemini/antigravity/brain/*/transcript.jsonl
    let antigravity_base = Path::new(&home).join(".gemini").join("antigravity").join("brain");
    if let Ok(entries) = fs::read_dir(&antigravity_base) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let transcript_path = entry.path().join("transcript.jsonl");
                if transcript_path.exists() {
                    candidates.push(transcript_path);
                }
            }
        }
    }

    // 2. Claude Code: ~/.claude/logs/*.jsonl
    let claude_base = Path::new(&home).join(".claude").join("logs");
    if let Ok(entries) = fs::read_dir(&claude_base) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "jsonl").unwrap_or(false) {
                candidates.push(entry.path());
            }
        }
    }

    // 3. Cursor: ~/.cursor/sessions/*.jsonl (or .cursor/sessions) - adjusting to likely paths
    // We will just check ~/.cursor/sessions
    let cursor_base = Path::new(&home).join(".cursor").join("sessions");
    if let Ok(entries) = fs::read_dir(&cursor_base) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "jsonl").unwrap_or(false) {
                candidates.push(entry.path());
            }
        }
    }

    // Find the candidate with the most recent modified time
    candidates.into_iter().max_by_key(|path| {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    })
}

/// Resolves the transcript path, either from explicit user input or via auto-discovery.
pub fn resolve_transcript_path(explicit_path: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path_str) = explicit_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            Ok(path)
        } else {
            Err(format!("Explicit transcript path not found: {}", path_str))
        }
    } else {
        discover_latest_transcript().ok_or_else(|| "Could not auto-discover any agent transcripts in ~/.gemini, ~/.claude, or ~/.cursor".to_string())
    }
}
