use super::{canary, report, DischargeCertificate};
use crate::checkin::TriageReceipt;
use crate::detox::DetoxReceipt;
use crate::reset::ResetReceipt;

use std::path::Path;

/// Executes the discharge verification and report generation.
pub fn execute_discharge(
    base_dir: &Path,
    triage: &TriageReceipt,
    detox: &DetoxReceipt,
    reset: &ResetReceipt,
) -> Result<DischargeCertificate, crate::OooError> {
    
    let agent_ooo_dir = base_dir.join(".agent-ooo");
    let canaries_passed = canary::verify_behavioral_canary(&agent_ooo_dir)
        .unwrap_or(false);

    let cert = report::generate_spa_report(base_dir, triage, detox, reset, canaries_passed)?;

    Ok(cert)
}
