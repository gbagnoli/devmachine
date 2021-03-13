# frozen_string_literal: true

return if node["rupik"]["skip_mounts"]

directory "/srv"

mount "/srv" do
  device "/dev/sda3"
  fstype "btrfs"
  action %i[mount enable]
end
