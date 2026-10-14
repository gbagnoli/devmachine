# Syncthing initial configuration

Implement a bootstrap-only seed for caller-selected Syncthing hosts. Existing
`config/config.xml` is application-owned and must never be overwritten by apply.
No real domains, Zone IDs or resolved private payloads belong in this repository.

## Implementation order

1. Add an atomic create-if-absent file effect with typed ownership. Native,
   recording and fake adapters share the same no-overwrite contract, including
   concurrent creation, existing content/metadata, symlinks and directories.
2. Add a Syncthing-owned initializer using the upstream container generator in
   network-disabled, run-owned staging. Use the caller's storage path and exact
   numeric namespace mapping; reuse a complete existing device keypair and never
   replace it. Reject an incomplete pair. Initialize the device name and GUI flag
   in the generated configuration via Rust XML APIs; publish complete artifacts
   atomically with durable retry state before enabling the normal container.
   A partial handwritten XML probe loaded the requested name but did not persist
   generated settings, so do not use that approach for initialization.
3. Deliver a private initial-settings payload through the shared encrypted SSH /
   systemd credential path. Resolve the selected environment's existing
   Cloudflare Zone ID to its authoritative zone name; do not derive it from the
   UI prefix or add another zone entry. Production uses host identity; owned VMs
   use their recorded runtime hostname and test environment. The caller's actual
   identity-protected backend declaration controls warning acknowledgement.
4. Stage/activate bootstrap before full application can start Syncthing in both
   physical and owned-VM orchestration. Converge new units before delivering
   credentials. Narrow delivery must not activate unrelated services. Preserve
   existing installs lacking bootstrap credentials without changing their XML.
5. Update module setup, secret-audit metadata/checklist, design and acceptance.
   Syncthing needs no new human-generated application credential; describe shared
   Cloudflare inputs and any generated/delivered configuration accurately.

## Acceptance

- Unit and adapter contracts prove missing-only initialization, existing XML and
  metadata preservation, host/environment/instance binding, malformed inputs,
  keypair preservation, publication races and interrupted initialization recovery.
- Credential-free CI uses fixture domains and the real Syncthing container to
  verify complete first-start configuration, correct local device name, GUI
  acknowledgement, API-key generation, restart and later UI-edit preservation.
- Run formatting, strict workspace Clippy, workspace/all-target tests and base
  integration before focused commits. No push unless requested.
- Physical one-time reset is authorized on clamps: stop Syncthing, save a private
  XML backup and identity fingerprints, remove only `config.xml`, initialize and
  restart. Keep certificate/key, database and data. Check unchanged device ID,
  desired name, GUI warning behaviour, protected HTTPS and subsequent no-op apply.
  Record results without private names or credential values in acceptance.

Status: implementation and local checks complete (466 workspace tests, strict
Clippy, formatting and repeat base integration). Credential-free real-consumer
acceptance passed API readiness, GUI acknowledgement, user-edit preservation and
XML-only reset with unchanged device ID. Physical clamps XML-only reset and full
apply passed with private backup, exact keypair/device-ID preservation, desired
name, GUI acknowledgement, API readiness and repeat-init byte/metadata preservation.
Tagged-client HTTPS still returns 403. An isolated physical systemd fixture proved
missing-credential conditions permit a requiring consumer while initializer errors
block it; fixture units were removed. No synchronized data was erased.

Remaining: complete owned-VM UI provisioning/application acceptance after its first
broad smoke attempt passed reboot/fixtures but found Caddy inactive. Revalidate the
final stricter dependency after deployment. Keep the VM failure as original evidence.
