use std::process::ExitCode;

/// Thin entry point: initialise logging, run the CLI, and map errors to the
/// operational exit code (2). Drift versus success (1 versus 0) is carried by
/// the [`ExitCode`] returned from [`outdatty::run`].
fn main() -> ExitCode {
    env_logger::init();
    match outdatty::run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}
