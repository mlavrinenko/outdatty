# Contributing to outdatty

## Development

Prerequisites: [Nix](https://nixos.org/) with flakes enabled.

```bash
direnv allow   # or: nix develop

just check     # fmt + clippy + tests + file-size check
just build
just test
just cover     # code coverage (70% minimum)
just fmt       # format code
```

## Worktrees

Work in parallel in a sibling worktree. `just fork` makes one and `just land`
merges it back; both wrap [worktrunk](https://worktrunk.dev) (`wt`, from the
dev shell) with the flags this repo needs, because a bare `wt merge` squashes
the commits together and commits a dirty tree.

```bash
just fork <slug>           # a sibling worktree on a new branch, .envrc allowed
just fork <slug> --base=@  # the same, from the current commit, not main
just land                  # in the worktree: gate, fast-forward, remove
```

A worktree only gets commits, never your working tree, so commit first. Do not
`git stash` in one: all worktrees share a single stash list.

[.config/wt.toml](.config/wt.toml) holds the hooks, so a clone with nix and
direnv needs no worktrunk config of its own. A new worktree runs `direnv allow`;
before the fast-forward `just check` runs, and a red gate stops the merge.

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

## Commit Conventions

Use [Conventional Commits](https://www.conventionalcommits.org/): a `type(scope):
summary` subject (`feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`, ...).

The backlog lives in `tasks/`, one Typst file per task, changed with `mt`. A task
gets its own commits when it's filed and when it's closed, apart from the work.
No commit carries a `Refs:` footer, since `.mindtape/config.toml` defines no
commit rules.

## Submitting Changes

1. Run `just check` before submitting — it runs clippy, tests, and file size checks
2. Run `just fmt` to format code
3. Ensure `just cover` meets the 70% threshold

## Releasing

Bump `version` in `Cargo.toml`, give it a dated `CHANGELOG.md` section and
commit; don't push. Run `just release X.Y.Z --dry-run`, then `just release
X.Y.Z` once it passes: it pushes main, waits for the `ci.yml` run on that
commit, and tags only if the run is green. The tag push is the only publish path:
never `cargo publish` by hand. A release whose workflow failed is finished on
the same tag with `gh workflow run release.yml -f tag=vX.Y.Z`; a pushed tag never
moves.
