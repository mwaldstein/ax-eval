//! Tests for result types.

use super::*;

#[test]
fn test_cache_key_compute_basic() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool = "opencode";
    let model = "gpt-4o";

    let key = CacheKey::compute(scenario_yaml, prompt, tool, model);

    assert_eq!(key.tool, "opencode");
    assert_eq!(key.model, "gpt-4o");
    assert!(!key.scenario_hash.is_empty());
    assert!(!key.prompt_hash.is_empty());
    assert!(!key.fixture_hash.is_empty());
}

#[test]
fn test_cache_key_consistent_hashes() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool = "opencode";
    let model = "gpt-4o";

    let key1 = CacheKey::compute(scenario_yaml, prompt, tool, model);
    let key2 = CacheKey::compute(scenario_yaml, prompt, tool, model);

    assert_eq!(key1.scenario_hash, key2.scenario_hash);
    assert_eq!(key1.prompt_hash, key2.prompt_hash);
    assert_eq!(key1.fixture_hash, key2.fixture_hash);
}

#[test]
fn test_cache_key_different_fixtures() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool = "opencode";
    let model = "gpt-4o";

    let key1 = CacheKey::compute_with_fixture(scenario_yaml, prompt, "fixture one", tool, model);
    let key2 = CacheKey::compute_with_fixture(scenario_yaml, prompt, "fixture two", tool, model);

    assert_eq!(key1.scenario_hash, key2.scenario_hash);
    assert_eq!(key1.prompt_hash, key2.prompt_hash);
    assert_ne!(key1.fixture_hash, key2.fixture_hash);
    assert_ne!(key1.as_string(), key2.as_string());
}

#[test]
fn test_cache_key_different_scenarios() {
    let scenario1 = "name: test1\ntask:\n  prompt: test";
    let scenario2 = "name: test2\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool = "opencode";
    let model = "gpt-4o";

    let key1 = CacheKey::compute(scenario1, prompt, tool, model);
    let key2 = CacheKey::compute(scenario2, prompt, tool, model);

    assert_ne!(key1.scenario_hash, key2.scenario_hash);
    assert_eq!(key1.prompt_hash, key2.prompt_hash);
}

#[test]
fn test_cache_key_different_prompts() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt1 = "Create a test note";
    let prompt2 = "Create a different note";
    let tool = "opencode";
    let model = "gpt-4o";

    let key1 = CacheKey::compute(scenario_yaml, prompt1, tool, model);
    let key2 = CacheKey::compute(scenario_yaml, prompt2, tool, model);

    assert_eq!(key1.scenario_hash, key2.scenario_hash);
    assert_ne!(key1.prompt_hash, key2.prompt_hash);
}

#[test]
fn test_cache_key_different_tools() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool1 = "opencode";
    let tool2 = "claude-code";
    let model = "gpt-4o";

    let key1 = CacheKey::compute(scenario_yaml, prompt, tool1, model);
    let key2 = CacheKey::compute(scenario_yaml, prompt, tool2, model);

    assert_eq!(key1.scenario_hash, key2.scenario_hash);
    assert_eq!(key1.prompt_hash, key2.prompt_hash);
    assert_ne!(key1.tool, key2.tool);
}

#[test]
fn test_cache_key_different_models() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool = "opencode";
    let model1 = "gpt-4o";
    let model2 = "claude-sonnet-4";

    let key1 = CacheKey::compute(scenario_yaml, prompt, tool, model1);
    let key2 = CacheKey::compute(scenario_yaml, prompt, tool, model2);

    assert_eq!(key1.scenario_hash, key2.scenario_hash);
    assert_eq!(key1.prompt_hash, key2.prompt_hash);
    assert_ne!(key1.model, key2.model);
}

#[test]
fn test_cache_key_as_string() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool = "opencode";
    let model = "gpt-4o";

    let key = CacheKey::compute(scenario_yaml, prompt, tool, model);
    let key_string = key.as_string();

    assert!(key_string.contains(&key.scenario_hash));
    assert!(key_string.contains(&key.prompt_hash));
    assert!(key_string.contains(&key.fixture_hash));
    assert!(key_string.contains(&key.tool));
    assert!(key_string.contains(&key.model));
}

