//! Step 2: Detox (Syntactic Cleanse & Bloat Eviction)

pub mod arena;
pub mod exporter;
pub mod pipeline;
pub mod sanitizer;
pub mod spillover;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetoxReceipt {
    pub raw_bytes: usize,
    pub sanitized_bytes: usize,
    pub evicted_tokens: usize,
    pub evicted_bytes: u64,
    pub spilled_payloads_count: usize,
    pub redactions_count: usize,
    pub recall_handles: Vec<String>,
}
