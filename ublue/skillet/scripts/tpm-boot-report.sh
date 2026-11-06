#!/bin/bash
# Read-only boot observations; write only to the explicitly selected directory.
set -euo pipefail

usage() {
  echo "Usage: bash $0 /mounted/usb/NEW_REPORT_DIRECTORY"
  echo 'Run as root in the boot to inspect. No network is required.'
  echo 'The parent directory must exist; existing reports are never overwritten.'
}

if [[ $# -eq 1 && ( "$1" == --help || "$1" == -h ) ]]; then
  usage
  exit 0
fi
if [[ $# -ne 1 ]]; then
  usage >&2
  exit 2
fi
[[ $EUID -eq 0 ]] || { echo 'Run from a root shell (sudo bash on a normal boot).' >&2; exit 1; }

output="${1%/}"
[[ "$output" == /* && -n "$output" ]] || { echo 'Use an absolute output directory.' >&2; exit 2; }
[[ -d "$(dirname -- "$output")" ]] || { echo 'Mount the writable USB first; the output parent must exist.' >&2; exit 1; }
event_log=/sys/kernel/security/tpm0/binary_bios_measurements
[[ -r "$event_log" ]] || {
  echo "Cannot read $event_log; no report was created." >&2
  echo 'Keep this boot running and report this error.' >&2
  exit 1
}

umask 077
mkdir -- "$output" || { echo 'Choose a new report directory on the writable USB.' >&2; exit 1; }
trap 'echo "Collection failed; partial diagnostics remain in $output" >&2' ERR
cp -- "$event_log" "$output/events.bin"
cp -- /proc/cmdline "$output/cmdline.txt"
uname -r > "$output/kernel.txt"
cat /proc/sys/kernel/random/boot_id > "$output/boot-id.txt"
if command -v tpm2_pcrread > /dev/null 2>&1; then
  tpm2_pcrread sha256:7 > "$output/pcr7.txt" 2> "$output/pcr7-errors.txt"
else
  echo 'tpm2_pcrread unavailable; raw event log collected.' > "$output/pcr7-unavailable.txt"
  echo 'Warning: tpm2_pcrread unavailable; raw event log collected.' >&2
fi
sync
echo "Saved boot diagnostics to $output"
echo 'Unmount the USB before removing it. No LUKS keys or TPM bindings were changed.'
