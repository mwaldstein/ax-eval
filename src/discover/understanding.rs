use std::fs;
use std::path::Path;

const UNDERSTANDING_HEADINGS: &[&str] = &[
    "## What the Tool Appears to Be For",
    "## Core Concepts and Mental Model",
    "## Primary Workflows",
    "## Useful Goal Areas",
    "## Evidence Consulted",
    "## Self-Description Quality",
    "## Ambiguities or Missing Information",
    "## Five Candidate Scenario Ideas",
];

pub(super) fn validate_understanding_artifact(path: &Path) -> std::result::Result<(), Vec<String>> {
    let content = fs::read_to_string(path).map_err(|error| {
        vec![format!(
            "understanding.md is missing or unreadable: {error}"
        )]
    })?;
    diagnose_understanding_content(&content)
}

fn diagnose_understanding_content(content: &str) -> std::result::Result<(), Vec<String>> {
    let trimmed = content.trim();
    let mut diagnostics = Vec::new();

    if trimmed.is_empty() {
        diagnostics.push("understanding.md is empty".to_string());
    }

    let first_nonempty = content.lines().find(|line| !line.trim().is_empty());
    let shell_prompt_lines = content
        .lines()
        .filter(|line| line.trim_start().starts_with("$ "))
        .count();
    let exit_code_lines = content
        .lines()
        .filter(|line| line.trim_start().starts_with("exit code:"))
        .count();
    if first_nonempty.is_some_and(|line| line.trim_start().starts_with("$ "))
        || shell_prompt_lines >= 3
        || exit_code_lines >= 3
    {
        diagnostics.push(
            "understanding.md appears to be a command transcript rather than synthesized Markdown"
                .to_string(),
        );
    }

    let lowercase_content = content.to_lowercase();
    for heading in UNDERSTANDING_HEADINGS {
        if !lowercase_content.contains(&heading.to_lowercase()) {
            diagnostics.push(format!(
                "understanding.md is missing required heading: {heading}"
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understanding_validation_rejects_transcript_fallback() {
        let content = r#"
$ qipu --help
Knowledge graph CLI designed for scripts and agents

exit code: 0

$ qipu create --help
Create a new note

exit code: 0
"#;

        let diagnostics =
            diagnose_understanding_content(content).expect_err("transcript should be rejected");

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("appears to be a command transcript")));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("## What the Tool Appears to Be For")));
    }

    #[test]
    fn understanding_validation_accepts_required_sections() {
        let content = format!(
            "# Discovery Understanding\n\n{}\n",
            UNDERSTANDING_HEADINGS
                .iter()
                .map(|heading| format!("{heading}\n\nSynthesis for this section."))
                .collect::<Vec<_>>()
                .join("\n\n")
        );

        diagnose_understanding_content(&content).expect("valid understanding");
    }
}
