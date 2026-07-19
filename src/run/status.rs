use crate::evaluation::{failed_gate_identifiers, EvaluationMetrics, GateStatus};
use crate::run::setup::SetupCommandReport;

#[derive(Debug, Clone, PartialEq)]
pub struct RunStatus {
    pub gate_status: GateStatus,
    pub failed_guardrails: Vec<String>,
    pub judge_passed: Option<bool>,
    pub judge_score: Option<f64>,
    pub judge_error: Option<String>,
}

impl RunStatus {
    pub fn from_metrics(metrics: &EvaluationMetrics) -> Self {
        Self {
            gate_status: metrics.gate_status,
            failed_guardrails: failed_gate_identifiers(&metrics.details),
            judge_passed: metrics.judge_passed,
            judge_score: metrics.judge_score,
            judge_error: metrics.judge_error.clone(),
        }
    }

    pub fn outcome(&self) -> String {
        if self.gate_status == GateStatus::Failed {
            format!("guardrail failed: {}", self.failed_guardrails.join(", "))
        } else if let Some(false) = self.judge_passed {
            format!(
                "judge score {:.2} below threshold",
                self.judge_score.unwrap_or(0.0)
            )
        } else if self.judge_error.is_some() {
            "judge error".to_string()
        } else {
            "completed".to_string()
        }
    }
}

pub fn determine_outcome(metrics: &EvaluationMetrics) -> String {
    RunStatus::from_metrics(metrics).outcome()
}

pub fn setup_failure_outcome(commands: &[SetupCommandReport]) -> String {
    commands
        .iter()
        .position(|command| !command.success)
        .map(|index| {
            format!(
                "setup failed: command {} exited with code {}",
                index + 1,
                commands[index].exit_code
            )
        })
        .unwrap_or_else(|| "setup failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::EvaluationMetrics;
    use crate::interaction_profile::InteractionEvidenceSource;
    use crate::transcript::EfficiencyMetrics;

    fn metrics(
        gate_status: GateStatus,
        details: Vec<crate::evaluation::GateResult>,
        judge_passed: Option<bool>,
        judge_score: Option<f64>,
    ) -> EvaluationMetrics {
        EvaluationMetrics {
            gate_status,
            details,
            judge_score,
            judge_response: None,
            judge_passed,
            judge_threshold: judge_passed.map(|_| 0.7),
            judge_error: None,
            efficiency: EfficiencyMetrics {
                total_commands: 0,
                unique_commands: 0,
                error_count: 0,
                tool_reuse_count: 0,
                help_invocations: 0,
                first_try_success_rate: 0.0,
                iteration_ratio: 0.0,
                completed: false,
            },
            interaction_evidence_source: InteractionEvidenceSource::StructuredToolCalls,
            warnings: vec![],
            composite_score: None,
            evaluator_results: vec![],
        }
    }

    #[test]
    fn status_projects_gate_failure_outcome() {
        let metrics = metrics(
            GateStatus::Failed,
            vec![crate::evaluation::GateResult {
                gate_type: "FileExists".to_string(),
                identifier: "file_exists(summary.md)".to_string(),
                passed: false,
                message: "missing".to_string(),
            }],
            None,
            None,
        );

        let status = RunStatus::from_metrics(&metrics);

        assert_eq!(
            status.outcome(),
            "guardrail failed: file_exists(summary.md)"
        );
    }

    #[test]
    fn status_projects_judge_failure_outcome() {
        let metrics = metrics(GateStatus::Passed, vec![], Some(false), Some(0.42));

        let status = RunStatus::from_metrics(&metrics);

        assert_eq!(status.outcome(), "judge score 0.42 below threshold");
    }

    #[test]
    fn status_projects_judge_execution_error() {
        let mut metrics = metrics(GateStatus::Passed, vec![], None, None);
        metrics.judge_error = Some("invalid judge response".to_string());

        assert_eq!(determine_outcome(&metrics), "judge error");
    }

    #[test]
    fn status_projects_pass_outcome() {
        let metrics = metrics(GateStatus::Passed, vec![], Some(true), Some(0.9));

        let status = RunStatus::from_metrics(&metrics);

        assert_eq!(status.outcome(), "completed");
    }

    #[test]
    fn setup_failure_outcome_identifies_failed_command() {
        let commands = vec![SetupCommandReport {
            command: "exit 7".to_string(),
            exit_code: 7,
            success: false,
            output: String::new(),
        }];

        assert_eq!(
            setup_failure_outcome(&commands),
            "setup failed: command 1 exited with code 7"
        );
    }
}
