//! Automated regression tests for every fixture scenario.
//!
//! These tests run the mock-tool commands directly without a real LLM, then
//! evaluate all gates to verify the scenario definitions and framework logic
//! are correct. They run automatically in CI via `cargo test`.
//!
//! For tests with actual LLM adapters, see `tests/e2e.rs` which requires
//! authenticated tools and `LLM_TOOL_TEST_E2E=1`.

#[cfg(test)]
mod tests {
    use crate::evaluation;
    use crate::run::setup::setup_scenario_env;
    use crate::scenario::load;
    use crate::script_runner::ScriptRunner;
    use crate::transcript::InteractionMetricsSource;
    use std::collections::HashMap;
    use std::path::Path;
    use std::process::Command as StdCommand;

    /// Run a fixture scenario end-to-end.
    ///
    /// 1. Load the scenario YAML.
    /// 2. Copy the template into a temp directory.
    /// 3. Run setup commands.
    /// 4. Run the provided agent commands.
    /// 5. Synthesise a transcript.raw.txt so that `no_transcript_errors` gates pass.
    /// 6. Run post-evaluation scripts (if any).
    /// 7. Evaluate gates and assert they all pass.
    fn run_fixture(scenario_path: &str, agent_commands: &[&str]) -> evaluation::EvaluationMetrics {
        let scenario = load(Path::new(scenario_path)).expect("load scenario");

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let results_dir = temp_dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");

        let (env, _scenario_yaml, _prompt) =
            setup_scenario_env(&scenario, Path::new(scenario_path), &results_dir)
                .expect("setup scenario env");

        let target_env: HashMap<String, String> = scenario.target.env.clone().unwrap_or_default();

        // Run setup commands
        if let Some(setup) = &scenario.setup {
            for cmd in &setup.commands {
                let output = run_shell_command(cmd, &env.root, &target_env);
                assert!(
                    output.status.success(),
                    "Setup command failed: {}\nstderr: {}",
                    cmd,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Run agent commands
        for cmd in agent_commands {
            let output = run_shell_command(cmd, &env.root, &target_env);
            assert!(
                output.status.success(),
                "Agent command failed: {}\nstderr: {}",
                cmd,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Synthesise transcript so transcript-based gates pass
        write_transcript(&env.root, &scenario.target.binary, agent_commands);

        // Run post scripts
        if let Some(scripts) = &scenario.scripts {
            let runner = ScriptRunner::new(
                env.root.clone(),
                results_dir.clone(),
                scenario.name.clone(),
                "fixture_test".to_string(),
                "default".to_string(),
                None,
                None,
                target_env.clone(),
            );
            for entry in &scripts.post {
                let result = runner
                    .run(&entry.command, entry.timeout_secs)
                    .expect("run post script");
                assert!(
                    result.succeeded(),
                    "Post script failed: {}\nstdout: {}\nstderr: {}",
                    entry.command,
                    result.stdout,
                    result.stderr
                );
            }
        }

        // Evaluate
        let script_runner = ScriptRunner::new(
            env.root.clone(),
            results_dir,
            scenario.name.clone(),
            "fixture_test".to_string(),
            "default".to_string(),
            None,
            None,
            target_env,
        );

        let metrics = evaluation::evaluate(
            &scenario,
            &env.root,
            true,
            Some(&script_runner),
            None,
            None,
            InteractionMetricsSource::TranscriptRegex,
            &[],
            true,
        )
        .expect("evaluate");

        assert_eq!(
            metrics.gates_passed, metrics.gates_total,
            "Expected all gates to pass for {}. Details: {:?}",
            scenario.name, metrics.details
        );

        metrics
    }

    fn run_shell_command(
        cmd: &str,
        cwd: &Path,
        env_vars: &HashMap<String, String>,
    ) -> std::process::Output {
        let mut command = StdCommand::new("sh");
        command.arg("-c").arg(cmd).current_dir(cwd);
        for (k, v) in env_vars {
            command.env(k, v);
        }
        command.output().expect("run shell command")
    }

    fn write_transcript(env_root: &Path, _binary: &str, commands: &[&str]) {
        let mut transcript = String::new();
        for cmd in commands {
            transcript.push_str(&format!("$ {}\n", cmd));
            transcript.push_str("exit code: 0\n\n");
        }
        std::fs::write(env_root.join("transcript.raw.txt"), transcript).expect("write transcript");

        // Also write events.jsonl so the transcript analyzer has a fallback
        let events: Vec<serde_json::Value> = commands
            .iter()
            .map(|cmd| {
                serde_json::json!({
                    "type": "tool_call",
                    "command": cmd,
                    "exit_code": 0,
                })
            })
            .collect();
        let events_jsonl = events
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(env_root.join("events.jsonl"), events_jsonl).expect("write events");
    }

    // ------------------------------------------------------------------
    // example_basic
    // ------------------------------------------------------------------
    #[test]
    fn example_basic_passes_all_gates() {
        let metrics = run_fixture(
            "fixtures/example_basic.yaml",
            &["./taskmgr create \"Implement scripts system\""],
        );
        assert_eq!(metrics.gates_passed, metrics.gates_total);
        assert!(
            !metrics.evaluator_results.is_empty(),
            "Should have evaluator results"
        );
    }

    // ------------------------------------------------------------------
    // example_gates_comprehensive
    // ------------------------------------------------------------------
    #[test]
    fn example_gates_comprehensive_passes_all_gates() {
        let metrics = run_fixture(
            "fixtures/example_gates_comprehensive.yaml",
            &[
                "./demo_tool init",
                "./demo_tool create \"test item\"",
                "./demo_tool show --json",
                "./demo_tool status",
            ],
        );
        assert_eq!(metrics.gates_passed, metrics.gates_total);
    }

    // ------------------------------------------------------------------
    // example_full_config
    // ------------------------------------------------------------------
    #[test]
    fn example_full_config_passes_all_gates() {
        let metrics = run_fixture(
            "fixtures/example_full_config.yaml",
            &["./config_tool write \"Hello from full config\""],
        );
        assert_eq!(metrics.gates_passed, metrics.gates_total);
    }

    // ------------------------------------------------------------------
    // example_judge
    // ------------------------------------------------------------------
    #[test]
    fn example_judge_passes_all_gates() {
        let metrics = run_fixture(
            "fixtures/example_judge.yaml",
            &["./simple_tool create \"Hello World\""],
        );
        assert_eq!(metrics.gates_passed, metrics.gates_total);
    }

    // ------------------------------------------------------------------
    // example_e2e
    // ------------------------------------------------------------------
    #[test]
    fn example_e2e_passes_all_gates() {
        let metrics = run_fixture("fixtures/example_e2e.yaml", &["./hello run"]);
        assert_eq!(metrics.gates_passed, metrics.gates_total);
    }

    // ------------------------------------------------------------------
    // example_guidance_minimal
    // ------------------------------------------------------------------
    #[test]
    fn example_guidance_minimal_passes_all_gates() {
        let mut commands = vec!["./notes init"];
        commands.push("./notes add \"Database migration strategy\" --tag project");
        commands.push("./notes add \"API design considerations\" --tag project");
        commands.push("./notes add \"Monitoring and alerting setup\"");
        commands.push("./notes add \"Deployment pipeline\"");

        // Link first note to last note
        commands.push("./notes link 1 4");

        commands.push("./notes list --tag project");
        commands.push("./notes search design");
        commands.push("./notes export --format json");

        let metrics = run_fixture("fixtures/example_guidance_minimal.yaml", &commands);
        assert_eq!(metrics.gates_passed, metrics.gates_total);
    }

    // ------------------------------------------------------------------
    // example_guidance_rich
    // ------------------------------------------------------------------
    #[test]
    fn example_guidance_rich_passes_all_gates() {
        let mut commands = vec!["./notes init"];
        commands.push("./notes add \"Database migration strategy\" --tag project");
        commands.push("./notes add \"API design considerations\" --tag project");
        commands.push("./notes add \"Monitoring and alerting setup\"");
        commands.push("./notes add \"Deployment pipeline\"");

        commands.push("./notes link 1 4");

        commands.push("./notes list --tag project");
        commands.push("./notes search design");
        commands.push("./notes export --format json");

        let metrics = run_fixture("fixtures/example_guidance_rich.yaml", &commands);
        assert_eq!(metrics.gates_passed, metrics.gates_total);
    }
}
