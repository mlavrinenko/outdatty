#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Report lock groups the manifest no longer declares",
  priority: 5,
  status: done(2026, 10, 17),
)

= Problem

`outdatty check` passes while `outdatty.lock` holds groups that `outdatty.yaml` no longer declares. Orphan hashes pile up unseen after a group is split or renamed.

- Seen in mindtape: its lock held `sweep-dedup` and `worktree-release-docs`, both absent from its manifest, and `check` still said every group was up to date. A split of its `cli-docs` group left the same kind of leftovers.
- Verified against this repo's binary: a manifest with group `g`, a lock with `g` plus a hand-added `orphan`, and `outdatty check` prints `1 group up to date`, exit 0.
- Cause: `engine::evaluate` in `src/engine.rs:218` loops over `manifest.groups` only and looks each one up in the lock (`lock.groups.get(&id)`, line 223). No code walks `lock.groups` for keys the manifest lacks.
- `outdatty update` with no `--group` already prunes them: `engine::build` (`src/engine.rs:313-322`) drops lock entries outside the manifest under `Filter::All` and reports them as `UpdateAction::Removed`. Only `check` and `update --group X` never see them.

= Change

- `check` and `status` report each lock group the manifest does not declare, by name, and `check` fails on it. The message names the fix, `outdatty update`.
- Under `--group X` the report stays scoped to X, as coverage is (`src/commands.rs:117`).
- Plain, json and `--format quiet` output all carry the orphans.
- Sync README, `www/index.html`, the schema docs and the skill where they describe what `check` reports.

= Open questions

- Should `outdatty update --group X` also prune orphans? It gains one command that heals the lock, but it makes a scoped update touch groups outside its scope, which the doc comment on `engine::build` rules out today. Pick: no. A plain `outdatty update` already prunes, and the new `check` error points at it.
- Should there be a dedicated `outdatty prune`? It gains a way to drop orphans without re-confirming every other group, at the cost of a fifth command on top of `update`. Pick: no, unless the new `check` error proves too heavy a nudge toward a full `update`.
- Should an orphan fail `check`, or only warn? Failing matches how a stale group behaves and keeps the gate honest. Pick: fail.

= Done when

- A test: a lock holding a group the manifest lacks makes `check` exit non-zero and name that group; `update` clears it; a `--group` scoped check ignores it.
- `just fix-check` is green.
