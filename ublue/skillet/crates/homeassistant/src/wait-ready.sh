#!/usr/bin/bash
# The proxy has no systemd readiness notification. Bound the socket publication
# wait so dependants cannot race its exec or wait indefinitely after a failure.
set -eu
for ((attempt = 0; attempt < 100; attempt++)); do
    if [[ -S /run/skillet-ha-bluetooth/system_bus_socket ]]; then
        exit 0
    fi
    sleep 0.1
done
echo 'Home Assistant Bluetooth proxy did not publish its socket' >&2
exit 1
