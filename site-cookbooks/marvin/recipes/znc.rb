apt_repository "znc" do
  uri "ppa:teward/znc"
end

package "znc"
zncd = "/var/lib/znc"

group "znc"

user "znc" do
  system true
  shell "/bin/false"
  home zncd
  gid "znc"
end

directory "/var/lib/znc" do
  user "znc"
  group "znc"
  mode "0750"
end

# directory must be filled manually for now
# TODO automate znc install

systemd_unit "znc.service" do
  content <<~EOU
    [Unit]
    Description=ZNC, an advanced IRC bouncer
    After=network-online.target

    [Service]
    ExecStart=/usr/bin/znc -f --datadir=#{zncd}
    User=znc

    [Install]
    WantedBy=multi-user.target
  EOU
  action %i(create enable)
end
