# Design decisions

These documents describe decisions, rationale and constraints. Commands live in
[crate guides](../skillet/crates/host-profiles/README.md); unfinished tasks live in
[plans](../plan/README.md).

- Architecture: [Skillet](skillet-architecture.md), [fleet](fleet-hosts.md),
  [Podman](podman-runtime.md), [VMs](smoke-vms.md)
- Provisioning: [secrets](secrets.md), [templates](configuration-templates.md),
  [host application](host-application.md), [access](administrative-access.md)
- Storage: [data/root](storage.md), [snapshots](btrbk.md), [Syncthing](syncthing.md)
- Access: [private UIs](private-ui-access.md), [identity](tailnet-ui-authentication.md),
  [file browsing](file-browsing.md), [public services](public-service-access.md)
- Network: [host profiles](host-networking.md), [Tailscale](tailscale.md),
  [Pi-hole](pihole-network.md), [Cloudflare UI lifecycle](cloudflare-ui-lifecycle.md),
  [DDNS](cloudflare-ddns.md)
- Services: [monitoring](monitoring.md), [mail](email-delivery.md),
  [UniFi](unifi.md), [UPS](ups.md), [Home Assistant](home-assistant.md)
