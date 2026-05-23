use assert_cmd::{cargo::cargo_bin_cmd, Command};

/// Get a Command for ax-eval
pub fn ax_eval() -> Command {
    cargo_bin_cmd!("ax-eval")
}
