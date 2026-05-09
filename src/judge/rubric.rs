//! Rubric loading and validation.
//!
//! This module provides functionality for loading rubrics from YAML files
//! and validating their structure.

use crate::judge::types::Rubric;
use anyhow::{Context, Result};
use std::path::Path;

/// Load a rubric from a YAML file and validate criterion weights sum to 1.0.
///
/// # Arguments
///
/// * `path` - Path to the rubric YAML file
///
/// # Returns
///
/// * `Ok(Rubric)` - Parsed and validated rubric
/// * `Err` - IO error, parse error, or weight validation error
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The YAML is malformed
/// - Criterion weights don't sum to approximately 1.0 (within 0.01 tolerance)
pub fn load_rubric(path: &Path) -> Result<Rubric> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read rubric file: {}", path.display()))?;
    let rubric: Rubric = yaml_serde::from_str(&content)
        .with_context(|| format!("Failed to parse rubric YAML: {}", path.display()))?;

    let total_weight: f64 = rubric.criteria.iter().map(|c| c.weight).sum();
    if (total_weight - 1.0).abs() > 0.01 {
        anyhow::bail!(
            "Rubric criterion weights must sum to 1.0, got {}",
            total_weight
        );
    }

    Ok(rubric)
}
