#!/bin/bash
set -eu
set -o pipefail

role="${1:-localhost_devmachinetest}"

group="wheel"
if getent group sudo &>/dev/null; then
  group="sudo"
fi

getent passwd giacomo &>/dev/null || (
  echo "Creating user giacomo"
  getent group giacomo &>/dev/null || groupadd -g 2000 giacomo
  useradd -u 2000 -g giacomo -G "$group" -m -s /bin/bash giacomo
  echo "giacomo        ALL=(ALL)       NOPASSWD: ALL" > /etc/sudoers.d/giacomo
)
sudo -i which pipenv || sudo -i pip3 install --user pipenv
pipenv="$(sudo -i python3 -m site --user-base)"/bin/pipenv
sudo -i bash -c "cd /vagrant && $pipenv install"
sudo -i bash -c "cd /vagrant && CHEF_LICENSE=accept $pipenv run -- fab run -H $role"
