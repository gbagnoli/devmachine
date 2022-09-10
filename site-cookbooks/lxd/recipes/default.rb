package "lxd"
package "lxd-client"
package "jq"

directory node["lxd"]["config_dir"] do
  recursive true
end

node.override["os-hardening"]["network"]["forwarding"] = true
node.override["os-hardening"]["network"]["ipv6"]["enable"] = true
node.override["sysctl"]["params"]["net"]["ipv4"]["ip_forward"] = 1
node.override["sysctl"]["params"]["net"]["ipv6"]["conf"]["all"]["forwarding"] = 1
node.override["sysctl"]["params"]["net"]["ipv6"]["conf"]["all"]["disable_ipv6"] = 0

[
  { domain: "*", type: "soft", item: "nofile", value: "1048576" },
  { domain: "*", type: "hard", item: "nofile", value: "1048576" },
  { domain: "root", type: "soft", item: "nofile", value: "1048576" },
  { domain: "root", type: "hard", item: "nofile", value: "1048576" },
  { domain: "*", type: "soft", item: "memlock", value: "unlimited" },
  { domain: "*", type: "hard", item: "memlock", value: "unlimited" },
].each do |conf|
  limit "#{conf[:path]}-#{conf[:domain]}-#{conf[:type]}-#{conf[:item]}" do
    domain conf[:domain]
    path '/etc/security/limits.conf'
    type conf[:type]
    item conf[:item]
    value conf[:value]
    comment 'https://lxd.readthedocs.io/en/latest/production-setup/'
  end
end

[
  { key: "fs.inotify.max_queued_events", value: "1048576" },
  { key: "fs.inotify.max_user_instances", value: "1048576" },
  { key: "fs.inotify.max_user_watches", value: "1048576" },
  { key: "vm.max_map_count", value: "262144" },
  { key: "kernel.dmesg_restrict", value: "1" },
  { key: "net.ipv4.neigh.default.gc_thresh3", value: "8192" },
  { key: "net.ipv6.neigh.default.gc_thresh3", value: "8192" },
  { key: "kernel.keys.maxkeys", value: "2000" },
  { key: "net.core.netdev_max_backlog", value: "182757" },
].each do |ctl|
  sysctl ctl[:key] do
    value ctl[:value]
  end
end
