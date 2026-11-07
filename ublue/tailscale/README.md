# Tailscale desktop operator

On Bazzite and Bluefin GNOME, this setup makes the user who logs in the Tailscale operator and starts the Tailscale system tray.

## Install

Install Tailscale and connect the machine first. Then run as your normal user, naming **both** local accounts:

```bash
./install.sh firstuser seconduser
```

The installer adds these users to `tailscale-operators`, installs a narrowly scoped passwordless sudo rule for the operator switch, installs a GNOME autostart entry, and disables the old boot-time operator service. Log out of both accounts and log back in to activate group membership. Each login sets its own user as operator, then runs `tailscale systray` in that desktop session. Do not run the installer with `sudo`; its certificate renewal setup applies to the invoking account.

## Using Mullvad exit nodes

GNOME needs an AppIndicator/StatusNotifier extension enabled to show the tray icon. Tailscale's systray requires version 1.88 or later. The GNOME tray currently selects Mullvad exit nodes by country, not by city. For precise selection, the active operator can run `tailscale set --exit-node=<node>`.

Tailscale supports only one Unix operator at a time: the last login wins. If both desktop sessions stay open during user switching, the earlier session's tray may lose permission to change settings. Log in again to make that user the operator.

## Permission scope

The operator can change more than the exit node, including other Tailscale preferences, Serve, and Funnel settings. Tailscale does not offer a built-in local role limited to exit-node selection. If that narrower permission is required, use a dedicated root-owned helper that validates and applies only approved exit nodes instead of granting operator access.

The installer also retains the existing certificate renewal timer and port configuration for the invoking account.

See the [Tailscale Linux operator documentation](https://tailscale.com/docs/reference/troubleshooting/linux/linux-operator-permission) and [Linux systray documentation](https://tailscale.com/docs/features/client/linux-systray) for upstream behavior and requirements.
