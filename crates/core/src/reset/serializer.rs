use crate::schema::session_checkpoint;
use crate::reset::registers::EnvironmentRegisters;
use crate::detox::arena::MemoryArena;
use capnp::message::Builder;
use capnp::serialize_packed;
use std::fs::File;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
    r4.set_value(&registers.modified_files.join(","));
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
