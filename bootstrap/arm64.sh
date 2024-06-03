#!/bin/bash

set -u
set -e
set -o pipefail

# looks like chef doesn't package for arm64.
# there is some packages for armhf here: https://mattray.github.io/arm/
# let's install for armhf using multiarch.

if [ $# -lt 1 ] ; then
  echo >&2 "usage: $0 <host> [user]"
  exit 1
fi

host="$1"
user="${2:-$USER}"
here="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
parent="$(realpath "$here/..")"

ssh -oBatchMode=yes "$user"@"$host" ls &>/dev/null
# shellcheck disable=SC2181
if [ $? -ne 0 ] ; then
  echo >&2 "cannot connect via ssh to $host as user '$user': is openssh up and pub key setup?"
  exit 1
fi

script=$(mktemp /tmp/"$host".XXXXXX)

cleanup() {
  rm -rf "$script"
}

trap cleanup EXIT

echo -n "Remote password for sudo: "
read -rs password
scriptname="$(basename "$script")"
cat > "$script" <<EOM
set -e
set -u
echo "$password" | sudo -S ls &>/dev/null
command -v cinc-client &>/dev/null || curl -L https://omnitruck.cinc.sh/install.sh | sudo bash -s -- -v 18
sudo ln -sf /usr/bin/cinc-client /usr/bin/chef-client
EOM

scp "$script" "$user"@"$host":
# shellcheck disable=SC2029
ssh "$user"@"$host" bash "$scriptname"

pushd "$parent" &>/dev/null
./run -H "$host"
