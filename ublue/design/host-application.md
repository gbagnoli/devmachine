# Host application

Remote workstation `apply HOST` owns the complete
production application workflow. The canonical caller declaration selects its
services; credentials never implicitly enable optional workloads. Local guest
`apply` retains its phase-based interface, and disposable provisioning retains
its owned VM lifecycle and test policy.

Credential delivery and consumer activation are separate effects. Standalone
`secrets deliver` stages by default; `--activate` is explicit because a consumer
may configure multiple applications. Remote apply stages the complete Pi-hole /
Tailscale input set before full apply, checks oneshot execution, waits for tailnet
addresses before Caddy DNS reconciliation, then applies independent phases with
monitoring last. Failure stops later steps; retained credentials and convergence
allow a retry. Existing Tailscale enrollment is reused; explicit delivery refreshes
it. Caddy staging still performs scoped-token and DNS provider operations.

For remote apply and standalone credential delivery, the mandatory
positional profile defaults to the matching SSH alias; optional
`--target HOST`/`USER@HOST` overrides connection identity only. OpenSSH resolves
its own config. Omitted user, port, key and known-host settings are not replaced
by workstation defaults that would override per-alias configuration. Strict host
verification and noninteractive authentication remain; encrypted keys use the
operator's SSH agent. Failed SSH and unreadable host identity are distinct from
a positively observed profile mismatch. Explicit `--local` or the retained
`--host-file` form selects guest apply; an omitted target never means local apply. VM keys remain explicit run-owned artifacts.
The active signed image owns persistent guest binaries at `/usr/bin`. Remote
`apply` verifies the selected host and image versions, stages the workstation's
generic CLI at a unique temporary path, then replaces each one-shot unit's
`ExecStart` through `/run/systemd/system` for that activation. This works with
older persistent `/etc` unit definitions too. Every persistent apply unit invokes
the generic image-owned `/usr/bin/skillet` with `/etc/skillet/host`; each unit's
`ExecStopPost` removes its override, and the workstation removes the temporary
CLI after the full sequence. `/usr/bin` is never replaced. `--no-deploy` skips
the temporary runner and uses the image-owned binary for apply too. New
long-running helpers still require a Skillet release, ucore-images pin and
activated image. VM provisioning retains its separate run-owned binaries and
lifecycle guards.


Caller-selected native networking prepares boot profiles before monitoring,
without switching the live management connection. Its credentials are excluded
from full guest application and disposable provisioning. See
[networking](host-networking.md).
