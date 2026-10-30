#!/usr/bin/env bash
# Execute only through the owned disposable guest transport, as root.
# Arguments are the caller's TV, movie and music source directories.
set -euo pipefail
[[ $# == 3 ]]
sources=("$1" "$2" "$3")
targets=(/media/tv /media/movies /media/music)
marker=".skillet-jellyfin-acceptance-$$"
cleanup() {
    for source in "${sources[@]}"; do
        rm -f -- "$source/$marker" "$source/$marker.write"
    done
}
trap cleanup EXIT
for index in 0 1 2; do
    source=${sources[$index]}
    target=${targets[$index]}
    [[ -d "$source" ]]
    printf 'skillet-media-read-fixture\n' > "$source/$marker"
    chown --reference="$source" "$source/$marker"
    chmod 0640 "$source/$marker"
    actual=$(podman exec jellyfin cat "$target/$marker")
    [[ "$actual" == skillet-media-read-fixture ]]
    if podman exec jellyfin touch "$target/$marker.write"; then
        echo 'Jellyfin unexpectedly wrote a read-only library' >&2
        exit 1
    fi
done
podman exec jellyfin curl --fail --silent --show-error --max-time 5 \
    --retry 20 --retry-connrefused --retry-delay 1 --retry-max-time 60 \
    http://127.0.0.1:8096/health
echo
echo 'Jellyfin health, library traversal/read and read-only enforcement passed'
