use super::super::analyzer::TranscriptAnalyzer;

#[test]
fn extracts_no_commands_from_empty_transcript() {
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes("");

    assert_eq!(commands.len(), 0);
}

#[test]
fn extracts_subcommands_with_success_exit_codes() {
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
fn extracts_explicit_exit_codes() {
    let transcript = "taskmgr create --title 'Test'\nExit Code: 0\ntaskmgr invalid\nExit status: 1";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].command, "create");
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].command, "invalid");
    assert_eq!(commands[1].exit_code, Some(1));
}

#[test]
fn treats_nearby_error_output_as_nonzero_exit() {
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
fn detects_help_invocations_as_help_command() {
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
fn ignores_plain_output_that_does_not_match_command_pattern() {
    let transcript = "Some random text\nWithout commands\nJust output";
    let commands = TranscriptAnalyzer::extract_commands_with_exit_codes(transcript);

    assert_eq!(commands.len(), 0);
}

#[test]
fn extracts_commands_with_custom_pattern() {
    let transcript = "tool:notes action=sync\nexit code: 0\ntool:other action=skip\n";
    let commands =
        TranscriptAnalyzer::extract_commands_with_pattern(transcript, "tool:notes action=(\\S+)");

    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "sync");
    assert_eq!(commands[0].exit_code, Some(0));
}

#[test]
fn extracts_raw_command_lines_for_target_evidence_fallback() {
    let transcript = "$ ./notes init\nexit code: 0\n\
                      $ bash -lc './notes add \"Hello\"'\nexit code: 1\n\
                      $ /tmp/work/notes list --help\nexit code: 0\n";
    let commands = TranscriptAnalyzer::extract_command_lines_with_exit_codes(transcript);

    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].command, "./notes init");
    assert_eq!(commands[0].exit_code, Some(0));
    assert_eq!(commands[1].command, "bash -lc './notes add \"Hello\"'");
    assert_eq!(commands[1].exit_code, Some(1));
    assert_eq!(commands[2].command, "/tmp/work/notes list --help");
    assert_eq!(commands[2].exit_code, Some(0));
}
