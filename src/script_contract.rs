use crate::evaluation::{EvaluatorResult, GateResult};
use crate::script_runner::{ScriptRunReport, ScriptRunStatus};
use serde::Deserialize;
use serde_json::Value;

pub fn script_gate_runner_unavailable() -> GateResult {
    GateResult {
        gate_type: "Script".to_string(),
        identifier: "script".to_string(),
        passed: false,
        message: "Script runner not available for script gate evaluation".to_string(),
    }
}

pub fn script_gate_execution_failed(command: &str, error: &anyhow::Error) -> GateResult {
    GateResult {
        gate_type: "Script".to_string(),
        identifier: format!("script({command})"),
        passed: false,
        message: format!("Failed to execute script '{}': {}", command, error),
    }
}

pub fn interpret_script_gate_report(report: &ScriptRunReport, description: &str) -> GateResult {
    if let ScriptRunStatus::TimedOut { timeout_secs } = report.status {
        return GateResult {
            gate_type: "Script".to_string(),
            identifier: format!("script({})", report.command),
            passed: false,
            message: format!(
                "Script '{}' timed out after {} seconds",
                report.command, timeout_secs
            ),
        };
    }

    let stdout = report.result.stdout.trim();
    if let Ok(parsed) = serde_json::from_str::<ScriptGateOutput>(stdout) {
        return GateResult {
            gate_type: "Script".to_string(),
            identifier: format!("script({})", report.command),
            passed: parsed.passed,
            message: parsed.message.unwrap_or_else(|| description.to_string()),
        };
    }

    let passed = report.succeeded();
    GateResult {
        gate_type: "Script".to_string(),
        identifier: format!("script({})", report.command),
        passed,
        message: format!(
            "Script '{}' {} (exit code: {}, description: {})",
            report.command,
            if passed { "passed" } else { "failed" },
            report.result.exit_code,
            description
        ),
    }
}

pub fn evaluator_runner_unavailable(name: &str) -> EvaluatorResult {
    EvaluatorResult {
        name: name.to_string(),
        metrics: None,
        score: None,
        summary: None,
        error: Some("Script runner not available".to_string()),
    }
}

pub fn evaluator_execution_failed(name: &str, error: &anyhow::Error) -> EvaluatorResult {
    EvaluatorResult {
        name: name.to_string(),
        metrics: None,
        score: None,
        summary: None,
        error: Some(format!("Execution failed: {}", error)),
    }
}

pub fn interpret_evaluator_report(name: &str, report: &ScriptRunReport) -> EvaluatorResult {
    if let Some(error) = report.failure_summary() {
        return EvaluatorResult {
            name: name.to_string(),
            metrics: None,
            score: None,
            summary: None,
            error: Some(error),
        };
    }

    interpret_evaluator_stdout(name, &report.result.stdout)
}

pub fn interpret_evaluator_stdout(name: &str, stdout: &str) -> EvaluatorResult {
    match serde_json::from_str::<Value>(stdout) {
        Ok(json) => {
            let metrics = json.get("metrics").cloned();
            let score = json.get("score").and_then(|v| v.as_f64());
            let summary = json
                .get("summary")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            EvaluatorResult {
                name: name.to_string(),
                metrics,
                score,
                summary,
                error: None,
            }
        }
        Err(e) => EvaluatorResult {
            name: name.to_string(),
            metrics: None,
            score: None,
            summary: Some(stdout.trim().to_string()),
            error: Some(format!("Invalid JSON output: {}", e)),
        },
    }
}

#[derive(Deserialize)]
struct ScriptGateOutput {
    passed: bool,
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_execution::{CommandExecutionMode, CommandResult};

    fn report(command: &str, exit_code: i32, stdout: &str, stderr: &str) -> ScriptRunReport {
        let status = if exit_code == 0 {
            ScriptRunStatus::Succeeded
        } else {
            ScriptRunStatus::Failed { exit_code }
        };

        ScriptRunReport {
            command: command.to_string(),
            timeout_secs: 30,
            result: CommandResult {
                exit_code,
                stdout: stdout.to_string(),
                stderr: stderr.to_string(),
                timed_out: false,
                mode: CommandExecutionMode::Piped,
            },
            status,
        }
    }

    #[test]
    fn script_gate_uses_structured_json_output() {
        let report = report(
            "check.sh",
            0,
            r#"{"passed":false,"message":"bad data"}"#,
            "",
        );

        let result = interpret_script_gate_report(&report, "fallback");

        assert!(!result.passed);
        assert_eq!(result.message, "bad data");
    }

    #[test]
    fn script_gate_uses_description_when_json_message_missing() {
        let report = report("check.sh", 0, r#"{"passed":true}"#, "");

        let result = interpret_script_gate_report(&report, "fallback");

        assert!(result.passed);
        assert_eq!(result.message, "fallback");
    }

    #[test]
    fn script_gate_classifies_timeout() {
        let mut report = report("slow.sh", -1, "", "");
        report.status = ScriptRunStatus::TimedOut { timeout_secs: 7 };
        report.result.timed_out = true;

        let result = interpret_script_gate_report(&report, "slow");

        assert!(!result.passed);
        assert!(result.message.contains("timed out after 7 seconds"));
    }

    #[test]
    fn evaluator_stdout_extracts_json_contract() {
        let result = interpret_evaluator_stdout(
            "quality",
            r#"{"score":0.75,"summary":"ok","metrics":{"warnings":2}}"#,
        );

        assert_eq!(result.name, "quality");
        assert_eq!(result.score, Some(0.75));
        assert_eq!(result.summary.as_deref(), Some("ok"));
        assert_eq!(
            result.metrics.unwrap()["warnings"],
            serde_json::Value::from(2)
        );
        assert!(result.error.is_none());
    }

    #[test]
    fn evaluator_stdout_reports_invalid_json() {
        let result = interpret_evaluator_stdout("quality", "plain summary");

        assert_eq!(result.summary.as_deref(), Some("plain summary"));
        assert!(result.error.unwrap().contains("Invalid JSON output"));
    }

    #[test]
    fn evaluator_report_uses_failure_summary() {
        let report = report("score.sh", 2, "", "bad args");

        let result = interpret_evaluator_report("quality", &report);

        assert_eq!(result.error.as_deref(), Some("Exit code 2: bad args"));
    }
}
