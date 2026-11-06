#!/usr/bin/env bash
# Destructive only to a fresh tmpfs regular-file fixture, never a block device.
set -euo pipefail
umask 077
work="$(mktemp -d /run/skillet-header-restore.XXXXXXXX)"
trap 'rm -rf -- "$work"' EXIT
dd bs=128 count=1 iflag=fullblock status=none of="$work/key"
[[ "$(stat -c %s "$work/key")" == 128 ]]
LC_ALL=C grep -Eq '^[0-9a-f]{128}$' "$work/key"
cat > "$work/header"
truncate -s 64M "$work/fixture"
cryptsetup luksHeaderRestore --batch-mode "$work/fixture" --header-backup-file "$work/header"
cryptsetup open --test-passphrase --key-file "$work/key" "$work/fixture"
