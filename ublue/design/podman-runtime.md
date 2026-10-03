# Podman runtime configuration

Container process identity, host volume ownership, and user namespace mapping
are separate inputs. Recipes use the image's declared user explicitly when
appropriate, or declare a named/numeric process identity. Host ownership of
data directories is configured by the owning application. A namespace mapping
is opt-in, maps an existing host account through its subordinate-ID ranges,
and never creates host accounts as a side effect.

Host-wide Podman settings are selected and applied once by host composition.
Individual container recipes declare their own service configuration and
network attachments; they do not change global daemon policy.
