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

The mandatory positional profile defaults to the matching SSH alias; optional
`--target HOST`/`USER@HOST` overrides connection identity only. OpenSSH resolves
its own config. Omitted user, port, key and known-host settings are not replaced
by workstation defaults that would override per-alias configuration. Strict host
verification and noninteractive authentication remain; encrypted keys use the
operator's SSH agent. Failed SSH and unreadable host identity are distinct from
a positively observed profile mismatch. Explicit `--local` or the retained
`--host-file` form selects guest apply; an omitted target never means local apply. VM keys remain explicit run-owned artifacts.
Remote apply deploys current generic and caller binaries before vault access or
service activation; `--no-deploy` explicitly reuses installed binaries. Standalone
`deploy` has no vault or service side effects. Both use Cargo-reported artifacts
and the VM delivery owner without manufacturing a VM identity. VM callers retain
run locks and ownership guards. Delivery verifies identity, bytes, root metadata
and version, and uses unique staging plus atomic replacement per executable.
A failed partial pair stops apply and can be retried; pair-wide atomicity and
concurrent independent deployment are not guaranteed. Temporary provisioning
sudo is required. Deployment does not migrate data, finalize access or isolate
controllers.


Caller-selected native networking prepares boot profiles before monitoring,
without switching the live management connection. Its credentials are excluded
from full guest application and disposable provisioning. See
[networking](host-networking.md).
