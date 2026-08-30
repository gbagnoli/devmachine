#!/bin/bash
set -eou pipefail

# shellcheck disable=all
{{RETRY_LOGIC}}

echo "Tailscale connected successfully. Setting operator." >&2
if ! tailscale set --operator="{{USER}}"; then
  echo "Error: Failed to set Tailscale operator." >&2
  exit 1
fi
exit 0
