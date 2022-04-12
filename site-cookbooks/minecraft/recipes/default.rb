openjdk_pkg_install '17'
package 'build-essential'

config = node["minecraft"]
if config["server"]["properties"]["rcon.password"].empty?
  raise Exception, "Please set rcon.password property in secrets"
end

group config["group"] do
  gid config["gid"]
  users config["group_users"]
end

user config["user"] do
  uid config["uid"]
  gid config["gid"]
  home config["data_directory"]
  manage_home true
end

["", "backups", "tools", "server", "server/plugins", "tools/spigot"].each do |dir|
  directory "#{config["data_directory"]}/#{dir}" do
    owner config["user"]
    group config["group"]
    mode 0o755
  end
end

build_d = "#{config["data_directory"]}/tools/spigot"
server_d = "#{config["data_directory"]}/server"
plugin_d = "#{server_d}/plugins"

# compile spigot
remote_file "#{build_d}/BuildTools.jar" do
  source config["spigot"]["buildtools"]["url"]
  owner config["user"]
  group config["group"]
  notifies :run, "execute[compile_spigot]", :immediately
end

execute "compile_spigot" do
  command "sudo -i -u #{config["user"]} /bin/bash -l -c "\
          "'cd #{build_d} && java -jar BuildTools.jar --rev latest'"
  notifies :run, "execute[install_spigot]", :immediately
  action :nothing
end

execute "install_spigot" do
  command "install -T -m 0755 -o #{config["user"]} -g #{config["group"]} "\
          "-p spigot-*.jar  #{server_d}/server.jar"
  cwd build_d
  action :nothing
  notifies :restart, "systemd_unit[minecraft.service]", :delayed
end

remote_file "#{plugin_d}/floodgate-spigot.jar" do
  source config["spigot"]["floodgate"]["url"]
  owner config["user"]
  group config["group"]
  notifies :restart, "systemd_unit[minecraft.service]", :delayed
end

mcrcon_d = "#{config["data_directory"]}/tools/mcrcon"
git mcrcon_d do
  repository config["mcrcon"]["repository"]
  revision config["mcrcon"]["revision"]
  action :sync
  user config["user"]
  group config["group"]
  notifies :run, "execute[compile_mcrcon]", :immediately
end

execute "compile_mcrcon" do
  action :nothing
  command "gcc -std=gnu11 -pedantic -Wall -Wextra -O2 -s -o mcrcon mcrcon.c"
  user config["user"]
  cwd mcrcon_d
end

file "/usr/local/bin/mcrcon" do
  owner 'root'
  group 'root'
  mode 0o755
  content ::File.read("#{mcrcon_d}/mcrcon")
  action :create
end


file "#{server_d}/eula.txt" do
  content <<~EOH
    # generated by chef
    eula=true
  EOH
end

template "#{server_d}/server.properties" do
  source "server.properties.erb"
  owner config["user"]
  group config["group"]
  mode 0o640
  variables properties: config["server"]["properties"]
  action :create_if_missing
end

rcon_addr = config["server"]["properties"]["server-ip"]
rcon_addr = "127.0.0.1" if rcon_addr.empty?
rconp = config["server"]["properties"]["rcon.password"]
rcon_port = config["server"]["properties"]["rcon.port"]

template "/usr/local/bin/rcon" do
  source "minecraft_rcon.sh.erb"
  mode 0o750
  user config["user"]
  group config["group"]
  variables(
    rcon_addr: rcon_addr,
    rcon_password: rconp,
    rcon_port: rcon_port,
  )
end

template "/usr/local/bin/minecraft-backup" do
  source "minecraft_backup.sh.erb"
  mode 0o750
  user config["user"]
  group config["group"]
  variables(
    data_directory: config["data_directory"],
  )
end

cron "minecraft backup" do
  command "/usr/local/bin/minecraft-backup"
  minute "15"
  hour "5"
  user config["user"]
end

jconf = config["server"]["java"]

systemd_unit "minecraft.service" do
  content <<~EOU
    [Unit]
    Description=Minecraft Server
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
    WorkingDirectory=#{server_d}
    ExecStart=/usr/bin/java -Xmx#{jconf["xmx"]} -Xms#{jconf["xms"]} -XX:+UseG1GC -XX:ParallelGCThreads=#{jconf["gcthreads"]} -jar server.jar nogui
    ExecStop=/usr/local/bin/mcrcon -H 127.0.0.1 -P #{rcon_port} -p #{rconp}  stop
    ExecStop=/bin/bash -c "while ps -p $MAINPID > /dev/null; do /bin/sleep 1; done"
    [Install]
    WantedBy=multi-user.target
  EOU
  action %i[create enable start]
end
