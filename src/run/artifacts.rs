use crate::fixture::TestEnv;
use crate::scenario::Scenario;
use crate::script_runner::{ScriptRunner, ScriptRunnerConfig};
use crate::transcript::TranscriptWriter;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RunArtifacts {
    results_dir: PathBuf,
    artifacts_dir: PathBuf,
    fixture_dir: PathBuf,
}

impl RunArtifacts {
    pub fn new(results_dir: &Path, env: &TestEnv) -> Self {
        let artifacts_dir = results_dir.join("artifacts");
        Self {
            results_dir: results_dir.to_path_buf(),
            artifacts_dir,
            fixture_dir: env.root.clone(),
        }
    }

    pub fn artifacts_dir(&self) -> &Path {
        &self.artifacts_dir
    }

    pub fn transcript_path(&self) -> PathBuf {
        self.artifacts_dir.join("transcript.raw.txt")
    }

    pub fn events_path(&self) -> PathBuf {
        self.artifacts_dir.join("events.jsonl")
    }

    pub fn fixture_transcript_path(&self) -> PathBuf {
        self.fixture_dir.join("transcript.raw.txt")
    }

    pub fn writer(&self) -> anyhow::Result<TranscriptWriter> {
        TranscriptWriter::new(self.artifacts_dir.clone(), self.results_dir.clone())
    }

    pub fn write_fixture_transcript(&self, content: &str) {
        std::fs::write(self.fixture_transcript_path(), content).ok();
    }

    pub fn script_runner(&self, scenario: &Scenario, tool: &str, model: &str) -> ScriptRunner {
        ScriptRunner::new(ScriptRunnerConfig {
            fixture_dir: self.fixture_dir.clone(),
            results_dir: self.results_dir.clone(),
            scenario_name: scenario.name.clone(),
            agent: tool.to_string(),
            model: model.to_string(),
            transcript_path: Some(self.transcript_path()),
            events_path: Some(self.events_path()),
            target_env: scenario.target.env.clone().unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_run_artifact_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("test env");
        let artifacts = RunArtifacts::new(&dir.path().join("results"), &env);

        assert_eq!(
            artifacts.artifacts_dir(),
            dir.path().join("results").join("artifacts").as_path()
        );
        assert_eq!(
            artifacts.transcript_path(),
            dir.path()
                .join("results")
                .join("artifacts")
                .join("transcript.raw.txt")
        );
        assert_eq!(
            artifacts.events_path(),
            dir.path()
                .join("results")
                .join("artifacts")
                .join("events.jsonl")
        );
        assert_eq!(
            artifacts.fixture_transcript_path(),
            dir.path().join("fixture").join("transcript.raw.txt")
        );
    }
}
