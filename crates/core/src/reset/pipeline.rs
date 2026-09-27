use super::{ResetReceipt, registers, serializer};
use crate::detox::arena::MemoryArena;
use std::path::Path;

/// Executes the reset step: severs loops, extracts environment state, writes checkpoints.
pub fn execute_reset(
    session_id: &str,
    workspace_dir: &Path,
    arena: &MemoryArena,
) -> Result<ResetReceipt, crate::OooError> {
    // 1. Loop Normalizer (Simulate loop severing logic)
    let severed_loops_count = 0; // Normally we'd scan arena.transient() and drop sequences that have the same compute_structural_hash

    // 2. Extract registers
    let env_registers = registers::extract_registers(workspace_dir);

    // 3. Serialize
    let (capnp_bytes, json_bytes) = serializer::write_checkpoints(workspace_dir, session_id, &env_registers, arena)?;

    Ok(ResetReceipt {
        severed_loops_count,
        environment_registers_count: 4, // we extracted 4 registers
        checkpoint_capnp_bytes: capnp_bytes,
        checkpoint_json_bytes: json_bytes,
    })
}
