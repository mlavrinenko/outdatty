# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/mlavrinenko/outdatty/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mlavrinenko/outdatty/releases/tag/v0.1.0
