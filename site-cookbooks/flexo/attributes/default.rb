default["flexo"]["media"]["gid"] = "2001"
default["flexo"]["media"]["uid"] = "2001"
# we pre-create the user for plex with a known
# id, and use it for sickrage/couchpotato etc.
default["flexo"]["media"]["username"] = "plex"
default["flexo"]["media"]["path"] = "/media"
default["flexo"]["media"]["sickchill"]["port"] = 3344
default["flexo"]["media"]["couchpotato"]["port"] = 5050
default["flexo"]["media"]["radarr"]["port"] = 7878

# make sure it ends with a forward slash
default["flexo"]["rclone"]["local_directory"] = "#{node["flexo"]["media"]["path"]}/downloads/"
default["flexo"]["putio"]["watcher_parent_id"] = "591184156"
