#!/bin/bash
sudo -n /usr/local/bin/bazzite_set_operator.sh || echo 'Could not set Tailscale operator.' >&2
exec tailscale systray
