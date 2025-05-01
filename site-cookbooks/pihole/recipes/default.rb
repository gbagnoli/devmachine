root = node["pihole"]["paths"]["root"]
directory root
directory "#{root}/conf"
directory "#{root}/dnsmasq.d"

directory node["pihole"]["paths"]["logs"] do
  mode "0755"
end

podman_image "pihole" do
  config(
    Image: ["Image=#{node["pihole"]["image"]["repository"]}/pihole/pihole:#{node["pihole"]["image"]["tag"]}"],
  )
end

unless node["pihole"]["dns"]["custom"].nil?
  template "/etc/pihole/conf/custom.list" do
    mode "0640"
    variables(custom: node["pihole"]["dns"]["custom"],
              fqdn: node["pihole"]["dns"]["custom_domain"])
    source "custom.list.erb"
    notifies :restart, "service[pihole]", :delayed
  end
end

conf = node["pihole"]["conf"]
extra_config = conf.sort.map { |k,v| "Environment=FTLCONF_#{k}=#{v}" }.join("\n")
container_conf = node["pihole"]["container"].sort.map do |k, v|
  "#{k}=#{v}"
end.join("\n")

extra_config = %W{
  #{container_conf}
  #{extra_config}
}

podman_container "pihole" do
  config(
    Container: %W{
      Image=pihole.image
      Volume=#{root}/conf:/etc/pihole
      Volume=#{root}/dnsmasq.d/etc/dnsmasq.d
      Volume=#{node["pihole"]["paths"]["logs"]}:/var/log/pihole
      Environment=TZ=#{node["pihole"]["tz"]}
    } + extra_config,
    Service: [
      "Restart=always",
    ],
    Unit: [
      "Description=Pi. Hole",
      "After=network-online.target",
      "Before=tailscale.service",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
end

if node["pihole"]["enable_datadog"]
  datadog_integration "datadog-pihole" do
    version "3.14.1"
    third_party true
  end
end

service "pihole" do
  action :start
end
