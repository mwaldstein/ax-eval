//! Judge calibration harness.
//!
//! A calibration suite is a set of frozen agent transcripts, each paired with a
//! known target score band. Running the suite feeds every transcript to the real
//! LLM-as-judge — through the exact production path ([`maybe_run_judge`]) — and
//! checks whether the judge's score lands in band. Because the cases span a
//! flawless run down to an outright failure, the aggregate signed error is a
//! direct measure of judge leniency: a lenient judge inflates the low cases.
//!
//! The suite data lives at `ax-eval-fixtures/calibration/calibration.yaml`. The
//! actual judge call needs a live CLI adapter, so the runner is invoked from the
//! `ax-eval calibrate` command (gated by `AX_EVAL_ENABLED`), never from a
//! hermetic unit test. The scoring/report logic below is pure and unit-tested.

use crate::evaluation::{maybe_run_judge, GateStatus};
use crate::interaction_evidence::InteractionInput;
use crate::judge::Criterion;
use crate::scenario::{
    default_prescriptiveness_discount, Evaluation, JudgeConfig, Scenario, TargetConfig, Task,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Tolerance applied when testing band membership, so a score exactly on a
/// boundary counts as in band.
const BAND_EPSILON: f64 = 1e-9;

/// A calibration suite: a shared task + rubric applied to a set of frozen cases.
#[derive(Debug, Clone, Deserialize)]
pub struct CalibrationSuite {
    /// Target tool the transcripts exercise (its name is shown to the judge).
    pub target: TargetConfig,
    /// The task all cases were attempting.
    pub task: Task,
    /// Optional rubric path (resolved relative to the suite file). Falls back to
    /// the default judge rubric when omitted and no inline criteria are given.
    #[serde(default)]
    pub rubric: Option<String>,
    /// Optional inline rubric criteria, used when `rubric` is omitted.
    #[serde(default)]
    pub criteria: Vec<Criterion>,
    /// Discount used to derive the informational adjusted score.
    #[serde(default = "default_prescriptiveness_discount")]
    pub prescriptiveness_discount: f64,
    /// The calibration cases.
    pub cases: Vec<CalibrationCase>,
}

/// One calibration case: a frozen transcript with a known target score band.
#[derive(Debug, Clone, Deserialize)]
pub struct CalibrationCase {
    /// Short identifier (e.g. `sloppy_success`).
    pub name: String,
    /// Human-readable description of what the transcript shows.
    #[serde(default)]
    pub label: String,
    /// Path to the frozen transcript, relative to the suite file.
    pub transcript: String,
    /// The score the judge is expected to assign this run.
    pub expected: ScoreBand,
    /// Optional agent guidance to stage as `AGENTS.md` for this case.
    #[serde(default)]
    pub guidance: Option<String>,
}

/// An inclusive target score interval.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ScoreBand {
    /// Lower bound (inclusive).
    pub min: f64,
    /// Upper bound (inclusive).
    pub max: f64,
}

impl ScoreBand {
    /// Whether `score` falls within the band (with a small boundary tolerance).
    pub fn contains(&self, score: f64) -> bool {
        score >= self.min - BAND_EPSILON && score <= self.max + BAND_EPSILON
    }

    /// The center of the band, used as the reference point for signed error.
    pub fn midpoint(&self) -> f64 {
        (self.min + self.max) / 2.0
    }

    /// `score - midpoint`. Positive means the judge scored above the intended
    /// center — i.e. more lenient than intended.
    pub fn signed_error(&self, score: f64) -> f64 {
        score - self.midpoint()
    }
}

/// Judge overrides for a calibration run.
#[derive(Debug, Clone, Copy, Default)]
pub struct CalibrationRun<'a> {
    /// Judge CLI tool override (e.g. `opencode`, `codex`, `claude`).
    pub judge_tool: Option<&'a str>,
    /// Judge model override.
    pub judge_model: Option<&'a str>,
}

/// The outcome of judging a single calibration case.
#[derive(Debug, Clone)]
pub struct CaseOutcome {
    /// Case identifier.
    pub name: String,
    /// Case label.
    pub label: String,
    /// The case's target band.
    pub expected: ScoreBand,
    /// The judge's raw weighted score, or `None` if the judge failed to run.
    pub score: Option<f64>,
    /// The judge's difficulty-adjusted score, if reported.
    pub adjusted_score: Option<f64>,
    /// The prescriptiveness level the judge assigned, if reported.
    pub prescriptiveness_level: Option<u8>,
    /// Failure detail when the judge could not be run or parsed.
    pub error: Option<String>,
}

