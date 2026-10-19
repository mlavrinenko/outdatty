#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Resolve manifest paths from an optional root key",
  priority: 6,
  status: proposed(2026, 10, 19),
)

= Problem

Every path in `outdatty.yaml` and every key in `outdatty.lock` resolves from the manifest's own directory (`base_dir` in `src/commands.rs`). A project can't keep the pair in `.config/` without prefixing every entry with `../`.

- Seen in mindtape: moving its pair into `.config/` would rewrite about 470 entries.
- `Manifest::discover` looks only for `outdatty.yaml` and `outdatty.yml` in the current directory, so a moved pair also needs `--manifest` on every call.

= Change

- An optional top-level `root:`, a path relative to the manifest's directory. Sources, dependents, globs, coverage and every lock key resolve from it.
- Without `root`, nothing changes: paths resolve from the manifest's directory, and `update` rewrites an existing lock byte for byte.
- The lockfile still defaults to `outdatty.lock` beside the manifest, wherever `root` points.
- The manifest and lockfile stay exempt from coverage when they sit under `root`.
- Discovery tries `.config/outdatty.yaml` and `.config/outdatty.yml` after the two root-level names.
- Sync the schema, README, `www/index.html` and the skill.

= Open questions

- Resolve `root` lexically or through the filesystem? Lexically: `.config/..` is the repo root with no symlink surprises, and coverage can strip the manifest's path from it. A missing `root` directory is an error, not an empty project.
- Walk up from a subdirectory to find the manifest? Pick: no. Discovery never walked up for `outdatty.yaml`, so `.config/` gets the same rule.

= Done when

- Tests: a `.config/` manifest with `root: ..` checks the same as the root-level pair and writes the same lock keys; a manifest without `root` keeps its lock byte-identical; discovery finds `.config/outdatty.yaml`; a missing `root` errors.
- `just fix-check` is green.
