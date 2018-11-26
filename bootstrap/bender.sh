#!/bin/bash

set -eu
set -o pipefail

ssh -oBatchMode=yes root@bender ls &>/dev/null

if [ $? -ne 0 ] ; then
  echo >&2 "cannot connect via ssh to bender as user 'root': is openssh up and pub key setup?"
  exit 1
fi

if ! [ -f "run" ]; then
  echo >&2 "Please run this script from the root of the repo"
  exit 1
fi

cleanup() {
  rm -rf /tmp/bootstrap_bender.sh
}

trap cleanup EXIT

cat > /tmp/boostrap_bender.sh <<EOM
set -e
set -u

cleanup() {
  rm -rf ~/chef.deb ~/boostrap_bender.sh
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

scp /tmp/boostrap_bender.sh root@bender:
ssh root@bender bash boostrap_bender.sh
./run -H bender
