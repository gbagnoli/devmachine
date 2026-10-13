# Host application

Remote workstation `apply --host HOST --target USER@HOST` owns the complete
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

Workstation SSH uses the operator's usual known-hosts file and OpenSSH identity
selection unless overridden, while retaining strict host verification and
noninteractive authentication. VM keys remain explicit run-owned artifacts.
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
