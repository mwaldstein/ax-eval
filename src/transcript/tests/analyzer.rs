use super::super::analyzer::TranscriptAnalyzer;

#[test]
fn test_analyze_empty_transcript() {
    let transcript = "";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.total_commands, 0);
    assert_eq!(metrics.unique_commands, 0);
    assert_eq!(metrics.error_count, 0);
    assert_eq!(metrics.retry_count, 0);
    assert_eq!(metrics.help_invocations, 0);
    assert_eq!(metrics.first_try_success_rate, 0.0);
    assert_eq!(metrics.iteration_ratio, 0.0);
}

#[test]
fn test_analyze_single_command() {
    let transcript = "taskmgr create --title 'Test Note'";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.total_commands, 1);
    assert_eq!(metrics.unique_commands, 1);
    assert_eq!(metrics.error_count, 0);
    assert_eq!(metrics.retry_count, 0);
    assert_eq!(metrics.first_try_success_rate, 1.0);
    assert_eq!(metrics.iteration_ratio, 1.0);
}

#[test]
fn test_analyze_multiple_commands() {
    let transcript =
        "taskmgr create --title 'Test 1'\ntaskmgr create --title 'Test 2'\ntaskmgr list";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.total_commands, 3);
    assert_eq!(metrics.unique_commands, 2);
    assert_eq!(metrics.retry_count, 1);
}

#[test]
fn test_analyze_with_errors() {
    let transcript =
        "taskmgr create --title 'Test 1'\nError: command failed\ntaskmgr create --title 'Test 1'";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.total_commands, 2);
    assert_eq!(metrics.error_count, 1);
}

#[test]
fn test_analyze_help_invocations() {
    let transcript = "taskmgr --help\ntaskmgr create --title 'Test'\ntaskmgr list --help";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.total_commands, 3);
    assert_eq!(metrics.help_invocations, 2);
}

#[test]
fn test_iteration_ratio() {
    let transcript = "taskmgr create\ntaskmgr create\ntaskmgr create\ntaskmgr list\ntaskmgr list";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.total_commands, 5);
    assert_eq!(metrics.unique_commands, 2);
    assert_eq!(metrics.retry_count, 3);
    assert_eq!(metrics.iteration_ratio, 2.5);
}

#[test]
fn test_extract_commands_basic() {
    let transcript = "taskmgr create --title 'Test'\ntaskmgr list\ntaskmgr link --from a --to b";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].command, "list");
    assert_eq!(commands[1].exit_code, Some(0));
    assert_eq!(commands[2].command, "link");
    assert_eq!(commands[2].exit_code, Some(0));
}

#[test]
fn test_extract_commands_with_explicit_exit_code() {
    let transcript = "taskmgr create --title 'Test'\nExit Code: 0\ntaskmgr invalid\nExit status: 1";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].command, "invalid");
    assert_eq!(commands[1].exit_code, Some(1));
}

#[test]
fn test_extract_commands_with_implicit_error() {
    let transcript =
        "taskmgr create --title 'Test'\nError: something failed\ntaskmgr create --title 'Test'";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[0].exit_code, Some(1));
    assert_eq!(commands[1].command, "create");
    assert_eq!(commands[1].exit_code, Some(0));
}

#[test]
fn test_extract_commands_help_detection() {
    let transcript = "taskmgr --help\ntaskmgr create --help\ntaskmgr list";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].command, "help");
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].command, "help");
    assert_eq!(commands[1].exit_code, Some(0));
    assert_eq!(commands[2].command, "list");
    assert_eq!(commands[2].exit_code, Some(0));
}

#[test]
fn test_extract_commands_various_exit_code_formats() {
    let transcript = "taskmgr create\nexit code: 0\ntaskmgr delete\nExit Status: 127\ntaskmgr search\nexit code 255";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].exit_code, Some(127));
    assert_eq!(commands[2].exit_code, Some(255));
}

#[test]
fn test_extract_commands_empty_transcript() {
    let transcript = "";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 0);
}

#[test]
fn test_extract_commands_no_matching_commands() {
    let transcript = "Some random text\nWithout commands\nJust output";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 0);
}

