default["podman"]["nginx"]["pod_extra_conf"] = []
default["podman"]["nginx"]["path"] = "/var/lib/nginx"
default["podman"]["nginx"]["user"] = "nginx"
default["podman"]["nginx"]["group"] = "nginx"
default["podman"]["nginx"]["uid"] = 101
default["podman"]["nginx"]["gid"] = 101

default["podman"]["nginx"]["container"]["etc"] = "/etc/nginx"
default["podman"]["nginx"]["container"]["www"] = "/var/www"
default["podman"]["nginx"]["container"]["cache"] = "/var/cache/www"
default["podman"]["nginx"]["container"]["logs"] = "/var/logs/nginx"
default["podman"]["nginx"]["container"]["ssl"] = "/etc/ssl/acme"

default["podman"]["nginx"]["acme"]["lego"]["uid"] = "5000"
default["podman"]["nginx"]["acme"]["lego"]["uid"] = "5000"
default["podman"]["nginx"]["acme"]["lego"]["gid"] = "5000"
default["podman"]["nginx"]["acme"]["lego"]["port"] = "4180"
default["podman"]["nginx"]["acme"]["lego"]["email"] = nil
default["podman"]["nginx"]["acme"]["certs_dir"] = "/etc/pki/acme"
default["podman"]["nginx"]["acme"]["key_type"] = "ec384"
default["podman"]["nginx"]["acme"]["renew_days"] = "30"

default["podman"]["nginx"]["oauth2_proxy"]["secrets"]["client-secret"] = nil
default["podman"]["nginx"]["oauth2_proxy"]["secrets"]["client-id"] = nil
default["podman"]["nginx"]["oauth2_proxy"]["secrets"]["cookie-secret"] = nil
