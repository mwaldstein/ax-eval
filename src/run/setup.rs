use crate::fixture::TestEnv;
use crate::scenario::{Scenario, Setup};
use crate::transcript::TranscriptWriter;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn setup_scenario_env(
    s: &Scenario,
    scenario_path: &std::path::Path,
    results_dir: &Path,
) -> anyhow::Result<(TestEnv, String, String)> {
    let scenario_yaml = std::fs::read_to_string(scenario_path)?;
    let prompt = s.task.prompt.clone();

    println!(
        "Setting up environment for template folder: {}",
        s.template_folder
    );
    let env_root = results_dir.join("fixture");
    let env = TestEnv::new(env_root)?;
    env.setup_fixture(&s.template_folder)?;

    println!("Environment created at: {:?}", env.root);

    Ok((env, scenario_yaml, prompt))
}

#[allow(clippy::type_complexity)]
pub fn execute_setup_commands(
    setup: &Setup,
    env: &TestEnv,
    writer: &TranscriptWriter,
    effective_timeout: u64,
    target_env: Option<&HashMap<String, String>>,
) -> anyhow::Result<(bool, Vec<(String, bool, String)>)> {
    println!("Running {} setup command(s)...", setup.commands.len());
    let runner = crate::session::SessionRunner::new();
    let mut setup_success = true;
    let mut setup_commands: Vec<(String, bool, String)> = Vec::new();
    let env_vars: Vec<(String, String)> = target_env
        .map(|vars| {
            vars.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<(String, String)>>()
        })
        .unwrap_or_default();

    for (i, cmd) in setup.commands.iter().enumerate() {
        println!("  Command {}/{}: {}", i + 1, setup.commands.len(), cmd);
        let (output, exit_code) = runner.run_command_with_env(
            "sh",
            &["-c", cmd],
            &env.root,
            effective_timeout,
            &env_vars,
        )?;

        let success = exit_code == 0;
        setup_commands.push((cmd.to_string(), success, output.clone()));

        writer.append_event(&serde_json::json!({
            "type": "setup_command",
            "index": i,
            "command": cmd,
            "exit_code": exit_code,
            "output": output,
            "success": success,
        }))?;

        if !success {
            setup_success = false;
            println!("  Command failed with exit code {}", exit_code);
        }
    }
    println!("Setup complete.");

    Ok((setup_success, setup_commands))
}

pub fn execute_health_check(
    command: &str,
    env: &TestEnv,
    writer: &TranscriptWriter,
    effective_timeout: u64,
    target_env: Option<&HashMap<String, String>>,
) -> anyhow::Result<()> {
    if command.trim().is_empty() {
        anyhow::bail!("target.health_check cannot be empty");
    }

    println!("Running target health check: {}", command);
    let runner = crate::session::SessionRunner::new();
    let env_vars: Vec<(String, String)> = target_env
        .map(|vars| {
            vars.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<(String, String)>>()
        })
        .unwrap_or_default();

    let (output, exit_code) = runner.run_command_with_env(
        "sh",
        &["-c", command],
        &env.root,
        effective_timeout,
        &env_vars,
    )?;
    let success = exit_code == 0;

    writer.append_event(&serde_json::json!({
        "type": "health_check",
        "command": command,
        "exit_code": exit_code,
        "output": output,
        "success": success,
    }))?;

    if !success {
        anyhow::bail!(
            "target health check failed with exit code {}: {}",
            exit_code,
            output.trim()
        );
    }

    Ok(())
}

#[allow(clippy::type_complexity)]
pub fn prepare_writer_and_setup(
    results_dir: &Path,
    env: &TestEnv,
    s: &Scenario,
    effective_timeout: u64,
) -> anyhow::Result<(PathBuf, TranscriptWriter, bool, Vec<(String, bool, String)>)> {
    let artifacts_dir = results_dir.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)?;
    let writer = TranscriptWriter::new(artifacts_dir.clone(), results_dir.to_path_buf())?;

    let (setup_success, setup_commands) = if let Some(setup) = &s.setup {
        execute_setup_commands(
            setup,
            env,
            &writer,
            effective_timeout,
            s.target.env.as_ref(),
        )?
    } else {
        (true, vec![])
    };

    if let Some(health_check) = &s.target.health_check {
        execute_health_check(
            health_check,
            env,
            &writer,
            effective_timeout,
            s.target.env.as_ref(),
        )?;
    }

    Ok((artifacts_dir, writer, setup_success, setup_commands))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Setup;
    use tempfile::tempdir;

    #[test]
    fn setup_commands_receive_target_env_vars() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");

        let artifacts_dir = dir.path().join("artifacts");
        let results_dir = dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");
        let writer = TranscriptWriter::new(artifacts_dir, results_dir).expect("create writer");

        let setup = Setup {
            commands: vec!["test \"$TARGET_ENV_TEST\" = \"works\"".to_string()],
        };
        let mut target_env = HashMap::new();
        target_env.insert("TARGET_ENV_TEST".to_string(), "works".to_string());

        let (setup_success, commands) =
            execute_setup_commands(&setup, &env, &writer, 10, Some(&target_env))
                .expect("run setup commands");

        assert!(setup_success);
        assert_eq!(commands.len(), 1);
        assert!(commands[0].1);
    }

    #[test]
    fn health_check_runs_in_fixture_dir_with_target_env_vars() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");

        let artifacts_dir = dir.path().join("artifacts");
        let results_dir = dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");
        let writer = TranscriptWriter::new(artifacts_dir, results_dir).expect("create writer");

        let mut target_env = HashMap::new();
        target_env.insert("TARGET_ENV_TEST".to_string(), "works".to_string());

        execute_health_check(
            "test \"$TARGET_ENV_TEST\" = \"works\"",
            &env,
            &writer,
            10,
            Some(&target_env),
        )
        .expect("run health check");
    }

    #[test]
    fn health_check_fails_on_nonzero_exit() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");

        let artifacts_dir = dir.path().join("artifacts");
        let results_dir = dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");
        let writer = TranscriptWriter::new(artifacts_dir, results_dir).expect("create writer");

        let result = execute_health_check("exit 7", &env, &writer, 10, None);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("target health check failed"));
    }
}
