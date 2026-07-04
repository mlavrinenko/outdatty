# outdatty

[![CI](https://github.com/mlavrinenko/outdatty/actions/workflows/ci.yml/badge.svg)](https://github.com/mlavrinenko/outdatty/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/outdatty.svg)](https://crates.io/crates/outdatty)
[![License: MIT](https://img.shields.io/crates/l/outdatty.svg)](LICENSE-MIT)

Declare a dependency graph between arbitrary files and have CI fail when a
source changes but its dependents were not re-confirmed. Language- and
format-agnostic: code, docs, `.docx`, configs — anything with bytes.

Unlike build systems (Task, Bazel, Buck2) it runs no commands, and unlike file
integrity monitors (AIDE) it models dependencies between files. It only answers
one question: did a source change without its dependents being confirmed?

## How it works

1. A YAML manifest declares groups: each couples `source` artifacts to the
   `dependents` that must stay in sync with them.
2. `outdatty update` records a `blake3` hash of every file into a committed
   lockfile (`outdatty.lock`) — this is the developer explicitly confirming the
   group is in sync.
3. `outdatty check` (in CI) recomputes hashes. If a source changed since the
   last confirmation, the group is stale and the check fails with exit code 1.
   Editing only a dependent is allowed (set `bidirectional: true` to flag that
   too).
4. The developer reviews, updates the dependents, and runs `outdatty update`
   to re-confirm.

## Install

### From crates.io

```bash
cargo install outdatty
```

### With Nix

Run without installing:

```bash
nix run github:mlavrinenko/outdatty -- check
```

Or add it to your flake inputs:

```nix
# flake.nix
outdatty.url = "github:mlavrinenko/outdatty";
```

### From binary releases

Download a pre-built binary from the
[latest release](https://github.com/mlavrinenko/outdatty/releases/latest).

## Usage

```bash
outdatty init                 # write a starter outdatty.yaml
outdatty update               # confirm: record current hashes into outdatty.lock
outdatty check                # CI gate: exit 1 if a source changed without re-confirmation
outdatty status               # show every group without failing
outdatty update --group docs  # confirm only one group
outdatty schema               # print the manifest JSON schema
```

Global flags: `--manifest <path>`, `--lock <path>`,
`--format plain|json|quiet|paths|paths0`, `--color auto|always|never`. Plain
output is colorized on an interactive terminal; `auto` (the default) disables
color when piped or when `NO_COLOR` is set.

Exit codes: `0` in sync, `1` drift (check only), `2` operational error
(unknown group, duplicate group name, unparseable manifest, incompatible
lockfile). A missing lockfile is not an operational error: `check` treats every
group as new and exits `1`, so run `outdatty update` to create it.

`--format=paths` (newline-delimited) and `--format=paths0` (NUL-delimited)
print just the deduped, sorted changed-source paths — no labels, no summary —
so you can pipe the drift straight into your own diff or editor. outdatty
stores hashes, not diffs, so it hands you the path set and you pick the tool:

```bash
outdatty status --format=paths0 | xargs -0 -r git diff --   # robust: handles spaces
git diff -- $(outdatty status --format=paths)                # simple
```

### Manifest

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/mlavrinenko/outdatty/main/schema/outdatty.schema.json
# gitignore: false                    # match every file; default skips .gitignored paths
groups:
  - name: feature
    source: [src/feature.rs]          # literal paths or globs like src/**/*.rs
    dependents: [docs/feature.mdx, tests/feature.rs]
    # bidirectional: true             # also flag the group when a dependent changes
```

See [examples/outdatty.yaml](examples/outdatty.yaml). A path that disappears —
a deleted literal, or a glob that no longer matches a previously locked file —
counts as a change to confirm. Glob expansion skips files ignored by git — the
`.gitignore` files (root and nested), the global excludes, and
`.git/info/exclude` — so build output never enters a group; set
`gitignore: false` at the top of the manifest to match every file. The `.git`
directory is never traversed, and symlinks are not followed during glob
expansion.

### Coverage: catch brand-new files

A glob source flags a file that *changed*. Coverage flags a file that no group
mentions *at all* — a brand-new file someone forgot to wire in. `require_tracked`
lists the files that must appear in some group's `source` or `dependents`; any
that do not fail `check` as `untracked`.

```yaml
# Default when omitted: ["**"] — every git-tracked file must belong to a group.
require_tracked:
  - "**"              # require everything, then carve out what has no coupling
  - "!vendor/**"      # last match wins; `!` excludes
  - "!LICENSE"
# require_tracked: ["src/**"]   # or require only a subtree (no negation needed)
# require_tracked: ["!**"]      # or opt out entirely
```

Patterns match last-wins: the last one to match a path decides, and a leading
`!` excludes it. The universe is every file git does not ignore (honouring
`gitignore`); the manifest and lockfile are always exempt. A failing check names
each untracked file so you can add it to a group or exclude it.

## CI and pre-commit

Gate a repository by running `check` in CI; it exits non-zero on drift.

```yaml
# .github/workflows/outdatty.yml
- run: outdatty check --format quiet
```

For a local guard, add a pre-commit hook:

```bash
# .git/hooks/pre-commit
outdatty check --format quiet || {
  echo "outdatty: a source changed without its dependents; run 'outdatty update'" >&2
  exit 1
}
```

## Contributing

Development setup and coding conventions live in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
