# outdatty

## Agent Rules

- See and use [Justfile](Justfile). Add any repeatable and regular operations there.
- At the end ensure that `just fix-check` is green.
- Be careful with the context. Omit non-necessary command outputs using `chronic` or `grep`.
- This project does not use task tracking. Omit the `Refs:` footer from all commits.

See [CONTRIBUTING.md](CONTRIBUTING.md) for project conventions and code standards.

## Worktrees

- Delegated work forks with `just fork <slug>` (a sibling worktree on a new
  branch, dev shell allowed) and lands with `just land` run in that worktree:
  it runs `just check`, fast-forwards main and removes the worktree. Both wrap
  [worktrunk](https://worktrunk.dev); [.config/wt.toml](.config/wt.toml) holds
  the hooks. Never run a bare `wt merge`: it squashes the commits together.
