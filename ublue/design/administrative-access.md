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
SSH provisioning automation currently assumes passwordless sudo; after finalization
use interactive sudo/root administration until an authenticated transport is added.
The policy applies to the selected account; other accounts remain unchanged.

The current production Butane bootstrap password hash is retained by this change.
Private bootstrap-password delivery is a separate future decision; never include
hashes, password fingerprints or passwords in logs, manifests or audit records.
