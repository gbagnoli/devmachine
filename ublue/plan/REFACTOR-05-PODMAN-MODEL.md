# 5. Podman configuration with explicit host policy

Status: implementation and clamps live acceptance complete. See
workstreams 2–4 in the [prerequisite roadmap](SKILLET-REFACTOR.md).

## Read and locate

- `../skillet/crates/podman/src/{lib,tests}.rs` and its Quadlet template.
- Application crates' container definitions and workstream 2's shared network
  and storage declarations; workstream 3's account/ownership interfaces.
- `../design/{storage,pihole-network,syncthing,unifi,private-ui-access}.md` and
  `../butane/includes/data-storage.bu`.

Current defects: runtime user/network settings can come from both typed fields
and raw directives; unused user fields misdescribe runtime identity. Per-container
apply also owns a global DNS-port policy and converges shared networks repeatedly.

## Progress

- Implemented: moved the host-wide Aardvark DNS listener setting out of generic
  container apply. The canonical host profile selects it when Pi-hole is
  declared, and host composition writes it once before service recipes. The
  focused Podman helper is idempotent; standalone container apply no longer
  changes global Podman DNS policy. Named-VM validation remains pending.
- Implemented: replaced the ambiguous container UID/GID plus host-user fields
  with image-default, named, and numeric process identities, and a separate
  optional namespace mapping. Removed unused implicit host-account creation;
  UniFi now declares its image's named process user as typed data. Invalid
  names and raw `User`/`UIDMap`/`GIDMap` conflicts fail before file effects.
- Implemented: shared bridge definitions are now created once by host
  composition and carry change-refusal markers; containers declare named bridge
  or host-network attachments. IPv4/IPv6 port publications are typed and
  validated, and conflicting raw `Network`/`PublishPort` directives fail
  before effects. Pi-hole, Syncthing, Caddy, UniFi, Tailscale, and the fixture
  were migrated without changing their intended ports or modes.
- Implemented: added a reusable typed mount dependency for persistent service
  containers. Pi-hole, Syncthing, UniFi, Caddy, and Tailscale use the shared
  data mount helper; invalid mount inputs fail before container effects.
- Implemented: typed bind volumes can declare root mode and numeric or named
  host ownership. Podman converges those fields only on the mount root; omitted
  metadata preserves the application's current ownership. UniFi declares its
  `999:999` root and `0750` mode in its volume. A test preserves a child file's
  bytes while checking the root metadata.
- Implemented: a representative comparison proves that adding root ownership
  metadata leaves the existing Quadlet bytes unchanged.
- Accepted: live inspection confirmed Pi-hole/Syncthing bridge networking,
  Tailscale/UniFi host networking, UniFi's declared process identity, expected
  persistent data paths, and healthy/running services. Repeating host
  provisioning preserved all four service container start times. An induced
  UniFi volume-root ownership drift was repaired without changing child
  ownership/mode. Caddy's staging flow also served the declared UI upstreams.

## Implementation sequence

1. Inventory actual resource usage before changing the model. Define explicit
   image-default/named/numeric process identity, separate host volume ownership,
   and optional namespace mapping. Reuse workstream 3's identity types. Remove
   inert fields and implicit user creation; retain mapping support only with an
   explicit validated caller and meaningful tests.
2. Make commonly used container settings typed, including identity, network
   attachments and declared port publications. Keep an extension section for
   unsupported directives with documented constraints. Reject conflicts with
   typed values and invalid input before effects. Validate repeated directives
   according to their cardinality; preserve order where it affects semantics.
3. Add explicit shared network/global Podman baseline convergence selected by
   the host profile. Move DNS listener policy out of generic container apply.
   Services reference an already-declared network rather than each owning its
   full definition. Preserve bridge DNS, dual stack, host networking and network
   change refusal until an explicit safe replacement workflow exists.
4. Add reusable storage dependency helpers for application units and shared
   data validation. Preserve the installation/runtime split: Ignition prepares
   the filesystem and graphroot; recipes validate and prepare application data.
   Carry caller-selected root metadata as typed volume data; never recursively
   change descendants or infer host ownership from container process identity.
5. Migrate every existing service, including Tailscale and the smoke fixture,
   preserving images, ports, capabilities, SELinux labels, identities, paths,
   secret directives, auto-update and startup dependencies. Explain intentional
   definition differences in the design; do not make service-policy changes
   merely to simplify a type or get a test passing.
6. Keep revision/activation behavior compatible with workstream 6. Document
   what omitted services/options mean: an omitted recipe leaves existing state
   unmanaged unless an explicit deactivation is requested. Do not remove data
   during the refactor or silently interpret omission as destructive cleanup.

## Validation and exit criteria

- Rendering/validation tests cover image-default and named/numeric users,
  explicit mappings, bridge and host networks, dual-stack publications, secrets,
  typed/raw conflicts and semantically ordered repeated directives.
- Baseline tests prove global/network policy is selected once from profile
  inputs and independent of application apply order. Network definition changes
  still fail safely instead of replacing a network in use.
- Unit and real-VM tests establish mount dependencies, volume root ownership,
  preserved descendant metadata, secret/config consumption and repeat apply.
  Compare representative old/new Quadlets and document intended changes.
- A named VM verifies actual process identity, DNS publications, service-to-
  service resolution, host-network services, and unchanged application state.
  Include stop/start and reboot checks for definitions that changed.
- Pass roadmap checks, update affected service designs and acceptance evidence,
  and retain separate process/mapping/ownership and global-policy AGENTS rules.
