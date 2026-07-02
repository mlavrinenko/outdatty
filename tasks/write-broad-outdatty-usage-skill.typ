#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "write broad outdatty usage skill",
  priority: 6,
  difficulty: 3,
  status: done(2026, 7, 2)[commit 04e2208; validated by sonnet, 3 fixes applied],
  depends-on: (link("report-declared-dependents-in-plain-and-json.typ"),),
)

= Goal

One portable skill for using outdatty in any repo, so no project carries a
copy-paste `just outdatty-review`. Broader than the review loop: manifest
authoring, CI gating, confirming, and review-before-update.

= Build via

`skill-creator`.

= Cover

- when to reach for outdatty (doc / artifact drift gating via a declared graph)
- `init` + manifest shape (source → dependents, `bidirectional`)
- `check` as a CI gate; exit codes 0 ok / 1 drift / 2 error
- reading drift: changed sources + the dependents to review (uses the enriched
  output from the dependency task)
- `update` to confirm, scoped with `--group`
- format choice: plain for eyeballing (now complete), json only when iterating
  programmatically — not "always json"; the maintainer's read is that json is
  the agent/script substrate, not the daily human read
- gotcha: outdatty compares against the locked hash, not git; a real diff needs
  the user's own VCS

= Depends

Report-dependents task (skill should document the final output shape). Revisit
if the changed-paths task lands — add the pipe-to-diff idiom then.
