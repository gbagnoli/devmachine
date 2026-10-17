# Tailscale host routing

Use a rootful host-network container with persistent state, giving the host a
normal interface and preserving host-level forwarding. Caller-selected routing
is separate from environment/instance identity. Versioned encrypted enrollment
carries the resolved policy; only the decoded one-use key becomes a Podman secret.
Workstation OAuth never reaches the host.

Production resolves advertise_exit_node from the caller; disposable policy
always disables it. TS_AUTH_ONCE preserves one-use enrollment. Startup auth-only
modes can skip later preferences, so supported tailscale set commands converge
routing after healthy readiness on every startup, including explicit withdrawal.

Retained raw-key payloads default to no advertisement; malformed versions fail
without exposing values. The guest rejects policy the caller did not select.
Approval/access grants and client exit-node selection remain operator policy,
not implicit effects of credential delivery. Exit-node selection does not imply
LAN subnet advertisement, route acceptance or disabled subnet SNAT.

[Setup and commands](../skillet/crates/host-profiles/docs/tailscale.md).
Joey's private routing work: [migration plan](../plan/JOEY-MIGRATION.md).
References: [exit nodes](https://tailscale.com/docs/features/exit-nodes),
[container startup](https://github.com/tailscale/tailscale/blob/main/cmd/containerboot/tailscaled.go).
