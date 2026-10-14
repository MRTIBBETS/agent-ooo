use std::path::Path;

/// Verifies checkpoint integrity: both checkpoint files exist, the Cap'n Proto file is
/// non-empty, and the JSON checkpoint parses with a non-empty session ID.
///
/// This is a structural check only. A behavioral probe (dry-running an LLM on the
/// checkpoint to confirm task awareness) is not implemented yet.
pub fn verify_behavioral_canary(checkpoint_dir: &Path) -> Result<bool, String> {
    let capnp_len = std::fs::metadata(checkpoint_dir.join("checkpoint.capnp"))
        .map(|m| m.len())
        .map_err(|e| format!("checkpoint.capnp missing: {e}"))?;
    if capnp_len == 0 {
        return Ok(false);
    }

    let json_str = std::fs::read_to_string(checkpoint_dir.join("checkpoint.json"))
        .map_err(|e| format!("checkpoint.json missing: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let has_session = json
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    Ok(has_session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fails_when_checkpoint_missing() {
        let dir = TempDir::new().unwrap();
        assert!(verify_behavioral_canary(dir.path()).is_err());
    }

    #[test]
    fn passes_with_valid_checkpoint() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("checkpoint.capnp"), b"\x00\x01").unwrap();
        std::fs::write(dir.path().join("checkpoint.json"), r#"{"session_id":"abc"}"#).unwrap();
        assert_eq!(verify_behavioral_canary(dir.path()), Ok(true));
    }
}
