# outdatty

[![CI](https://github.com/mlavrinenko/outdatty/actions/workflows/ci.yml/badge.svg)](https://github.com/mlavrinenko/outdatty/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/outdatty.svg)](https://crates.io/crates/outdatty)
[![License: MIT](https://img.shields.io/crates/l/outdatty.svg)](LICENSE-MIT)

Declare which files depend on which, and CI fails when a source changes but
nobody re-confirmed its dependents. Language and format don't matter. Code,
docs, `.docx`, configs, anything with bytes.

Unlike a build system (Task, Bazel, Buck2) it runs no commands, and unlike a
file integrity monitor (AIDE) it models links between files. It answers one
question. Did a source change without its dependents being confirmed?

## How it works

1. A YAML manifest declares groups. Each couples `source` files to the
   `dependents` that must stay in sync with them.
2. `outdatty update` records a `blake3` hash of every file in a committed
   lockfile, `outdatty.lock`. That's you confirming the group is in sync.
3. `outdatty check` in CI recomputes the hashes. A source that changed since
   the last confirmation makes its group stale, and the check exits 1. Editing
   only a dependent is fine, unless the group sets `bidirectional: true`.
4. You review, fix the dependents and run `outdatty update` to re-confirm.

## Install

```bash
cargo install outdatty                        # from crates.io
nix run github:mlavrinenko/outdatty -- check  # with Nix, nothing installed
```

In a flake, add `outdatty.url = "github:mlavrinenko/outdatty";` to your inputs.
Pre-built binaries live on the
[latest release](https://github.com/mlavrinenko/outdatty/releases/latest).

## Usage

```bash
outdatty init                 # write a starter outdatty.yaml
outdatty update               # confirm, recording current hashes in outdatty.lock
outdatty check                # CI gate, exits 1 if a source changed unconfirmed
outdatty status               # show every group without failing
outdatty update --group docs  # confirm only one group
outdatty update --group docs --dependent README.md  # record one reviewed dependent
outdatty schema               # print the manifest JSON schema
```

Plain `check` prints only what failed, then one summary line. A passing run
prints `outdatty: 5 groups up to date`.

```
[ stale ]  cli-docs
    source changed:    src/report.rs
    review dependent:  README.md
    confirm with:      outdatty update --group cli-docs

outdatty: 1 of 5 groups out of date
```

`--dependent` (repeatable, needs `--group`) records only the named dependents'
hashes and leaves the sources and other dependents as locked, so a
dependent-only edit claims review of that file alone. Each path lands in every
named group that declares it, and a path none of them declares is an error.

The global flags are `--manifest <path>`, `--lock <path>`,
`--format plain|json|quiet|paths|paths0` and `--color auto|always|never`. Plain
output is colored on an interactive terminal, and `auto`, the default, drops
color when piped or when `NO_COLOR` is set.

`check` exits `0` in sync and `1` on drift. Any command exits `2` on an
operational error, like an unknown group, a duplicate group name, an unparseable
manifest or an incompatible lockfile. A missing lockfile isn't one, since
`check` treats every group as new and exits `1`. Run `outdatty update` to make it.

`--format=paths` (newline-delimited) and `--format=paths0` (NUL-delimited)
print just the sorted, deduped changed-source and unrecorded-dependent paths,
with no labels or summary. outdatty stores hashes, not diffs, so it hands you
the paths and you pipe them into whatever diff or editor you like.

```bash
outdatty status --format=paths0 | xargs -0 -r git diff --   # robust, handles spaces
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

See [examples/outdatty.yaml](examples/outdatty.yaml). A deleted literal, or a
glob that no longer matches a locked file, counts as a change to confirm. A file
a dependents glob newly matches, with no hash in the lockfile, fails `check` as
`unrecorded`, and outdatty names the command that records it.

```
[ unrec ]  cli-docs
    unrecorded: www/content/cli/add.typ
      record it: outdatty update --group cli-docs --dependent www/content/cli/add.typ
```

A group you split, rename or drop leaves its hashes in the lockfile. `check`
fails on each such orphan and names it, and a plain `outdatty update` prunes
them. A `--group` check skips orphans, as it skips coverage.

```
[orphaned]  1 lock group(s) the manifest no longer declares
    orphaned group:    old-docs
    prune with:        outdatty update
```

Globs skip whatever git ignores (root and nested `.gitignore` files, the global
excludes, `.git/info/exclude`), so build output never sneaks into a group. Set
`gitignore: false` at the top of the manifest to match every file. outdatty
never walks `.git` and never follows a symlink while expanding a glob.

### Keep the manifest in `.config/`

Every path in the manifest, and every key in the lockfile, resolves from
`root`, which defaults to the manifest's own directory. Point it at the repo
root and the pair can live in `.config/` with paths written as before.

```yaml
# .config/outdatty.yaml
root: ..                              # paths resolve from the repo root
groups:
  - name: feature
    source: [src/feature.rs]
    dependents: [docs/feature.mdx]
```

Without `--manifest`, outdatty tries `outdatty.yaml` and `outdatty.yml`, then
the same two under `.config/`. The lockfile stays beside the manifest, here
`.config/outdatty.lock`, and its keys don't change. So moving an existing pair
takes that one `root: ..` line and no `update`.

### Coverage: catch brand-new files

A glob source flags a file that *changed*. Coverage flags a file no group
mentions *at all*, the brand-new one someone forgot to wire in.
`require_tracked` lists the files that must appear in some group's `source` or
`dependents`, and any that don't fail `check` as `untracked`.

```yaml
# Default when omitted: ["**"] — every git-tracked file must belong to a group.
require_tracked:
  - "**"              # require everything, then carve out what has no coupling
  - "!vendor/**"      # last match wins; `!` excludes
  - "!LICENSE"
# require_tracked: ["src/**"]   # or require only a subtree (no negation needed)
# require_tracked: ["!**"]      # or opt out entirely
```

The last pattern to match a path decides, and a leading `!` excludes it. The
universe is every file under `root` that git doesn't ignore (honoring
`gitignore`), and the manifest and lockfile are always exempt. A failing check
names each untracked file, so you can add it to a group or exclude it.

## CI and pre-commit

Gate a repo by running `check` in CI, which exits non-zero on drift. `--format
quiet` prints nothing on a pass. On a failure it prints the failing groups to
stderr just as plain `check` would, so a red gate explains itself.

```yaml
# .github/workflows/outdatty.yml
- run: outdatty check --format quiet
```

For a local guard, add a pre-commit hook.

```bash
# .git/hooks/pre-commit
exec outdatty check --format quiet
```

## Contributing

Development setup and coding conventions live in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
