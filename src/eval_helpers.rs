/// Computes a composite score from judge score and efficiency metrics.
pub fn compute_composite_score(
    judge_score: Option<f64>,
    gate_status: crate::evaluation::GateStatus,
    efficiency: &crate::transcript::EfficiencyMetrics,
    weights: Option<&crate::scenario::CompositeConfig>,
) -> Option<f64> {
    if gate_status == crate::evaluation::GateStatus::Failed {
        return None;
    }

    let (judge_weight, efficiency_weight) = match weights {
        Some(w) => (w.judge_weight, w.interaction_weight),
        None => (0.55, 0.10),
    };

    let efficiency_component = efficiency.first_try_success_rate;
    let components = if let Some(judge_component) = judge_score {
        vec![
            (judge_weight, judge_component),
            (efficiency_weight, efficiency_component),
        ]
    } else {
        vec![(efficiency_weight, efficiency_component)]
    };

    let total_weight: f64 = components.iter().map(|(weight, _)| weight).sum();
    if total_weight <= f64::EPSILON {
        return None;
    }

    let composite: f64 = components
        .into_iter()
        .map(|(weight, score)| (weight / total_weight) * score)
        .sum();
    Some(composite.clamp(0.0, 1.0))
}
