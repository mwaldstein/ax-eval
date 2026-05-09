pub mod analyzer;
mod redact;
pub mod types;
pub mod writer;

pub use analyzer::TranscriptAnalyzer;
pub use types::{
    CommandEvent, EfficiencyMetrics, EvaluationReport, InteractionInput, RunMetadata, RunReport,
    TokenUsage,
};
pub use writer::TranscriptWriter;

#[cfg(test)]
mod tests {
    mod analyzer;
    mod logging_tests;
    mod redact;
    mod writer_tests;
}
