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
the auto-update timer.

Pi-hole's host port 53 conflicts with Netavark's default Aardvark DNS listener.
Skillet moves Aardvark to port 54 through Podman's rootful `dns_bind_port`,
preserving service-name DNS while Pi-hole owns host port 53. This is global
Podman configuration and must be verified on the target uCore release. Keep the
bridge non-internal; alternate Aardvark ports have a known limitation on
internal networks.

Podman does not update an existing network's settings from a changed Quadlet.
Skillet records the accepted network definition and refuses drift. Changing
subnets or other network options therefore requires an explicit network
recreation with its consumers stopped.

References: [Podman network Quadlets](https://docs.podman.io/en/stable/markdown/podman-network.unit.5.html),
[Quadlet container networking](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html),
[Pi-hole Docker networking](https://docs.pi-hole.net/docker/).
