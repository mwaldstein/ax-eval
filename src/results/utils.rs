//! Utility functions for result handling.
//!
//! Provides helper functions for generating run IDs
//! and estimating costs from token usage.

use chrono::Utc;

/// Generate a unique run ID based on current timestamp.
///
/// Format: `run-YYYYMMDD-HHMMSS-microseconds`
///
/// # Returns
///
/// A unique run ID string
///
/// # Example
///
/// ```rust
/// use ax_eval::results::generate_run_id;
///
/// let run_id = generate_run_id();
/// assert!(run_id.starts_with("run-"));
/// ```
pub fn generate_run_id() -> String {
    let now = Utc::now();
    format!("run-{}", now.format("%Y%m%d-%H%M%S-%f"))
}
