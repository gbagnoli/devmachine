config = node["pihole"]["sync"]
if config.nil? || config.empty?
  Chef::Log.error("missing or empty config for pihole nebula sync")
  return
end

if config["enabled"] == false
  Chef::Log.info("nebula-sync not enabled")
  return
end

password = config["password"]
primary = config["primary"]
secondary = config["secondary"]

if password.nil? || primary.nil? || secondary.nil?
  Chef::Log.error("missing or empty password/primary/secondary for pihole nebula sync")
  return
end


podman_container "pihole-sync" do
  config(
    Container: %W{
    Image=ghcr.io/lovelaze/nebula-sync:latest
    Pull=missing
    Environment=PRIMARY="#{primary}|#{password}"
    Environment=REPLICAS="#{secondary}|#{password}"
    Environment=FULL_SYNC=false
    Environment=RUN_GRAVITY=true
    Environment=TZ=Europe/Madrid
    Environment=CLIENT_RETRY_DELAY_SECONDS=10
    Environment=CLIENT_TIMEOUT_SECONDS=30
    Environment=SYNC_CONFIG_DNS=true
    Environment=SYNC_CONFIG_DHCP=true
    Environment=SYNC_CONFIG_NTP=true
    Environment=SYNC_CONFIG_RESOLVER=true
    Environment=SYNC_CONFIG_DATABASE=true
    Environment=SYNC_CONFIG_MISC=true
    Environment=SYNC_CONFIG_DEBUG=true
    Environment=SYNC_GRAVITY_DHCP_LEASES=true
    Environment=SYNC_GRAVITY_GROUP=true
    Environment=SYNC_GRAVITY_AD_LIST=true
    Environment=SYNC_GRAVITY_AD_LIST_BY_GROUP=true
    Environment=SYNC_GRAVITY_DOMAIN_LIST=true
    Environment=SYNC_GRAVITY_DOMAIN_LIST_BY_GROUP=true
    Environment=SYNC_GRAVITY_CLIENT=true
    Environment=SYNC_GRAVITY_CLIENT_BY_GROUP=true
    Environment=NS_DEBUG=true
    },
    Service: %w{
    Type=oneshot
    },
    Unit: [
      "Description=Nebula Sync One-Shot Task",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
  restart_service false
  start_service false
end


systemd_unit "pihole-sync.timer" do
  content <<~EOH
    [Unit]
    Description=Run Nebula PiHole Sync Periodically

    [Timer]
    Unit=pihole-sync.service
    Persistent=true
    OnCalendar=hourly

    [Install]
    WantedBy=timers.target
  EOH
  action %i(create enable start)
end
