#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Usage: %s HOST INSTANCE [--with-applications]\n' "$0" >&2
}

if [[ $# -lt 2 || $# -gt 3 ]]; then
    usage
    exit 2
fi

host=$1
instance=$2
with_applications=${3:-}
if [[ -n $with_applications && $with_applications != --with-applications ]]; then
    usage
    exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
skillet_dir=$(cd -- "$script_dir/.." && pwd)
cd "$skillet_dir"

skillet() {
    cargo run --quiet --release -p skillet -- "$@"
}

application_args=()
if [[ $with_applications == --with-applications ]]; then
    application_args+=(--with-applications)
fi

printf 'Injecting interruption after the smoke checkpoint for %s/%s\n' "$host" "$instance"
output_file=$(mktemp)
trap 'rm -f -- "$output_file"' EXIT
set +e
skillet test smoke "$host" --instance "$instance" \
    "${application_args[@]}" --interrupt-before-reboot 2>&1 | tee "$output_file"
result=${PIPESTATUS[0]}
set -e
output=$(<"$output_file")
if [[ $result -eq 0 ]]; then
    printf 'Fault injection unexpectedly completed successfully\n' >&2
    exit 1
fi
if [[ $result -eq 0 ]] || [[ $output != *'injected interruption after persisting the pre-reboot checkpoint'* ]]; then
    printf 'Smoke failed for a reason other than the requested injected interruption\n' >&2
    exit 1
fi

status=$(skillet test vm status "$host" "$instance")
printf '%s\n' "$status"
if [[ $status != *'Lifecycle: Started'* ]]; then
    printf 'Interrupted run did not remain in the recoverable Started phase\n' >&2
    exit 1
fi

printf 'Recovering readiness after the interruption\n'
skillet test vm ready "$host" "$instance"
status=$(skillet test vm status "$host" "$instance")
printf '%s\n' "$status"
if [[ $status != *'Lifecycle: Ready'* ]]; then
    printf 'Readiness did not restore the Ready phase\n' >&2
    exit 1
fi

printf 'Running the normal smoke reboot after recovery\n'
skillet test smoke "$host" --instance "$instance" "${application_args[@]}"
