#!/bin/bash
#
# AIM: boostrap a distrobox/toolbox container
# this assumes ubuntu-24.04
PYTHON_VERSION=3.12.2

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

if command -v cinc-shell; then
  echo "* cinc-workstation already installed"
else
  echo "* Installing cinc-workstation (might be very slow)"
  curl -L https://omnitruck.cinc.sh/install.sh | sudo bash -s -- -P cinc-workstation -v 22
fi

echo "* Installing rbenv and cinc-workstation plugin"
apt_get install rbenv git
if [ ! -d "$(rbenv root)"/plugins ]; then
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

if [ ! -d ~/.pyenv ]; then
  git clone https://github.com/pyenv/pyenv.git ~/.pyenv
  exec "$SHELL"
  rbenv shell cinc-workstation
fi

if ! pyenv version | grep $PYTHON_VERSION -q; then
  echo "* Installing python $PYTHON_VERSION"
  apt_get install make build-essential libssl-dev zlib1g-dev libbz2-dev libreadline-dev libsqlite3-dev wget curl llvm libncurses5-dev xz-utils tk-dev libxml2-dev libxmlsec1-dev libffi-dev
  pyenv install $PYTHON_VERSION
fi

echo "* Entering pyenv shell"
export PYENV_ROOT="$HOME/.pyenv"
[[ -d $PYENV_ROOT/bin ]] && export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init -)"
pyenv shell $PYTHON_VERSION
pip install -U pip pipenv

[ ! -L .git/hooks/pre-commit ] && ln -s hooks/pre-commit.sh .git/hooks/pre-commit
[ ! -L .git/hooks/pre-push ] && ln -s hooks/pre-push.sh .git/hooks/pre-push
pipenv install
# we need to fix ubuntu's gid for the ubuntu group to avoid a clash
getent group ubuntu | grep 1010 -q || sudo groupmod -g 1010 ubuntu

# so it might be that chef has been run in another container, as such
# some cache files are present but the binary is not installed
# this will confuse chef as it won't run the unpack.
# let's remove the cached files?
sudo rm -fv local-mode-cache/cache/go*

pipenv run ./run -H localhost_toolbox
