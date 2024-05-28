conf = node["syncthing"]["podman"]

podman_image "syncthing" do
  config(
    Image: ["Image=docker.io/syncthing/syncthing"],
  )
end

ipv4_gui = conf["ipv4"]["gui"].empty? ? "" : "#{conf["ipv4"]["gui"]}:"
ipv4_service = conf["ipv4"]["service"].empty? ? "" : "#{conf["ipv4"]["service"]}:"

container_conf = %W{
      Image=syncthing.image
      PublishPort=[#{conf["ipv6"]["gui"]}]:8384:8384
      PublishPort=#{ipv4_gui}8384:8384
      PublishPort=[#{conf["ipv6"]["service"]}]:22000:22000/tcp
      PublishPort=[#{conf["ipv6"]["service"]}]:22000:22000/udp
      PublishPort=#{ipv4_service}22000:22000/tcp
      PublishPort=#{ipv4_service}22000:22000/udp
      Volume=#{conf["directory"]}:/var/syncthing
}

container_conf << "Environment=PUID=#{conf["uid"]}" unless conf["uid"].nil?
container_conf << "Environment=PGID=#{conf["gid"]}" unless conf["gid"].nil?

container_conf.concat conf["extra_conf"]

podman_container "syncthing" do
  config(
    Container: container_conf,
    Service: %w{
      Restart=always
    },
    # description has spaces, use a normal list
    Unit: [
      "Description=Start Syncthing file synchronization",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end
