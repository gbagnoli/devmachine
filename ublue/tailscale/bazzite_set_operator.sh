#!/bin/bash
set -euo pipefail

if [[ $EUID -ne 0 || -z ${SUDO_USER:-} || $SUDO_USER == root ]]; then
    echo 'Run through sudo from an authorized desktop user.' >&2
    exit 1
fi

for ((attempt=1; attempt<=30; attempt++)); do
    if tailscale set --operator="$SUDO_USER"; then
        exit 0
    fi
    sleep 1
done

echo 'Could not set the Tailscale operator after 30 attempts.' >&2
exit 1
