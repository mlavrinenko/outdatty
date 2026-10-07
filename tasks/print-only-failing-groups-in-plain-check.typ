#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "print only failing groups in plain check",
  priority: 5,
  difficulty: 2,
  status: done(2026, 10, 7)[commit da80f26; README, site and skill synced],
)

= Problem

Plain `outdatty check` prints `[  ok   ]  <group>` for every passing group, so
in a repo with twenty groups the one that failed is buried. mindtape's
Justfile formats `check --format json` with jq to get a readable failure,
which every consumer would have to repeat.

= Change

- Plain `check` prints only the failing groups (stale, new, unrecorded) and
  the untracked files, each with what changed, the dependents to review and
  the command that confirms it, then one summary line:
  `outdatty: 1 of 20 groups out of date`.
- A passing `check` prints one line, `outdatty: 20 groups up to date`.
  `--format quiet` already covers silence.
- `status` keeps listing every group.
- `--format paths` stays: it feeds a diff tool, which plain output cannot.
- Sync README, `www/index.html` and the skill, which show check's output.
