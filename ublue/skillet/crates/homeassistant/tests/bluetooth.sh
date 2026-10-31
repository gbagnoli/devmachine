#!/usr/bin/bash
# Guest assertions only; invoke through the locked, owned VM guest adapter.
set -euo pipefail
[[ $EUID -eq 0 && $(hostname) == *-test-* ]]
systemd-detect-virt --vm >/dev/null
[[ $(getenforce) == Enforcing ]]
diagnostics() {
    journalctl -b -u skillet-ha-bluetooth-policy.service -u skillet-ha-bluetooth.service \
        --no-pager -n 40 || true
}
trap diagnostics ERR
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
case ${1:-} in
    ''|--verify) ;;
    *) echo 'Usage: bluetooth.sh [--verify]' >&2; exit 1 ;;
esac
if [[ ${1:-} == --verify ]]; then
    # Do not repair boot ordering before checking it.
    systemctl is-active --quiet skillet-ha-bluetooth-policy.service
    systemctl is-active --quiet skillet-ha-bluetooth.service
else
    install -d -m 755 /etc/skillet/homeassistant-bluetooth
    install -m 644 "$source_dir/skillet-ha-bluetooth.cil" "$source_dir/wait-ready.sh" /etc/skillet/homeassistant-bluetooth/
    install -m 644 "$source_dir/skillet-ha-bluetooth-policy.service" "$source_dir/skillet-ha-bluetooth.service" /etc/systemd/system/
    systemctl daemon-reload
    systemctl restart skillet-ha-bluetooth-policy.service
    systemctl enable skillet-ha-bluetooth.service
    systemctl restart skillet-ha-bluetooth.service
fi
# The VM has no radio. Load the subsystem so BlueZ can answer an empty inventory.
modprobe bluetooth
systemctl start bluetooth.service
original_label=$(ls -Z /run/dbus/system_bus_socket)
if [[ ${1:-} != --verify ]]; then
    podman build -t localhost/skillet-bluetooth-acceptance -f "$source_dir/bluetooth.Containerfile" "$source_dir"
fi
image=localhost/skillet-bluetooth-acceptance
bluez=(--address=unix:path=/run/dbus/system_bus_socket --timeout=10s call org.bluez / org.freedesktop.DBus.ObjectManager GetManagedObjects)
probe() {
    podman run --rm --network=none --volume=/run/skillet-ha-bluetooth:/run/dbus:ro "$image" busctl "$@"
}
probe "${bluez[@]}"
busctl --system call org.freedesktop.systemd1 /org/freedesktop/systemd1 \
    org.freedesktop.systemd1.Manager GetUnit s bluetooth.service >/dev/null
if probe --address=unix:path=/run/dbus/system_bus_socket --timeout=10s call \
    org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager GetUnit s bluetooth.service; then
    echo 'FAIL: unrelated host bus service is reachable' >&2
    exit 1
fi
if podman run --rm --network=none --volume=/run/dbus:/run/dbus:ro "$image" busctl "${bluez[@]}"; then
    echo 'FAIL: direct host bus unexpectedly accessible to ordinary containers' >&2
    exit 1
fi
systemctl restart skillet-ha-bluetooth.service
probe "${bluez[@]}"
[[ $(ls -Z /run/dbus/system_bus_socket) == "$original_label" ]]
[[ $(getenforce) == Enforcing ]]
pid=$(systemctl show --value -p MainPID skillet-ha-bluetooth.service)
ps -p "$pid" -o label= | grep -F 'system_u:system_r:skillet_ha_bluetooth_t:s0'
echo 'PASS: confined filtered Bluetooth bus, unrelated-service denial, direct-bus denial, restart and preserved host socket label'
