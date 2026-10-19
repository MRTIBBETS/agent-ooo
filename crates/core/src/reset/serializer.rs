use crate::schema::session_checkpoint;
use crate::reset::registers::EnvironmentRegisters;
use crate::detox::arena::MemoryArena;
use capnp::message::{Builder, ReaderOptions};
use capnp::serialize_packed;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterSnapshot {
    pub key: String,
    pub value: String,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointInspection {
    pub integrity_passed: bool,
    pub session_id: String,
    pub timestamp: u64,
    pub checkpoint_capnp_bytes: u64,
    pub checkpoint_json_bytes: u64,
    pub registers: Vec<RegisterSnapshot>,
    pub arena_gen0_count: usize,
    pub arena_transient_count: usize,
    pub evicted_bytes: u64,
}

pub fn write_checkpoints(
    base_dir: &Path,
    session_id: &str,
    registers: &EnvironmentRegisters,
    arena: &MemoryArena,
) -> Result<(usize, usize), crate::OooError> {
    let agent_ooo_dir = base_dir.join(".agent-ooo");
    std::fs::create_dir_all(&agent_ooo_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 1. Cap'n Proto Checkpoint
    let mut message = Builder::new_default();
    let mut checkpoint = message.init_root::<session_checkpoint::Builder>();

    checkpoint.set_session_id(session_id);
    checkpoint.set_timestamp(timestamp);

    let mut reg_list = checkpoint.reborrow().init_registers(4);
    
    // Branch
    let mut r1 = reg_list.reborrow().get(0);
    r1.set_key("git_branch");
    r1.set_value(&registers.git_branch);
    r1.set_origin(crate::schema::TrustOrigin::System);

    // Commit
    let mut r2 = reg_list.reborrow().get(1);
    r2.set_key("git_commit");
    r2.set_value(&registers.git_commit);
    r2.set_origin(crate::schema::TrustOrigin::System);

    // Working Dir
    let mut r3 = reg_list.reborrow().get(2);
    r3.set_key("working_directory");
    r3.set_value(&registers.working_directory);
    r3.set_origin(crate::schema::TrustOrigin::System);

    // Modified Files
    let mut r4 = reg_list.reborrow().get(3);
    r4.set_key("modified_files");
    r4.set_value(registers.modified_files.join(","));
    r4.set_origin(crate::schema::TrustOrigin::System);

    // Evicted info placeholder (from arena size logic)
    let mut loss = checkpoint.reborrow().init_loss_manifest();
    loss.set_evicted_bytes(0); // We would populate from detox receipt in full logic

    let capnp_path = agent_ooo_dir.join("checkpoint.capnp");
    let mut capnp_file = File::create(&capnp_path)?;
    serialize_packed::write_message(&mut capnp_file, &message)?;
    let capnp_bytes = capnp_file.metadata()?.len() as usize;

    // 2. JSON Checkpoint
    let json_val = serde_json::json!({
        "session_id": session_id,
        "timestamp": timestamp,
        "registers": {
            "git_branch": registers.git_branch,
            "git_commit": registers.git_commit,
            "working_directory": registers.working_directory,
            "modified_files": registers.modified_files,
        },
        "arena": {
            "gen0_count": arena.gen0().len(),
            "transient_count": arena.transient().len(),
        }
    });

    let json_path = agent_ooo_dir.join("checkpoint.json");
    let json_str = serde_json::to_string_pretty(&json_val)?;
    std::fs::write(&json_path, &json_str)?;
    let json_bytes = json_str.len();

    Ok((capnp_bytes, json_bytes))
}

/// Reads and verifies `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json` inside `base_dir`.
pub fn inspect_checkpoint(base_dir: &Path) -> Result<CheckpointInspection, crate::OooError> {
    let agent_ooo_dir = if base_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == ".agent-ooo")
        .unwrap_or(false)
    {
        base_dir.to_path_buf()
    } else {
        base_dir.join(".agent-ooo")
    };

    let capnp_path = agent_ooo_dir.join("checkpoint.capnp");
    let json_path = agent_ooo_dir.join("checkpoint.json");

    let capnp_bytes = std::fs::metadata(&capnp_path)?.len();
    let json_bytes = std::fs::metadata(&json_path)?.len();

    let capnp_file = File::open(&capnp_path)?;
    let mut reader = BufReader::new(capnp_file);
    let message_reader = serialize_packed::read_message(&mut reader, ReaderOptions::new())?;
    let root = message_reader.get_root::<session_checkpoint::Reader>()?;

    let session_id = root
        .get_session_id()?
        .to_str()
        .map_err(|e| crate::OooError::General(format!("Invalid UTF-8 in session_id: {e}")))?
        .to_string();
    let timestamp = root.get_timestamp();

    let mut registers = Vec::new();
    for reg in root.get_registers()?.iter() {
        let key = reg
            .get_key()?
            .to_str()
            .map_err(|e| crate::OooError::General(format!("Invalid UTF-8 in register key: {e}")))?
            .to_string();
        let value = reg
            .get_value()?
            .to_str()
            .map_err(|e| crate::OooError::General(format!("Invalid UTF-8 in register value: {e}")))?
            .to_string();
        let origin = match reg.get_origin() {
            Ok(crate::schema::TrustOrigin::System) => "system",
            Ok(crate::schema::TrustOrigin::User) => "user",
            Ok(crate::schema::TrustOrigin::Model) => "model",
            Ok(crate::schema::TrustOrigin::Tool) => "tool",
            Err(_) => "unknown",
        }
        .to_string();
        registers.push(RegisterSnapshot { key, value, origin });
    }

    let evicted_bytes = root.get_loss_manifest()?.get_evicted_bytes();

    let json_str = std::fs::read_to_string(&json_path)?;
    let json_val: serde_json::Value = serde_json::from_str(&json_str)?;

    let json_session_id = json_val
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let arena_gen0_count = json_val
        .get("arena")
        .and_then(|a| a.get("gen0_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let arena_transient_count = json_val
        .get("arena")
        .and_then(|a| a.get("transient_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let canary_ok = crate::discharge::canary::verify_behavioral_canary(&agent_ooo_dir)
        .unwrap_or(false);
    let integrity_passed = canary_ok && !session_id.is_empty() && session_id == json_session_id;

    Ok(CheckpointInspection {
        integrity_passed,
        session_id,
        timestamp,
        checkpoint_capnp_bytes: capnp_bytes,
        checkpoint_json_bytes: json_bytes,
        registers,
        arena_gen0_count,
        arena_transient_count,
        evicted_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn roundtrip_write_and_inspect_checkpoint() {
        let dir = TempDir::new().unwrap();
        let regs = EnvironmentRegisters {
            git_branch: "main".to_string(),
            git_commit: "deadbeef".to_string(),
            working_directory: "/tmp/work".to_string(),
            modified_files: vec!["src/lib.rs".to_string()],
        };
        let mut arena = MemoryArena::new(10);
        arena.pin_gen0(json!({"source": "SYSTEM", "content": "rule"}));
        arena.push_transient(json!({"source": "USER", "content": "hi"}));

        let (capnp_len, json_len) =
            write_checkpoints(dir.path(), "sess-42", &regs, &arena).unwrap();
        assert!(capnp_len > 0);
        assert!(json_len > 0);

        let inspection = inspect_checkpoint(dir.path()).unwrap();
        assert!(inspection.integrity_passed);
        assert_eq!(inspection.session_id, "sess-42");
        assert_eq!(inspection.checkpoint_capnp_bytes, capnp_len as u64);
        assert_eq!(inspection.checkpoint_json_bytes, json_len as u64);
        assert_eq!(inspection.arena_gen0_count, 1);
        assert_eq!(inspection.arena_transient_count, 1);
        assert_eq!(inspection.registers.len(), 4);
        assert_eq!(inspection.registers[0].key, "git_branch");
        assert_eq!(inspection.registers[0].value, "main");
    }
}
