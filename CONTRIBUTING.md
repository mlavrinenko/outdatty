# Contributing to outdatty

## Code Style

All clippy lints are set to `deny` level — the project will not compile with violations.

Key restrictions:
- No `unwrap()` — use the `?` operator or `thiserror` error handling
- No `todo!()`, `unimplemented!()`, `unreachable!()` — handle all cases
- No `unsafe` code
- No wildcard imports (`use foo::*`)
- No single-character variable names (minimum 2 characters)
- Functions: max 70 lines, max 5 arguments, max cognitive complexity 20

## Error Handling

- Use `thiserror::Error` for the library error type (`src/error.rs`); add a
  variant rather than stringly-typed errors
- The binary maps any error to exit code 2; drift versus success is carried by
  the returned `ExitCode`
- Propagate errors with `?` — never `unwrap()` or `expect()` in non-test code

## Project Structure

Keep `main.rs` as a thin entry point — argument parsing, logger init, and a call into
library code. All logic belongs in `lib.rs` (and its modules). `main.rs` is excluded from
coverage, so anything there is untested by default.

## Code Coverage

Minimum 70% coverage enforced via `cargo-tarpaulin`. Run `just cover` to check.
`main.rs` is excluded — keep it thin and move testable logic to `lib.rs`.

## File Size Limits

- Rust files: 500 lines max
- Markdown files: 200 lines max

When a file exceeds the limit, split it into modules or separate documents.

## Submitting Changes

1. Run `just check` before submitting — it runs clippy, tests, and file size checks
2. Run `just fmt` to format code
3. Ensure `just cover` meets the 70% threshold
