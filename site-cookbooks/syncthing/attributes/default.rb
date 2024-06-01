default["syncthing"]["install_type"] = "users"
default["syncthing"]["users"] = {}
default["syncthing"]["skip_service"] = false
default["syncthing"]["package_action"] = "upgrade"
default["syncthing"]["repo_action"] = "add"

default["syncthing"]["podman"]["directory"] = "/var/lib/syncthing"
default["syncthing"]["podman"]["uid"] = nil
default["syncthing"]["podman"]["gid"] = nil
default["syncthing"]["podman"]["ipv6"]["gui"] = "::1"
default["syncthing"]["podman"]["ipv6"]["service"] = "::"
default["syncthing"]["podman"]["ipv4"]["gui"] = "127.0.0.1"
default["syncthing"]["podman"]["ipv4"]["service"] = ""
default["syncthing"]["podman"]["extra_conf"] = []