#[test]
fn test_cache_key_equality() {
    let scenario_yaml = "name: test\ntask:\n  prompt: test";
    let prompt = "Create a test note";
    let tool = "opencode";
    let model = "gpt-4o";

    let key1 = CacheKey::compute(scenario_yaml, prompt, tool, model);
    let key2 = CacheKey::compute(scenario_yaml, prompt, tool, model);

    assert_eq!(key1, key2);
}

#[test]
fn test_result_record_json_round_trip() {
    let original = ResultRecord {
        id: "test-run-id".to_string(),
        scenario_id: "test-scenario".to_string(),
        scenario_hash: "hash123".to_string(),
        tool: "opencode".to_string(),
        model: "gpt-4o".to_string(),
        timestamp: chrono::Utc::now(),
        duration_secs: 45.5,
        cost_usd: Some(0.01),
        token_usage: Some(TokenUsageRecord {
            input: 1500,
            output: 800,
        }),
        gate_status: "passed".to_string(),
        metrics: EvaluationMetricsRecord {
            gate_status: "passed".to_string(),
            details: vec![GateResultRecord {
                gate_type: "min_notes".to_string(),
                identifier: "min_notes".to_string(),
                passed: true,
                message: "Passed".to_string(),
            }],
            judge_passed: Some(true),
            judge_threshold: Some(0.8),
            efficiency: EfficiencyMetricsRecord {
                total_commands: 3,
                unique_commands: 2,
                error_count: 0,
                retry_count: 1,
                help_invocations: 0,
                first_try_success_rate: 1.0,
                iteration_ratio: 1.5,
                completed: true,
            },
            interaction_evidence_source: None,
            warnings: vec![],
            composite_score: Some(0.95),
            evaluator_results: vec![],
        },
        judge_score: Some(0.9),
        outcome: "PASS".to_string(),
        transcript_path: "/path/to/transcript.txt".to_string(),
        cache_key: Some("cache-key-123".to_string()),
    };

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: ResultRecord = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, original.id);
    assert_eq!(deserialized.scenario_id, original.scenario_id);
    assert_eq!(deserialized.scenario_hash, original.scenario_hash);
    assert_eq!(deserialized.tool, original.tool);
    assert_eq!(deserialized.model, original.model);
    assert_eq!(deserialized.timestamp, original.timestamp);
    assert_eq!(deserialized.duration_secs, original.duration_secs);
    assert_eq!(deserialized.cost_usd, original.cost_usd);
    assert_eq!(deserialized.token_usage, original.token_usage);
    assert_eq!(deserialized.gate_status, original.gate_status);
    assert_eq!(
        deserialized.metrics.gate_status,
        original.metrics.gate_status
    );
    assert_eq!(
        deserialized.metrics.efficiency.total_commands,
        original.metrics.efficiency.total_commands
    );
    assert_eq!(deserialized.judge_score, original.judge_score);
    assert_eq!(deserialized.outcome, original.outcome);
    assert_eq!(deserialized.transcript_path, original.transcript_path);
    assert_eq!(deserialized.cache_key, original.cache_key);
}

#[test]
fn test_result_record_json_skip_none_cache_key() {
    let record = ResultRecord {
        id: "test-run-id".to_string(),
        scenario_id: "test-scenario".to_string(),
        scenario_hash: "hash123".to_string(),
        tool: "opencode".to_string(),
        model: "gpt-4o".to_string(),
        timestamp: chrono::Utc::now(),
        duration_secs: 45.5,
        cost_usd: Some(0.01),
        token_usage: None,
        gate_status: "passed".to_string(),
        metrics: EvaluationMetricsRecord {
            gate_status: "passed".to_string(),
            details: vec![],
            judge_passed: None,
            judge_threshold: None,
            efficiency: EfficiencyMetricsRecord {
                total_commands: 3,
                unique_commands: 2,
                error_count: 0,
                retry_count: 1,
                help_invocations: 0,
                first_try_success_rate: 1.0,
                iteration_ratio: 1.5,
                completed: true,
            },
            interaction_evidence_source: None,
            warnings: vec![],
            composite_score: Some(0.85),
            evaluator_results: vec![],
        },
        judge_score: None,
        outcome: "PASS".to_string(),
        transcript_path: "/path/to/transcript.txt".to_string(),
        cache_key: None,
    };

    let json = serde_json::to_string(&record).unwrap();
    assert!(!json.contains("\"cache_key\""));
    assert!(json.contains("\"judge_score\":null"));
}

