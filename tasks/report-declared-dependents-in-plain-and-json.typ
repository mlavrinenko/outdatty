#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "report declared dependents in plain and json",
  priority: 8,
  difficulty: 4,
  status: done(2026, 7, 2)[commit 3798aad; www synced, selfcheck green],
)

= Context

Downstream feedback: `outdatty check` says "a source drifted" but never names
the docs to re-check. Those docs are the group's declared dependents — outdatty
owns them (manifest) but never surfaces them for a failing group. Today plain
prints changed sources plus "confirm with: …"; json carries `changed_dependents`
but not the full declared list, so a consumer still has to parse the manifest
(the feedback's `yq` refinement). One missing fact, missing from both formats.

= Change

- `GroupReport` (`src/engine.rs`): add the group's declared dependents (the
  review targets), wired in `evaluate_group`.
- plain (`src/report.rs`): for failing groups (stale/new) print a
  "review dependents:" block. Keep ok groups terse, matching today's rule that
  only failing groups get the "confirm with" line. Does not conflict with
  `plain_omits_dependent_only_changes` — that suppresses dependent-change noise
  on ok directed groups; this lists review targets on failing groups.
- json: include declared dependents always, so consumers stop parsing the
  manifest.

= Files

- `src/engine.rs` — `GroupReport` field + wiring
- `src/report.rs` — plain + json render + tests
- `README.md`, `www/index.html` — this repo's `cli-docs` group gates
  `src/report.rs` → docs, so this change trips it by design; re-sync the shown
  output and `outdatty update --group cli-docs`

= Done when

- failing groups list their dependents in plain
- json `GroupReport` carries the declared dependents
- tests cover both formats
- `just check` green (selfcheck passes after re-confirming `cli-docs`)
