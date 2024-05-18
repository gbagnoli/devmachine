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

systemd_unit 'unifi.service' do
  content <<EOU
  [Unit]
  Description=Podman container-7d1ce893c913637f1977fe61011eb693a007ad93ba2080a64e59e7a42299a35f.service
  Documentation=man:podman-generate-systemd(1)
  Wants=network-online.target
  After=network-online.target
  RequiresMountsFor=/run/containers/storage

  [Service]
  Environment=PODMAN_SYSTEMD_UNIT=%n
  Restart=on-failure
  TimeoutStopSec=70
  ExecStart=/usr/bin/podman start 7d1ce893c913637f1977fe61011eb693a007ad93ba2080a64e59e7a42299a35f
  ExecStop=/usr/bin/podman stop -t 10 7d1ce893c913637f1977fe61011eb693a007ad93ba2080a64e59e7a42299a35f
  ExecStopPost=/usr/bin/podman stop -t 10 7d1ce893c913637f1977fe61011eb693a007ad93ba2080a64e59e7a42299a35f
  PIDFile=/run/containers/storage/btrfs-containers/7d1ce893c913637f1977fe61011eb693a007ad93ba2080a64e59e7a42299a35f/userdata/conmon.pid
  Type=forking

  [Install]
  WantedBy=multi-user.target default.target
EOU
  action %i(create enable start)
end
