paths = node["calculon"]["storage"]["paths"]
root = paths["root"]
username = node["user"]["login"]
group = node["user"]["group"]

data_volume = "#{root}/containers-#{username}"
directory data_volume do
  owner username
  group group
  mode "0775"
end

%w{.config .config/containers}.each do |dir|
  directory "/home/#{username}/#{dir}" do
    owner username
    group username
    mode "0755"
  end
end

template "/home/#{username}/.config/containers/storage.conf" do
  source "podman_storage.conf.erb"
  owner username
  group username
  mode "0755"
  variables(
    volume: data_volume,
    driver: "btrfs",
    runroot: "#{data_volume}/run",
    graphroot: "#{data_volume}/graph"
  )
end

package "distrobox"
