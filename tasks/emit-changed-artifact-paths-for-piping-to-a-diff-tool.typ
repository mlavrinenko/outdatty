#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "emit changed artifact paths for piping to a diff tool",
  priority: 5,
  difficulty: 4,
  status: proposed(2026, 7, 2),
  depends-on: (link("report-declared-dependents-in-plain-and-json.typ"),),
)

= Problem

Users want to review changed sources with their own tools (git diff, `$EDITOR`,
delta) before `outdatty update`. outdatty knows the changed paths but only emits
them as prose (plain) or nested json — piping needs jq/yq. The suggested
`just outdatty-review` recipe is the ugly workaround. We store hashes, not
content, and will NOT anchor the lock to a git ref, so outdatty cannot diff. It
can hand the user a clean path list to feed any tool.

= Idea — git-plumbing-style path output

Precedent: `git diff --name-only`, `git ls-files -z`, `rg -l`. Emit bare changed
paths, one per line, so `$(...)` / `xargs` compose:

```
git diff -- $(outdatty status --format=paths)
outdatty check --format=paths | xargs -r "$EDITOR"
```

VCS-agnostic: outdatty exposes the changed-set it already computes; the user
picks the diff / editor tool.

= Open decisions (settle before building)

- surface: new `--format=paths` (fits the format enum) vs a
  `--name-only` / `--changed` flag on `status`
- which paths: changed sources only (the drift trigger — the lean choice) vs
  sources + dependents vs selectable
- delimiter: newline default plus a NUL variant (`-z` / `--format=paths0`) for
  paths with spaces, matching git

Lean: `--format=paths` emitting changed sources one per line, plus a NUL option.
Kills the jq recipe, keeps outdatty VCS-agnostic.

= Non-goal

No git ref in the lock. No built-in `--diff` that shells to git — it misanchors
(git shows working-tree-vs-HEAD, not vs the locked hash).
