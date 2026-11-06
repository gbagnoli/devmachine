#!/usr/bin/env bash
# Isolated TPM/LUKS tool acceptance: no KVM, vault or host block devices.
set -euo pipefail
[[ $# -eq 1 && -f "$1" ]] || { echo "Usage: $0 /absolute/path/to/workstation-test-binary" >&2; exit 2; }
binary="$(realpath "$1")"
script_dir="$(dirname "$(realpath "${BASH_SOURCE[0]}")")"
image=localhost/skillet-tpm-bootstrap-acceptance
podman build --tag "$image" --file "$script_dir/tpm-bootstrap.Containerfile" "$script_dir"
podman run --rm --network none --security-opt label=disable --tmpfs /run:rw,size=1g \
  --volume "$binary:/workstation-tests:ro" \
  --volume "$script_dir/tpm-bootstrap-container.sh:/driver.sh:ro" \
  --volume "$script_dir/../scripts/root-recovery.sh:/root-recovery.sh:ro" \
  "$image" bash /driver.sh
