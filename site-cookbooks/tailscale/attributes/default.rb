default["tailscale"]["install_type"] = "distro"
default["tailscale"]["package_action"] = "install"
default["tailscale"]["authkey"] = nil
default["tailscale"]["port"] = "39129"

default["tailscale"]["podman"]["user"] = "nobody"
default["tailscale"]["podman"]["group"] = "nobody"
default["tailscale"]["podman"]["config_dir"] = "/var/lib/tailscale"
default["tailscale"]["podman"]["hostname"] = nil
default["tailscale"]["podman"]["extra_env"] = nil
