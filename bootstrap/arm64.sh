#!/bin/bash

set -u
set -e
set -o pipefail

# looks like chef doesn't package for arm64.
# there is some packages for armhf here: https://mattray.github.io/arm/
# let's install for armhf using multiarch.

if [ $# -lt 2 ] ; then
  echo >&2 "usage: $0 <host> <chef_deb_path> [user]"
  exit 1
fi

host="$1"
chef_deb_path="$(realpath "$2")"
chef_deb="$(basename "$chef_deb_path")"
user="${3:-$USER}"
here="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
parent="$(realpath "$here/..")"

if [ ! -f "$chef_deb_path" ]; then
  echo >&2 "Cannot find deb file at $chef_deb"
  exit 1
fi

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
sudo dpkg --add-architecture armhf
sudo apt update
sudo apt full-upgrade -y
sudo apt install wget -y
sudo apt-get install apt-transport-https
sudo apt-get install libc6:armhf -y
sudo dpkg -i $chef_deb
rm -f $scriptname $chef_deb
EOM

scp "$chef_deb_path" "$user"@"$host":"$chef_deb"
scp "$script" "$user"@"$host":
# shellcheck disable=SC2029
ssh "$user"@"$host" bash "$scriptname"

pushd "$parent" &>/dev/null
./run -H "$host"
