#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Ship a git merge driver for outdatty.lock",
  priority: 5,
  status: proposed(2026, 10, 28),
)

= Problem

Two branches that each run `outdatty update` on a different group change hash lines a line or two apart in `outdatty.lock`. Git's text merge conflicts on them, though neither branch touched the other's group, and `git checkout --ours` drops the other branch's recorded reviews.

- Seen in mindtape: three of five rebases on 2026-10-06 stopped on the lock. It answered with a 280-line bash merge driver (`.just/scripts/outdatty-lock-merge.sh`, needing `yq`, `jq` and `b3sum`) plus an e2e suite, all to work around this.
- The lock is a map of maps, so it merges key by key. Only outdatty knows its shape and when a hash is a review.

= Change

- `outdatty merge-lock <base> <ours> <theirs>`, a git merge driver (`%O %A %B`). It writes the merged lock over `<ours>` and exits 0.
- Every hash (group, `source` or `dependents`, path) merges three-way: a key only one side changed takes that side.
- A hash both sides changed to different values, or one changed and the other dropped, gets a hash no file has. `check` then fails the group, since no side's review covers the merged file, and stderr names the group, the path and the command that records it. The rebase doesn't stop.
- A group one side dropped stays dropped, since only a plain `update` drops a group and only when the manifest no longer declares it. A rename on one branch so leaves no orphan from the other.
- A side that doesn't parse as a lock exits 2, which git treats as a conflict.
- README, `www/index.html` and the skill show the `.gitattributes` line and the `git config` that register it.

= Open questions

- Should a contested hash take the side whose hash matches the merged file? It would spare a review when one side only carried a stale hash along. But git hands a driver no merged tree (merge-ort writes the worktree last), so it means parsing conflict labels to find the commits. On a base that passes `check`, a source both sides changed means both edited it, so neither matches. The match could only pick a dependent of a directed group, whose hash `check` never reads. Pick: no.
- Should outdatty register the driver itself (`outdatty init --git`)? Pick: no, two documented lines are enough.

= Done when

- A test: two branches that recorded different groups merge through the driver without a conflict, a hash both changed fails `check`, and a group one side dropped stays out.
- `just fix-check` is green.
