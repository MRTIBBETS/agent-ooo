//! Step 4: Discharge (Exit Exam & Spa Report)

pub mod canary;
pub mod pipeline;
pub mod report;

use crate::checkin::{AgentVitals, WellnessTier};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DischargeCertificate {
    pub status: WellnessTier,
    pub checkin_condition: WellnessTier,
    pub rot_index: f64,
    pub vitals: AgentVitals,
    pub canaries_passed: bool,
    pub canary_report: canary::CanaryReport,
    pub token_reduction_percentage: f64,
    pub developer_hours_saved: f64,
    pub net_savings_usd: f64,
    pub spa_report_path: String,
    pub execution_time_ms: f64,
}