impl CaseOutcome {
    /// Whether the judge's score landed in band. `None` if the case did not score.
    pub fn in_band(&self) -> Option<bool> {
        self.score.map(|score| self.expected.contains(score))
    }

    /// Signed error against the band midpoint (positive = lenient). `None` if the
    /// case did not score.
    pub fn signed_error(&self) -> Option<f64> {
        self.score.map(|score| self.expected.signed_error(score))
    }
}

/// The result of running a full calibration suite.
#[derive(Debug, Clone)]
pub struct CalibrationReport {
    /// Per-case outcomes, in suite order.
    pub outcomes: Vec<CaseOutcome>,
}

impl CalibrationReport {
    /// Cases that produced a score (excludes judge errors).
    fn scored(&self) -> impl Iterator<Item = &CaseOutcome> {
        self.outcomes.iter().filter(|o| o.score.is_some())
    }

    /// Number of cases that produced a score.
    pub fn scored_count(&self) -> usize {
        self.scored().count()
    }

    /// Number of scored cases whose score landed in band.
    pub fn in_band_count(&self) -> usize {
        self.scored()
            .filter(|o| o.in_band().unwrap_or(false))
            .count()
    }

    /// Scored cases that landed outside their band.
    pub fn out_of_band(&self) -> Vec<&CaseOutcome> {
        self.scored()
            .filter(|o| !o.in_band().unwrap_or(false))
            .collect()
    }

    /// Mean signed error across scored cases (positive = lenient overall).
    /// `None` when no case scored.
    pub fn leniency_index(&self) -> Option<f64> {
        let errors: Vec<f64> = self
            .scored()
            .filter_map(CaseOutcome::signed_error)
            .collect();
        if errors.is_empty() {
            return None;
        }
        Some(errors.iter().sum::<f64>() / errors.len() as f64)
    }

    /// Render a human-readable report table with a leniency summary.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "Judge Calibration");
        let _ = writeln!(
            out,
            "{:<16} {:>11} {:>7} {:>7} {:>7} {:>6} {:>8}  label",
            "case", "target", "score", "adj", "signed", "presc", "in-band"
        );
        for o in &self.outcomes {
            let target = format!("{:.2}-{:.2}", o.expected.min, o.expected.max);
            let (score, adj, signed, presc, in_band) = match o.score {
                Some(s) => (
                    format!("{s:.2}"),
                    o.adjusted_score
                        .map(|a| format!("{a:.2}"))
                        .unwrap_or_else(|| "-".to_string()),
                    format!("{:+.2}", o.expected.signed_error(s)),
                    o.prescriptiveness_level
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    if o.expected.contains(s) { "yes" } else { "NO" }.to_string(),
                ),
                None => (
                    "err".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "err".to_string(),
                ),
            };
            let _ = writeln!(
                out,
                "{:<16} {target:>11} {score:>7} {adj:>7} {signed:>7} {presc:>6} {in_band:>8}  {}",
                o.name, o.label
            );
            if let Some(err) = &o.error {
                let _ = writeln!(out, "                 error: {err}");
            }
        }
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "In band: {}/{} scored ({} case(s) did not run)",
            self.in_band_count(),
            self.scored_count(),
            self.outcomes.len() - self.scored_count()
        );
        match self.leniency_index() {
            Some(index) => {
                let _ = writeln!(
                    out,
                    "Leniency index (mean signed error): {index:+.3}  (positive = judge scores above target)"
                );
            }
            None => {
                let _ = writeln!(out, "Leniency index: n/a (no cases scored)");
            }
        }
        out
    }
}

/// Load and run a calibration suite, judging every case.
pub fn run_suite(suite_path: &Path, run: &CalibrationRun) -> Result<CalibrationReport> {
    let suite = load_suite(suite_path)?;
    let suite_dir = suite_path.parent().unwrap_or_else(|| Path::new("."));
    let outcomes = suite
        .cases
        .iter()
        .map(|case| run_case(&suite, suite_dir, suite_path, case, run))
        .collect();
    Ok(CalibrationReport { outcomes })
}

