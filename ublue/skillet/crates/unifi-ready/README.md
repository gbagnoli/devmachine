# UniFi readiness helper

A credential-free probe for the local controller's JSON API. Success requires
HTTP 200, JSON content type, `meta.rc=ok` and `meta.up=true`. Connection failure,
startup HTML and redirects are retried until the three-minute deadline. The
fixed default is `https://127.0.0.1:8443/status`; self-signed TLS is accepted only
for literal loopback URLs. Response bodies and attempts are bounded.

Build/test from the Skillet workspace:

```bash
cargo test -p skillet-unifi-ready
cargo build --release -p skillet-unifi-ready
podman build -f crates/unifi-ready/Containerfile -t localhost/skillet-unifi-ready .
podman run --rm --network=host --read-only --cap-drop=all localhost/skillet-unifi-ready
```

The multi-stage build compiles for musl; the runtime is scratch, non-root and
contains only the executable. Devmachine CI publishes `latest` and a commit tag
from the default branch, rebuilds daily, and builds PRs without publishing.
Normal unit tests use a disposable local HTTP fixture; no controller, credentials
or vault access is required. Optional `--url` (loopback only) and
`--timeout-secs` (1–180) support bounded local diagnostics and fixture testing.

Deployment and HA ordering belong to the [UniFi module](../unifi/README.md).
