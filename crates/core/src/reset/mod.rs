//! Step 3: Reset (Structural State Alignment & Loop Severance)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetReceipt {
    pub severed_loops_count: usize,
    pub environment_registers_count: usize,
    pub checkpoint_capnp_bytes: usize,
    pub checkpoint_json_bytes: usize,
}
