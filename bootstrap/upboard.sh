#!/bin/bash

set -u
set -o pipefail

if [ $# -ne 1 ] ; then
  echo >&2 "usage: ./upboard.sh <ip addr>"
  exit 1
fi

ip="$1"

ssh -oBatchMode=yes up@"$ip" ls &>/dev/null
if [ $? -ne 0 ] ; then
  echo >&2 "cannot connect via ssh to $ip as user 'up': is openssh up and pub key setup?"
  exit 1
fi

cat > /tmp/boostrap_upboard.sh <<EOM
set -e
set -u
echo "up" | sudo -S ls &>/dev/null
sudo apt update
sudo apt full-upgrade -y
sudo apt install gnupg wget -y

wget 'https://packages.chef.io/stable/debian/8/chef_12.14.60-1_amd64.deb' -O chef.deb
sudo dpkg -i chef.deb
rm chef.deb
EOM

scp /tmp/boostrap_upboard.sh up@"$ip":
ssh up@"$ip" bash boostrap_upboard.sh
