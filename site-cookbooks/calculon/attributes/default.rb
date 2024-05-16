default["calculon"]["rocky"]["btrfs"]["btrfs-progs"] = {
  version: "6.8-1.el9",
  url: "https://cbs.centos.org/kojifiles/packages/btrfs-progs",
  packages: %w{btrfs-progs btrfs-progs-devel libbtrfs libbtrfsutil python3-btrfsutil}
}
default["calculon"]["rocky"]["btrfs"]["compsize"] = {
  version: "1.5-3.hsx.el9",
  url: "https://cbs.centos.org/kojifiles/packages/compsize",
  packages: %w{compsize}
}

default["calculon"]["TZ"] = "Europe/Madrid"
default["calculon"]["acme"]["lego"]["uid"] = "5000"
default["calculon"]["acme"]["lego"]["gid"] = "5000"
default["calculon"]["acme"]["lego"]["port"] = "4180"
default["calculon"]["acme"]["lego"]["email"] = nil
default["calculon"]["acme"]["certs_dir"] = "/etc/pki/acme"
default["calculon"]["acme"]["key_type"] = "ec384"
default["calculon"]["acme"]["renew_days"] = "30"

default["calculon"]["data"]["username"] = "media"
default["calculon"]["data"]["group"] = "data"
default["calculon"]["data"]["uid"] = "2001"
default["calculon"]["data"]["gid"] = "2001"


default["calculon"]["storage"]["manage"] = true
default["calculon"]["storage"]["dev"] = "/dev/sda5"
default["calculon"]["storage"]["paths"]["root"] = "/var/lib/data"
default["calculon"]["storage"]["paths"]["tmp"] = "/var/lib/data/tmp"
default["calculon"]["storage"]["paths"]["sync"] = "/var/lib/data/sync"
default["calculon"]["storage"]["paths"]["www"] = "/var/lib/data/www"
default["calculon"]["storage"]["paths"]["tailscale"] = "/var/lib/data/tailscale"
default["calculon"]["storage"]["paths"]["tdarr"] = "/var/lib/data/tdarr"
default["calculon"]["storage"]["paths"]["radarr"] = "/var/lib/data/radarr"
default["calculon"]["storage"]["paths"]["sonarr"] = "/var/lib/data/sonarr"
default["calculon"]["storage"]["paths"]["lidarr"] = "/var/lib/data/lidarr"
default["calculon"]["storage"]["paths"]["putioarr"] = "/var/lib/data/putioarr"
default["calculon"]["storage"]["paths"]["prowlarr"] = "/var/lib/data/prowlarr"
default["calculon"]["storage"]["paths"]["jellyfin"] = "/var/lib/data/jellyfin"
default["calculon"]["storage"]["paths"]["plex"] = "/var/lib/data/plex"
default["calculon"]["storage"]["paths"]["media"] = "/var/lib/data/media"
default["calculon"]["storage"]["paths"]["downloads"] = "/var/lib/data/media/downloads"
default["calculon"]["storage"]["snapshots_volumes"] = %w{sync magiusstaff/sync}
default["calculon"]["storage"]["library_dirs"] = {
  "movies" => {
    "service" => "radarr",
    "mount": true,
  },
  "series" => {
    "service" => "sonarr",
    "mount": true,
  },
  "music" => {
    "service" => "lidarr",
    "mount": false,
  }
}

default["calculon"]["nginx"]["user"] = "nginx"
default["calculon"]["nginx"]["group"] = "nginx"
default["calculon"]["nginx"]["uid"] = 101
default["calculon"]["nginx"]["gid"] = 101

# set to a valid domain to enable
default["calculon"]["www"]["domain"] = nil
default["calculon"]["www"]["media_domain"] = nil
default["calculon"]["www"]["upstreams"] = {}

default["calculon"]["nginx"]["container"]["etc"] = "/etc/nginx"
default["calculon"]["nginx"]["container"]["www"] = "/var/www"
default["calculon"]["nginx"]["container"]["cache"] = "/var/cache/www"
default["calculon"]["nginx"]["container"]["logs"] = "/var/logs/nginx"
default["calculon"]["nginx"]["container"]["ssl"] = "/etc/ssl/acme"

default["calculon"]["containers"]["storage"]["volume"] = "/var/lib/data/containers"
default["calculon"]["containers"]["storage"]["driver"] = "btrfs"
default["calculon"]["containers"]["storage"]["runroot"] = "/var/lib/data/containers/run"
default["calculon"]["containers"]["storage"]["graphroot"] = "/var/lib/data/containers/graph"

default["calculon"]["network"]["containers"]["ipv4"]["addr"] = "172.25.25.1"
default["calculon"]["network"]["containers"]["ipv4"]["addr_cidr"] = "172.25.25.1/24"
default["calculon"]["network"]["containers"]["ipv4"]["network"] = "172.25.25.0/24"
default["calculon"]["network"]["containers"]["ipv6"]["addr"] = "fd05:f439:6192:ffff::1"
default["calculon"]["network"]["containers"]["ipv6"]["addr_cidr"] = "fd05:f439:6192:ffff::1/64"
default["calculon"]["network"]["containers"]["ipv6"]["network"] = "fd05:f439:6192:ffff::0/64"

# oauth2 proxy
default["server"]["oauth2_proxy"]["secrets"]["client-secret"] = nil
default["server"]["oauth2_proxy"]["secrets"]["client-id"] = nil
default["server"]["oauth2_proxy"]["secrets"]["cookie-secret"] = nil

default["calculon"]["magiusstaff"]["username"] = "magiusstaff"
default["calculon"]["magiusstaff"]["group"] = "magiusstaff"
default["calculon"]["magiusstaff"]["uid"] = "2002"
default["calculon"]["magiusstaff"]["gid"] = "2002"
default["calculon"]["magiusstaff"]["user_emails"] = nil
default["calculon"]["magiusstaff"]["paths"]["root"] = "/var/lib/data/magiusstaff"
