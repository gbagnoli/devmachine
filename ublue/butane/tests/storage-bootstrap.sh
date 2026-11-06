#!/usr/bin/env bash
set -euo pipefail
source_dir="$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")"
tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
mkdir "$tmp/bin"
yq -r '.storage.files[] | select(.path == "/var/usrlocal/bin/skillet-data-top") | .contents.inline' \
  "$source_dir/includes/data-storage.bu" > "$tmp/top.sh"
cat > "$tmp/bin/findmnt" <<'MOCK'
#!/usr/bin/env bash
case "$*" in *FSTYPE*) echo btrfs ;; *UUID*) echo fixture-uuid ;; *) exit 2 ;; esac
MOCK
cat > "$tmp/bin/blkid" <<'MOCK'
#!/usr/bin/env bash
echo fixture-uuid
MOCK
cat > "$tmp/bin/mount" <<'MOCK'
#!/usr/bin/env bash
path="${!#}"
case "$CASE_MODE" in
  fresh) ;;
  plain-data) mkdir "$path/data" ;;
  *) mkdir "$path/data"; touch "$path/data/.subvolume"
     case "$CASE_MODE" in
       existing-both) mkdir "$path/data/containers"; touch "$path/data/containers/.subvolume" ;;
       plain-containers) mkdir "$path/data/containers" ;;
     esac ;;
esac
MOCK
cat > "$tmp/bin/btrfs" <<'MOCK'
#!/usr/bin/env bash
[[ "$1" == subvolume ]] || exit 2
case "$2" in
  show) [[ -f "$3/.subvolume" ]] ;;
  create) mkdir "$3"; touch "$3/.subvolume"; basename "$3" >> "$CASE_LOG" ;;
  *) exit 2 ;;
esac
MOCK
cat > "$tmp/bin/mountpoint" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
cat > "$tmp/bin/umount" <<'MOCK'
#!/usr/bin/env bash
[[ "$1" == "$CASE_DIR"/mount.* ]] || exit 2
rm -rf -- "$1/data"
MOCK
chmod +x "$tmp/bin/"*
check() {
  local mode="$1" expected="$2" expected_exit="$3" status=0
  local directory="$tmp/$mode"
  mkdir "$directory"
  sed "s@/run/skillet-data-top.XXXXXX@$directory/mount.XXXXXX@g" "$tmp/top.sh" > "$directory/run.sh"
  CASE_MODE="$mode" CASE_DIR="$directory" CASE_LOG="$directory/created" PATH="$tmp/bin:$PATH" \
    bash "$directory/run.sh" > "$directory/output" 2>&1 || status=$?
  [[ "$status" == "$expected_exit" && "$(cat "$directory/created" 2>/dev/null || true)" == "$expected" ]] || {
    cat "$directory/output" >&2; echo "FAIL $mode" >&2; exit 1;
  }
  echo "PASS $mode"
}
check fresh $'data\ncontainers' 0
check existing-data containers 0
check existing-both '' 0
check plain-data '' 1
check plain-containers '' 1
yq -e '.systemd.units[] | select(.name == "ucore-bootstrap.service") | .dropins[] | .contents | contains("Requires=var-lib-data.mount")' \
  "$source_dir/includes/data-storage.bu" >/dev/null
