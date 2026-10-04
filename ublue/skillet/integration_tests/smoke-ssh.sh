#!/usr/bin/env bash
set -euo pipefail

target=
fixture_binary=
fixture_sha256=
generic_sha256=
host_sha256=
host_binary=
host=
credentials_required=
port=22
identity=
disposable=false
while (($#)); do
  case "$1" in
    --target) target=${2:?}; shift 2 ;;
    --fixture-binary) fixture_binary=${2:?}; shift 2 ;;
    --fixture-sha256) fixture_sha256=${2:?}; shift 2 ;;
    --generic-sha256) generic_sha256=${2:?}; shift 2 ;;
    --host-sha256) host_sha256=${2:?}; shift 2 ;;
    --host-binary) host_binary=${2:?}; shift 2 ;;
    --host) host=${2:?}; shift 2 ;;
    --credentials-required) credentials_required=${2:?}; shift 2 ;;
    --port) port=${2:?}; shift 2 ;;
    --identity) identity=${2:?}; shift 2 ;;
    --disposable-target) disposable=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ -z $target || -z $fixture_binary || -z $fixture_sha256 || -z $generic_sha256 || -z $host_sha256 || -z $host_binary || -z $host || -z $credentials_required || $disposable != true ]]; then
  echo 'usage: smoke-ssh.sh --target USER@DISPOSABLE_VM --host HOST --credentials-required yes|no [--port PORT] [--identity PRIVATE_KEY] --disposable-target --fixture-binary LOCAL_FIXTURE --fixture-sha256 HASH --generic-sha256 HASH --host-sha256 HASH --host-binary REMOTE_SKILLET_HOST' >&2
  exit 2
fi
if [[ ! $target =~ ^[A-Za-z0-9_.-]+@[A-Za-z0-9_.-]+$ ]]; then
  echo 'target must be an explicit USER@HOST or USER@IPv4 address' >&2
  exit 2
fi
if [[ ! $host =~ ^[a-z][a-z0-9-]*$ || ! $credentials_required =~ ^(yes|no)$ ]]; then
  echo 'invalid host profile or credential capability' >&2
  exit 2
fi
if [[ $target == *@"$host" || $target == *@"$host".* ]] ||
  { [[ $target == *@localhost || $target == *@127.0.0.1 ]] && [[ $port == 22 ]]; }; then
  echo "refusing the production host identity $host or a local SSH service on port 22" >&2
  exit 2
fi
if [[ ! -f $fixture_binary || ! $fixture_sha256 =~ ^[a-f0-9]{64}$ || ! $generic_sha256 =~ ^[a-f0-9]{64}$ || ! $host_sha256 =~ ^[a-f0-9]{64}$ || $host_binary != /* || ! $host_binary =~ ^/[A-Za-z0-9_./-]+$ ]]; then
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
scp -O "${scp_opts[@]}" "$fixture_binary" "$target:/var/tmp/skillet-smoke-fixture"
scp -O "${scp_opts[@]}" "$script_dir/smoke-guest.sh" "$target:/var/tmp/skillet-smoke-guest.sh"
ssh "${ssh_opts[@]}" "$target" 'sudo install -m 0755 /var/tmp/skillet-smoke-fixture /var/usrlocal/bin/skillet-smoke-fixture'
fixture_remote_sha256=$(ssh "${ssh_opts[@]}" "$target" 'sha256sum /var/usrlocal/bin/skillet-smoke-fixture | cut -d " " -f 1')
if [[ $fixture_remote_sha256 != "$fixture_sha256" ]]; then
  echo 'transferred fixture binary does not match the selected Cargo artifact' >&2
  exit 1
fi
generic_remote_sha256=$(ssh "${ssh_opts[@]}" "$target" 'sha256sum /var/usrlocal/bin/skillet | cut -d " " -f 1')
if [[ $generic_remote_sha256 != "$generic_sha256" ]]; then
  echo 'installed generic binary does not match the ready VM manifest' >&2
  exit 1
fi
# shellcheck disable=SC2029
host_remote_sha256=$(ssh "${ssh_opts[@]}" "$target" "sha256sum '$host_binary' | cut -d ' ' -f 1")
if [[ $host_remote_sha256 != "$host_sha256" ]]; then
  echo 'installed host binary does not match the ready VM manifest' >&2
  exit 1
fi
boot_before=$(ssh "${ssh_opts[@]}" "$target" 'cat /proc/sys/kernel/random/boot_id')
# The remote path is validated above and intentionally expanded on the client.
# shellcheck disable=SC2029
ssh "${ssh_opts[@]}" "$target" "sudo bash /var/tmp/skillet-smoke-guest.sh before-reboot '$host_binary' '$credentials_required'"
ssh "${ssh_opts[@]}" "$target" 'sudo systemctl reboot' || true

deadline=$((SECONDS + 240))
while (( SECONDS < deadline )); do
  boot_after=$(ssh "${ssh_opts[@]}" "$target" 'cat /proc/sys/kernel/random/boot_id' 2>/dev/null || true)
  if [[ -n $boot_after && $boot_after != "$boot_before" ]]; then
    # shellcheck disable=SC2029
    ssh "${ssh_opts[@]}" "$target" "sudo bash /var/tmp/skillet-smoke-guest.sh after-reboot '$host_binary' '$credentials_required'"
    exit 0
  fi
  sleep 5
done
echo 'VM did not return after reboot within 240 seconds' >&2
exit 1
