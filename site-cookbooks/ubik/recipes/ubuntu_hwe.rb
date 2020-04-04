# frozen_string_literal: true

return unless platform? "ubuntu"

if node["lsb"]["codename"] == "xenial"
  %w[linux-generic-hwe-16.04 xserver-xorg-hwe-16.04].each do |pkg|
    apt_package pkg do
      options "--install-recommends"
    end
  end
end
