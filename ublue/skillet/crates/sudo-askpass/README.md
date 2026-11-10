# Sudo askpass helper

Internal static guest executable for interactive workstation sudo authentication.
It connects to `askpass.sock` beside its staged executable, requests one bounded
response and writes it only to sudo's askpass pipe. It has no shell, provider or
vault dependencies and stores no passwords. Operators use normal `skillet apply`,
not this helper directly.

The workstation owns the private directory, SSH forward, memory-only broker and
cleanup. See [sudo authentication](../workstation/README.md#sudo-authentication)
and [the design](../../../design/administrative-access.md).
