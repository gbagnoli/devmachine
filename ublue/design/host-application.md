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
The workflow assumes installed guest binaries and temporary provisioning sudo;
it does not deploy binaries, migrate data, finalize access or isolate controllers.
