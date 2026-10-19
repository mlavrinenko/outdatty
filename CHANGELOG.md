# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.0] - 2026-10-19

### Added

- An optional top-level `root:` in the manifest, relative to the manifest's
  directory, sets where every path and every lockfile key resolves from. A
  pair kept in `.config/` sets `root: ..` and keeps the paths and lock keys it
  had at the repo root. Without `root`, nothing changes. The lockfile still
  defaults to `outdatty.lock` beside the manifest, and a `root` that names no
  directory is an error.
- Without `--manifest`, outdatty also finds `.config/outdatty.yaml` and
  `.config/outdatty.yml`, after `outdatty.yaml` and `outdatty.yml`.

## [0.8.0] - 2026-10-17

### Added

- `check` fails on each group the lockfile holds but `outdatty.yaml` no longer
  declares, which used to pile up unseen after a split or rename. Plain output
  names each one under `[orphaned]` with the fix, `outdatty update`, which
  prunes them; the summary counts them (`1 orphaned lock group`); `status`
  lists them without failing; JSON gains a top-level `orphaned` array. A
  `--group` check skips them, as it skips coverage.

### Changed

- `check --format quiet` still prints nothing on a pass, but a failure now
  prints what plain `check` would (the failing groups, what changed, the
  dependents to review and the confirming command) to stderr, so a gate that
  runs it no longer goes red without saying why. `status`, `init` and `update`
  stay silent under `quiet`.

## [0.7.0] - 2026-10-07

### Changed

- Plain `check` lists only the failing groups and untracked files, so one stale
  group no longer hides among `[  ok   ]` lines; a passing run prints one line.
  The summary reads `outdatty: 1 of 20 groups out of date, 2 untracked files`
  or `outdatty: 20 groups up to date`, and no longer suggests a blanket
  `outdatty update`, since each failing group names its own confirming command.
  `status` still lists every group.

## [0.6.0] - 2026-10-07

### Added

- `check` now fails a directed group whose dependents glob expands to a file the
  lockfile holds no hash for (a newly added page), which it used to ignore
  forever. Plain output names the file and the command that records it
  (`record it: outdatty update --group <id> --dependent <path>`); `status`
  shows it without failing; JSON gains status `unrecorded` and a per-group
  `unrecorded_dependents` array; `paths`/`paths0` include those files. A new
  source file already failed as a changed source, and a group with no lock
  entry stays `new`.

## [0.5.0] - 2026-09-22

### Added

- `update --group <id> --dependent <path>` (both repeatable) records only the
  named dependents' hashes, leaving the group's sources and other pending
  dependents as locked, so confirming one reviewed file no longer claims review
  of the rest. Each path is recorded in whichever named groups declare it, and
  plain output lists it under the group as `recorded dependent:` (JSON: a
  `recorded` array). A path no named group declares, or a group never recorded,
  is an error and writes nothing.

## [0.4.0] - 2026-07-04

### Added

- Whole-project coverage via a manifest-level `require_tracked` (default
  `["**"]`): every file git does not ignore must appear in some group's `source`
  or `dependents`, so a brand-new file that no group tracks fails `check` as
  `untracked`. Patterns match last-wins with `!` negation (e.g.
  `["**", "!vendor/**"]`); the manifest and lockfile are always exempt, and
  `["!**"]` opts out. Coverage runs only on a whole-manifest check, not under a
  scoped `--group` selection. JSON `check` output gained a top-level `untracked`
  array, and `failed` is set when it is non-empty.

## [0.3.0] - 2026-07-02

### Added

- `--format=paths` (newline-delimited) and `--format=paths0` (NUL-delimited)
  print the deduped, sorted changed-source paths with no labels or summary, so
  drift can be piped straight into your own diff or editor tool.
- json `GroupReport` now always carries the group's full declared `dependents`,
  so tooling can find review targets without re-parsing the manifest.
- Plain output for a failing group lists a `review dependent:` line for each
  declared dependent, alongside `source changed:`.
- Portable usage skill (`skills/outdatty/SKILL.md`) covering manifest
  authoring, CI gating, and the review-before-update loop.

### Fixed

- Group `dependents` is no longer a required manifest key; omitting it now
  defaults to an empty list, matching `bidirectional`.

## [0.2.0] - 2026-06-14

### Changed

- Group `name` is now required and must be non-empty and unique. Lockfile
  entries are keyed by it, so the positional fallback id (`group[N]`) is gone:
  reordering or inserting groups can no longer silently remap a confirmation.
  A missing or blank name is an operational error (exit 2).
- Dependent-only changes in a directed group are no longer surfaced in plain
  `check`/`status` output; they were never failures. The `dependent_drift`
  status is removed (a directed dependent edit now reads as `ok`); `--format
  json` still lists the differing paths under `changed_dependents`.

## [0.1.0] - 2026-06-06

### Added

- Declarative YAML manifest (`outdatty.yaml`) describing dependency groups of
  source and dependent artifacts, with an optional `bidirectional` flag.
- Committed lockfile (`outdatty.lock`) recording `blake3` hashes per group.
- Commands: `init`, `check`, `update`, `status`, `schema`.
- `check` fails (exit 1) when a source changed without its dependents being
  re-confirmed; `update` records the confirmation. Operational errors exit 2.
- Literal and glob path patterns; `--group` targeting; `--format plain|json|quiet`.
- JSON Schema for the manifest, generated from the Rust types and committed
  under `schema/`, with a `yaml-language-server` modeline for editor validation.
- Manifest validation: duplicate group names and groups with an empty `source`
  are rejected (exit 2).
- Gitignore-aware glob expansion, on by default; set `gitignore: false` at the
  top of the manifest to match every file including ignored build output.
- Lockfile compatibility checks: a newer `version` or a foreign hash
  `algorithm` is rejected instead of silently mis-comparing.
- `outdatty.yaml` gating this repository (the tool checks itself in CI).

### Changed

- Glob expansion now honours the full gitignore chain (nested `.gitignore`,
  global excludes, `.git/info/exclude`) instead of only the root `.gitignore`,
  never traverses `.git`, and no longer follows symlinks. Internally it uses
  `globset` + `ignore` in place of the `glob` crate.
- Files are hashed in parallel.
- `check` plain output now prints the exact `outdatty update --group <id>`
  command for each out-of-date group, and warnings (missing literals, zero-match
  globs) are shown by default.
- `update` JSON output gained a `version`/`total` envelope, matching `check`.
- A missing literal source is now treated as a change (drift) rather than a
  hard error, matching how a vanished glob match behaves.
- The lockfile is written atomically (temp file + rename).
- JSON output carries a top-level `failed`/`total`/`out_of_date` summary and a
  `version`; `status` and update `action` values use `snake_case`; payloads end
  with a newline.
- `update` reports pruned orphan lockfile entries with a `removed` action.

[Unreleased]: https://github.com/mlavrinenko/outdatty/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/mlavrinenko/outdatty/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/mlavrinenko/outdatty/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/mlavrinenko/outdatty/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/mlavrinenko/outdatty/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/mlavrinenko/outdatty/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/mlavrinenko/outdatty/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mlavrinenko/outdatty/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mlavrinenko/outdatty/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/mlavrinenko/outdatty/releases/tag/v0.1.0
