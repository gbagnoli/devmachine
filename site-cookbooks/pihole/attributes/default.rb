default["pihole"]["dns"]["custom"] = nil
default["pihole"]["dns"]["custom_domain"] = nil
default["pihole"]["enable_datadog"] = false

default["pihole"]["paths"]["root"] = "/etc/pihole"
default["pihole"]["paths"]["logs"] = "/var/log/pihole"

default["pihole"]["image"]["tag"] = "latest"
default["pihole"]["image"]["repository"] = "docker.io"

default["pihole"]["tz"] = "Europe/Madrid"
# every key value is passed as environment as FTLCONF_<key>=<value>
# see https://docs.pi-hole.net/docker/configuration
default["pihole"]["conf"]["dns_upstreams"] = "1.1.1.1;8.8.8.8"
default["pihole"]["conf"]["webserver_port"] = "8088o,[::]:8088o"

default["pihole"]["container"]["pod"] = nil
