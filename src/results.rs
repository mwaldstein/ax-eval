//! Results management for Agent Experience Eval runs.
//!
//! This module provides persistent storage and caching of test results,
//! including a JSONL-based results database and a file-based cache system.
//!
//! # Submodules
//!
//! - `cache` - File-based result caching
//! - `db` - JSONL results database
//! - `types` - Result data structures
//! - `utils` - Utility functions for result handling
//!
//! # Example
//!
//! ```rust,no_run
//! use ax_eval::results::{Cache, ResultsDB, ResultRecord};
//! use std::path::Path;
//!
//! let cache = Cache::new(Path::new("./test-data"));
//! let db = ResultsDB::new(Path::new("./test-data"));
//! ```

pub mod cache;
pub mod db;
pub mod types;
pub mod utils;

#[cfg(test)]
pub mod test_helpers;

pub use cache::Cache;
pub use db::ResultsDB;
pub use types::*;
pub use utils::generate_run_id;

#[cfg(test)]
mod tests {
    // Tests have been moved to their respective submodule test modules:
    // - utils::tests for estimate_cost_from_tokens
    // - types::tests for CacheKey and ResultRecord
    // - db::tests for ResultsDB
}