/// Parse a calibration suite from YAML.
pub fn load_suite(suite_path: &Path) -> Result<CalibrationSuite> {
    let content = std::fs::read_to_string(suite_path)
        .with_context(|| format!("Failed to read calibration suite {}", suite_path.display()))?;
    yaml_serde::from_str(&content)
        .with_context(|| format!("Failed to parse calibration suite {}", suite_path.display()))
}

/// A self-cleaning scratch workspace for staging one case's judge inputs.
///
/// Avoids a runtime dependency on `tempfile` (a dev-only dep here) while still
/// removing the directory when the case finishes.
struct Workspace {
    path: PathBuf,
}

impl Workspace {
    fn new() -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = format!(
            "ax-eval-calibration-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).with_context(|| {
            format!("Failed to create calibration workspace {}", path.display())
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run_case(
    suite: &CalibrationSuite,
    suite_dir: &Path,
    suite_path: &Path,
    case: &CalibrationCase,
    run: &CalibrationRun,
) -> CaseOutcome {
    match judge_case(suite, suite_dir, suite_path, case, run) {
        Ok(result) => CaseOutcome {
            name: case.name.clone(),
            label: case.label.clone(),
            expected: case.expected,
            score: result.score,
            adjusted_score: result.adjusted_score,
            prescriptiveness_level: result.prescriptiveness_level,
            error: result.error,
        },
        Err(error) => CaseOutcome {
            name: case.name.clone(),
            label: case.label.clone(),
            expected: case.expected,
            score: None,
            adjusted_score: None,
            prescriptiveness_level: None,
            error: Some(format!("{error:#}")),
        },
    }
}

struct JudgedCase {
    score: Option<f64>,
    adjusted_score: Option<f64>,
    prescriptiveness_level: Option<u8>,
    error: Option<String>,
}

fn judge_case(
    suite: &CalibrationSuite,
    suite_dir: &Path,
    suite_path: &Path,
    case: &CalibrationCase,
    run: &CalibrationRun,
) -> Result<JudgedCase> {
    let transcript_src = suite_dir.join(&case.transcript);
    let transcript = std::fs::read_to_string(&transcript_src)
        .with_context(|| format!("Failed to read transcript {}", transcript_src.display()))?;

    // Stage a workspace that mirrors what a real run leaves behind: the frozen
    // transcript at the path the judge reads, plus any agent guidance.
    let env = Workspace::new()?;
    std::fs::write(env.path().join("transcript.raw.txt"), transcript)
        .context("Failed to stage transcript")?;
    if let Some(guidance) = &case.guidance {
        std::fs::write(env.path().join("AGENTS.md"), guidance)
            .context("Failed to stage agent guidance")?;
    }

    let scenario = calibration_scenario(suite, case);
    let result = maybe_run_judge(
        &scenario,
        env.path(),
        suite_path,
        false,
        GateStatus::NotConfigured,
        run.judge_model,
        run.judge_tool,
        &InteractionInput::TranscriptRegex,
    )?;

    Ok(JudgedCase {
        score: result.score,
        adjusted_score: result.response.as_ref().and_then(|r| r.adjusted_score),
        prescriptiveness_level: result
            .response
            .as_ref()
            .and_then(|r| r.prescriptiveness.as_ref())
            .map(|p| p.level),
        error: result.error,
    })
}

/// Build the synthetic scenario used to judge one calibration case. It carries
/// the suite's target, task, and rubric with the judge enabled; gates and
/// composite are irrelevant here.
fn calibration_scenario(suite: &CalibrationSuite, case: &CalibrationCase) -> Scenario {
    Scenario {
        name: format!("calibration_{}", case.name),
        description: format!("Calibration case {}", case.name),
        template_folder: ".".to_string(),
        target: suite.target.clone(),
        task: suite.task.clone(),
        evaluation: Evaluation {
            gates: vec![],
            judge: Some(JudgeConfig {
                enabled: true,
                tool: None,
                rubric: suite.rubric.clone(),
                criteria: suite.criteria.clone(),
                // Pass threshold is irrelevant: calibration reads the raw score,
                // not the pass/fail verdict.
                pass_threshold: 0.0,
                prescriptiveness_discount: suite.prescriptiveness_discount,
            }),
            composite: None,
        },
        tier: 0,
        tool_matrix: None,
        setup: None,
        tags: vec![],
        run: None,
        scripts: None,
        interaction: Default::default(),
        agent_env: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(min: f64, max: f64) -> ScoreBand {
        ScoreBand { min, max }
    }

    fn outcome(name: &str, expected: ScoreBand, score: Option<f64>) -> CaseOutcome {
        CaseOutcome {
            name: name.to_string(),
            label: String::new(),
            expected,
            score,
            adjusted_score: None,
            prescriptiveness_level: None,
            error: score.is_none().then(|| "judge failed".to_string()),
        }
    }

    #[test]
    fn band_contains_is_inclusive_with_boundary_tolerance() {
        let b = band(0.45, 0.62);
        assert!(b.contains(0.45));
        assert!(b.contains(0.62));
        assert!(b.contains(0.50));
        assert!(!b.contains(0.44));
        assert!(!b.contains(0.63));
    }

    #[test]
    fn band_signed_error_is_positive_when_score_exceeds_midpoint() {
        let b = band(0.0, 0.20); // midpoint 0.10
        assert!((b.signed_error(0.85) - 0.75).abs() < 1e-9);
        assert!((b.signed_error(0.10)).abs() < 1e-9);
        assert!(b.signed_error(0.05) < 0.0);
    }

    #[test]
    fn leniency_index_is_mean_signed_error_over_scored_cases() {
        // failed case judged far too high (out of band); flawless judged correctly.
        let report = CalibrationReport {
            outcomes: vec![
                outcome("failed", band(0.0, 0.20), Some(0.80)), // signed +0.70, out of band
                outcome("flawless", band(0.88, 1.0), Some(0.94)), // signed 0.0, in band
            ],
        };
        let index = report.leniency_index().expect("index");
        assert!((index - 0.35).abs() < 1e-9, "got {index}");
        assert_eq!(report.scored_count(), 2);
        assert_eq!(report.in_band_count(), 1);
        assert_eq!(report.out_of_band().len(), 1);
    }

    #[test]
    fn out_of_band_and_all_in_band_reflect_scores() {
        let report = CalibrationReport {
            outcomes: vec![
                outcome("sloppy", band(0.45, 0.62), Some(0.85)), // out of band
                outcome("flawless", band(0.88, 1.0), Some(0.95)),
            ],
        };
        assert_eq!(report.out_of_band().len(), 1);
        assert_eq!(report.out_of_band()[0].name, "sloppy");
        assert_eq!(report.in_band_count(), 1);
    }

    #[test]
    fn errored_cases_are_excluded_from_scoring_and_block_all_in_band() {
        let report = CalibrationReport {
            outcomes: vec![
                outcome("flawless", band(0.88, 1.0), Some(0.95)),
                outcome("failed", band(0.0, 0.20), None),
            ],
        };
        assert_eq!(report.scored_count(), 1);
        assert_eq!(report.in_band_count(), 1);
        // render must not panic on an errored case.
        assert!(report.render().contains("case"));
    }

    #[test]
    fn bundled_suite_parses_and_transcripts_exist() {
        let suite_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("ax-eval-fixtures/calibration/calibration.yaml");
        let suite = load_suite(&suite_path).expect("bundled calibration suite parses");
        let suite_dir = suite_path.parent().unwrap();

        assert!(!suite.cases.is_empty());
        for case in &suite.cases {
            assert!(
                case.expected.min <= case.expected.max,
                "case {} has inverted band",
                case.name
            );
            assert!(
                case.expected.min >= 0.0 && case.expected.max <= 1.0,
                "case {} band out of [0,1]",
                case.name
            );
            let transcript = suite_dir.join(&case.transcript);
            assert!(
                transcript.exists(),
                "case {} transcript missing: {}",
                case.name,
                transcript.display()
            );
        }
    }

    #[test]
    fn calibration_scenario_enables_judge_with_suite_discount() {
        let suite_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("ax-eval-fixtures/calibration/calibration.yaml");
        let suite = load_suite(&suite_path).expect("suite");
        let scenario = calibration_scenario(&suite, &suite.cases[0]);
        let judge = scenario.evaluation.judge.expect("judge configured");
        assert!(judge.enabled);
        assert_eq!(
            judge.prescriptiveness_discount,
            suite.prescriptiveness_discount
        );
    }
}
