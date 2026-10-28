#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Publish to crates.io through Trusted Publishing",
  priority: 5,
  status: proposed(2026, 10, 28),
)

= Problem

`.github/workflows/release.yml` publishes `outdatty` with `CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}`, a repo-level secret set in June 2026. A stored token expires, and the publish then fails. On 2026-10-28 mindtape's release failed that way, with its token set on 2026-03-07:

```
error: failed to publish mindtape v0.3.0 to registry at https://crates.io ... (status 403 Forbidden): authentication failed
```

Every repo with a stored token breaks the same way when its token expires.

= Change

The publish job asks crates.io for a short-lived token through Trusted Publishing, as nix-why-build (commit 9743ef71 "ci(release): publish to crates.io through Trusted Publishing") and mindtape (commit b958a586) already do. The job gets:

```yaml
    environment: crates-io
    permissions:
      contents: read
      id-token: write
    steps:
      # ... checkout, toolchain ...
      - name: Authenticate with crates.io
        id: auth
        uses: rust-lang/crates-io-auth-action@c6f97d42243bad5fab37ca0427f495c86d5b1a18 # v1.0.5
      - name: Publish ...
        env:
          CARGO_REGISTRY_TOKEN: ${{ steps.auth.outputs.token }}
```

and `secrets.CARGO_REGISTRY_TOKEN` goes away. The auth step sits before the existing "Publish unless already on crates.io" step, which keeps its crates.io lookup and only takes its token from the auth step.

Manual steps for the owner, since an agent must not touch crates.io or GitHub state:

+ Before the workflow change lands, open https://crates.io/crates/outdatty/settings, then Trusted Publishing, Add, GitHub. Enter owner `mlavrinenko`, repo `outdatty`, workflow `release.yml`, environment `crates-io`. The crate to register is `outdatty`, the only one this repo publishes.
+ After the first green publish, run `gh secret delete CARGO_REGISTRY_TOKEN -R mlavrinenko/outdatty`, and optionally revoke the token on crates.io.

= Done when

- The publish job in `release.yml` authenticates through `rust-lang/crates-io-auth-action` and no longer reads `secrets.CARGO_REGISTRY_TOKEN`.
- A release after the Trusted Publishing entry exists publishes `outdatty` green.
