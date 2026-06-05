use std::process::ExitCode;

/// Thin entry point: initialise logging, run the CLI, and map errors to the
/// operational exit code (2). Drift versus success (1 versus 0) is carried by
/// the [`ExitCode`] returned from [`outdatty::run`].
fn main() -> ExitCode {
    // Default to showing warnings (missing literals, zero-match globs) so drift
    // hints are visible in CI without requiring RUST_LOG; still overridable.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    match outdatty::run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}
