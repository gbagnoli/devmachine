#!/bin/bash
#
# AIM: boostrap a distrobox/toolbox container
# this assumes ubuntu

set -euo pipefail
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
pushd "$SCRIPT_DIR"/../ &>/dev/null

apt_get () {
  DEBIAN_FRONTEND=noninteractive \
  sudo apt-get \
  -o Dpkg::Options::=--force-confold \
  -o Dpkg::Options::=--force-confdef \
  -y --allow-downgrades --allow-remove-essential --allow-change-held-packages "$@"
}
apt_get update
apt_get dist-upgrade

if command -v cinc-shell; then
  echo "* cinc-workstation already installed"
else
  echo "* Installing cinc-workstation (might be very slow)"
  curl -L https://omnitruck.cinc.sh/install.sh | sudo bash -s -- -P cinc-workstation -v 25
fi

echo "* Installing utilities"
apt_get install rbenv lsb-release git
export PATH="$PATH:/home/linuxbrew/.linuxbrew/bin"
if command -v brew; then
  echo "* brew already installed"
else
  echo "* Installing brew"
  NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

brew install shellcheck uv
if [ ! -d "$(rbenv root)"/plugins ]; then
  echo "* Installing rbenv chefdk plugin"
  mkdir -p "$(rbenv root)"/plugins
  git clone -b cinc-workstation https://github.com/david-alpert-nl/rbenv-chef-workstation.git "$(rbenv root)"/plugins/ruby-build
  mkdir -p "$(rbenv root)/versions/cinc-workstation"
fi

echo "* Entering rbenv shell"
set +u
eval "$(rbenv init -)"
rbenv shell cinc-workstation
set -u
rbenv rehash

echo "* Creating uv venv"
uv venv --allow-existing
uv sync --dev

[ ! -L .git/hooks/pre-commit ] && ln -s "$(pwd)/hooks/pre-commit.sh" .git/hooks/pre-commit
[ ! -L .git/hooks/pre-push ] && ln -s "$(pwd)/hooks/pre-push.sh" .git/hooks/pre-push
# we need to fix ubuntu's gid for the ubuntu group to avoid a clash
getent group ubuntu | grep 1010 -q || sudo groupmod -g 1010 ubuntu

# so it might be that chef has been run in another container, as such
# some cache files are present but the binary is not installed
# this will confuse chef as it won't run the unpack.
# let's remove the cached files?
sudo rm -fv local-mode-cache/cache/{go,fzf,zoxide,rust-analyzer}*

uv run ./run -H localhost_toolbox
