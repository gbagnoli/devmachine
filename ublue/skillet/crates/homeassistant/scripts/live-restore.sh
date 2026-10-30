#!/usr/bin/env bash
# Restore a Skillet Home Assistant backup on its original host.
set -euo pipefail

if (( EUID != 0 || $# != 1 )) || [[ "$1" != /* || ! -f "$1" || -L "$1" ]]; then
  echo 'Usage: sudo live-restore.sh /absolute/path/to/ha-archive.tar' >&2
  exit 2
fi

archive=$1
data=/var/lib/data/homeassistant
live=$data/config
helper=$(dirname "$(realpath "$0")")/restore-stage.py
[[ -f "$helper" && -d "$live" ]]
systemctl is-active --quiet homeassistant.service
image=$(podman inspect --format '{{.ImageName}}' homeassistant)
[[ -n "$image" ]]

preflight=$(mktemp -d "$data/restore-preflight.XXXXXX")
rollback=$(mktemp -d "$data/restore-rollback.XXXXXX")
chmod 700 "$preflight" "$rollback"
stopped=0
moved=0
success=0
finish() {
  if (( success == 1 )); then
    rm -rf -- "$preflight"
    echo "Previous configuration retained at $rollback/config"
    return
  fi
  rm -f -- "$preflight/.HA_RESTORE"
  echo "Restore failed; offline preflight retained at $preflight" >&2
  if (( stopped == 1 )); then
    systemctl stop homeassistant.service || true
    if (( moved == 1 )); then
      if [[ -d "$live" ]]; then
        rm -f -- "$live/.HA_RESTORE"
        mv "$live" "$rollback/failed-config" || true
      fi
      mv "$rollback/config" "$live" || true
    fi
    systemctl start homeassistant.service || true
    echo 'Previous configuration returned to the live path' >&2
  fi
}
trap finish EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# The preflight runs the pinned image without a network or live devices.
mkdir -m 700 "$preflight/backups"
cp --reflink=auto "$archive" "$preflight/backups/source.tar"
cmp "$archive" "$preflight/backups/source.tar"
python3 "$helper" "$preflight" "$preflight/backups/source.tar"
podman run --rm --pull=never --network=none --entrypoint=python3 \
  --volume "$preflight:/config:Z" "$image" \
  -c 'from homeassistant.backup_restore import restore_backup; assert restore_backup("/config")'
python3 - "$preflight" <<'PY'
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
assert json.loads((root / ".HA_RESTORE_RESULT").read_text())["success"] is True
auth = json.loads((root / ".storage/auth").read_text())
assert sum(bool(u.get("is_owner") and u.get("is_active")) for u in auth["data"]["users"]) == 1
assert (root / "configuration.yaml").is_file()
PY
echo 'Offline restore passed'

systemctl stop homeassistant.service
stopped=1
mv "$live" "$rollback/config"
moved=1
mkdir -m 700 "$live"
mkdir -m 700 "$live/backups"
cp --reflink=auto "$preflight/backups/source.tar" "$live/backups/source.tar"
cmp "$preflight/backups/source.tar" "$live/backups/source.tar"
python3 "$helper" "$live" "$live/backups/source.tar"
systemctl start homeassistant.service

ready=0
for ((attempt = 1; attempt <= 90; attempt++)); do
  if curl -fsS --max-time 2 -o /dev/null http://127.0.0.1:8123/ 2>/dev/null; then
    ready=1
    break
  fi
  sleep 2
done
(( ready == 1 ))
python3 - "$preflight" "$live" <<'PY'
import json
from pathlib import Path
import sys

reference, current = map(Path, sys.argv[1:])
def owner(root):
    auth = json.loads((root / ".storage/auth").read_text())
    users = [u["id"] for u in auth["data"]["users"]
             if u.get("is_owner") and u.get("is_active")]
    assert len(users) == 1
    return users[0]
assert owner(reference) == owner(current)
assert not (current / ".HA_RESTORE").exists()
result = current / ".HA_RESTORE_RESULT"
assert not result.exists() or json.loads(result.read_text())["success"] is True
assert (current / "configuration.yaml").is_file()
assert "trusted_networks" in (current / "skillet-auth.yaml").read_text()
PY
systemctl is-active --quiet homeassistant.service
cmp "$preflight/backups/source.tar" "$live/backups/source.tar"
success=1
systemctl start skillet-homeassistant-backup.service
echo 'Live restore and backup exporter verified'
