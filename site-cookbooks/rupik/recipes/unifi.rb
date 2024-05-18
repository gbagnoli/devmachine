group 'unifi' do
  gid '2666'
end

user 'unifi' do
  uid '2666'
  gid '2666'
  home '/srv/unifi/'
  shell '/bin/sh'
end

%w[/srv/unifi /srv/unifi/data/ /srv/unifi/data/logs].each do |dir|
  directory dir do
    mode "750"
    action :create
  end
end

podman_image "unifi" do
  config(
    Image: ["Image=docker.io/jacobalberty/unifi:latest"],
  )
end

podman_container "unifi" do
  config(
    Container: %w{
      Network=bridge
      Image=unifi.image
      Volume=/srv/unifi:/unifi
      User=unifi
      Environment=TZ=Europe/Madrid
      PublishPort=8080:8080/tcp
      PublishPort=8443:8443/tcp
      PublishPort=3748:3748/udp
    },
    Service: [
      "Restart=always",
    ],
    Unit: [
      "Description=Unifi Controller",
      "Wants=network.target",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
end
