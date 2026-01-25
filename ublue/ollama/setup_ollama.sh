#!/bin/bash

set -euo pipefail

script_dir="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"
# shellcheck source-path=SCRIPTDIR
# shellcheck source=../lib/utils.sh
source "$(realpath "$script_dir/../lib/utils.sh")"

usage() {
  exit_code=0
  echo -e >&2 "${BB}Usage${BE}: $(basename "$0") [-Ch]"
  echo -e >&2
  echo -e >&2 "${BB}Options${BE}"
  echo -e >&2 "  ${BB}-h${BE}: print this help"
  echo -e >&2 "  ${BB}-C${BE}: disable colors"
  [ $# -ge 1 ] && exit_code="$1" && shift
  [ $# -ge 1 ] && echo -e >&2 "$@"
  exit "$exit_code"
}

while getopts "Ch" opt; do
  case $opt in
    C) disable_colors ;;
    h) usage 0;;
    \?) echo "Invalid option -$OPTARG" >&2; exit 1 ;;
  esac
done

exit_if_running_in_distrobox
ensure_sudo

quadlets=(ollama.pod openwebui.container ollama.container)
updated_quadlets=()

log "Creating directories"
[ ! -d "/var/lib/ollama" ] && sudo mkdir -v -p -m 775 /var/lib/ollama
[ ! -d "/var/lib/open-webui/" ] && sudo mkdir -v -p -m 775 /var/lib/open-webui/

quadlets_dir="/etc/containers/systemd"
log "Copying quadlets to ${quadlets_dir}/"
for quadlet in "${quadlets[@]}"; do
  quadlet_path="$(realpath "$script_dir/files/$quadlet")"
  [ ! -f "$quadlet_path" ] && log "Cannot find quadlet file for $quadlet at $quadlet_path" && exit
  if copy "$quadlet_path" "${quadlets_dir}/$quadlet" -m 644; then
    updated_quadlets+=("$quadlet")
  fi
done

if [ ${#updated_quadlets[@]} -gt 0 ]; then
  log "Some quadlets are to be updated, reloading systemd"
  sudo systemctl daemon-reload
  log "Stopping openwebui"
  sudo systemctl stop openwebui.service
  log "Stopping ollama"
  sudo systemctl stop ollama.service
  sudo systemctl stop ollama-pod.service
  log "Starting ollamai and all services"
  sudo systemctl start ollama
fi


log "Linking scripts to /usr/local/bin"
scripts=(ollama update-ollama)
for script in "${scripts[@]}"; do
  script_path="$(realpath "$script_dir/files/$script")"
  [ ! -f "$script_path" ] && log "Cannot find script at $script_path" && exit
  [ ! -L "/usr/local/bin/$script" ] && sudo rm "/usr/local/bin/$script"
  sudo ln -sfv "$(realpath "$script_path")" "/usr/local/bin/$script"
done

# remove suspend - both for current user AND for gdm
declare -A settings
settings["power-button-action"]="interactive"
settings["power-saver-profile-on-low-battery"]="true"
settings["sleep-inactive-ac-timeout"]="0"
settings["sleep-inactive-ac-type"]="nothing"
settings["sleep-inactive-battery-timeout"]="900"
settings["sleep-inactive-battery-type"]="suspend"

log "Setting gsettings to disable standby on $USER and gdm"
schema="org.gnome.settings-daemon.plugins.power"
for setting in "${!settings[@]}"; do
  log "gsettings set $schema $setting ${settings[$setting]}"
  gsettings set "$schema" "$setting" "${settings[$setting]}"
  sudo -u gdm dbus-launch gsettings set "$schema" "$setting" "${settings[$setting]}"
done
