use crate::script_contract::{
    interpret_script_gate_report, script_gate_execution_failed, script_gate_runner_unavailable,
};
use crate::script_runner::ScriptRunner;

use super::GateResult;

pub(super) fn eval_script(
    command: &str,
    description: &str,
    timeout_secs: u64,
    script_runner: Option<&ScriptRunner>,
) -> GateResult {
    let runner = match script_runner {
        Some(r) => r,
        None => return script_gate_runner_unavailable(),
    };

    let report = match runner.run_report(command, timeout_secs) {
        Ok(r) => r,
        Err(e) => return script_gate_execution_failed(command, &e),
    };

    interpret_script_gate_report(&report, description)
}
