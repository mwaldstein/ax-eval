use crate::evaluation::EvaluationMetrics;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunStatus {
    pub gates_passed: bool,
    pub judge_passed: Option<bool>,
    pub judge_score: Option<f64>,
}

impl RunStatus {
    pub fn from_metrics(metrics: &EvaluationMetrics) -> Self {
        Self {
            gates_passed: metrics.gates_passed >= metrics.gates_total,
            judge_passed: metrics.judge_passed,
            judge_score: metrics.judge_score,
        }
    }

    pub fn legacy_gates_passed(self) -> bool {
        self.gates_passed && self.judge_passed.unwrap_or(true)
    }

    pub fn outcome(self, gates_passed: usize, gates_total: usize) -> String {
        if !self.gates_passed {
            format!("Fail: {gates_passed}/{gates_total} gates passed")
        } else if let Some(false) = self.judge_passed {
            format!(
                "Fail: judge score {:.2} below threshold",
                self.judge_score.unwrap_or(0.0)
            )
        } else {
            "Pass".to_string()
        }
    }
}

pub fn determine_outcome(metrics: &EvaluationMetrics) -> String {
    RunStatus::from_metrics(metrics).outcome(metrics.gates_passed, metrics.gates_total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::EvaluationMetrics;
    use crate::interaction_profile::InteractionEvidenceSource;
    use crate::transcript::EfficiencyMetrics;

    fn metrics(
        gates_passed: usize,
        gates_total: usize,
        judge_passed: Option<bool>,
        judge_score: Option<f64>,
    ) -> EvaluationMetrics {
        EvaluationMetrics {
            gates_passed,
            gates_total,
            details: vec![],
            judge_score,
            judge_response: None,
            judge_passed,
            judge_threshold: judge_passed.map(|_| 0.7),
            efficiency: EfficiencyMetrics {
                total_commands: 0,
                unique_commands: 0,
                error_count: 0,
                retry_count: 0,
                help_invocations: 0,
                first_try_success_rate: 0.0,
                iteration_ratio: 0.0,
                completed: false,
            },
            interaction_evidence_source: InteractionEvidenceSource::StructuredToolCalls,
            composite_score: None,
            evaluator_results: vec![],
        }
    }

    #[test]
    fn status_projects_gate_failure_outcome() {
        let metrics = metrics(1, 2, None, None);

        let status = RunStatus::from_metrics(&metrics);

        assert_eq!(status.outcome(1, 2), "Fail: 1/2 gates passed");
        assert!(!status.legacy_gates_passed());
    }

    #[test]
    fn status_projects_judge_failure_outcome() {
        let metrics = metrics(2, 2, Some(false), Some(0.42));

        let status = RunStatus::from_metrics(&metrics);

        assert_eq!(
            status.outcome(2, 2),
            "Fail: judge score 0.42 below threshold"
        );
        assert!(!status.legacy_gates_passed());
    }

    #[test]
    fn status_projects_pass_outcome() {
        let metrics = metrics(2, 2, Some(true), Some(0.9));

        let status = RunStatus::from_metrics(&metrics);

        assert_eq!(status.outcome(2, 2), "Pass");
        assert!(status.legacy_gates_passed());
    }
}
