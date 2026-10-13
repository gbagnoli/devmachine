# Private file browsing

Select Mikochi as the shared file-management UI on Syncthing hosts. Users already
authorized by [tailnet identity](tailnet-ui-authentication.md) must not enter another
application password. Serve caller-declared user files, never Syncthing config or
the entire data filesystem. Keep application state outside synchronized data.

## Selection

| Candidate | Tradeoff |
| --- | --- |
| [Mikochi](https://github.com/zer0tonin/Mikochi) | Small browsing/upload/download/rename/delete UI; NO_AUTH fits the authenticated proxy and shared file scope. Selected. |
| [FileBrowser Quantum](https://github.com/gtsteffaniak/filebrowser) | Richer sources/permissions and proxy auth, with more configuration/state migration. |
| [Copyparty](https://github.com/9001/copyparty) | Broader file server, per-user/folder rights, resumable uploads and WebDAV. |
| [Filestash](https://www.filestash.app/) | Multi-backend browser client rather than this simple local-folder scope. |

Mikochi's single-login model does not supply per-user folder permissions. With
native auth disabled, the explicit proxy allowlist controls entry and both users
share the declared mount. Different folder rights require evaluating a richer
application rather than pretending this mapping supplies them.

The pinned Mikochi deployment shares Syncthing's mapped numeric process identity
and uses its data directory, outside config. Only Caddy joins the strictly isolated
backend; no host port is published. Existing synchronized paths are not moved
implicitly. NO_AUTH is allowed only with that verified sole private entry point.

Setup: [Mikochi guide](../skillet/crates/mikochi/README.md).
Browser/containment acceptance: [UI plan](../plan/GENERIC-PRIVATE-UIS.md).
