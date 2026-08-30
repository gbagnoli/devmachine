#!/bin/bash
set -euo pipefail

# Check if required directories exist
if [ ! -d "/usr/local/bin" ]; then
    echo "Error: Directory /usr/local/bin does not exist. Aborting." >&2
    exit 1
fi

if [ ! -d "/etc/systemd/system" ]; then
    echo "Error: Directory /etc/systemd/system does not exist. Aborting." >&2
    exit 1
fi

# Resolve script directory so install works when called from any cwd (e.g., via setup-llama-server)
SCRIPT_DIR="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"
cd "$SCRIPT_DIR"

USER_NAME=$(whoami)
RETRY_LOGIC=$(cat "$SCRIPT_DIR/lib/wait_for_tailscale.sh")

echo "Preparing bazzite_set_operator.sh..." >&2

# We'll use a temporary file to construct the final script
TMP_SCRIPT=$(mktemp)
TMP_RENEW_SCRIPT=$(mktemp)
cleanup() {
  rm -f "$TMP_SCRIPT" "${TMP_RENEW_SCRIPT}"
}
trap cleanup EXIT

# Use python3 to perform substitutions as it handles multi-line strings and special characters better than sed
# We pass RETRY_LOGIC and USER_NAME as environment variables to avoid issues with quotes and delimiters
export RETRY_LOGIC
export USER_NAME
export SCRIPT_DIR
python3 -c 'import os, sys; content = open(os.path.join(os.environ["SCRIPT_DIR"], "bazzite_set_operator.sh")).read().replace("{{RETRY_LOGIC}}", os.environ["RETRY_LOGIC"]).replace("{{USER}}", os.environ["USER_NAME"]); sys.stdout.write(content)' > "$TMP_SCRIPT"

echo "Copying bazzite_set_operator.sh to /usr/local/bin/" >&2
sudo cp "$TMP_SCRIPT" /usr/local/bin/bazzite_set_operator.sh
sudo chmod +x /usr/local/bin/bazzite_set_operator.sh

echo "Copying bazzite_tailscaled_operator.service to /etc/systemd/system/" >&2
sudo cp "$SCRIPT_DIR/bazzite_tailscaled_operator.service" /etc/systemd/system/

echo "Reloading systemd daemon..." >&2
sudo systemctl daemon-reload
sudo systemctl enable bazzite_tailscaled_operator.service

echo >&2 "Create tailscale-certs directory"
CERT_DIR="$HOME/.config/tailscale-certs"
mkdir -p "$CERT_DIR"
chown "$USER_NAME":"$USER_NAME" "$CERT_DIR"
chmod 700 "$CERT_DIR"

echo >&2 "Create renew-cert.sh script"
RENEW_CERT_SCRIPT="$CERT_DIR/renew-cert.sh"
python3 -c 'import os, sys; content = open(os.path.join(os.environ["SCRIPT_DIR"], "renew-cert.sh")).read().replace("{{RETRY_LOGIC}}", os.environ["RETRY_LOGIC"]).replace("{{USER}}", os.environ["USER_NAME"]); sys.stdout.write(content)' > "$TMP_RENEW_SCRIPT"
cp "$TMP_RENEW_SCRIPT" "$RENEW_CERT_SCRIPT"
chmod +x "$RENEW_CERT_SCRIPT"

echo >&2 "Write sysctl config"
echo "net.ipv4.ip_unprivileged_port_start=443" | sudo tee > /dev/null /etc/sysctl.d/99-rootless-podman.conf
sudo sysctl -p /etc/sysctl.d/99-rootless-podman.conf

echo >&2 "Create systemd units for cert renewal"
USER_SYSTEMD_DIR="$HOME/.config/systemd/user"
mkdir -p "$USER_SYSTEMD_DIR"
chown "$USER_NAME":"$USER_NAME" "$USER_SYSTEMD_DIR"

cat <<EOF > "$USER_SYSTEMD_DIR/tailscale-renew-cert.service"
[Unit]
Description=Renew Tailscale Certificate

[Service]
Type=oneshot
ExecStart=$RENEW_CERT_SCRIPT

[Install]
WantedBy=default.target
EOF

cat <<EOF > "$USER_SYSTEMD_DIR/tailscale-renew-cert.timer"
[Unit]
Description=Daily Tailscale Certificate Renewal

[Timer]
OnBootSec=1h
OnUnitActiveSec=1d

[Install]
WantedBy=timers.target
EOF

echo >&2 "Reload systemd and enable the timer"
systemctl --user daemon-reload
systemctl --user enable --now tailscale-renew-cert.timer

echo "Installation complete." >&2
unset RETRY_LOGIC
unset USER_NAME
