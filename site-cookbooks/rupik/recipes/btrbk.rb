include_recipe "rupik::mounts"
include_recipe "btrbk"

directory "/srv/snapshots/sync" do
  recursive true
end

directory "/etc/btrbk" do
  mode "0755"
end

file "/etc/btrbk/btrbk.conf" do
  mode "0644"
  content <<~EOH
    timestamp_format        long
    snapshot_preserve_min   6h
    snapshot_preserve       24h 31d 6m

    volume /srv
      snapshot_dir snapshots/sync
      subvolume sync
  EOH
end

file "/etc/cron.hourly/btrbk" do
  content <<~EOH
    #!/bin/sh
    exec /usr/bin/btrbk -q run
  EOH
  mode "0755"
end
