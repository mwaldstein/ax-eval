//! Scenario loading and types for Agent Experience Eval.
//!
//! This module provides the core scenario structures and loading functionality.
//! Scenarios define test cases for evaluating LLM tools against CLI workflows.
//!
//! # Example
//!
//! ```rust
//! use ax_eval::scenario;
//!
//! let scenario = scenario::load("path/to/scenario.yaml").unwrap();
//! println!("Running scenario: {}", scenario.name);
//! ```

pub mod catalog;
pub mod discovery;
pub mod types;
pub mod validation;

pub use types::*;

use std::path::Path;

/// Load a scenario from a YAML file.
///
/// # Arguments
///
/// * `path` - Path to the YAML scenario file
///
/// # Returns
///
/// * `Ok(Scenario)` - Parsed scenario on success
/// * `Err` - IO or parsing error
///
/// # Example
///
/// ```rust,no_run
/// use ax_eval::scenario;
/// use std::path::Path;
///
/// let scenario = scenario::load(Path::new("scenarios/basic_note.yaml")).unwrap();
/// ```
pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Scenario> {
    let content = std::fs::read_to_string(path)?;
    let scenario: Scenario = yaml_serde::from_str(&content)?;
    Ok(scenario)
}

#[cfg(test)]
mod tests;
