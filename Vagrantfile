# -*- mode: ruby -*-
# vi: set ft=ruby :

Vagrant.configure("2") do |config|
  # The most common configuration options are documented and commented below.
  # For a complete reference, please see the online documentation at
  # https://docs.vagrantup.com.

  # Every Vagrant development environment requires a box. You can search for
  # boxes at https://vagrantcloud.com/search.
  config.vm.define "ubuntu" do |ubuntu|
    ubuntu.vm.box = "ubuntu/jammy64"

    # Share an additional folder to the guest VM. The first argument is
    # the path on the host to the actual folder. The second argument is
    # the path on the guest to mount the folder. And the optional third
    # argument is a set of non-required options.
    # ubuntu.vm.synced_folder "../data", "/vagrant_data"

    ubuntu.vm.provider "virtualbox" do |vb|
      #   # Display the VirtualBox GUI when booting the machine
      #   vb.gui = true
      #
      #   # Customize the amount of memory on the VM:
       vb.memory = "1024"
    end
    ubuntu.vm.provision :host_shell do |hs|
      command = "sudo rm -rf #{__dir__}/local-mode-cache"
      hs.inline = command
    end
    ubuntu.vm.provision "shell", path: "bootstrap/install_chef.sh", args: %w{ubuntu}
    ubuntu.vm.provision "shell", path: "bootstrap/vagrant.sh"
  end

  config.vm.define "calculon" do |calculon|
    calculon.vm.box = "fedora/39-cloud-base"

    # Share an additional folder to the guest VM. The first argument is
    # the path on the host to the actual folder. The second argument is
    # the path on the guest to mount the folder. And the optional third
    # argument is a set of non-required options.
    # calculon.vm.synced_folder "../data", "/vagrant_data"

    calculon.vm.provider "virtualbox" do |vb|
      #   # Display the VirtualBox GUI when booting the machine
      #   vb.gui = true
      #
      #   # Customize the amount of memory on the VM:
       vb.memory = "1024"
    end
    calculon.vm.provision :host_shell do |hs|
      command = "sudo rm -rf #{__dir__}/local-mode-cache"
      hs.inline = command
    end
    calculon.vm.provision "shell", path: "bootstrap/install_chef.sh", args: %w{fedora}
    calculon.vm.provision "shell", path: "bootstrap/vagrant.sh", args: %w{localhost_calculon}
  end
end
