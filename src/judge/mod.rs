//! Judge module for LLM-as-judge evaluation.
//!
//! This module provides rubric-based evaluation functionality for assessing
//! LLM tool performance. The judge is executed via CLI tools (not direct API
//! calls), reusing the same adapter framework used to run scenarios.
//!
//! # Example
//!
//! ```rust,no_run
//! use ax_eval::judge;
//! use std::path::Path;
//!
//! let rubric = judge::load_rubric(Path::new("rubrics/quality.yaml")).unwrap();
//! println!("Loaded rubric with {} criteria", rubric.criteria.len());
//! ```

pub mod eval;
pub mod rubric;
pub mod types;

pub use eval::{build_judge_prompt_for_target, JudgeTargetView};
pub use rubric::*;
pub use types::*;

#[cfg(test)]
mod tests;
