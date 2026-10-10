#!/usr/bin/env bash
# Guest assertions only. Rust owns SSH, resource ownership, and power cycling.
set -euo pipefail
umask 077
device=/dev/disk/by-partlabel/root
state=/run/skillet-encryption-test
case "${1:-}" in
  prepare)
    mkdir -p "$state"
    cat > "$state/recovery.key"
    [[ "$(stat -c %s "$state/recovery.key")" == 64 ]]
    if ! cryptsetup open --test-passphrase --key-file "$state/recovery.key" "$device"; then
      slot="$(clevis luks list -d "$device" | awk 'NR == 1 {sub(/:$/, "", $1); print $1}')"
      [[ "$slot" =~ ^[0-9]+$ ]]
      clevis luks pass -d "$device" -s "$slot" |
        cryptsetup luksAddKey --key-file=- "$device" "$state/recovery.key"
    fi
    rm -f "$state/luks.header"
    cryptsetup luksHeaderBackup "$device" --header-backup-file "$state/luks.header"
    printf '%s\n' 'skillet encrypted-root persistence' > /var/lib/data/.skillet-encryption-test
    sync -f /var/lib/data/.skillet-encryption-test
    ;;
  verify)
    [[ "$(cat /var/lib/data/.skillet-encryption-test)" == 'skillet encrypted-root persistence' ]]
    mokutil --sb-state | grep -Fx 'SecureBoot enabled'
    cryptsetup status root | grep -F 'LUKS2'
    [[ "$(findmnt -T /var -n -o SOURCE)" == /dev/mapper/root* ]]
    [[ "$(findmnt -M /var/lib/data -n -o UUID)" == "$(findmnt -M /var -n -o UUID)" ]]
    [[ "$(findmnt -T /var -n -o FSTYPE)" == btrfs ]]
    [[ "$(findmnt -M /var/lib/data -n -o FSROOT)" == /data ]]
    [[ "$(blkid -s UUID -o value /dev/disk/by-label/root)" == "$(findmnt -T /var -n -o UUID)" ]]
    cryptsetup open --test-passphrase --key-file=- "$device"
    ;;
  *) exit 2 ;;
esac
