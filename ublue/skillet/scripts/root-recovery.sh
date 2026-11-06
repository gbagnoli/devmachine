#!/usr/bin/env bash
# Focused guest enrollment: the workstation owns vault persistence and transport.
set -euo pipefail
umask 077
device=/dev/disk/by-partlabel/root
key="$(mktemp /run/skillet-root-recovery.XXXXXXXX)"
work=""
trap 'rm -f -- "$key"; if [[ -n "$work" ]]; then rm -rf -- "$work"; fi' EXIT
cat > "$key"
[[ "$(stat -c %s "$key")" == 128 ]]
LC_ALL=C grep -Eq '^[0-9a-f]{128}$' "$key"
case "${1:-}" in
  enroll)
    if ! cryptsetup open --test-passphrase --key-file "$key" "$device"; then
      mapfile -t slots < <(clevis luks list -d "$device" | awk '{sub(/:$/, "", $1); print $1}')
      enrolled=false
      for slot in "${slots[@]}"; do
        [[ "$slot" =~ ^[0-9]+$ ]]
        if clevis luks pass -d "$device" -s "$slot" 2>/dev/null |
          cryptsetup luksAddKey --key-file=- "$device" "$key"; then
          enrolled=true
          break
        fi
      done
      "$enrolled" || { echo 'No working Clevis binding could enroll recovery.' >&2; exit 1; }
    fi
    ;;
  header)
    work="$(mktemp -d /run/skillet-root-header.XXXXXXXX)"
    header="$work/luks.header"
    cryptsetup luksHeaderBackup "$device" --header-backup-file "$header"
    [[ "$(cryptsetup luksUUID --header "$header" "$device")" == "$(cryptsetup luksUUID "$device")" ]]
    cryptsetup open --test-passphrase --header "$header" --key-file "$key" "$device"
    cat "$header"
    ;;
  verify) cryptsetup open --test-passphrase --key-file "$key" "$device" ;;
  *) exit 2 ;;
esac
