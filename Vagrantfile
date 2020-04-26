# -*- mode: ruby -*-
# vi: set ft=ruby :

Vagrant.configure("2") do |config|
  # The most common configuration options are documented and commented below.
  # For a complete reference, please see the online documentation at
  # https://docs.vagrantup.com.

  # Every Vagrant development environment requires a box. You can search for
  # boxes at https://vagrantcloud.com/search.
  config.vm.box = "fasmat/ubuntu2004-desktop"

  # Share an additional folder to the guest VM. The first argument is
  # the path on the host to the actual folder. The second argument is
  # the path on the guest to mount the folder. And the optional third
  # argument is a set of non-required options.
  # config.vm.synced_folder "../data", "/vagrant_data"

  config.vm.provider "virtualbox" do |vb|
    #   # Display the VirtualBox GUI when booting the machine
    #   vb.gui = true
    #
    #   # Customize the amount of memory on the VM:
     vb.memory = "1024"
  end
  config.vm.provision "shell", inline: <<-SHELL
    set -eu
    set -o pipefail

    CHEFDK='https://packages.chef.io/files/stable/chefdk/4.7.73/ubuntu/18.04/chefdk_4.7.73-1_amd64.deb'

    apt-get update
    apt-get -y -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confnew upgrade
    apt-get install python3-pip rbenv git -y

    if dpkg -l chefdk &>/dev/null; then
      echo "Skipping chef install"
    else
      echo "Installing chef from $CHEFDK"
      curl -L "$CHEFDK" -o chefdk.deb
      dpkg -i chefdk.deb
      rm -rf chefdk.deb
    fi

    grep -q "giacomo" /etc/passwd || (
      echo "Creating user giacomo"
      groupadd -g 2000 giacomo
      useradd -u 2000 -g giacomo -G sudo -m -s /bin/bash giacomo
      echo "giacomo        ALL=(ALL)       NOPASSWD: ALL" > /etc/sudoers.d/giacomo
    )
    sudo -i -u vagrant which pipenv || sudo -i -u vagrant pip3 install --user pipenv
    pipenv="$(sudo -i -u vagrant python3 -m site --user-base)"/bin/pipenv
    sudo -i -u vagrant bash -c "cd /vagrant && $pipenv install"
    yes | sudo -i -u vagrant bash -c "cd /vagrant && $pipenv run -- fab run -H localhost_ubiktest"
  SHELL
end
