podman_image "syncthing" do
  config(
    Image: ["Image=docker.io/syncthing/syncthing"],
  )
end

podman_container "syncthing" do
  config(
    Container: %W{
      Image=syncthing.image
      Environment=PUID=#{node["calculon"]["data"]["uid"]}
      Environment=PGID=#{node["calculon"]["data"]["gid"]}
      PublishPort=[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]:8384:8384
      PublishPort=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}:8384:8384
      PublishPort=[::]:22000:22000/tcp
      PublishPort=[::]:22000:22000/udp
      PublishPort=22000:22000/tcp
      PublishPort=22000:22000/udp
      Volume=#{node["calculon"]["storage"]["paths"]["sync"]}:/var/syncthing
      HostName=sync.tigc.eu
      Network=calculon.network
    },
    Service: %w{
      Restart=always
    },
    # description has spaces, use a normal list
    Unit: [
      "Description=Start Syncthing file synchronization",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

calculon_firewalld_port "syncthing" do
  port %w{22000/tcp 22000/udp}
end

calculon_vhost "calculon.tigc.eu" do
  server_name "calculon.tigc.eu"
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port 8384
  oauth2_proxy(
    emails: node["calculon"]["oauth2_proxy"]["secrets"]["syncthing_authenticated_emails"],
    port: 4000
  )
  cloudflare true
  action :delete
end

calculon_www_upstream "/sync" do
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port 8384
end
