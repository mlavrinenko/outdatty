#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "link the integration tests into one binary",
  priority: 5,
  status: proposed(2026, 10, 11),
)

= Problem

Cargo builds one executable per file in `tests/`, and each one links every dependency again. outdatty has two (`tests/cli.rs`, `tests/unrecorded.rs`), so two binaries link `clap`, `assert_cmd`, `tempfile` and the library.

Measured 2026-10-11 in mindtape: 74 test files gave `target/debug/deps` 263 executables averaging 298 MB, 80 GB in all. `just cover` builds the whole set a second time, instrumented: 51 GB in one worktree, and the disk filled. canibump's 7 test files built 28 binaries of about 37 MB each with full debug info. Debug info size is handled host-wide in `~/.nix`; this task is the per-repo half. Two files is a small saving today, but every new file under `tests/` would add a binary.

= Change

+ Record the baseline: `just count-tests` (it sums every `test result:` line of `cargo test --workspace`), and after `cargo test --no-run` the output of `find target/debug/deps -maxdepth 1 -type f -executable | wc -l`.
+ `git mv tests/cli.rs tests/unrecorded.rs tests/it/`, then add `tests/it/main.rs` with `mod cli; mod unrecorded;`. Cargo autodiscovers `tests/it/main.rs` as the test target `it`.
+ Helpers: both files define their own `bin()` and `write()`, private to the module, so they do not collide. Leave them, or lift the copies into `tests/it/common/mod.rs` declared once in `main.rs` as `mod common;`.
+ Relative paths: neither file uses `include_str!`, `include_bytes!`, `#[path]` or `env!("CARGO_MANIFEST_DIR")`.
+ `Cargo.toml` has no `[[test]]` entries. Add none unless autodiscovery fails.
+ outdatty watches its own tests: the `code` group in `outdatty.yaml:46-51` lists `tests/cli.rs` and `tests/unrecorded.rs` as dependents, and `outdatty.lock:34-35` records their hashes. Replace the two entries with the glob `tests/it/**/*.rs`, run `just outdatty-update`, and commit the refreshed `outdatty.lock` with the move. The Justfile runs `cargo test --workspace` with no `--test` flag, and `tarpaulin.toml` names no test file. Re-run `grep -rn "tests/" --exclude-dir=target --exclude-dir=.git .` to confirm nothing else changed; the other hits (`examples/outdatty.yaml`, `README.md`, `www/index.html`, `src/commands.rs`, `skills/outdatty/SKILL.md`) are invented `tests/feature.rs` examples and stay.

= Done when

- `just count-tests` prints the same total as before the move.
- `cargo test --no-run` builds one integration executable, `it`, not two.
- `just check` is green, `outdatty.lock` is refreshed, and no file still names `tests/cli.rs`.
- Out of scope: inline `#[cfg(test)]` unit tests (they already share one binary per crate) and debug info settings.
