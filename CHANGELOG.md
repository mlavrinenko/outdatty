# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