#[test]
fn test_extract_commands_mixed_with_output() {
    let transcript = "Starting session...\ntaskmgr create --title 'Test'\nNote created successfully\ntaskmgr list\nList output\nDone";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[1].command, "list");
}

#[test]
fn test_extract_commands_case_insensitive_exit() {
    let transcript =
        "taskmgr create\nEXIT CODE: 0\ntaskmgr delete\nexit code: 1\ntaskmgr search\nExit Code: 2";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].exit_code, Some(1));
    assert_eq!(commands[2].exit_code, Some(2));
}

#[test]
fn test_extract_commands_with_multiple_errors_keywords() {
    let transcript =
        "taskmgr create\nERROR: invalid input\ntaskmgr delete\nFailed: not found\ntaskmgr search";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].exit_code, Some(1));
    assert_eq!(commands[1].exit_code, Some(1));
    assert_eq!(commands[2].exit_code, Some(0));
}

#[test]
fn test_extract_commands_nonzero_exit_code() {
    let transcript = "taskmgr create\nExit code: 130";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[0].exit_code, Some(130));
}

#[test]
fn test_extract_commands_large_exit_code() {
    let transcript = "taskmgr create\nExit code: 255";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[0].exit_code, Some(255));
}

#[test]
fn test_extract_commands_exit_code_takes_precedence() {
    let transcript = "taskmgr create\nExit code: 0\ntaskmgr delete\nError: failed\nExit code: 1";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].exit_code, Some(1));
}

#[test]
fn test_extract_commands_subcommand_with_flags() {
    let transcript = "taskmgr create --title 'Test' --tag work\ntaskmgr list --format json\ntaskmgr link --from a --to b --type reference";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[1].command, "list");
    assert_eq!(commands[2].command, "link");
}

#[test]
fn test_extract_commands_with_hypothetical_command_examples() {
    let transcript = "taskmgr create --title 'Ship v1'\nnotes-cli list --format json\nacme-tool deploy --env staging";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[1].command, "list");
    assert_eq!(commands[2].command, "deploy");
}

#[test]
fn test_analyze_with_target_binary_default_pattern() {
    let transcript = "mytool create\nmytool list\nothertool list";
    let metrics =
        TranscriptAnalyzer::analyze_with_exit_codes_for_target(transcript, "mytool", None);

    assert_eq!(metrics.total_commands, 2);
    assert_eq!(metrics.unique_commands, 2);
}

#[test]
fn test_analyze_with_target_custom_pattern_no_capture_group() {
    let transcript = "mytool create\nmytool list\nmytool --help\nothertool list";
    let metrics = TranscriptAnalyzer::analyze_with_exit_codes_for_target(
        transcript,
        "mytool",
        Some("mytool"),
    );

    assert_eq!(metrics.total_commands, 3);
    assert_eq!(metrics.help_invocations, 1);
}

#[test]
fn test_first_try_success_rate_all_succeed() {
    let transcript = "taskmgr create\ntaskmgr list";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.first_try_success_rate, 1.0);
}

#[test]
fn test_first_try_success_rate_first_fails_retry_succeeds() {
    let transcript = "taskmgr create\nError: failed\ntaskmgr create";
    let metrics = TranscriptAnalyzer::analyze(transcript);

    assert_eq!(metrics.first_try_success_rate, 0.0);
}

#[test]
fn test_first_try_success_rate_mixed_commands() {
    let transcript =
        "taskmgr init\nExit code: 0\ntaskmgr add\nExit code: 1\ntaskmgr add\nExit code: 0";
    let metrics = TranscriptAnalyzer::analyze_with_exit_codes(transcript);

    assert_eq!(metrics.total_commands, 3);
    assert_eq!(metrics.first_try_success_rate, 1.0 / 3.0);
}

#[test]
fn test_first_try_success_rate_unrelated_error_does_not_affect_other_commands() {
    let transcript = "taskmgr init\nExit code: 1\ntaskmgr add\nExit code: 0";
    let metrics = TranscriptAnalyzer::analyze_with_exit_codes(transcript);

    assert_eq!(metrics.first_try_success_rate, 0.5);
}
