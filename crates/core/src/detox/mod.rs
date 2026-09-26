//! Step 2: Detox (Syntactic Cleanse & Bloat Eviction)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetoxReceipt {
    pub raw_bytes: usize,
    pub sanitized_bytes: usize,
    pub evicted_tokens: usize,
    pub spilled_payloads_count: usize,
    pub redactions_count: usize,
}
