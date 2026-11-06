#!/usr/bin/env bash
# Explicit recovery after TPM replacement; caller records the original slot.
set -euo pipefail
umask 077
device=/dev/disk/by-partlabel/root
old="${1:-}"
[[ "$old" =~ ^[0-9]+$ ]]
key="$(mktemp /run/skillet-root-rebind.XXXXXXXX)"
trap 'rm -f -- "$key"' EXIT
cat > "$key"
[[ "$(stat -c %s "$key")" == 128 ]]
LC_ALL=C grep -Eq '^[0-9a-f]{128}$' "$key"
cryptsetup open --test-passphrase --key-file "$key" "$device"
mapfile -t slots < <(clevis luks list -d "$device" | awk '{sub(/:$/, "", $1); print $1}')
[[ "${#slots[@]}" -ge 1 && "${#slots[@]}" -le 2 ]]
new=""
old_present=false
for slot in "${slots[@]}"; do
  [[ "$slot" =~ ^[0-9]+$ ]]
  if [[ "$slot" == "$old" ]]; then old_present=true; else new="$slot"; fi
done
if "$old_present"; then
  if clevis luks pass -d "$device" -s "$old" 2>/dev/null |
    cryptsetup open --test-passphrase --key-file=- "$device" 2>/dev/null; then
    echo 'Original TPM still unlocks; refusing replacement' >&2
    exit 1
  fi
  if [[ -z "$new" ]]; then
    clevis luks bind -y -d "$device" -k "$key" tpm2 '{"pcr_bank":"sha256","pcr_ids":"7"}'
    mapfile -t slots < <(clevis luks list -d "$device" | awk '{sub(/:$/, "", $1); print $1}')
    [[ "${#slots[@]}" == 2 ]]
    for slot in "${slots[@]}"; do
      [[ "$slot" =~ ^[0-9]+$ ]]
      if [[ "$slot" != "$old" ]]; then new="$slot"; fi
    done
  fi
fi
[[ "$new" =~ ^[0-9]+$ ]]
clevis luks pass -d "$device" -s "$new" |
  cryptsetup open --test-passphrase --key-file=- "$device"
if "$old_present"; then clevis luks unbind -f -d "$device" -s "$old"; fi
cryptsetup open --test-passphrase --key-file "$key" "$device"
