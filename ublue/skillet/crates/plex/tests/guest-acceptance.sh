#!/usr/bin/env bash
# Run as root through the owned disposable guest transport.
# Arguments: caller TV, movie and music source directories.
set -euo pipefail
[[ $# == 3 ]]
sources=("$1" "$2" "$3")
targets=(/media/tv /media/movies /media/music)
marker=".skillet-plex-acceptance-$$"
cleanup() {
    for source in "${sources[@]}"; do
        rm -f -- "$source/$marker" "$source/$marker.write"
    done
}
trap cleanup EXIT
uid=$(podman exec plex id -u plex)
gid=$(podman exec plex id -g plex)
[[ "$uid" != 0 && "$gid" != 0 ]]
for index in 0 1 2; do
    source=${sources[$index]}
    target=${targets[$index]}
    printf 'skillet-plex-read-fixture\n' > "$source/$marker"
    chown --reference="$source" "$source/$marker"
    chmod 0640 "$source/$marker"
    actual=$(podman exec --user "$uid:$gid" plex cat "$target/$marker")
    [[ "$actual" == skillet-plex-read-fixture ]]
    if podman exec --user "$uid:$gid" plex touch "$target/$marker.write"; then
        echo 'Plex unexpectedly wrote a read-only library' >&2
        exit 1
    fi
done
curl --fail --silent --show-error --max-time 5 --retry 20 --retry-connrefused \
    --retry-delay 1 --retry-max-time 60 --output /dev/null http://127.0.0.1:32400/identity
podman exec plex pgrep -u "$uid" -f 'Plex Media Server' > /dev/null
echo 'Plex worker identity, loopback health and read-only library access passed'
code=$(curl --silent --show-error --max-time 10 --output /dev/null \
    --write-out '%{http_code}' http://127.0.0.1:32400/library/sections)
echo "Unauthenticated library API status: $code (claim/account acceptance is separate)"
