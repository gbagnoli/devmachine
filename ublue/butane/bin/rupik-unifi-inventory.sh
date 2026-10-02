#!/usr/bin/env bash

log="$HOME/rupik-unifi-inventory-$(date -u +%Y%m%dT%H%M%SZ).log"
exec > >(tee "$log") 2>&1

printf 'UniFi inventory log: %s\n' "$log"
uname -m
sudo podman inspect unifi --format 'image={{.ImageName}} image-id={{.Image}} user={{.Config.User}} network={{.HostConfig.NetworkMode}} state={{.State.Status}}'
sudo podman inspect unifi --format '{{range .Mounts}}{{printf "mount %s -> %s writable=%t\n" .Source .Destination .RW}}{{end}}'
sudo podman top unifi user huser pid
sudo podman exec unifi dpkg-query -W -f='UniFi package version=${Version}\n' unifi
getent passwd unifi
getent group unifi
sudo stat -c '%u:%g mode=%a %n' /srv/unifi /srv/unifi/data /srv/unifi/data/logs
findmnt -T /srv/unifi -o TARGET,SOURCE,FSTYPE
sudo du -sh /srv/unifi /srv/unifi/data /srv/unifi/data/logs
sudo ss -lntup | grep -E ':(8080|8443|3478|10001|8843|8880|6789)([[:space:]]|$)'
curl -k --silent --output /dev/null --max-time 5 --write-out 'HTTPS 8443 status=%{http_code}\n' https://127.0.0.1:8443/
sudo find /srv/unifi -type f -iname '*.unf' -printf '%TY-%Tm-%Td %s bytes\n'
