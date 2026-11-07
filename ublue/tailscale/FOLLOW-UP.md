# Deferred desktop session activation

The current GNOME autostart entry assigns the operator on a new login. Switching back to an existing session does not run it again, so the most recently logged-in user retains operator access. This limitation is accepted for now.

If revisited, track graphical session activation and assign the operator to the authorized user whose session becomes active. Validate two open sessions, switches in both directions, logout, daemon restart, and disconnected Tailscale. Serialize competing assignments and confirm the final operator belongs to the active session. Keep the tray running as the desktop user.

This follow-up is unimplemented; current setup is documented in [README.md](README.md).
