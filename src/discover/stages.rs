use crate::adapter::ToolRunOutput;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn stage_transcript_path(root_dir: &Path, stage: &str) -> PathBuf {
    root_dir
        .join("stages")
        .join(stage)
        .join("transcript.raw.txt")
}

pub(super) fn write_stage_output(
    root_dir: &Path,
    stage: &str,
    output: &ToolRunOutput,
) -> Result<()> {
    let stage_dir = root_dir.join("stages").join(stage);
    fs::create_dir_all(&stage_dir)?;
    fs::write(stage_dir.join("transcript.raw.txt"), &output.transcript)?;
    if let Some(raw) = &output.raw_output {
        fs::write(stage_dir.join("tool-output.raw.txt"), raw)?;
    }
    if output.token_usage.is_some() || output.cost_usd.is_some() {
        let (input, out_tokens) = output
            .token_usage
            .as_ref()
            .map(|tokens| (tokens.input, tokens.output))
            .unwrap_or((0, 0));
        fs::write(
            stage_dir.join("usage.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "input_tokens": input,
                "output_tokens": out_tokens,
                "cost_usd": output.cost_usd,
            }))?,
        )?;
    }
    Ok(())
}

pub(super) fn write_understanding_diagnostics(
    root_dir: &Path,
    stage: &str,
    diagnostics: &[String],
) -> Result<()> {
    let stage_dir = root_dir.join("stages").join(stage);
    fs::create_dir_all(&stage_dir)?;
    fs::write(
        stage_dir.join("understanding-diagnostics.txt"),
        diagnostics.join("\n"),
    )?;
    Ok(())
}
