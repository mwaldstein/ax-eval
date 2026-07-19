use super::super::types::{EfficiencyReport, EvaluationReport, GateDetail, RunReport};
use super::super::writer::TranscriptWriter;
use crate::evaluation::GateStatus;
use std::fs;

#[test]
fn test_write_report_basic() {
    let dir = tempfile::tempdir().unwrap();
    let writer = TranscriptWriter::new(dir.path().to_path_buf(), dir.path().to_path_buf()).unwrap();

    let report = RunReport {
        scenario_id: "test_scenario".to_string(),
        tool: "claude-code".to_string(),
        model: "claude-3-5-sonnet".to_string(),
        timestamp: "2025-01-27T12:00:00Z".to_string(),
        duration_secs: 45.3,
        cost_usd: Some(0.0234),
        token_usage: None,
        gate_status: GateStatus::Passed,
        judge_score: Some(0.8),
        judge_passed: Some(true),
        judge_threshold: Some(0.7),
        composite_score: Some(0.82),
        gate_details: vec![],
        efficiency: EfficiencyReport {
            total_commands: 10,
            unique_commands: 5,
            error_count: 0,
            tool_reuse_count: 1,
            help_invocations: 2,
            first_try_success_rate: 0.9,
            iteration_ratio: 2.0,
            completed: true,
        },
        setup_success: true,
        setup_commands: vec![],
    };

    writer.write_report(&report).unwrap();

    let report_path = dir.path().join("report.md");
    assert!(report_path.exists());

    let content = fs::read_to_string(&report_path).unwrap();
    assert!(content.contains("# Test Run Report"));
    assert!(content.contains("test_scenario"));
    assert!(content.contains("claude-code"));
    assert!(content.contains("claude-3-5-sonnet"));
    assert!(content.contains("45.30s"));
    assert!(content.contains("$0.0234"));
    assert!(content.contains("completed; judge threshold met"));
    assert!(content.contains("## Interaction Metrics"));
    assert!(content.contains("## Guardrails"));
}

#[test]
fn test_write_evaluation_basic() {
    let dir = tempfile::tempdir().unwrap();
    let writer = TranscriptWriter::new(dir.path().to_path_buf(), dir.path().to_path_buf()).unwrap();

    let evaluation = EvaluationReport {
        scenario_id: "test_scenario".to_string(),
        tool: "opencode".to_string(),
        model: "gpt-4o".to_string(),
        judge_score: Some(0.8),
        judge_passed: Some(true),
        judge_threshold: Some(0.7),
        gate_status: GateStatus::Failed,
        gate_details: vec![GateDetail {
            gate_type: "FileExists".to_string(),
            identifier: "file_exists(summary.md)".to_string(),
            passed: false,
            message: "missing".to_string(),
        }],
        duration_secs: 30.0,
        cost_usd: Some(0.015),
        composite_score: Some(0.82),
        efficiency: EfficiencyReport {
            total_commands: 8,
            unique_commands: 6,
            error_count: 1,
            tool_reuse_count: 1,
            help_invocations: 2,
            first_try_success_rate: 0.75,
            iteration_ratio: 1.33,
            completed: true,
        },
        interaction_evidence_source:
            crate::interaction_profile::InteractionEvidenceSource::StructuredToolCalls,
        judge_feedback: vec![
            "**Issues:**\nMinor formatting issue".to_string(),
            "**Highlights:**\nGood structure".to_string(),
            "**Criteria Scores:**\n- relevance: 0.85\n- clarity: 0.90".to_string(),
        ],
        evaluator_results: vec![],
    };

    writer.write_evaluation(&evaluation).unwrap();

    let eval_path = dir.path().join("evaluation.md");
    assert!(eval_path.exists());

    let content = fs::read_to_string(&eval_path).unwrap();
    assert!(content.contains("# Evaluation"));
    assert!(content.contains("test_scenario"));
    assert!(content.contains("opencode"));
    assert!(content.contains("gpt-4o"));
    assert!(content.contains("0.80 (0.70) +0.10"));
    assert!(content.contains("guardrail failed: file_exists(summary.md)"));
    assert!(content.contains("## Quality Signals"));
    assert!(content.contains("## Interaction Metrics"));
    assert!(content.contains("## Guardrails"));
    assert!(content.contains("30.00s"));
    assert!(content.contains("$0.0150"));
    assert!(content.contains("0.82"));
    assert!(content.contains("## Judge Feedback"));
    assert!(content.contains("**Issues:**"));
    assert!(content.contains("**Highlights:**"));
    assert!(content.contains("**Criteria Scores:**"));
    assert!(content.contains("## Human Review"));
    assert!(content.contains("<!--"));
    assert!(content.contains("Human Score: __/5"));
    assert!(content.contains("Further Human Notes:"));
    assert!(content.contains("-->"));
    assert!(content.contains("## Links"));
    assert!(content.contains("[Transcript](transcript.raw.txt)"));
    assert!(content.contains("[Metrics](metrics.json)"));
    assert!(content.contains("[Events](events.jsonl)"));
    assert!(content.contains("[Fixture](../fixture/)"));
}

#[test]
fn test_write_evaluation_without_judge_score() {
    let dir = tempfile::tempdir().unwrap();
    let writer = TranscriptWriter::new(dir.path().to_path_buf(), dir.path().to_path_buf()).unwrap();

    let evaluation = EvaluationReport {
        scenario_id: "test_scenario".to_string(),
        tool: "claude-code".to_string(),
        model: "claude-3-5-sonnet".to_string(),
        judge_score: None,
        judge_passed: None,
        judge_threshold: Some(0.7),
        gate_status: GateStatus::Passed,
        gate_details: vec![],
        duration_secs: 20.0,
        cost_usd: Some(0.01),
        composite_score: Some(0.75),
        efficiency: EfficiencyReport {
            total_commands: 3,
            unique_commands: 3,
            error_count: 0,
            tool_reuse_count: 0,
            help_invocations: 1,
            first_try_success_rate: 1.0,
            iteration_ratio: 1.0,
            completed: true,
        },
        interaction_evidence_source:
            crate::interaction_profile::InteractionEvidenceSource::StructuredToolCalls,
        judge_feedback: vec![],
        evaluator_results: vec![],
    };

    writer.write_evaluation(&evaluation).unwrap();

    let eval_path = dir.path().join("evaluation.md");
    let content = fs::read_to_string(&eval_path).unwrap();
    assert!(content.contains("- **Judge Score**: Not run (0.70)"));
    assert!(!content.contains("## Judge Feedback"));
}
