#!/bin/bash

set -eu
set -o pipefail

fail=0
[ ! -d data/ ] && fail=1
[ ! -d data/gpg ] && fail=1
[ ! -f data/ssh.tar.gz.gpg ] && fail=1
[ ! -d data/syncthing ] && fail=1

[ $fail -eq 1 ] && echo "Please copy gpg/ssh/syncthing data to $(pwd)/data" && exit 2

sudo apt update
sudo apt full-upgrade -y
sudo apt install gnupg -y
mkdir $HOME/.gnupg
cp data/*.gpg $HOME/.gnupg/
mkdir $HOME/.config
cp data/syncthing $HOME/.config/syncthing
gpg --decrypt --output data/ssh.tar.gz data/ssh.tar.gz.gpg 
tar -xf data/ssh.tar.gz -C ~/

curl -L https://packages.chef.io/stable/ubuntu/12.04/chefdk_0.17.17-1_amd64.deb -o chefdk.deb
dpkg -i chefdk.deb
rm -rf chefdk.deb
./run.sh
