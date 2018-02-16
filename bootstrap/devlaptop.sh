#!/bin/bash

set -eu
set -o pipefail

CHEFDK='https://packages.chef.io/files/stable/chefdk/2.4.17/ubuntu/16.04/chefdk_2.4.17-1_amd64.deb'

sudo apt update
sudo apt full-upgrade -y
sudo apt install python-pip -y

curl -L "$CHEFDK" -o chefdk.deb
sudo dpkg -i chefdk.deb
rm -rf chefdk.deb

pip install --user pipenv

pipenv="$(python -m site --user-base)"/bin/pipenv
"$pipenv" install
"$pipenv" run -- fab run
