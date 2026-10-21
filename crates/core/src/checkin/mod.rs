//! Step 1: Checkin (Intake, Discovery & Triage)

pub mod discovery;
pub mod parser;
pub mod triage;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WellnessTier {
    Elite,
    #[default]
    Fit,
    Fair,
    Fatigued,
}

impl WellnessTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            WellnessTier::Elite => "Elite",
            WellnessTier::Fit => "Fit",
            WellnessTier::Fair => "Fair",
            WellnessTier::Fatigued => "Fatigued",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolUsageStat {
    pub tool_name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentVitals {
    pub agent_harness: String,
    pub model_id: String,
    pub total_turns: usize,
    pub user_turns: usize,
    pub assistant_turns: usize,
    pub total_tool_calls: usize,
    pub failed_tool_calls: usize,
    pub tool_error_rate_pct: f64,
    pub top_tools: Vec<ToolUsageStat>,
    pub thinking_ratio_pct: f64,
    pub avg_tokens_per_turn: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageReceipt {
    pub session_id: String,
    pub transcript_path: String,
    pub total_input_tokens: usize,
    pub loop_lock_detected: bool,
    pub repetition_count: usize,
    pub degradation_score: f64,
    pub condition: WellnessTier,
    pub vitals: AgentVitals,
}
