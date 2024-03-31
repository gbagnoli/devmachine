#!/bin/bash
set -eu
set -o pipefail

if [ $# -ne 1 ]; then
  echo >&2 "Usage: $0 <ubuntu|centos|fedora>"
  exit 1
fi

if [[ "$1" == "ubuntu" ]]
then
  CHEF_WORKSTATION='https://packages.chef.io/files/stable/chef-workstation/21.10.640/ubuntu/20.04/chef-workstation_21.10.640-1_amd64.deb'
  CWDEB='/tmp/chef-workstation.deb'
  APTGET="sudo apt-get -y -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confnew"

  sudo apt-get update
  $APTGET dist-upgrade
  $APTGET install python3-pip rbenv git

  if dpkg -l chef-workstation &>/dev/null; then
    echo "chef-workstation already installed, skipping"
  else
    trap 'rm -rf $CWDEB' EXIT
    curl -L "$CHEF_WORKSTATION" -o "$CWDEB"
    sudo dpkg -i "$CWDEB"
  fi
elif [[ "$1" == "centos" ]] || [[ "$1" == "fedora" ]] ; then
  CHEF_WORKSTATION="https://packages.chef.io/files/stable/chef-workstation/21.10.640/el/8/chef-workstation-21.10.640-1.el8.x86_64.rpm"
  CWRPM='/tmp/chef-workstation.rpm'
  DNF="sudo dnf"

  $DNF upgrade -y
  if $DNF list chef-workstation &>/dev/null; then
    echo "chef-workstation already installed, skipping"
  else
    trap 'rm -rf $CWRPM' EXIT
    curl -L "$CHEF_WORKSTATION" -o "$CWRPM"
    $DNF install -y "$CWRPM"
  fi
   $DNF install -y python3-pip
   $DNF install -y libxcrypt-compat
else
  echo >&2 "Invalid distribution '$1'"
  exit 2
fi
