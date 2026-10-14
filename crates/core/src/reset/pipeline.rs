use super::{ResetReceipt, registers, serializer};
use crate::detox::arena::MemoryArena;
use std::path::Path;

/// Executes the reset step: severs loops, extracts environment state, writes checkpoints.
pub fn execute_reset(
    session_id: &str,
    workspace_dir: &Path,
    arena: &MemoryArena,
) -> Result<ResetReceipt, crate::OooError> {
    // 1. Loop severing is not implemented yet (planned for v0.2). Report 0 rather than a simulated value.
    let severed_loops_count = 0;

    // 2. Extract registers
    let env_registers = registers::extract_registers(workspace_dir);
    let environment_registers_count = [
        !env_registers.git_branch.is_empty(),
        !env_registers.git_commit.is_empty(),
        !env_registers.working_directory.is_empty(),
        !env_registers.modified_files.is_empty(),
    ]
    .iter()
    .filter(|captured| **captured)
    .count();

    // 3. Serialize
    let (capnp_bytes, json_bytes) = serializer::write_checkpoints(workspace_dir, session_id, &env_registers, arena)?;

    Ok(ResetReceipt {
        severed_loops_count,
        environment_registers_count,
        checkpoint_capnp_bytes: capnp_bytes,
        checkpoint_json_bytes: json_bytes,
    })
}
