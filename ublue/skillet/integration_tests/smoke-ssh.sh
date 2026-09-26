#!/usr/bin/env bash
set -euo pipefail

target=
binary=
clamps_binary=
port=22
identity=
disposable=false
while (($#)); do
  case "$1" in
    --target) target=${2:?}; shift 2 ;;
    --binary) binary=${2:?}; shift 2 ;;
    --clamps-binary) clamps_binary=${2:?}; shift 2 ;;
    --port) port=${2:?}; shift 2 ;;
    --identity) identity=${2:?}; shift 2 ;;
    --disposable-target) disposable=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ -z $target || -z $binary || -z $clamps_binary || $disposable != true ]]; then
  echo 'usage: smoke-ssh.sh --target USER@DISPOSABLE_VM [--port PORT] [--identity PRIVATE_KEY] --disposable-target --binary LOCAL_SKILLET --clamps-binary REMOTE_SKILLET_CLAMPS' >&2
  exit 2
fi
if [[ ! $target =~ ^[A-Za-z0-9_.-]+@[A-Za-z0-9_.-]+$ ]]; then
  echo 'target must be an explicit USER@HOST or USER@IPv4 address' >&2
  exit 2
fi
if [[ $target == *@clamps || $target == *@clamps.* ]] ||
  { [[ $target == *@localhost || $target == *@127.0.0.1 ]] && [[ $port == 22 ]]; }; then
  echo 'refusing production clamps or a local SSH service on port 22' >&2
  exit 2
fi
if [[ ! -f $binary || $clamps_binary != /* || ! $clamps_binary =~ ^/[A-Za-z0-9_./-]+$ ]]; then
  echo 'binary paths must name a local file and an absolute remote path' >&2
  exit 2
fi
if [[ ! $port =~ ^[0-9]+$ ]] || ((port < 1 || port > 65535)); then
  echo 'port must be between 1 and 65535' >&2
  exit 2
fi
if [[ -n $identity && ! -f $identity ]]; then
  echo 'identity must name an existing private-key file' >&2
  exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ssh_opts=(-o BatchMode=yes -o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new -p "$port")
scp_opts=(-o BatchMode=yes -o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new -P "$port")
if [[ -n $identity ]]; then
  known_hosts="$(dirname -- "$identity")/known_hosts"
  ssh_opts+=(-i "$identity" -o IdentitiesOnly=yes -o UserKnownHostsFile="$known_hosts")
  scp_opts+=(-i "$identity" -o IdentitiesOnly=yes -o UserKnownHostsFile="$known_hosts")
fi
scp "${scp_opts[@]}" "$binary" "$target:/var/tmp/skillet-smoke"
scp "${scp_opts[@]}" "$script_dir/smoke-guest.sh" "$target:/var/tmp/skillet-smoke-guest.sh"
ssh "${ssh_opts[@]}" "$target" 'sudo install -m 0755 /var/tmp/skillet-smoke /var/lib/skillet/skillet-smoke'
boot_before=$(ssh "${ssh_opts[@]}" "$target" 'cat /proc/sys/kernel/random/boot_id')
# The remote path is validated above and intentionally expanded on the client.
# shellcheck disable=SC2029
ssh "${ssh_opts[@]}" "$target" "sudo bash /var/tmp/skillet-smoke-guest.sh before-reboot '$clamps_binary'"
ssh "${ssh_opts[@]}" "$target" 'sudo systemctl reboot' || true

deadline=$((SECONDS + 240))
while (( SECONDS < deadline )); do
  boot_after=$(ssh "${ssh_opts[@]}" "$target" 'cat /proc/sys/kernel/random/boot_id' 2>/dev/null || true)
  if [[ -n $boot_after && $boot_after != "$boot_before" ]]; then
    # shellcheck disable=SC2029
    ssh "${ssh_opts[@]}" "$target" "sudo bash /var/tmp/skillet-smoke-guest.sh after-reboot '$clamps_binary'"
    exit 0
  fi
  sleep 5
done
echo 'VM did not return after reboot within 240 seconds' >&2
exit 1
