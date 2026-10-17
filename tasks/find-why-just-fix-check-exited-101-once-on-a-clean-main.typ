#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Find why just fix-check exited 101 once on a clean main",
  priority: 4,
  status: done(2026, 10, 17),
)

= Problem

On 2026-10-11, right after `wt merge` fast-forwarded main to 1b7d6b5 (the integration tests linked into one `it` binary), `just fix-check` on the primary checkout exited 101 with its output discarded. The tree was clean. The next `just fix-check` exited 0, and four `cargo test --workspace -q` runs after it were all green, so the failure is intermittent and its message is lost.

101 is what cargo returns when a test fails or a build panics. `check` runs its arms concurrently through `parallel -j 0` under `chronic`, so a test that races clippy or another arm over `target/`, a temp dir or an env var could fail only under that load. wt was also removing the merged worktree in the background at that moment.

= Investigate

- Loop `just fix-check` (and `just check` alone) until it fails, keeping the output: `for i in $(seq 50); do just check > /tmp/oc-$i.log 2>&1 || { echo "fail $i"; break; }; done`.
- If it never fails, loop it beside a background `cargo build` in a sibling worktree to reproduce the load.
- Read the failing test for shared state: fixed paths, `set_var`, the current directory, or a port.

= Done when

The cause is named and fixed with a test that fails without the fix, or 50 looped runs stay green and the task records that, with the logs.

= Findings

- 50 looped `just check` runs in a sibling worktree (`just fork`), on 2026-10-17 at 69d648b, all exited 0, 3–4 s each warm. Meanwhile the primary checkout was editing, testing and running `fix-check`, so `cargo` load ran in parallel.
- A replay of the merge also exited 0: a worktree built at 3c64327 (clippy, test, `just check`), then moved to 1b7d6b5 and ran `just fix-check`.
- The 101 did not come from `just check`: GNU `parallel` exits with the number of failed jobs, 1 to 6 here, and `just` passes that through. It came from a bare cargo arm of `fix-check`, `cargo fmt` or, more likely, `cargo clippy --fix`. Neither runs under `chronic`, so their output reached the terminal, not a discarded log.
- Not reproduced; closed per the done-when clause. If it recurs, keep the full `just fix-check` output; the failing cargo arm names itself there.
