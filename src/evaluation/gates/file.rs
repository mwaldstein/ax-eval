use regex::Regex;
use std::path::Path;

use super::GateResult;

pub(super) fn eval_file_exists(path: &str, env_root: &Path) -> GateResult {
    let full_path = env_root.join(path);
    let passed = full_path.exists();
    GateResult {
        gate_type: "FileExists".to_string(),
        identifier: format!("file_exists({path})"),
        passed,
        message: format!("File '{}' exists: {}", full_path.display(), passed),
    }
}

pub(super) fn eval_file_contains(path: &str, substring: &str, env_root: &Path) -> GateResult {
    let full_path = env_root.join(path);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let passed = content.contains(substring);
            GateResult {
                gate_type: "FileContains".to_string(),
                identifier: format!("file_contains({path})"),
                passed,
                message: format!(
                    "File '{}' contains substring '{}': {}",
                    full_path.display(),
                    substring,
                    passed
                ),
            }
        }
        Err(e) => GateResult {
            gate_type: "FileContains".to_string(),
            identifier: format!("file_contains({path})"),
            passed: false,
            message: format!("Failed to read file '{}': {}", full_path.display(), e),
        },
    }
}

pub(super) fn eval_file_matches(path: &str, pattern: &str, env_root: &Path) -> GateResult {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(e) => {
            return GateResult {
                gate_type: "FileMatches".to_string(),
                identifier: format!("file_matches({path})"),
                passed: false,
                message: format!("Invalid regex pattern '{}': {}", pattern, e),
            }
        }
    };

    let full_path = env_root.join(path);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let passed = regex.is_match(&content);
            GateResult {
                gate_type: "FileMatches".to_string(),
                identifier: format!("file_matches({path})"),
                passed,
                message: format!(
                    "File '{}' matches pattern '{}': {}",
                    full_path.display(),
                    pattern,
                    passed
                ),
            }
        }
        Err(e) => GateResult {
            gate_type: "FileMatches".to_string(),
            identifier: format!("file_matches({path})"),
            passed: false,
            message: format!("Failed to read file '{}': {}", full_path.display(), e),
        },
    }
}
