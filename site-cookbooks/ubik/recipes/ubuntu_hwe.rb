# frozen_string_literal: true

return unless platform? "ubuntu"

if node["lsb"]["codename"] == "bionic"
  %w[linux-generic-hwe-18.04 xserver-xorg-hwe-18.04].each do |pkg|
    apt_package pkg do
      options "--install-recommends"
    end
  end
end
