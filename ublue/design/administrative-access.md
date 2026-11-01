# Administrative access lifecycle

Production provisioning temporarily uses uCore's passwordless sudo-group policy.
Finalization is an explicit operator step after service and credential provisioning,
not part of base apply or a boot-triggered expiry service. A provisioning-complete
signal cannot be inferred from base readiness when later credentials are optional.

`skillet access finalize --user <account>` installs a final user-specific sudo rule
requiring that user's password and expires its existing bootstrap password. The
operator changes the password at the next interactive login. SSH continues to use
keys; finalization does not enable SSH password authentication. The existing
bootstrap password must be known, and console recovery should be available.

A root-only per-account journal records intent before expiry and completion after
success. Interrupted delivery is retryable; a fingerprint of the bootstrap hash
allows recovery to preserve a password already changed by the operator. Repeat
finalization restores sudo policy without expiring the password again. A root-held
lock serializes finalization. Sudo syntax is checked before and after installation;
a later sudoers include is rejected rather than risking a conflicting override.

Smoke VMs retain temporary passwordless access unless explicitly finalized. Base
apply, boot and image rebases preserve the final policy and expiry receipt. Later
SSH provisioning supports interactive password-required sudo after finalization.
Unattended execution fails closed if it lacks noninteractive privileges.
The policy applies to the selected account; other accounts remain unchanged.

Private installation inputs are supplied at media generation; never include
passwords/hashes/fingerprints in logs, manifests or audit records.
Instructions: [access finalization](../skillet/crates/cli-common/README.md#administrative-access).


Eternal Terminal is part of the shared OS image. Its TCP 2022 transport is allowed
from the standard Tailscale IPv4/IPv6 ranges and directly attached private IPv4
LAN subnets; native SSH authentication remains required. This administrative
transport policy is separate from the personal-device identity gate for UIs.
The image supplies static tailnet grants. Shared Skillet baseline convergence
derives LAN subnets from active hardware NIC addresses and netmasks and uses
each interface's firewalld zone. Container/VPN interfaces and public subnets are
excluded. Hosts without a private LAN skip those grants; services that require
one retain an explicit fail-closed policy. No extra vault entry is introduced.

SSH TCP 22 stays source-unrestricted and listens on both address families,
independent of Tailscale. ET policy never replaces that recovery path. Firewall
convergence persists before runtime activation without a global reload. Rules
are additive; retiring a network requires explicit cleanup of obsolete grants.
The image supplies English UTF-8 locale data rather than suppressing requested
locales or changing SSH authentication.


SSH-key unlock stays with OpenSSH's agent; sudo authentication is a separate
credential with a separate 30-minute fixed-expiry kernel cache. Workstation provisioning uses an ephemeral static Rust
askpass helper and a private SSH-forwarded Unix socket, independently of command
stdin. This prevents sudo from consuming credential payloads or receiving a
password when its policy does not request one. No password enters argv, environment
or files, or the vault. The vault keeps its independent three-hour cache. Shared deploy/apply/delivery paths
use the same adapter; raw disposable automation keeps its noninteractive policy.
The helper is a separate minimal guest artifact, so the first update does not
require a newer installed Skillet and does not link workstation vault/provider
code into the helper. Forwarding uses the existing SSH target/trust/agent options
and ownership checks; command exit removes the ephemeral resources.

The shared workstation transport caches sudo only after a successful operation
actually used the password. Its scope hashes effective local SSH configuration
and pinned host-trust file contents, binding the remote user, host and connection
policy. Trust changes invalidate reuse; dynamic or unavailable trust disables
persistent caching. This conservative binding can miss after unrelated known-host
changes. A target/trust change during an invocation aborts reuse.

Each privileged operation rereads the kernel entry. Hits never extend expiry;
rejected/unverified answers are evicted. Valid cached credentials also support
noninteractive commands; a miss requires a terminal, without retrying an ambiguous
privileged operation. If kernel expiry is unavailable, use only bounded in-process
memory. `skillet auth lock` clears sudo credentials, independently of vault unlock
and ssh-agent. Other workstation sudo consumers reuse this transport and generic
purpose-separated kernel cache rather than adding another password mechanism.
