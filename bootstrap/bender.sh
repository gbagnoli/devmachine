#!/bin/bash

set -eu
set -o pipefail

host="bender"
[ $# -eq 1 ] && host="$1"

if [[ "$host" == "bender" ]]; then
  ssh-keygen -f "$HOME/.ssh/known_hosts" -R "bender.tigc.eu" &>/dev/null || true
fi
ssh -oBatchMode=yes root@"$host" ls &>/dev/null

if [ $? -ne 0 ] ; then
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
  curl -L https://packages.chef.io/files/stable/chef/14.7.17/ubuntu/18.04/chef_14.7.17-1_amd64.deb -o chef.deb
  sudo dpkg -i chef.deb
)
EOM

scp /tmp/boostrap_"$host".sh root@"$host":
ssh root@"$host" bash boostrap_"$host".sh
./run -H "$host"
