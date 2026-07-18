use crate::interaction_profile::InteractionProfile;

use super::GateResult;

pub(super) fn eval_no_transcript_errors(interaction_profile: &InteractionProfile) -> GateResult {
    let no_errors = interaction_profile.metrics.error_count == 0;
    GateResult {
        gate_type: "NoTranscriptErrors".to_string(),
        identifier: "no_transcript_errors".to_string(),
        passed: no_errors,
        message: format!(
            "Interaction-quality guardrail found zero target-tool command errors: {}",
            no_errors
        ),
    }
}
