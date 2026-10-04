# Podman runtime configuration

Container process identity, host volume ownership, and user namespace mapping
are separate inputs. Recipes use the image's declared user explicitly when
appropriate, or declare a named/numeric process identity. Host ownership of
data directories is configured by the owning application. A namespace mapping
is opt-in, maps an existing host account through its subordinate-ID ranges,
and never creates host accounts as a side effect.

Host-wide Podman settings are selected and applied once by host composition.
Host composition also creates and records each shared network definition once.
Container recipes attach by network name and declare host-network mode or port
publications with typed values; they do not carry duplicate network definitions
or change global daemon policy.

Persistent service containers use the shared `/var/lib/data` mount dependency:
their units require its preparation service, bind to the mount, and assert the
mountpoint exists. Ignition creates and prepares the Btrfs filesystem and
Podman graphroot; application recipes validate mounts and manage their own
service directories and ownership.
