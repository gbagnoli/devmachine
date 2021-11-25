#!/bin/bash

set -eu
set -o pipefail

# SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"
# BUNDLER_VERSION="$( grep "BUNDLED WITH" -A 1 "$SCRIPTPATH"/../Gemfile.lock | tail -n 1 | sed 's/ *//g')"

host="bender"
[ $# -eq 1 ] && host="$1"

if [[ "$host" == "bender" ]]; then
  ssh-keygen -f "$HOME/.ssh/known_hosts" -R "bender.tigc.eu" &>/dev/null || true
fi

if ! ssh -oBatchMode=yes root@"$host" ls &>/dev/null ; then
  echo >&2 "cannot connect via ssh to $host as user 'root': is openssh up and pub key setup?"
  exit 1
fi

if ! [ -f "run" ]; then
  echo >&2 "Please run this script from the root of the repo"
  exit 1
fi

cleanup() {
  rm -rf /tmp/bootstrap_"$host".sh
}

trap cleanup EXIT

cat > /tmp/boostrap_"$host".sh <<EOM
set -e
set -u

cleanup() {
  rm -rf ~/chef.deb ~/boostrap_$host.sh
}

trap cleanup EXIT

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get dist-upgrade -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold"
apt-get install curl -y
grep -q "giacomo" /etc/passwd || (
  useradd -u 4000 -g sudo giacomo
  passwd giacomo
)

dpkg -l chef &>/dev/null || (
  wget https://omnitruck.chef.io/chef/install.sh -O install_chef.sh
  bash install_chef.sh -v 16
)
EOM

scp /tmp/boostrap_"$host".sh root@"$host":
ssh root@"$host" bash boostrap_"$host".sh
./run -H "$host"
