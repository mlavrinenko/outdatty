#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "scope a watermark record to one dependent",
  priority: 4,
  difficulty: 3,
  status: proposed(2026, 9, 22),
)

= Problem

`outdatty update` takes `--group <ID>` and nothing finer, so confirming one
dependent you actually reviewed also re-records every other pending dependent
in that group. The lock diff then claims a review that never happened — the
exact misattribution the blanket `update` (no `--group`) is warned against for.

Surfaced in the mindtape repo, whose `outdatty.yaml` header argues the
watermark rule and whose ten groups make it routine: one agent editing
`www/content/priority.typ` absorbed pending hashes for `cli-docs`,
`prelude-docs` and `dev-docs`; another editing
`docs/src/contributing/windows.typ` absorbed `docs/src/contributing.typ`'s
pre-existing drift in `dev-docs`. Neither had a way to record less.

The group is the right unit for `check` — a source changing obliges every
dependent. It is the wrong unit for `update`, where the claim being made is
per-file.

= Change

- `src/cli.rs`: `update` gains a repeatable `--dependent <PATH>` (name
  bikesheddable), valid only alongside `--group`, recording that path's hash
  and leaving the group's other pending dependents pending.
- `src/lock.rs` (or wherever the refresh writes): refresh the named entries
  rather than the whole group's dependent map.
- Refuse a path the named group does not declare, naming it — a typo must not
  silently record nothing and report success.
- Regression test: a group with two drifted dependents, `update --group g
  --dependent a`, asserting `a` moved and `b` did not.
- `schema/outdatty.schema.json` is unaffected (this is CLI surface, not
  manifest), but the usage skill and `README.md` both describe `update`, so
  check the `cli-docs` group.

= Note

Downstream consumers work around this today by re-recording the whole group and
accepting the over-claim, which is what makes it worth fixing rather than
documenting: the workaround is invisible in review.
