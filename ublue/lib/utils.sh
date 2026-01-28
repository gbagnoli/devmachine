#!/bin/bash

lib_dir="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"

for script in "$lib_dir"/*.sh; do
  if [[ "$(basename "$script")" == "utils.sh" ]]; then
    continue
  fi
  # shellcheck source=/dev/null
  source "$(realpath "${script}")"
done

ensure_sudo() {
  if ! sudo -v; then
    log "Root privileges are required. Exiting."
    exit 1
  fi
}

running_in_distrobox() {
  [ -f "/run/.containerenv" ] && [ -f "/run/host/etc/hostname" ]
}

exit_if_running_in_distrobox() {
  if running_in_distrobox; then
    log "Script is running in a distrobox container"
    log "This it not supported. Exiting"
    exit 1
  fi
}

exit_if_not_running_in_distrobox() {
  if ! running_in_distrobox; then
    log "Script is NOT running in a distrobox container"
    log "This is not supported"
    exit 1
  fi
}

run_on_host() {
  if running_in_distrobox; then
    distrobox-host-exec "$@"
  else
    "$@"
  fi
}
