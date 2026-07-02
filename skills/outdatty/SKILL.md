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

outdatty catches the failure where you edit a source (code, a schema, a config)
and forget to update the things that describe or depend on it (a README, a
generated file, a test). You declare those couplings once in `outdatty.yaml`;
outdatty records a content hash of every file in `outdatty.lock` when you
confirm a coupling is consistent. Later, if a source's hash no longer matches
the locked one, the group is "stale" — a signal to go re-check its dependents.

Key mental model: outdatty compares against the last confirmed hash in the
lockfile, not against git. "Drift" means "a source changed since the last
`outdatty update`", independent of commits. And it stores only hashes, never
content — so it can tell you what changed, but it cannot show you a diff. You
bring your own diff tool (see the review-before-update loop below).

## Commands

- `outdatty init [--force]` — write a starter `outdatty.yaml`.
- `outdatty check [--group ID]...` — fail if any selected group is stale or new.
  This is the CI gate. Exit codes: 0 = all confirmed, 1 = drift, 2 = error
  (bad manifest, missing file, etc.).
- `outdatty status [--group ID]...` — same report as check but never fails
  (exit 0). Use it locally to look without gating.
- `outdatty update [--group ID]...` — re-hash the selected groups and rewrite
  the lockfile, confirming the current state. Unscoped, it also prunes lockfile
  entries whose group no longer exists in the manifest.
- `outdatty schema` — print the manifest's JSON schema.

Global flags (all subcommands): `--manifest <path>`, `--lock <path>`,
`--format <plain|json|quiet|paths|paths0>`, `--color <auto|always|never>`.
`--group` is repeatable to scope to specific groups.

## The manifest

`outdatty.yaml` declares groups. A change to any `source` marks the group stale
until you re-confirm; `dependents` are the files to review when that happens.

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

Required keys: `name` and a non-empty `source`. `dependents` defaults to empty
(a source-only group has nothing to review); `bidirectional` and `gitignore`
are optional. Gitignore filtering applies only to glob matches — an explicitly
listed path is always included, even if git-ignored.

Directed (default): editing a dependent alone is fine — only source changes
stale the group. Bidirectional: a dependent change stales it too. Use
bidirectional when the two sides must stay mutually consistent (e.g. a spec and
its implementation); use directed when one side is generated from or documents
the other.

Statuses: `ok`, `stale` (a source changed), `new` (no locked snapshot yet —
fails check until the first `update`).

## The review-before-update loop

The core discipline: never run `outdatty update` blind. Updating just re-hashes
and silences the alarm — if you haven't looked, you've confirmed drift you never
reviewed. Instead:

1. See what drifted: `outdatty check` (in CI) or `outdatty status` (locally).
   Read two things per stale group — the changed sources (what moved) and the
   listed dependents (what to eyeball).
2. Look at the source changes with your own diff tool. outdatty hands you the
   paths; you pick the tool:

   ```sh
   # robust — NUL-delimited, safe for paths with spaces
   outdatty status --format=paths0 | xargs -0 -r git diff --
   # or, simply
   git diff -- $(outdatty status --format=paths)
   # or open them for review
   outdatty status --format=paths0 | xargs -0 -r "$EDITOR"
   ```

   Note the caveat: `git diff` shows working-tree-vs-HEAD, which equals
   "changed since locked" only when your commit cadence matches your `outdatty
   update` cadence. It is a good-enough proxy for review, not an exact
   since-locked diff.
3. For each stale group, update the dependents that actually need it (edit the
   README, regenerate the file, fix the test). If a dependent needs no change,
   that's fine — the point is that you looked.
4. Confirm: `outdatty update --group <id>`. Now `outdatty check` passes for it.

## Reading the output

Plain output (default) is complete for a human or an agent eyeballing — failing
groups list both the changed sources and the dependents to review:

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
  for JSON just to see what drifted — plain already names the sources and the
  dependents.
- `json`: when you need to iterate programmatically over groups. Stable,
  versioned envelope; each group carries `status`, `changed_sources`,
  `changed_dependents`, and the full declared `dependents` (so you never have to
  re-parse the manifest to find review targets):

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

- `paths` / `paths0`: bare changed-source paths (all groups, sorted, deduped),
  for piping into a diff or editor. `paths` is newline-delimited; `paths0` is
  NUL-delimited — prefer `paths0 | xargs -0` so paths with spaces survive.
  Empty when nothing drifted, so a clean repo pipes to nothing.
- `quiet`: no output; rely on the exit code.

`--format` applies to `check` too, and `check` still sets exit 1 on drift — so
`outdatty check --format=paths0 | xargs -0 -r ...` both prints paths and gates.

## Setting it up in a repo

1. `outdatty init`, then edit `outdatty.yaml`: one group per coupling you care
   about (source files → the docs/tests/generated files that must track them).
2. `outdatty update` once to record the baseline lockfile. Commit both
   `outdatty.yaml` and `outdatty.lock`.
3. Gate CI with `outdatty check` (exit 1 fails the build on unreviewed drift).
   In a `just`/make-based repo, add `outdatty check` to the existing check
   target rather than hand-rolling a review recipe — the review workflow above
   plus the built-in output is the portable substitute for a per-project
   `outdatty-review` script.

## Gotchas

- A failing `check` does not mean a dependent is wrong. It means a source moved
  and the group hasn't been re-confirmed. Review, fix if needed, then `update`.
- outdatty is VCS-agnostic and diff-free by design. If you want a real
  since-locked diff, that is not something outdatty produces — use the paths
  output with your own tooling.
- `new` groups fail `check` until the first `update` records their snapshot.
- Scope with `--group` when confirming after a review, so you only re-hash what
  you actually looked at.
- A file that was locked and then deleted surfaces as drift (in
  `changed_sources`), not an error — a missing literal path is logged as a
  warning and treated as removed, so `check` fails rather than crashing.
- `--format` is accepted everywhere but only shapes the read commands
  (`check` / `status`). `init` honours only `quiet` (to silence its message),
  `schema` always prints the schema, and `update` emits no `paths`/`paths0`
  output.
