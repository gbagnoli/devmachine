# Tailscale host routing

Run Tailscale rootfully with host networking and persistent state. The canonical
host caller selects `advertise_exit_node`; clamps enables it. Production delivery
resolves that declaration into a versioned encrypted enrollment payload. Disposable
delivery always selects false. Never infer environment from a runtime hostname.
Only the decoded enrollment key becomes the Podman secret; policy stays in clear
managed Quadlet configuration. No new vault entries or OAuth scopes are required.

Keep `TS_AUTH_ONCE=true` to preserve one-use enrollment. Extra startup arguments
alone cannot converge preferences on already authenticated nodes. A native
`tailscale set --advertise-exit-node=<bool>` runs after the container's status
healthcheck satisfies Quadlet `Notify=healthy`, including every service startup.
Explicit false withdraws a retained advertisement. No shell program is embedded.
The existing host baseline already enables IPv4 and IPv6 forwarding.

Retained raw-key credentials remain readable and default to no advertisement.
Redeliver production Tailscale credentials to enable caller-selected routing;
new disposable credentials explicitly disable it. Invalid versioned payloads
fail without exposing values. Host composition rejects advertisements its caller
did not select.

Tailnet exit-node approval remains operator policy; this change does not edit
access controls or make clients select an exit node. Local policy, compatibility,
rendering and Quadlet compilation are checked. Live routing exercises are omitted
by operator decision; they are not an additional acceptance gate for this slice.

References: [exit-node policy](https://tailscale.com/docs/features/exit-nodes),
[container startup](https://github.com/tailscale/tailscale/blob/main/cmd/containerboot/tailscaled.go),
[Quadlet readiness](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html#notify-defaults-to-false).
