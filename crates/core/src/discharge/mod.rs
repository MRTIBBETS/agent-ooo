//! Step 4: Discharge (Clinical Canary Verification & Spa Report)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DischargeCertificate {
    pub canaries_passed: bool,
    pub token_reduction_percentage: f64,
    pub developer_hours_saved: f64,
    pub net_savings_usd: f64,
    pub spa_report_path: String,
}
