//! JSONL-based results database.
//!
//! Provides persistent append-only storage of test results
//! in JSON Lines format for easy querying and analysis.

use crate::results::types::ResultRecord;
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// JSONL-based results database.
///
/// Stores test results as JSON Lines in a `results.jsonl` file,
/// providing append-only writes and full/ID-based loading.
///
/// # Example
///
/// ```rust,no_run
/// use ax_eval::results::{ResultsDB, ResultRecord};
/// use std::path::Path;
///
/// let db = ResultsDB::new(Path::new("./test-data"));
///
/// // Append a result
/// db.append(&record).unwrap();
///
/// // Load all results
/// let all_results = db.load_all().unwrap();
///
/// // Load specific result by ID
/// if let Some(record) = db.load_by_id("run-20250130-120000").unwrap() {
///     println!("Found result: {}", record.scenario_id);
/// }
/// ```
pub struct ResultsDB {
    results_path: PathBuf,
}

impl ResultsDB {
    /// Create a new results database in the specified base directory.
    ///
    /// Results will be stored in a `results.jsonl` file directly in the base directory.
    ///
    /// # Arguments
    ///
    /// * `base_dir` - Base directory for the results database
    ///
    /// # Returns
    ///
    /// A new `ResultsDB` instance
    pub fn new(base_dir: &Path) -> Self {
        std::fs::create_dir_all(base_dir).ok();
        Self {
            results_path: base_dir.join("results.jsonl"),
        }
    }

    /// Append a result record to the database.
    ///
    /// # Arguments
    ///
    /// * `record` - Result record to append
    ///
    /// # Returns
    ///
    /// * `Ok(())` - On success
    /// * `Err` - IO or serialization error
    pub fn append(&self, record: &ResultRecord) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.results_path)
            .context("Failed to open results.jsonl")?;

        let line = serde_json::to_string(record)?;
        writeln!(file, "{}", line).context("Failed to write to results.jsonl")?;
        Ok(())
    }

    /// Load all result records from the database.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<ResultRecord>)` - All records, or empty vector if file doesn't exist
    /// * `Err` - IO or parse error
    pub fn load_all(&self) -> Result<Vec<ResultRecord>> {
        if !self.results_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.results_path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line.context("Failed to read line from results.jsonl")?;
            let record: ResultRecord =
                serde_json::from_str(&line).context("Failed to parse result record")?;
            records.push(record);
        }

        Ok(records)
    }

    /// Load a specific result record by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Run ID to search for
    ///
    /// # Returns
    ///
    /// * `Ok(Some(ResultRecord))` - Record if found
    /// * `Ok(None)` - If no record with the given ID exists
    /// * `Err` - IO or parse error during search
    pub fn load_by_id(&self, id: &str) -> Result<Option<ResultRecord>> {
        let records = self.load_all()?;
        Ok(records.into_iter().find(|r| r.id == id))
    }
}

#[cfg(test)]
mod tests {
    use crate::results::test_helpers::{create_test_record, TestDb};

    #[test]
    fn test_results_db_append_and_load_all() {
        let test_db = TestDb::new();

        let record1 = create_test_record("run-1");
        let record2 = create_test_record("run-2");

        test_db.db.append(&record1).unwrap();
        test_db.db.append(&record2).unwrap();

        let loaded = test_db.db.load_all().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "run-1");
        assert_eq!(loaded[1].id, "run-2");
    }

    #[test]
    fn test_results_db_load_empty() {
        let test_db = TestDb::new();

        let loaded = test_db.db.load_all().unwrap();
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    fn test_results_db_load_by_id() {
        let test_db = TestDb::new();

        let record1 = create_test_record("run-1");
        let record2 = create_test_record("run-2");

        test_db.db.append(&record1).unwrap();
        test_db.db.append(&record2).unwrap();

        let loaded = test_db.db.load_by_id("run-1").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "run-1");

        let not_found = test_db.db.load_by_id("run-3").unwrap();
        assert!(not_found.is_none());
    }
}
