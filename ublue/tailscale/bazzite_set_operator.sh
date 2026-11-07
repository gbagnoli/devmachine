#!/bin/bash
set -euo pipefail

if [[ $EUID -ne 0 || -z ${SUDO_USER:-} || $SUDO_USER == root ]]; then
    echo 'Run through sudo from an authorized desktop user.' >&2
    exit 1
fi

for ((attempt=1; attempt<=30; attempt++)); do
    if tailscale status --json 2>/dev/null | jq -e '.BackendState == "Running"' >/dev/null; then
        exec tailscale set --operator="$SUDO_USER"
    fi
    sleep 1
done

echo 'Tailscale did not connect within 30 seconds.' >&2
exit 1
