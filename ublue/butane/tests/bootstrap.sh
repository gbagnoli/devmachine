#!/usr/bin/env bash
set -euo pipefail

source_dir="$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")"
tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
mkdir "$tmp/bin"
yq -r '.storage.files[] | select(.path == "/var/usrlocal/bin/ucore-rebase.sh") | .contents.inline' \
  "$source_dir/includes/ucore-bootstrap.bu" > "$tmp/rebase.sh"

cat > "$tmp/bin/rpm-ostree" <<'MOCK'
#!/usr/bin/env bash
if [[ "$1" == status && "$2" == --json ]]; then
  cat "$CASE_STATUS"
elif [[ "$1" == rebase && "$2" == --bypass-driver ]]; then
  printf '%s\n' "$3" >> "$CASE_REBASE_LOG"
  [[ "${CASE_FAIL_REBASE:-0}" == 0 ]]
else
  exit 2
fi
MOCK
cat > "$tmp/bin/systemctl" <<'MOCK'
#!/usr/bin/env bash
[[ "$1" == reboot && "$2" == --no-block ]] || exit 2
printf 'reboot\n' >> "$CASE_REBOOT_LOG"
MOCK
chmod +x "$tmp/bin/rpm-ostree" "$tmp/bin/systemctl"

image=ghcr.io/gbagnoli/ucore-clamps
unsigned="ostree-unverified-registry:$image:latest"
signed="ostree-image-signed:docker://$image:latest"

check() {
  local name="$1" status="$2" expected_rebase="$3" expected_reboots="$4"
  local expected_ready="$5" expected_exit="$6" attempts="${7:-}" fail_rebase="${8:-0}"
  local configured_image="${9:-$image}"
  local case_dir="$tmp/$name" actual_exit=0 actual_rebase actual_reboots actual_ready
  mkdir -p "$case_dir/state"
  printf '%s\n' "$configured_image" > "$case_dir/image"
  printf '%s\n' "$status" > "$case_dir/status.json"
  if [[ -n "$attempts" ]]; then printf '%s\n' "$attempts" > "$case_dir/state/reboot-attempts"; fi
  sed -e "s@/etc/ucore-bootstrap-image@$case_dir/image@g" \
    -e "s@/var/lib/ucore-bootstrap@$case_dir/state@g" \
    -e "s@/run/ucore-bootstrap-ready@$case_dir/ready@g" \
    "$tmp/rebase.sh" > "$case_dir/rebase.sh"
  CASE_STATUS="$case_dir/status.json" CASE_REBASE_LOG="$case_dir/rebases" \
    CASE_REBOOT_LOG="$case_dir/reboots" CASE_FAIL_REBASE="$fail_rebase" \
    PATH="$tmp/bin:$PATH" bash "$case_dir/rebase.sh" > "$case_dir/output" 2>&1 || actual_exit=$?
  actual_rebase="$(cat "$case_dir/rebases" 2>/dev/null || true)"
  if [[ -f "$case_dir/reboots" ]]; then
    actual_reboots="$(wc -l < "$case_dir/reboots")"
  else
    actual_reboots=0
  fi
  actual_ready=0
  [[ ! -e "$case_dir/ready" ]] || actual_ready=1
  if [[ "$actual_rebase" != "$expected_rebase" || "$actual_reboots" != "$expected_reboots" ||
        "$actual_ready" != "$expected_ready" || "$actual_exit" != "$expected_exit" ]]; then
    echo "FAIL $name: rebase=$actual_rebase reboots=$actual_reboots ready=$actual_ready exit=$actual_exit" >&2
    cat "$case_dir/output" >&2
    exit 1
  fi
  echo "PASS $name"
}

check fresh '{"deployments":[{"booted":true,"container-image-reference":""}]}' "$unsigned" 1 0 0
check bender_fresh '{"deployments":[{"booted":true,"container-image-reference":""}]}' \
  'ostree-unverified-registry:ghcr.io/gbagnoli/ucore-bender:latest' 1 0 0 '' 0 \
  'ghcr.io/gbagnoli/ucore-bender'
check unsigned_with_signed_rollback \
  "{\"deployments\":[{\"booted\":true,\"container-image-reference\":\"$unsigned\"},{\"booted\":false,\"container-image-reference\":\"$signed\"}]}" \
  "$signed" 1 0 0
check signed_pending \
  "{\"deployments\":[{\"booted\":false,\"container-image-reference\":\"$signed\"},{\"booted\":true,\"container-image-reference\":\"$unsigned\"}]}" \
  '' 1 0 0 1
check signed_booted \
  "{\"deployments\":[{\"booted\":true,\"container-image-reference\":\"$signed\"}]}" \
  '' 0 1 0 1
check signed_digest_booted \
  "{\"deployments\":[{\"booted\":true,\"container-image-reference\":\"ostree-image-signed:docker://$image@sha256:$(printf 'a%.0s' {1..64})\"}]}" \
  '' 0 1 0
check pending_retry_limit \
  "{\"deployments\":[{\"booted\":false,\"container-image-reference\":\"$signed\"},{\"booted\":true,\"container-image-reference\":\"$unsigned\"}]}" \
  '' 0 0 1 2
check rebase_failure '{"deployments":[{"booted":true,"container-image-reference":""}]}' \
  "$unsigned" 0 0 1 '' 1
