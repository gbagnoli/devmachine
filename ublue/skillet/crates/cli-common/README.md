# Shared guest CLI

## Administrative access

Finish service configuration and credential delivery first. On the host, run:

```sh
sudo /var/usrlocal/bin/skillet access finalize --user giacomo
```

Keep the current administrative session open. Start a second interactive login
(with an SSH TTY or the console), use the known bootstrap password to change it,
and verify sudo asks for the new password. SSH authentication stays key-only.
The command requires an existing unlocked password; it does not create one.

Finalization is explicit and persistent, with interrupted-operation recovery.
Repeating it never expires the changed password again. Base apply does not undo
it. Smoke VMs are not finalized automatically. Subsequent workstation credential
and VM automation assumes passwordless sudo: finish those operations before
finalization, then use interactive sudo or root for further administration.
Do not delete `/var/lib/skillet/access/` to rerun provisioning.

Design: [administrative access](../../../design/administrative-access.md).

## Apply diagnostics

Guest `apply --phase base` needs no application credentials. Full and separate
service phases load their systemd credential inputs. `apply --record PATH` writes
version 2 YAML with host, overall result and operation outcomes, including failed
applies; it stores no content, hashes or credential values. The network phase
rejects recording. See [convergence policy](../../../design/skillet-architecture.md).
