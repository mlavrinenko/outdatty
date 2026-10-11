#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Fork and land worktrees with in-repo worktrunk hooks",
  priority: 4,
  status: wip(2026, 10, 11),
)

= Problem

`~/.nix` no longer ships worktrunk user hooks, so nothing in this checkout forks a worktree or gates a landing.

- `wt switch --create scratch`, then `just check` there: direnv refuses the new `.envrc` ("is blocked"), so `just`, `mt` and `linecop` do not resolve until someone runs `direnv allow` by hand.
- `wt merge` fast-forwards main with no gate, so `just check` never runs before a landing.
- `worktrunk` is not in the dev shell of `flake.nix`.

= Change

- `flake.nix`: add `worktrunk` to the dev shell packages. If the locked nixpkgs ships a worktrunk that cannot parse `pre-start`, bump nixpkgs and confirm `just check` on the new pin.
- `.config/wt.toml` (new): `pre-start` runs `direnv allow`; `pre-merge` runs `direnv exec . just check`. No `mt check --commits` step: `.mindtape/config.toml` defines no commit rules. No `pre-remove` hook: there is no `.mmz/`, and `just cover` (tarpaulin) rebuilds its report instead of reading a persisted lcov.
- `Justfile`: recipes `fork SLUG *ARGS` (`wt switch --create --yes`) and `land *ARGS` (`wt merge --no-squash --no-commit --yes`).
- `AGENTS.md`, `CONTRIBUTING.md`: delegate with `just fork <slug>`, land with `just land`. `outdatty.yaml` lists `.config/wt.toml` under `dev-standards`, which `require_tracked` demands.

= Done when

- `just fork <slug>` makes a sibling worktree whose `mt --version` resolves at once.
- `just land`, run in the worktree, runs `just check` before the fast-forward, and a red gate stops the merge.
- `--no-squash --no-commit` keep every commit and refuse a dirty tree.
- `just fix-check` is green.
