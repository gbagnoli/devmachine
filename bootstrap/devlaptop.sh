#!/bin/bash

set -eu
set -o pipefail

CHEFDK='https://packages.chef.io/files/stable/chefdk/4.7.73/ubuntu/18.04/chefdk_4.7.73-1_amd64.deb'

sudo apt update
sudo apt full-upgrade -y
sudo apt install python3-pip -y

curl -L "$CHEFDK" -o chefdk.deb
sudo dpkg -i chefdk.deb
rm -rf chefdk.deb

pip3 install --user pipenv

pipenv="$(python3 -m site --user-base)"/bin/pipenv
"$pipenv" install
"$pipenv" run -- fab run
