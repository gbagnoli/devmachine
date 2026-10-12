#!/usr/bin/env bash
# Exercise refusal paths in an isolated container with no host block devices.
set -euo pipefail
butane_dir="$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT

# Exercise the production trust predicate without any real firmware or disks.
sed -n '/^mok_is_enrolled() {$/,/^}$/p' "$butane_dir/bin/install-hardware" > "$work/mok-check.sh"
# shellcheck source=/dev/null
source "$work/mok-check.sh"
mkdir "$work/bin"
printf fixture > "$work/ublue.der"
cat > "$work/bin/mokutil" <<'MOCK'
#!/usr/bin/env bash
[[ "$1" == --test-key && "$LC_ALL" == C ]] || exit 2
printf '%s %s\n' "$2" "$CASE_MOK_MESSAGE"
exit "$CASE_MOK_STATUS"
MOCK
chmod +x "$work/bin/mokutil"
check_mok() {
  local name="$1" message="$2" status="$3" expected="$4" actual=0
  PATH="$work/bin:$PATH" CASE_MOK_MESSAGE="$message" CASE_MOK_STATUS="$status" \
    mok_is_enrolled "$work/ublue.der" > "$work/mok-output" 2>&1 || actual=$?
  if [[ "$actual" != "$expected" ]]; then
    cat "$work/mok-output" >&2
    echo "FAIL $name: exit=$actual expected=$expected" >&2
    exit 1
  fi
  echo "PASS $name"
}
check_mok enrolled_zero 'is already enrolled' 0 0
check_mok enrolled_one 'is already enrolled' 1 0
check_mok enrolled_error 'is already enrolled' 255 1
check_mok not_enrolled 'is not enrolled' 1 1
check_mok not_enrolled_zero 'is not enrolled' 0 1
check_mok pending_request 'is already in the enrollment request' 0 1
check_mok blocked_key 'is blocked in dbx' 0 1
check_mok blocked_mok 'is blocked in MokListX' 0 1
check_mok tool_error 'read error' 2 1
check_mok unknown_success 'unexpected result' 0 1
rm "$work/ublue.der"
if mok_is_enrolled "$work/ublue.der"; then
  echo 'FAIL missing_certificate: unexpectedly trusted' >&2
  exit 1
fi
echo 'PASS missing_certificate'

# Confirmation acknowledges the existing target; it never selects a new disk.
sed -n '/^disk_confirmation_matches() {$/,/^}$/p' "$butane_dir/bin/install-hardware" > "$work/disk-check.sh"
# shellcheck source=/dev/null
source "$work/disk-check.sh"
check_confirmation() {
  local name="$1" resolved="$2" confirmation="$3" expected="$4" actual=0
  disk_confirmation_matches /dev/disk/by-id/prepared-test-disk "$resolved" "$confirmation" || actual=$?
  if [[ "$actual" != "$expected" ]]; then
    echo "FAIL $name: exit=$actual expected=$expected" >&2
    exit 1
  fi
  echo "PASS $name"
}
check_confirmation short_nvme /dev/nvme0n1 nvme0n1 0
check_confirmation short_sata /dev/sda sda 0
check_confirmation full_identifier /dev/nvme0n1 /dev/disk/by-id/prepared-test-disk 0
check_confirmation wrong_disk /dev/nvme0n1 nvme1n1 1
check_confirmation wrong_sata /dev/nvme0n1 sda 1
check_confirmation partition /dev/nvme0n1 nvme0n1p1 1
check_confirmation wrong_identifier /dev/nvme0n1 /dev/disk/by-id/other-test-disk 1
check_confirmation empty /dev/nvme0n1 '' 1
check_confirmation unrelated_path /dev/nvme0n1 /tmp/nvme0n1 1

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
