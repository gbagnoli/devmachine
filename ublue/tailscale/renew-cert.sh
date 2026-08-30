#!/bin/bash
set -eou pipefail

# shellcheck disable=all
{{RETRY_LOGIC}}

# Navigate to the script's directory
cd "$(dirname "$(readlink -f "$0")")"

MAGIC_DNS=$(/usr/bin/tailscale status --json 2>/dev/null | /usr/bin/jq -r '.Self.DNSName' | sed 's/\.$//')

if [ -z "$MAGIC_DNS" ] || [ "$MAGIC_DNS" == "null" ]; then
    MAGIC_DNS=$(/usr/bin/tailscale status --json 2>/dev/null | /usr/bin/jq -r '.DNSName' | sed 's/\.$//')
    if [ -z "$MAGIC_DNS" ] || [ "$MAGIC_DNS" == "null" ]; then
        echo "Error: Could not determine DNS name from tailscale status." >&2
        /usr/bin/tailscale status --json 2>/dev/null | /usr/bin/jq '.'
        exit 1
    fi
fi

echo "Fetching certificate for $MAGIC_DNS..." >&2
/usr/bin/tailscale cert "$MAGIC_DNS"
