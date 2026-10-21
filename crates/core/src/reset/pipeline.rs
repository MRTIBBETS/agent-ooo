use super::{analyzer, registers, serializer, ResetReceipt};
use crate::detox::arena::MemoryArena;
use crate::detox::DetoxReceipt;
use std::path::Path;

/// Executes the reset step: severs loops, extracts environment state, writes checkpoints.
pub fn execute_reset(
    session_id: &str,
    workspace_dir: &Path,
    arena: &mut MemoryArena,
    detox_receipt: &DetoxReceipt,
) -> Result<ResetReceipt, crate::OooError> {
    // 1. Analyze arena turns, extract tool events and negative constraints, and sever recursive loops
    let analysis = analyzer::analyze_and_sever_arena(arena);
    let severed_loops_count = analysis.severed_loops_count;
    let event_stream_count = analysis.events.len();
    let constraints_count = analysis.constraints.len();

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
    let (capnp_bytes, json_bytes) = serializer::write_checkpoints(
        workspace_dir,
        session_id,
        &env_registers,
        arena,
        &analysis,
        detox_receipt,
    )?;

    Ok(ResetReceipt {
        severed_loops_count,
        environment_registers_count,
        event_stream_count,
        constraints_count,
        checkpoint_capnp_bytes: capnp_bytes,
        checkpoint_json_bytes: json_bytes,
    })
}
