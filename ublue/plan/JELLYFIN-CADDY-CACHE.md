# Jellyfin caching through Caddy

Status: implementation planned, 2026-10-11. Caddy caching is selected for donbot;
no nginx fallback is selected. Prerequisites are the shared ingress work in
[donbot migration](DONBOT-MIGRATION.md) and the existing
[refactoring gates](SKILLET-REFACTOR.md). The source image and 2 MB video-slice
caches are recorded in [the inventory](DONBOT-INVENTORY.md).

## Target and boundaries

Use pinned `github.com/caddyserver/cache-handler` (Souin-based). Build a controlled
Caddy service image in CI with Cloudflare DNS support and the selected storage
module; do not compile plugins on guests or install tools on the workstation.
Verify compatible pinned versions and required modules in the built image.
Application-image tooling remains separate from uCore host packages. Include the
cache/storage modules through the same build. Tailnet UI authorization now uses
the shared Rust forward-auth endpoint and does not need a Caddy auth plugin.

Start with a filesystem provider, evaluating
[Simplefs](https://docs.souin.io/docs/storages/simplefs/) for the compatible pinned
cache release. Provider sizing/eviction semantics must be demonstrated; an entry
count, per-directory option or Go heap limit is not a proven total disk cap.
Use `/var/lib/data/caddy/cache/jellyfin` as disposable cache state, separately
mounted from certificates. Keep it out of Syncthing, btrbk and backups. Bound
both resident memory and total disk bytes; establish limits from host capacity
and source usage. No external Redis/database cluster is required by this plan.

Expose caching as an optional typed service policy; private UIs, Gokapi, Plex and
unrelated APIs are not cached simply because Jellyfin selects it. Render ordered
routing explicitly and validate with the actual built Caddy binary. Configurable
policy includes allowed routes/methods, response sizes, TTL, storage caps and
keys. Real hostnames and any private values stay outside the repository.

## Implementation order and exits

### 1. Build and prove the cache component

Add the reproducible service-image build and tests with an instrumented synthetic
upstream. Check module loading, adapted configuration and storage ownership.
Use only safe read methods and opt-in response statuses. Cache management/purge
APIs stay on a restricted local interface, never the public media site. Disable
credential-bearing debug logs and exposed cache keys.

**Exit:** a second permitted request demonstrates a real cache hit, expiry and
purge work, the chosen provider evicts/bounds disk and memory, and cache loss or
corruption can be cleared without harming Caddy certificates or service state.
CI performs these checks without KeePassXC, Cloudflare or live library data.

### 2. Enable approved image/static routes

Inventory the selected Jellyfin version's exact routes and auth/cache behavior.
Start with approved images and static assets; exclude login/session, user/profile,
admin APIs and responses with `Set-Cookie` or no-store/private policy unless a
specific reviewed, isolated policy exists. Never globally bypass upstream cache
headers. A Cache-Control header alone does not establish a server cache.

Retain scheme, host, full path/query and representation inputs in keys. For
protected requests separate authentication inputs and hash keys; query tokens,
Authorization and Jellyfin auth headers must not appear in logs or Cache-Status.
A token-specific key alone does not enforce revocation or changed library rights:
protected hits require upstream authorization/revalidation before returning bytes,
with no stale-on-error bypass. Prove that the consumer/module can enforce this;
keep unsupported protected routes uncached and report the gap.

**Exit:** image cache hits and invalidation are proven against real Jellyfin;
changed image size/format/query parameters do not collide. Tests cover two users,
missing/changed/revoked tokens, changed library access, errors and cookies.
Protected data is never served merely because it was previously cached. Benchmark
image loads and observe upstream work/latency without recording private requests.

### 3. Prove video and range caching before enabling it

Evaluate the pinned module against real media routes and synthetic byte fixtures:
200/206/416, matching and different ranges, repeated seeks, If-Range/ETag changes,
HEAD versus GET, transcoding/subtitle/query variants and concurrent readers.
Preserve content length/range/encoding and request limits. Exclude live streams,
WebSockets and mutation routes. Bound the maximum cached response size and avoid
buffering entire large videos into memory.

The former nginx 2 MB slice scheme is **not assumed to exist** in cache-handler.
Determine whether bounded segment/range caching provides equivalent useful
behavior. If it does not, record the exact missing feature and leave that route
uncached pending a Caddy-compatible solution; do not silently declare video cache
parity or substitute nginx. Image caching can be accepted as an intermediate
slice while video acceptance remains pending. Authorize protected cache hits as
in slice 2, and do not serve stale protected video after upstream/auth failure.

**Exit:** byte-for-byte range/seek and concurrent playback tests pass, cache
contents/keys cannot cross representations or authorization boundaries, and real
clients can play/resume/transcode without excessive memory/disk growth. Record
any difference from the original slice cache before production cutover.

### 4. Lifecycle and monitoring acceptance

Module-owned metrics cover cache hits/misses, upstream requests, failures and
storage pressure without tokens or private URLs. Repeat apply preserves declared
policy and certificates; reboot starts correctly with retained or absent cache.
Changing/removing a cache policy must not delete application data or enable
caching elsewhere. Pin/update the service image through the shared container
workflow and record named-VM evidence in
[ACCEPTANCE.md](archive/ACCEPTANCE.md).

Sources: [cache-handler](https://github.com/caddyserver/cache-handler),
[storage/build integration](https://docs.souin.io/docs/middlewares/caddy/), and
[Jellyfin proxy requirements](https://jellyfin.org/docs/general/post-install/networking/reverse-proxy/).
This plan specifies acceptance targets; no plugin build, cache benchmark or video
range acceptance has run yet.
