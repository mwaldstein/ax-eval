use crate::eval_helpers::*;
use crate::evaluation::*;

#[test]
fn test_compute_composite_score_with_judge() {
    let efficiency = crate::transcript::EfficiencyMetrics {
        total_commands: 5,
        unique_commands: 3,
        error_count: 0,
        retry_count: 1,
        help_invocations: 0,
        first_try_success_rate: 0.8,
        iteration_ratio: 1.5,
        completed: true,
    };

    let composite = compute_composite_score(Some(0.9), 3, 3, &efficiency, None);

    let expected = (0.55 * 0.9) + (0.35 * 1.0) + (0.10 * 0.8);
    assert!((composite - expected).abs() < 0.001);
}

#[test]
fn test_compute_composite_score_without_judge() {
    let efficiency = crate::transcript::EfficiencyMetrics {
        total_commands: 5,
        unique_commands: 3,
        error_count: 0,
        retry_count: 1,
        help_invocations: 0,
        first_try_success_rate: 0.8,
        iteration_ratio: 1.5,
        completed: true,
    };

    let composite = compute_composite_score(None, 3, 3, &efficiency, None);

    let expected = (0.55 * 0.0) + (0.35 * 1.0) + (0.10 * 0.8);
    assert!((composite - expected).abs() < 0.001);
}

#[test]
fn test_compute_composite_score_empty_store() {
    let efficiency = crate::transcript::EfficiencyMetrics {
        total_commands: 0,
        unique_commands: 0,
        error_count: 0,
        retry_count: 0,
        help_invocations: 0,
        first_try_success_rate: 0.0,
        iteration_ratio: 0.0,
        completed: false,
    };

    let composite = compute_composite_score(None, 0, 0, &efficiency, None);

    assert_eq!(composite, 0.0);
}

#[test]
fn test_compute_composite_score_clamped() {
    let efficiency = crate::transcript::EfficiencyMetrics {
        total_commands: 5,
        unique_commands: 3,
        error_count: 0,
        retry_count: 1,
        help_invocations: 0,
        first_try_success_rate: 1.5,
        iteration_ratio: 1.5,
        completed: true,
    };

    let composite = compute_composite_score(Some(1.5), 3, 3, &efficiency, None);

    assert!(composite <= 1.0);
    assert!(composite >= 0.0);
}

#[test]
fn test_score_tier_excellent() {
    assert_eq!(ScoreTier::from_score(0.95), ScoreTier::Excellent);
    assert_eq!(ScoreTier::from_score(0.90), ScoreTier::Excellent);
    assert_eq!(ScoreTier::from_score(1.00), ScoreTier::Excellent);
}

#[test]
fn test_score_tier_good() {
    assert_eq!(ScoreTier::from_score(0.85), ScoreTier::Good);
    assert_eq!(ScoreTier::from_score(0.75), ScoreTier::Good);
    assert_eq!(ScoreTier::from_score(0.70), ScoreTier::Good);
}

#[test]
fn test_score_tier_acceptable() {
    assert_eq!(ScoreTier::from_score(0.65), ScoreTier::Acceptable);
    assert_eq!(ScoreTier::from_score(0.55), ScoreTier::Acceptable);
    assert_eq!(ScoreTier::from_score(0.50), ScoreTier::Acceptable);
}

#[test]
fn test_score_tier_poor() {
    assert_eq!(ScoreTier::from_score(0.45), ScoreTier::Poor);
    assert_eq!(ScoreTier::from_score(0.25), ScoreTier::Poor);
    assert_eq!(ScoreTier::from_score(0.00), ScoreTier::Poor);
}

#[test]
fn test_score_tier_boundary_cases() {
    assert_eq!(ScoreTier::from_score(0.8999), ScoreTier::Good);
    assert_eq!(ScoreTier::from_score(0.9000), ScoreTier::Excellent);
    assert_eq!(ScoreTier::from_score(0.6999), ScoreTier::Acceptable);
    assert_eq!(ScoreTier::from_score(0.7000), ScoreTier::Good);
    assert_eq!(ScoreTier::from_score(0.4999), ScoreTier::Poor);
    assert_eq!(ScoreTier::from_score(0.5000), ScoreTier::Acceptable);
}

#[test]
fn test_score_tier_display() {
    assert_eq!(format!("{}", ScoreTier::Excellent), "Excellent");
    assert_eq!(format!("{}", ScoreTier::Good), "Good");
    assert_eq!(format!("{}", ScoreTier::Acceptable), "Acceptable");
    assert_eq!(format!("{}", ScoreTier::Poor), "Poor");
}
