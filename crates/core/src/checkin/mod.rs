//! Step 1: Checkin (Intake, Discovery & Triage)

pub mod discovery;
pub mod parser;
pub mod triage;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageReceipt {
    pub session_id: String,
    pub transcript_path: String,
    pub total_input_tokens: usize,
    pub loop_lock_detected: bool,
    pub repetition_count: usize,
    pub degradation_score: f64,
}
