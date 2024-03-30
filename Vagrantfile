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

    calculon.vm.synced_folder(
      ".", "/vagrant",
      type: "rsync",
      rsync__exclude: ["local-mode-cache", ".git"],
      rsync__auto: true,
      rsync__rsync_ownership: true,
    )

    ubuntu.vm.provider "virtualbox" do |vb|
       vb.memory = "1024"
    end
    ubuntu.vm.provision "shell", path: "bootstrap/install_chef.sh", args: %w{ubuntu}
    ubuntu.vm.provision "shell", path: "bootstrap/vagrant.sh"
  end

  config.vm.define "calculon" do |calculon|
    calculon.vm.box = "fedora/39-cloud-base"

    calculon.vm.synced_folder(
      ".", "/vagrant",
      type: "rsync",
      rsync__exclude: ["local-mode-cache", ".git"],
      rsync__auto: true,
      rsync__rsync_ownership: true,
    )

    calculon.vm.provider "virtualbox" do |vb|
       vb.memory = "1024"
    end

    calculon.vm.provision "shell", path: "bootstrap/install_chef.sh", args: %w{fedora}
    calculon.vm.provision "shell", path: "bootstrap/vagrant.sh", args: %w{localhost_calculon}
  end
end
