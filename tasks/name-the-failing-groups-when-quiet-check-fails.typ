#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Name the failing groups when quiet check fails",
  priority: 5,
  status: proposed(2026, 10, 17),
)

= Problem

`outdatty check --format quiet` is what a memoized gate runs: silent when green. When a group is stale it exits non-zero and prints nothing, so the gate reads as an unexplained red arm.

- Seen in jmav and typst-world, whose `just check` arms run `outdatty check --format quiet`: each failure needed a second, plain `outdatty check` to learn which group drifted.
- Plain `check` already prints only the failing groups plus a summary line (`print-only-failing-groups-in-plain-check`), so the text exists; quiet just drops it.

= Change

- `--format quiet` stays silent on a pass. On a failure it prints what plain `check` prints for the failing groups and untracked files (stale group, what changed, dependents to review, the confirming command), to stderr.
- `status`, `init`, `update` keep their quiet behaviour.
- Sync README, `www/index.html` and the skill where they describe quiet.

= Done when

- A test: a stale group under `--format quiet` exits non-zero and its stderr names the group and its dependents; a clean tree prints nothing.
- `just fix-check` is green.
