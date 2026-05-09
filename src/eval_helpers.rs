/// Computes a composite score from judge score, gates, and efficiency metrics.
pub fn compute_composite_score(
    judge_score: Option<f64>,
    gates_passed: usize,
    gates_total: usize,
    efficiency: &crate::transcript::EfficiencyMetrics,
    weights: Option<&crate::scenario::CompositeConfig>,
) -> f64 {
    let (judge_weight, gates_weight, efficiency_weight) = match weights {
        Some(w) => (w.judge_weight, w.gate_weight, w.interaction_weight),
        None => (0.55, 0.35, 0.10), // Default weights
    };

    let judge_component = judge_score.unwrap_or(0.0);

    let gates_component = if gates_total > 0 {
        gates_passed as f64 / gates_total as f64
    } else {
        0.0
    };

    let efficiency_component = efficiency.first_try_success_rate;

    let composite = (judge_weight * judge_component)
        + (gates_weight * gates_component)
        + (efficiency_weight * efficiency_component);

    composite.clamp(0.0, 1.0)
}
