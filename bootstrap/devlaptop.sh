#!/bin/bash

set -eu
set -o pipefail

CHEF_WORKSTATION='https://packages.chef.io/files/stable/chef-workstation/0.18.3/ubuntu/20.04/chef-workstation_0.18.3-1_amd64.deb'
CWDEB='/tmp/chef-workstation.deb'

sudo apt update
sudo apt full-upgrade -y
sudo apt install python3-pip -y

trap 'rm -rf $CWDEB' EXIT
curl -L "$CHEF_WORKSTATION" -o "$CWDEB"
sudo dpkg -i "$CWDEB"

pip3 install --user pipenv

pipenv="$(python3 -m site --user-base)"/bin/pipenv
"$pipenv" install
"$pipenv" run -- fab run
