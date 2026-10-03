# Pi-hole container network

Run Pi-hole and future Caddy as separate containers on one Quadlet-managed
dual-stack bridge. Keep Podman's network DNS enabled and refer to Pi-hole by
its container name (`pihole`) from Caddy, so neither container needs a fixed
address and they keep independent lifecycles. Do not use a shared pod.

Publish Pi-hole DNS on host IPv4 and IPv6 port 53 for both TCP and UDP, matching
rupik. Keep its web listener on port 8088 inside the bridge; Caddy will proxy
to `http://pihole:8088` without publishing the admin port on the host. Pi-hole
uses `dns_listeningMode=ALL` for bridge-network clients. Mark its fully
qualified registry image for Quadlet auto-update; the OS image already enables
the auto-update timer. The planned private UI route and Cloudflare DNS-only
records are described in the [private UI design](private-ui-access.md).

The planned second Pi-hole on beelzebot beside clamps is a separate DNS
instance on the same power and internet connection. A possible third on remote
bender adds site diversity. All instances offered to the same clients must
serve consistent local records and filtering policy; the synchronization
mechanism and LAN client resolver settings remain to be decided before those
hosts are deployed.

Pi-hole's host port 53 conflicts with Netavark's default Aardvark DNS listener.
When a host profile declares Pi-hole, host composition writes Podman's rootful
`dns_bind_port=54` policy once before applying its services. Container recipes
only declare their network attachments; they do not own this global setting.
This must be verified on the target uCore release. Keep the bridge
non-internal; alternate Aardvark ports have a known limitation on internal
networks.

Podman does not update an existing network's settings from a changed Quadlet.
Skillet records the accepted network definition and refuses drift. Changing
subnets or other network options therefore requires an explicit network
recreation with its consumers stopped.

References: [Podman network Quadlets](https://docs.podman.io/en/stable/markdown/podman-network.unit.5.html),
[Quadlet container networking](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html),
[Pi-hole Docker networking](https://docs.pi-hole.net/docker/).
