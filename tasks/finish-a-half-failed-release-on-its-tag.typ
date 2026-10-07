#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "finish a half-failed release on its tag",
  priority: 7,
  difficulty: 2,
  status: proposed(2026, 10, 7),
)

= Problem

v0.6.0 was published to crates.io by hand before its tag was pushed, so the
tag's workflow failed at `cargo publish` on "already exists" and the GitHub
release, which waits on publish, never happened: v0.6.0 has no binaries.
Nothing let the workflow finish on the same tag, and `just release` tagged
whatever it was given after `just check`.

= Change

- Adopt cratemplate's hardened `release.yml`: `cargo publish` is skipped for a
  version already on crates.io, the workflow re-runs on an existing tag through
  `workflow_dispatch`, and a re-released old tag does not take "Latest".
- Adopt its `just release X.Y.Z [--dry-run]` preflight, keeping `just check` as
  the gate.
- Re-run v0.6.0 with `gh workflow run release.yml -f tag=v0.6.0` so it gets
  its GitHub release and binaries.
