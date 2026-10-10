#!/usr/bin/env bash
# Exercise refusal paths in an isolated container with no host block devices.
set -euo pipefail
butane_dir="$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
printf '{"ignition":{"version":"3.5.0"}}\n' > "$work/config.ign"
printf /dev/disk/by-id/skillet-nonexistent-test-disk > "$work/disk"
printf unencrypted > "$work/profile"
run_case() {
  local expected="$1"
  local status=0
  podman run --rm --network none --security-opt label=disable \
    --volume "$work:/etc/skillet-install:ro" \
    --volume "$butane_dir/bin/install-hardware:/helper:ro" \
    registry.fedoraproject.org/fedora:latest bash /helper /etc/skillet-install/config.ign \
    > "$work/output" 2>&1 || status=$?
  if [[ "$status" != 1 ]] || ! grep -Fq "$expected" "$work/output"; then
    cat "$work/output" >&2; echo "FAIL expected refusal: $expected" >&2; exit 1
  fi
  echo "PASS $expected"
}
printf wrong-digest > "$work/ignition.sha256"
run_case 'Ignition checksum does not match'
sha256sum "$work/config.ign" | cut -d ' ' -f 1 > "$work/ignition.sha256"
run_case 'Prepared whole-disk identifier is absent'
