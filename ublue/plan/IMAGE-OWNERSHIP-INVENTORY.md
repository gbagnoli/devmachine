# Image ownership inventory

Scope: canonical fleet profiles, btrbk pilot and optional maintenance/collector units;
other runtime owners remain.
Companion: [implementation slices](IMAGE-OWNED-CONFIGURATION.md).

| Outputs/effects | Current owner/location | Image candidate | Retained runtime responsibility |
| --- | --- | --- | --- |
| Generic/caller executables | Matching clean binaries in `/usr/bin`; unique workstation runner only during remote apply | Public release manifest and exact source pin | Early bootstrap and verified legacy migration; temporary apply cleanup/recovery |
| Storage preparation and collector/proxy helpers | Runtime installation and generated units | Versioned programs under `/usr/libexec/skillet` or `/usr/bin` | Observed disks, mounts, Btrfs subvolumes, identities and actual metrics |
| SSH/sysctl/ET hardening | Image files plus idempotent runtime convergence | Vendor static defaults | Preserve admin overrides; distinguish managed receipts before migration |
| Apply, credential, kexec and initialization units | Image `/usr/bin` defaults; remote apply may use a temporary runtime environment override | Static vendor units under `/usr/lib/systemd/system` | Private credential drop-ins, activation gates, convergence receipts and explicit reboot policy |
| btrbk | Vendor bundle with verified retirement of legacy `/etc` definitions; scoped native retained-VM acceptance passed | `/usr/share/skillet/btrbk` config/marker, `/usr/lib/systemd/system` units, image Rust guard | Require mounted data and leaf subvolume; snapshot-directory preparation, activation/rollback reconciliation and snapshots themselves |
| Podman networks, graphroot, firewall/storage prerequisites | Runtime resource adapters | Public declaration/rendering only where static | Inspect/converge actual storage, forwarding and observed private LAN policy |
| Pi-hole, Syncthing, File Browser Quantum, Homer, UniFi | Generated Quadlets/config, mutable volumes | Public container/service skeletons and bundled UI assets | Private DNS, UI/environment inputs, first-use config, application edits and data |
| Home Assistant, NUT and companions | HA export/freshness and NUT collector vendor units; container/hardware config remains runtime | Static capability/security policy and public unit structure | Hardware access, private integration credentials, initialization and application state |
| Caddy/tailnet UI | Guest-built proxy/cache/helper images and runtime config | Move public image builds to release CI; later pinned images/units | Domains, certificates, identities, private routes, tokens and provider APIs |
| DDNS, SMTP, Datadog | Private runtime config and generated services | Public unit/program/template structure | Addresses, sender/provider input, authentication, observed host identity and metrics |
| Joplin/AirTrail and per-app PostgreSQL | Runtime Quadlets and scheduled backup/restore | Public service/database skeletons and backup programs | Dedicated passwords, initialized databases/uploads and native account state |
| Arr/Putioarr, Jellyfin/Plex/cache, Gokapi | Runtime Quadlets/bootstrap/cache/ingress config | Public structure and later pinned images | API/bootstrap keys, native ownership claim, media, mixed-exposure private input and application state |
| Configuration templates | Public template plus private expansion | Unexpanded public templates where useful | Resolve vault references only at runtime; never publish expanded values |
| TPM recovery, enrollment and networking | Ignition plus private runtime state | Generic preparation tools only | LUKS/TPM/key material, recovery records, machine enrollment, Wi-Fi and LAN configuration |
| Backup schedules/exporters | HA export vendor units; other backup definitions remain runtime | Static units/programs once prerequisites are explicit | Backup/restore, retention, independent receipt and actual data |

Bundle allowlist: `/usr/bin/skillet`, `/usr/bin/skillet-<host>`, selected btrbk
vendor artifacts, optional maintenance/collector units and disabled presets, and
`/usr/share/skillet/release.json`. The manifest records exact
source commit, matching version, public service names and all output hashes/modes.
No runtime apply is executed to produce it. Other service definitions retain
their current runtime owners.

Pilot migration contract: btrbk must retain its declared Syncthing leaf source.
Separate pure rendering from mount/subvolume checks; keep runtime preparation.
Move config/service/timer to vendor paths with explicit executable references.
Remove an old `/etc` override only when its content is verified as Skillet-owned;
preserve unknown edits and fail with guidance. Do not delete mutable binaries or
rewrite general boot units in this pilot. Retained-state native VM migration,
partial-state recovery, conflicts, reboot and rollback passed. Fresh Ignition,
whole-host application acceptance, published signed-image and production gates remain.

Kexec activation/reboot units, HA backup export/freshness units and NUT collector
units now render into every host image bundle with disabled presets. Runtime
caller/credential gates retain activation; shared reconciliation retires verified
`/etc` overrides, checks available legacy receipts and preserves recovery copies.
NUT UPS/socket remain runtime input. Unit definitions use `/usr/bin/skillet`.
Old images keep legacy installation until the complete vendor group is available.
New unit migration, signed-image rollout and physical acceptance are tracked in
the implementation plan; this does not expand ownership of unrelated boot units.
