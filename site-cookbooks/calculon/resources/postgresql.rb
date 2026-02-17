resource_name :calculon_postgresql
provides :calculon_postgresql
unified_mode true

path_callback = {
  "should start with /" => lambda { |path|
    path.start_with?("/")
  },
}

property :port, [String, Integer], required: true
property :user, String, required: true
property :password, String, required: true
property :backup_path, [String, NilClass], callbacks: path_callback
property :database_path, [String, NilClass], callbacks: path_callback
property :podman_pod, [String, NilClass]
default_action :create

action :create do
  [database_path, backup_path].each do |dir|
    directory dir do
      owner node["calculon"]["data"]["username"]
      group node["calculon"]["data"]["group"]
      mode "2755"
    end
  end

  podman_container service_name do
    config(
      Container: [
        "Image=postgres.image",
        "Exec=-p #{new_resource.port}",
        "Environment=POSTGRES_DATABASE=#{new_resource.name}",
        "Environment=POSTGRES_USER=#{new_resource.user}",
        "Environment=POSTGRES_PASSWORD=#{new_resource.password}",
        "Volume=#{database_path}:/var/lib/postgresql",
        "HealthCmd=pg_isready -p #{new_resource.port} -U #{new_resource.user} -d #{new_resource.name}",
        "HealthInterval=5s",
        "HealthRetries=5",
      ] + pod_quadlet_config,
      Service: %w{
        Restart=always
      },
      Unit: [
        "Description=#{new_resource} Postgresql Database",
        "After=network-online.target",
      ],
      Install: [
        "WantedBy=multi-user.target default.target"
      ]
    )
  end

  template "/usr/local/bin/postgresql_backup_#{new_resource.name}" do
    source "postgresql_backup.erb"
    variables(
      container: service_name,
      backup_dir: backup_path,
      db_port: new_resource.port,
      db_user: new_resource.user,
      db: new_resource.name,
    )
    mode '0755'
  end

  template "/usr/local/bin/postgresql_restore_#{new_resource.name}" do
    source "postgresql_restore.erb"
    variables(
      container: service_name,
      backup_dir: backup_path,
      db_port: new_resource.port,
      db_user: new_resource.user,
      db: new_resource.name,
    )
    mode '0755'
  end

  systemd_unit "postgresql_backup_#{new_resource.name}.service" do
    content <<~EOH
  [Unit]
  Description=Daily #{new_resource.name} Postgresql Backup
  After=#{service_name}.service

  [Service]
  Type=oneshot
  ExecStart=/usr/local/bin/postgresql_backup_#{new_resource.name}

  [Install]
  WantedBy=default.target
    EOH
    action %i(create enable)
  end

  systemd_unit "postgresql_backup_#{new_resource.name}.timer" do
    content <<~EOH
  [Unit]
  Description=Run #{new_resource.name} Postgresql Backup Daily

  [Timer]
  OnCalendar=daily
  RandomizedDelaySec=4h
  Persistent=true

  [Install]
  WantedBy=timers.target
    EOH
    action %i(create enable start)
  end
end

action :remove do
  # TODO
end

action_class do
  def service_name
    "postgresql-#{new_resource.name}"
  end

  def pod_quadlet_config
    if new_resource.podman_pod.nil?
      []
    else
      ["Pod=#{new_resource.podman_pod.gsub(".pod", "")}.pod"]
    end
  end

  def backup_path
    if new_resource.backup_path.nil?
      "#{node["calculon"]["storage"]["paths"]["backups"]}/#{new_resource.name}"
    else
      new_resource.backup_path
    end
  end

  def database_path
    if new_resource.database_path.nil?
      "#{node["calculon"]["storage"]["paths"]["databases"]}/#{new_resource.name}"
    else
      new_resource.database_path
    end
  end
end
