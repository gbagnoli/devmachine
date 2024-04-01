default["calculon"]["containers"]["storage"]["driver"] = "btrfs"
default["calculon"]["containers"]["storage"]["runroot"] = "/var/lib/containers/run"
default["calculon"]["containers"]["storage"]["graphroot"] = "/var/lib/containers/graph"


default["calculon"]["data"]["username"] = "media"
default["calculon"]["data"]["group"] = "data"
default["calculon"]["data"]["uid"] = "2001"
default["calculon"]["data"]["gid"] = "2001"

default["calculon"]["paths"]["root"] = "/var/lib/data"
default["calculon"]["paths"]["sync"] = "/var/lib/data/sync"
default["calculon"]["paths"]["media"] = "/var/lib/data/media"
default["calculon"]["paths"]["downloads"] = "/var/lib/data/media/downloads"
default["calculon"]["paths"]["library"] = "/var/lib/data/media/library"
default["calculon"]["paths"]["library_dirs"] = %w{movies series}
