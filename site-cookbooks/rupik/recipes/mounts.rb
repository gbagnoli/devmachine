package "btrfs-progs"

return if node["rupik"]["skip_mounts"]

root = node["rupik"]["storage"]["path"]
dev = node["rupik"]["storage"]["dev"]
directory root

mount root do
  device dev
  fstype "btrfs"
  action %i(mount enable)
end
