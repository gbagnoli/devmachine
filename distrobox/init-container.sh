#!/bin/bash
#
# boostrap a distrobox ubuntu container

set -euo pipefail
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

# shellcheck source-path=SCRIPTDIR
# shellcheck source=../lib/utils.sh
source "$(realpath "$SCRIPT_DIR/../lib/utils.sh")"
pushd "$SCRIPT_DIR"/../../ &>/dev/null

exit_if_not_running_in_distrobox

apt_get () {
  DEBIAN_FRONTEND=noninteractive \
  sudo apt-get \
  -o Dpkg::Options::=--force-confold \
  -o Dpkg::Options::=--force-confdef \
  -y --allow-downgrades --allow-remove-essential --allow-change-held-packages "$@"
}
apt_get update
apt_get dist-upgrade

if [ -d /opt/cinc-workstation/ ]; then
  echo "* cinc-workstation already installed"
else
  echo "* Installing cinc-workstation (might be very slow)"
  curl -L https://omnitruck.cinc.sh/install.sh | sudo bash -s -- -P cinc-workstation -v 25
fi

echo "* Installing utilities"
apt_get install rbenv lsb-release git vim vim-nox neovim

if [ ! -d "$(rbenv root)"/plugins ]; then
  echo "* Installing rbenv chefdk plugin"
  mkdir -p "$(rbenv root)"/plugins
  git clone -b cinc-workstation https://github.com/david-alpert-nl/rbenv-chef-workstation.git "$(rbenv root)"/plugins/ruby-build
  mkdir -p "$(rbenv root)/versions/cinc-workstation"
fi

user=giacomo
group=giacomo

# we need to fix ubuntu's gid for the ubuntu group to avoid a clash
getent group ubuntu | grep 1010 -q || sudo groupmod -g 1010 ubuntu
getent group $group &>/dev/null || sudo groupadd -g 1000 $group
getent group $group | grep 1000 -q || sudo groupmod -g 1000 $group

home="$(getent passwd "$user" | cut -d: -f6)"
chgrp $group "$home"/.ssh
chmod 700 "$home"/.ssh
