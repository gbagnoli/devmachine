#!/bin/bash

set -eu
set -o pipefail

CHEFDK='https://packages.chef.io/files/stable/chefdk/1.6.1/ubuntu/16.04/chefdk_1.6.1-1_amd64.deb'

sudo apt update
sudo apt full-upgrade -y
sudo apt install python-pip -y

curl -L "$CHEFDK" -o chefdk.deb
sudo dpkg -i chefdk.deb
rm -rf chefdk.deb

pip install --user pipenv

# omit host, as it will be autodetected by the fabfile.
"$(python -m site --user-base)"/bin/pipenv run -- fab run
