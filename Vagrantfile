# -*- mode: ruby -*-
# vi: set ft=ruby :
#
puts "Removing local-mode-cache folder (requires sudo)"
command = "sudo rm -rf #{__dir__}/local-mode-cache"
puts "running: #{command}"
system(command)

Vagrant.configure("2") do |config|
  # The most common configuration options are documented and commented below.
  # For a complete reference, please see the online documentation at
  # https://docs.vagrantup.com.

  # Every Vagrant development environment requires a box. You can search for
  # boxes at https://vagrantcloud.com/search.
  config.vm.box = "chenhan/ubuntu-desktop-20.04"

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
  config.vm.provision "shell", path: "bootstrap/install_chef.sh"
  config.vm.provision "shell", path: "bootstrap/vagrant.sh"
end
