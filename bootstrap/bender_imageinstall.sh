#!/bin/bash
set -eu
set -o pipefail

ssh -oBatchMode=yes root@bender ls &>/dev/null

if [ $? -ne 0 ] ; then
  echo >&2 "cannot connect via ssh to bender as user 'root': is openssh up and pub key setup?"
  exit 1
fi

cleanup() {
  rm -rf /tmp/autosetup_bender
}

trap cleanup EXIT

cat > /tmp/installimage_bender <<EOM
DRIVE1 /dev/sda
DRIVE2 /dev/sdb
SWRAID 1
SWRAIDLEVEL 1
BOOTLOADER grub
HOSTNAME bender.tigc.eu
PART / ext4 85G
PART swap swap 5G
PART /data btrfs all
IMAGE /root/.oldroot/nfs/install/../images/Ubuntu-1804-bionic-64-minimal.tar.gz
EOM

ssh-keygen -f "~/.ssh/known_hosts" -R "bender.tigc.eu"
scp /tmp/autosetup_bender root@bender:/autosetup
ssh root@bender installimage
