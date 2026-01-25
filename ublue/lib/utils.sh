#!/bin/bash

script_dir="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"

# shellcheck source=/dev/null
source "$(realpath "$script_dir/logging.sh")"

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
