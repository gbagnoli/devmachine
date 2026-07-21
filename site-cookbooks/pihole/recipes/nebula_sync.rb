config = node["pihole"]["sync"]
return if config.nil? || config.empty?

password = config["password"]
primary = config["primary"]
secondary = config["secondary"]

return if password.nil? || primary.nil? || secondary.nil?
return if passwod.empty? || primary.empty? || secondary.empty?

podman_cointainer "pihole-sync" do
  config(
    Container: %W{
    Image=ghcr.io/lovelaze/nebula-sync:latest
    Pull=missing
    Environment=PRIMARY="#{primary}|#{password}"
    Environment=REPLICAS="#{secondary}|#{password}"
    Environment=FULL_SYNC=true
    Environment=RUN_GRAVITY=true
    },
    Service: %w{
    Type=oneshot"
    },
    Unit: [
      "Description=Nebula Sync One-Shot Task",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
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
