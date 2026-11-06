# Initial image ownership inventory

Scope: current canonical fleet profiles, before changing runtime ownership.
Companion: [implementation slices](IMAGE-OWNED-CONFIGURATION.md).

| Outputs/effects | Current owner/location | Image candidate | Retained runtime responsibility |
| --- | --- | --- | --- |
| Generic/caller executables | Workstation delivery, `/var/usrlocal/bin/skillet*` | Matching clean binaries in `/usr/bin`, public release manifest | Explicit live repair; migrate mutable precedence before switching consumers |
| Storage preparation and collector/proxy helpers | Runtime installation and generated units | Versioned programs under `/usr/libexec/skillet` or `/usr/bin` | Observed disks, mounts, Btrfs subvolumes, identities and actual metrics |
| SSH/sysctl/ET hardening | Image files plus idempotent runtime convergence | Vendor static defaults | Preserve admin overrides; distinguish managed receipts before migration |
| Apply, credential, kexec and initialization units | Generated `/etc/systemd/system` | Static vendor units under `/usr/lib/systemd/system` | Private credential drop-ins, activation gates, convergence receipts and explicit reboot policy |
| btrbk | `/etc/btrbk/btrbk.conf`, generated service/timer | First service pilot: caller-selected static source/config and units | Require mounted data and leaf subvolume; snapshot-directory preparation; snapshots themselves |
| Podman networks, graphroot, firewall/storage prerequisites | Runtime resource adapters | Public declaration/rendering only where static | Inspect/converge actual storage, forwarding and observed private LAN policy |
| Pi-hole, Syncthing, Mikochi, Homer, UniFi | Generated Quadlets/config, mutable volumes | Public container/service skeletons and bundled UI assets | Private DNS, UI/environment inputs, first-use config, application edits and data |
| Home Assistant, NUT and companions | Runtime units/config plus image prerequisites | Static capability/security policy and public unit structure | Hardware access, private integration credentials, initialization and application state |
| Caddy/tailnet UI | Guest-built proxy/cache/helper images and runtime config | Move public image builds to release CI; later pinned images/units | Domains, certificates, identities, private routes, tokens and provider APIs |
| DDNS, SMTP, Datadog | Private runtime config and generated services | Public unit/program/template structure | Addresses, sender/provider input, authentication, observed host identity and metrics |
| Joplin/AirTrail and per-app PostgreSQL | Runtime Quadlets and scheduled backup/restore | Public service/database skeletons and backup programs | Dedicated passwords, initialized databases/uploads and native account state |
| Arr/Putioarr, Jellyfin/Plex/cache, Gokapi | Runtime Quadlets/bootstrap/cache/ingress config | Public structure and later pinned images | API/bootstrap keys, native ownership claim, media, mixed-exposure private input and application state |
| Configuration templates | Public template plus private expansion | Unexpanded public templates where useful | Resolve vault references only at runtime; never publish expanded values |
| TPM recovery, enrollment and networking | Ignition plus private runtime state | Generic preparation tools only | LUKS/TPM/key material, recovery records, machine enrollment, Wi-Fi and LAN configuration |
| Backup schedules/exporters | Runtime-generated units and programs | Static units/programs once prerequisites are explicit | Backup/restore, retention, independent receipt and actual data |

First bundle allowlist: only `/usr/bin/skillet`, `/usr/bin/skillet-<host>` and
`/usr/share/skillet/release.json`. The manifest records the exact source commit,
matching version, public service names and binary hashes/modes. No runtime apply
is executed to produce it. Service selection is descriptive in this first bundle;
service definitions have not moved yet.

Pilot migration contract: btrbk must retain its declared Syncthing leaf source.
Separate pure rendering from mount/subvolume checks; keep runtime preparation.
Move config/service/timer to vendor paths with explicit executable references.
Remove an old `/etc` override only when its content is verified as Skillet-owned;
preserve unknown edits and fail with guidance. Do not delete mutable binaries or
rewrite existing boot units in the artifact-only slice. Fresh/retained-state boots,
interruption, conflicts and rollback remain acceptance gates for the pilot.