#[test]
fn old_format_result_record_deserializes_with_unknown_gate_status() {
    let json = r#"{
        "id": "old-run-id",
        "scenario_id": "old-scenario",
        "scenario_hash": "hash123",
        "tool": "opencode",
        "model": "gpt-4o",
        "timestamp": "2026-07-18T12:00:00Z",
        "duration_secs": 12.0,
        "cost_usd": null,
        "token_usage": null,
        "gates_passed": true,
        "metrics": {
            "gates_passed": 2,
            "gates_total": 2,
            "details": [],
            "judge_passed": null,
            "judge_threshold": null,
            "efficiency": {
                "total_commands": 1,
                "unique_commands": 1,
                "error_count": 0,
                "retry_count": 0,
                "help_invocations": 0,
                "first_try_success_rate": 1.0,
                "iteration_ratio": 1.0,
                "completed": true
            },
            "interaction_evidence_source": null,
            "composite_score": null,
            "evaluator_results": []
        },
        "judge_score": null,
        "outcome": "PASS",
        "transcript_path": "transcript.raw.txt",
        "cache_key": null
    }"#;

    let record: ResultRecord = serde_json::from_str(json).expect("old result row");

    assert_eq!(record.gate_status, "unknown");
    assert_eq!(record.metrics.gate_status, "unknown");
}

#[test]
fn evaluation_metrics_record_preserves_full_evaluation_profile() {
    let metrics = crate::evaluation::EvaluationMetrics {
        gate_status: crate::evaluation::GateStatus::Failed,
        details: vec![crate::evaluation::GateResult {
            gate_type: "file_contains".to_string(),
            identifier: "file_contains(summary.md)".to_string(),
            passed: false,
            message: "missing expected text".to_string(),
        }],
        judge_score: Some(0.62),
        judge_response: None,
        judge_passed: Some(false),
        judge_threshold: Some(0.7),
        efficiency: crate::transcript::EfficiencyMetrics {
            total_commands: 4,
            unique_commands: 3,
            error_count: 1,
            retry_count: 1,
            help_invocations: 1,
            first_try_success_rate: 0.5,
            iteration_ratio: 0.75,
            completed: true,
        },
        interaction_evidence_source:
            crate::interaction_profile::InteractionEvidenceSource::StructuredToolCalls,
        warnings: vec![],
        composite_score: Some(0.58),
        evaluator_results: vec![crate::evaluation::EvaluatorResult {
            name: "quality".to_string(),
            metrics: Some(serde_json::json!({ "items": 2 })),
            score: Some(0.8),
            summary: Some("usable".to_string()),
            error: None,
        }],
    };

    let record = EvaluationMetricsRecord::from(metrics);

    assert_eq!(record.gate_status, "failed");
    assert_eq!(record.details[0].gate_type, "file_contains");
    assert_eq!(record.details[0].identifier, "file_contains(summary.md)");
    assert_eq!(record.details[0].message, "missing expected text");
    assert_eq!(record.judge_passed, Some(false));
    assert_eq!(record.judge_threshold, Some(0.7));
    assert_eq!(record.efficiency.total_commands, 4);
    assert_eq!(record.efficiency.help_invocations, 1);
    assert_eq!(
        record.interaction_evidence_source,
        Some(crate::interaction_profile::InteractionEvidenceSource::StructuredToolCalls)
    );
    assert_eq!(record.composite_score, Some(0.58));
    assert_eq!(record.evaluator_results[0].name, "quality");
    assert_eq!(record.evaluator_results[0].score, Some(0.8));
}
