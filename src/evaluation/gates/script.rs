use crate::script_runner::ScriptRunner;
use serde::Deserialize;

use super::GateResult;

pub(super) fn eval_script(
    command: &str,
    description: &str,
    timeout_secs: u64,
    script_runner: Option<&ScriptRunner>,
) -> GateResult {
    let runner = match script_runner {
        Some(r) => r,
        None => {
            return GateResult {
                gate_type: "Script".to_string(),
                passed: false,
                message: "Script runner not available for script gate evaluation".to_string(),
            };
        }
    };

    let result = match runner.run(command, timeout_secs) {
        Ok(r) => r,
        Err(e) => {
            return GateResult {
                gate_type: "Script".to_string(),
                passed: false,
                message: format!("Failed to execute script '{}': {}", command, e),
            };
        }
    };

    if result.timed_out {
        return GateResult {
            gate_type: "Script".to_string(),
            passed: false,
            message: format!(
                "Script '{}' timed out after {} seconds",
                command, timeout_secs
            ),
        };
    }

    #[derive(Deserialize)]
    struct ScriptGateOutput {
        passed: bool,
        message: Option<String>,
    }

    let stdout = result.stdout.trim();
    if let Ok(parsed) = serde_json::from_str::<ScriptGateOutput>(stdout) {
        return GateResult {
            gate_type: "Script".to_string(),
            passed: parsed.passed,
            message: parsed.message.unwrap_or_else(|| description.to_string()),
        };
    }

    let passed = result.succeeded();
    GateResult {
        gate_type: "Script".to_string(),
        passed,
        message: format!(
            "Script '{}' {} (exit code: {}, description: {})",
            command,
            if passed { "passed" } else { "failed" },
            result.exit_code,
            description
        ),
    }
}
