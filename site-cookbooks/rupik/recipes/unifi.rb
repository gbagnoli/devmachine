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
      Network=host
      Image=unifi.image
      Volume=/srv/unifi:/unifi
      User=unifi
      Environment=TZ=Europe/Madrid
    },
    Service: [
      "Restart=always",
    ],
    Unit: [
      "Description=Unifi Controller",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
end
