include_recipe "calculon::nginx"

lego_image = "docker.io/goacme/lego"
podman_image "lego" do
  config(
    Image: ["Image=#{lego_image}"],
  )
end

group "lego" do
  comment "ACME lego"
  system true
  gid node["calculon"]["acme"]["lego"]["gid"]
end

user "lego" do
  comment "ACME lego"
  system true
  shell "/bin/nologin"
  uid node["calculon"]["acme"]["lego"]["uid"]
  gid node["calculon"]["acme"]["lego"]["gid"]
end

directory node["calculon"]["acme"]["certs_dir"] do
  owner "lego"
  group "lego"
  mode "0755"
end

certificates_d = "#{node["calculon"]["acme"]["certs_dir"]}/certificates"
directory certificates_d  do
  owner "lego"
  group "lego"
  mode "2750"
end

execute "setfacl_#{certificates_d}" do
  command "setfacl -R -d -m g::r -m o::- #{certificates_d}"
  user "root"
  not_if "getfacl #{certificates_d} 2>/dev/null | grep 'default:' -q"
end

file "#{node["calculon"]["storage"]["paths"]["www"]}/etc/default.d/lego.conf" do
  content <<~EOH
    location /.well-known/acme-challenge/ {
        proxy_pass http://127.0.0.1:#{node["calculon"]["acme"]["lego"]["port"]};
        proxy_set_header Host $host;
    }
  EOH
  notifies :reload, "service[nginx]", :immediately
end

if node["calculon"]["acme"]["lego"]["email"].nil?
  raise 'email not set for ACME - node["calculon"]["acme"]["lego"]["email"]'
end

file "/usr/local/bin/lego" do
  owner "root"
  group "root"
  mode "0750"
  content <<~EOH
    #!/bin/bash
    set -eu
    podman run \
    --rm \
    --net calculon \
    --read-only \
    -q \
    --user #{node["calculon"]["acme"]["lego"]["uid"]} \
    -p #{node["calculon"]["acme"]["lego"]["port"]}:#{node["calculon"]["acme"]["lego"]["port"]} \
    -v #{node["calculon"]["acme"]["certs_dir"]}:#{node["calculon"]["acme"]["certs_dir"]} \
    -w #{node["calculon"]["acme"]["certs_dir"]} \
    -e /usr/bin/lego \
    #{lego_image} \
    "$@"
  EOH
end

template "/usr/local/bin/lego_periodic_renew" do
  source "lego_periodic_renew.sh.erb"
  mode "0750"
  user "root"
  group "root"
  variables(
    lego_path: node["calculon"]["acme"]["certs_dir"],
    lego_port: node["calculon"]["acme"]["lego"]["port"],
    email: node["calculon"]["acme"]["lego"]["email"],
    lego: "/usr/local/bin/lego",
    key_type: node["calculon"]["acme"]["key_type"],
    renew_days: node["calculon"]["acme"]["renew_days"]
  )
end

template "/usr/local/bin/lego_request" do
  source "lego_request.sh.erb"
  mode "0750"
  user "root"
  group "root"
  variables(
    lego_path: node["calculon"]["acme"]["certs_dir"],
    lego_port: node["calculon"]["acme"]["lego"]["port"],
    email: node["calculon"]["acme"]["lego"]["email"],
    lego: "/usr/local/bin/lego",
    key_type: node["calculon"]["acme"]["key_type"],
  )
end

systemd_unit "lego_renew_certificates.service" do
	content <<~EOH
   [Unit]
   Description=Renew certificates from letsencrypt

   [Service]
   Type=oneshot
   ExecStart=/usr/local/bin/lego_periodic_renew
   User=root
   Group=systemd-journal
	EOH
  action %i(create enable)
end

systemd_unit "lego_renew_certificates.timer" do
  content <<~EOH
    [Unit]
    Description=Renew certificates

    [Timer]
    Unit=lego_renew_certificates.service
    Persistent=true

    # instead, use a randomly chosen time:
    OnCalendar=*-*-* 6:13
    # add extra delay, here up to 1 hour:
    RandomizedDelaySec=1h

    [Install]
    WantedBy=timers.target
  EOH
  action %i(create enable start)
end
