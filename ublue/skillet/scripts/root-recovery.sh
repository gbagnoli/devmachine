#!/usr/bin/env bash
# Focused guest enrollment: the workstation owns vault persistence and transport.
set -euo pipefail
umask 077
device=/dev/disk/by-partlabel/root
key="$(mktemp /run/skillet-root-recovery.XXXXXXXX)"
trap 'rm -f -- "$key"' EXIT
cat > "$key"
[[ "$(stat -c %s "$key")" == 128 ]]
LC_ALL=C grep -Eq '^[0-9a-f]{128}$' "$key"
case "${1:-}" in
  enroll)
    if ! cryptsetup open --test-passphrase --key-file "$key" "$device"; then
      slot="$(clevis luks list -d "$device" | awk 'NR == 1 {sub(/:$/, "", $1); print $1}')"
      [[ "$slot" =~ ^[0-9]+$ ]]
      clevis luks pass -d "$device" -s "$slot" |
        cryptsetup luksAddKey --key-file=- "$device" "$key"
    fi
    ;;
  verify) cryptsetup open --test-passphrase --key-file "$key" "$device" ;;
  *) exit 2 ;;
esac
