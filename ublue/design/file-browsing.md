# Private file browsing

Select File Browser Quantum as the shared file-management UI composed from each
shared Syncthing declaration. The feature branch is a trial; phone acceptance
and production activation remain in [the plan](../plan/FILEBROWSER-QUANTUM.md).
The prior [Mikochi choice](../plan/archive/MIKOCHI-SELECTION.md) is retained as history.

Use the official pinned stable image with native proxy authentication, consuming
Caddy's verified `X-Skillet-User` identity. The existing per-request WhoIs allowlist,
origin checks and isolated backend remain authoritative. This supports separate
application accounts without another password or identity provider. Disable
password signup/login, public sharing and user API-token creation; ordinary
accounts share the caller-selected synchronized files, not the whole data mount.

Reuse Syncthing's declared host/container identity without recursively changing
ownership or moving files. Persist Quantum's SQLite database and cache outside
synchronized documents. Skillet owns deployment and public configuration; Quantum
owns account/session state. Its signing material stays in that protected state.
No new vault entry is required. Database backup/export is not implemented yet;
existing Syncthing/btrbk coverage protects document data, not the application DB.

Verified legacy retirement preserves the old Quadlet and checks its applied
receipt before stopping it. Interrupted reload retries remain recoverable. Older
Caddy payload readers retain the Mikochi refresh compatibility mode, but newly
rendered Quantum sites do not enable it.

The richer frontend and preview support motivate this replacement. Mobile PDF
rendering still depends on the browser; current stable explicitly uses an
open/download fallback on mobile Chromium. Native container/API acceptance is
separate from actual phone approval. Do not add an office server or custom viewer
until the trial establishes whether either is needed.

Operations: [Quantum guide](../skillet/crates/filebrowser/README.md).
Upstream: [proxy auth](https://filebrowserquantum.com/en/docs/configuration/authentication/proxy/),
[PDF previews](https://filebrowserquantum.com/en/docs/features/previewing-files/).
