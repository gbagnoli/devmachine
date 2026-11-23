#!/usr/bin/env bash
# Isolated TPM/LUKS tool acceptance: no KVM, vault or host block devices.
set -euo pipefail
[[ $# -eq 1 && -f "$1" ]] || { echo "Usage: $0 /absolute/path/to/workstation-test-binary" >&2; exit 2; }
binary="$(realpath "$1")"
script_dir="$(dirname "$(realpath "${BASH_SOURCE[0]}")")"
fedora="${SKILLET_TPM_TEST_FEDORA:-44}"
[[ "$fedora" =~ ^[0-9]+$ ]] || { echo "Invalid Fedora test version" >&2; exit 2; }
image="localhost/skillet-tpm-bootstrap-acceptance:$fedora"
podman build --tag "$image" --build-arg "FEDORA_VERSION=$fedora" \
  --file "$script_dir/tpm-bootstrap.Containerfile" "$script_dir"
podman run --rm --network none --security-opt label=disable --tmpfs /run:rw,size=1g \
  --volume "$binary:/workstation-tests:ro" \
  --volume "$script_dir/tpm-bootstrap-container.sh:/driver.sh:ro" \
  --volume "$script_dir/../scripts/root-recovery.sh:/root-recovery.sh:ro" \
  "$image" bash /driver.sh
