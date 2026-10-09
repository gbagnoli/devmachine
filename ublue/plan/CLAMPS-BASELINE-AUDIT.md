# Clamps baseline audit

Status: audited 2026-10-09; configuration unchanged. Compare the public Rupik
Chef role and pinned cookbook sources with Butane, Skillet, uCore image inputs
and read-only observations on the retained owned `clamps-test-smoke` VM.
Private Chef attributes and physical Clamps access remain outside this evidence.

## Results

| Area | Result | Remaining decision or check |
| --- | --- | --- |
| SSH | Server syntax and client configuration parse passed. Key authentication, password/interactive authentication disabled, key-only root access, TCP forwarding and internal SFTP match the Chef role. Server enabled and active; Ignition keys supported; home/SSH permissions accepted. Image and Skillet SSH files are identical. | Physical LAN/tailnet access and final key inventory. No new SSH defect found. |
| sudo | Syntax passed. uCore supplies `%sudo ALL=(ALL) NOPASSWD: ALL`; Giacomo belongs to this group. VM also has its own passwordless rule. | Confirm retaining passwordless administration: the public Chef sysadmins rule required a password by default. Production forced-password-change bootstrap is removed from VM staging and has not been accepted here. |
| Eternal Terminal | Package installed, unit enabled and active, listening on port 2022 for IPv4 and IPv6. Chef supplied no custom ET configuration. | End-to-end ET client and physical firewall reachability remain unverified. |
| User identity | Core is UID/GID 1000; Giacomo is 1001 with Bash and wheel/sudo/docker membership. Ignition has five baseline keys; the live user authorized_keys has seven, plus the VM injection in the Ignition file. | Decide whether to retain UID 1001 or migrate account IDs; account for existing data ownership. Verify final production keys without assuming all keys come from Ignition. Dario is not enabled by the public Rupik role; do not add it automatically. |
| Bootstrap password | Production Butane contains a fixed password hash and a first-login password-expiry service. SSH password login remains disabled. | Review the committed bootstrap hash and production console/password-change policy before cutover. Never reproduce the hash in audit records. VM staging removes forced expiry, so VM success does not accept that flow. |
| Homebrew/dotfiles | Both oneshots completed successfully, markers exist and Brew is installed. Current bootstrap runs the dotfiles installer with `-b core -V`. | `-V` skips Vim plugins. Their installation remains deferred by the operator's earlier choice. Existing parity text saying the entire install hook was deferred was stale. |
| OS hardening | SSH/sysctl convergence implemented; image copies identical. IPv4/IPv6 forwarding enabled, SELinux enforcing, auditd enabled and active. | Skillet's OS-hardening function remains a placeholder. Broader Chef policy has not been reconciled with uCore defaults; see below. |
| Wake-on-LAN | Common image and VM have Fedora `wol`. | Chef generated per-target shortcuts, but its public target map is empty. Obtain private target inventory only if those shortcuts are required; actual wake-up is unverified. |

## Hardening reconciliation needed

The pinned Chef OS cookbook manages login/password policy, limits, PAM,
profile/umask policy, securetty, audit configuration, filesystem module
blacklists and selected access permissions. Their equivalence on uCore has
not been established by the SSH/sysctl implementation.

Observed login defaults are maximum/minimum/warning days `99999/0/7`, while
Chef's server role declares `730/-1/30`. Chef-specific limits, profile and module
blacklist files are absent. This establishes a policy difference; it does not
establish that Fedora lacks equivalent protections elsewhere. Auditd and SELinux
already run. Do not copy obsolete distribution-specific Chef rules wholesale.

## Follow-up order and exit criteria

1. Decide production account IDs, required keys, passwordless sudo and bootstrap
   password/expiry handling. Record intentional changes and test a fresh production
   account path without the VM-specific account overrides. Preserve data ownership
   and a working administrative access path.
2. Reconcile each Chef OS policy with the current uCore baseline. Implement only
   retained requirements through the correct image/runtime owner, or document the
   deliberate replacement. Remove the misleading placeholder status only after
   requirements are accounted for; validate syntax, repeat apply and effective state.
3. Keep Vim plugin installation deferred unless explicitly resumed. Confirm WOL
   targets privately if needed. Complete physical SSH/ET/firewall access at cutover.

Evidence: `../butane/ACCEPTANCE.md`. This audit does not authorize or perform
account, authentication, service or kernel-policy changes.
