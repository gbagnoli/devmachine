include_recipe 'minecraft::default'

config = node["minecraft"]
gdir = "#{config["data_directory"]}/geyser"
directory gdir do
  owner config["user"]
  group config["group"]
  mode 0o755
end

remote_file "#{gdir}/geyser.jar" do
  source config["geyser"]["url"]
  notifies :restart, "systemd_unit[geyser.service]", :delayed
end

jconf = config["server"]["java"]
systemd_unit "geyser.service" do
  content <<~EOU
    [Unit]
    Description=Geyser Server for Bedrock clients
    After=network.target
    [Service]
    User=#{config["user"]}
    Nice=1
    KillMode=none
    SuccessExitStatus=0 1
    ProtectHome=true
    ProtectSystem=full
    PrivateDevices=true
    NoNewPrivileges=true
    WorkingDirectory=#{gdir}
    ExecStart=/usr/bin/java -Xmx#{jconf["xmx"]} -Xms#{jconf["xms"]} -jar geyser.jar nogui
    [Install]
    WantedBy=multi-user.target
  EOU
  action %i[create enable start]
end
