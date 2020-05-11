#!/bin/bash
set -eu
set -o pipefail

grep -q "giacomo" /etc/passwd || (
  echo "Creating user giacomo"
  groupadd -g 2000 giacomo
  useradd -u 2000 -g giacomo -G sudo -m -s /bin/bash giacomo
  echo "giacomo        ALL=(ALL)       NOPASSWD: ALL" > /etc/sudoers.d/giacomo
)
sudo -i -u vagrant which pipenv || sudo -i -u vagrant pip3 install --user pipenv
pipenv="$(sudo -i -u vagrant python3 -m site --user-base)"/bin/pipenv
sudo -i -u vagrant bash -c "cd /vagrant && $pipenv install"
sudo -i -u vagrant bash -c "cd /vagrant && CHEF_LICENSE=accept pipenv run -- fab run -H localhost_devmachinetest"
