---
name: outdatty
description: >-
  Detect and resolve in-repo artifact drift with outdatty — a tool that couples
  sources to their dependents in a declared graph (outdatty.yaml) and confirms
  each coupling by content hash (outdatty.lock). Use this skill whenever a repo
  contains an outdatty.yaml or outdatty.lock, whenever the `outdatty` command
  shows up (check / update / status / init / schema), whenever `outdatty check`
  reports a stale or new group or fails in CI, or whenever the user wants docs,
  schemas, tests, or generated files kept in sync with the code they describe —
  e.g. "gate my README against code changes", "why is outdatty failing", "what
  drifted", "review the drift", "confirm the docs". Covers manifest authoring,
  CI gating, reviewing drift before confirming, and the review-before-update
  loop. Do NOT use for language package managers ("update my npm/cargo/pip
  dependencies", "bump the lockfile") — outdatty tracks coupling between files
  inside one repo, not external package versions.
---

# outdatty

## What it is and why

outdatty catches the edit to a source (code, a schema, a config) that leaves
what describes or depends on it (a README, a generated file, a test) behind.
Declare each coupling once in `outdatty.yaml`; confirming records every file's
content hash in `outdatty.lock`. When a source's hash no longer matches the
locked one, its group is "stale" — go re-check its dependents.

Mental model: outdatty compares against the lockfile, not git. "Drift" means
"changed since the last `outdatty update`", independent of commits. It stores
hashes, never content, so it names what changed but cannot diff it — bring
your own diff tool (see the review-before-update loop).

## Commands

- `outdatty init [--force]` — write a starter `outdatty.yaml`.
- `outdatty check [--group ID]...` — fail if any selected group is stale or new.
  The CI gate. Exit codes: 0 = all confirmed, 1 = drift, 2 = error (bad
  manifest, missing file, etc.).
- `outdatty status [--group ID]...` — the check report, but always exit 0. Use
  it locally to look without gating.
- `outdatty update [--group ID]... [--dependent PATH]...` — re-hash the selected
  groups into the lockfile, confirming the current state. Unscoped, it also
  prunes entries whose group left the manifest. `--dependent` (needs `--group`)
  records only that dependent's hash, in whichever named groups declare it,
  leaving sources and other dependents as locked; a path none declares errors.
- `outdatty schema` — print the manifest's JSON schema.

Global flags (all subcommands): `--manifest <path>`, `--lock <path>`,
`--format <plain|json|quiet|paths|paths0>`, `--color <auto|always|never>`.
`--group` and `--dependent` are repeatable.

## The manifest

`outdatty.yaml` declares groups. A change to any `source` stales the group until
you re-confirm; `dependents` are the files to review when that happens.

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/mlavrinenko/outdatty/main/schema/outdatty.schema.json
groups:
  - name: cli-docs            # unique id; used in reports and --group
    source:                   # globs allowed, e.g. src/**/*.rs
      - src/cli.rs
      - src/report.rs
    dependents:
      - README.md
      - www/index.html
    # bidirectional: true     # also stale the group when a dependent changes
gitignore: true               # default: glob expansion skips git-ignored paths
```

Required: `name` and a non-empty `source`. `dependents` defaults to empty (a
source-only group has nothing to review); `bidirectional` and `gitignore` are
optional. Gitignore filtering applies only to glob matches — an explicitly
listed path is always included.

Directed (default): editing a dependent alone is fine — only source changes
stale the group. Bidirectional: a dependent change stales it too. Use
bidirectional when both sides must stay mutually consistent (a spec and its
implementation); directed when one side is generated from or documents the
other.

Statuses: `ok`, `stale` (a source changed), `new` (no locked snapshot yet —
fails check until the first `update`).

## Coverage: require_tracked

`require_tracked` (manifest-level, default `["**"]`) names files that must
appear in some group's `source` or `dependents`; any that do not fail `check`
as `untracked` — catching a brand-new file nobody wired in. Last match wins and
`!` excludes (`["**", "!vendor/**"]`); the manifest and lockfile are exempt, and
`["!**"]` opts out. Runs only on a whole-manifest check, not with `--group`.

## The review-before-update loop

Never run `outdatty update` blind: it re-hashes and silences the alarm, so
updating without looking confirms drift nobody reviewed. Instead:

1. See what drifted: `outdatty check` (CI) or `outdatty status` (locally). Per
   stale group, read the changed sources (what moved) and the listed
   dependents (what to eyeball).
2. Diff the source changes with your own tool; outdatty hands you the paths:

   ```sh
   # robust — NUL-delimited, safe for paths with spaces
   outdatty status --format=paths0 | xargs -0 -r git diff --
   # or, simply
   git diff -- $(outdatty status --format=paths)
   # or open them for review
   outdatty status --format=paths0 | xargs -0 -r "$EDITOR"
   ```

   Caveat: `git diff` is working-tree-vs-HEAD, which equals "changed since
   locked" only when you commit as often as you `outdatty update`. A
   good-enough proxy for review, not an exact since-locked diff.
3. Per stale group, update the dependents that need it (edit the README,
   regenerate the file, fix the test). A dependent needing no change is fine —
   the point is that you looked.
4. Confirm: `outdatty update --group <id>`. Now `check` passes for it.

A recorded dependent hash is a review watermark. After editing a dependent on
its own, record just it: `outdatty update --group <id> --dependent <path>`. A
whole-group update would also claim review of other pending dependent edits.

## Reading the output

Plain output (default) is complete for a human or an agent — failing groups
list both the changed sources and the dependents to review:

```
[ stale ]  cli-docs
    source changed:    src/report.rs
    review dependent:  README.md
    review dependent:  www/index.html
    confirm with:      outdatty update --group cli-docs

1 of 1 group(s) out of date; review and run `outdatty update`
```

## Choosing a format

- `plain` (default): the daily read, for humans and agents alike. Do not reach
  for JSON just to see what drifted — plain names sources and dependents.
- `json`: to iterate over groups programmatically. Stable, versioned envelope;
  each group carries `status`, `changed_sources`, `changed_dependents` (pending
  dependent edits, even when `ok`), and the full declared `dependents`, so you
  never re-parse the manifest for review targets:

  ```json
  {
    "version": 1, "failed": true, "total": 1, "out_of_date": 1,
    "groups": [
      { "id": "cli-docs", "status": "stale",
        "changed_sources": ["src/report.rs"],
        "changed_dependents": [],
        "dependents": ["README.md", "www/index.html"] }
    ]
  }
  ```

- `paths` / `paths0`: bare changed-source paths (all groups, sorted, deduped)
  for piping into a diff or editor; newline- or NUL-delimited — prefer
  `paths0 | xargs -0` so paths with spaces survive. Empty when nothing drifted.
- `quiet`: no output; rely on the exit code.

`check` keeps exit 1 on drift under any format, so
`outdatty check --format=paths0 | xargs -0 -r ...` both prints paths and gates.

## Setting it up in a repo

1. `outdatty init`, then edit `outdatty.yaml`: one group per coupling you care
   about (sources → the docs/tests/generated files that must track them).
2. `outdatty update` once to record the baseline. Commit both `outdatty.yaml`
   and `outdatty.lock`.
3. Gate CI with `outdatty check` (exit 1 fails the build on unreviewed drift).
   In a `just`/make repo, add it to the existing check target; the loop above
   plus the built-in output replaces a per-project `outdatty-review` script.

## Gotchas

- A failing `check` does not mean a dependent is wrong, only that a source
  moved and the group is unconfirmed. Review, fix if needed, then `update`.
- outdatty is VCS-agnostic and diff-free by design; for a real since-locked
  diff, feed the paths output to your own tooling.
- `new` groups fail `check` until the first `update` records their snapshot.
- Confirm only what you looked at: `--group` for the groups you reviewed,
  `--dependent` for a dependent-only edit.
- A locked file later deleted is drift (in `changed_sources`), not an error: a
  missing literal path is warned about and treated as removed.
- `--format` only shapes `check` / `status`. `init` honours only `quiet`,
  `schema` always prints the schema, and `update` emits no `paths`/`paths0`.
