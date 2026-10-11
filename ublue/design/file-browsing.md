# Private file browsing

Decision, 2026-10-11: provide a shared file-management UI on Syncthing hosts,
with Caddy HTTPS and [verified tailnet identity](tailnet-ui-authentication.md).
The two intended users must enter without an internal password prompt. Serve
only caller-declared user folders; do not mount Syncthing configuration or the
whole data filesystem. Keep any app state outside synchronized files. The
magiusstaff second instance is deferred.

## Selection

| Candidate | Fit / tradeoff |
| --- | --- |
| [Mikochi](https://github.com/zer0tonin/Mikochi) | Small Go/Preact UI for browsing/upload/download/rename/delete and folders. One configured login/root; `NO_AUTH` fits the proxy-authenticated flow. Selected for the initial rollout. |
| [FileBrowser Quantum](https://github.com/gtsteffaniak/filebrowser) | Closest Filebrowser-style experience, richer sources/permissions; supports proxy authentication. More configuration and explicit old-state migration. |
| [Copyparty](https://github.com/9001/copyparty) | Existing folders, per-user/per-folder permissions, resumable uploads and WebDAV; a broader file-server feature set and different interface. |
| [Filestash](https://www.filestash.app/) | Browser client for varied storage backends; useful when protocol/cloud access is needed, adding integration choices for this local-folder use case. |

The requested [Awesome Selfhosted section](https://github.com/awesome-selfhosted/awesome-selfhosted#file-transfer---web-based-file-managers)
provided the initial shortlist; capabilities above were checked against upstream
projects. Mikochi's latest [1.12.0 release](https://github.com/zer0tonin/Mikochi/releases/tag/1.12.0)
was published 2026-09-25 and includes authentication fixes. Its documented single
login is not a per-user permission model: with `NO_AUTH=true`, the proxy allowlist
controls access and both users share the mounted scope. Confirm supported
operations and use a current pinned image in a disposable trial.

Mikochi 1.12.0 is declared alongside Syncthing on clamps and joey, with
`files.<host>.<ui-domain>`, no application login, and a private backend. The
caller declares its user-file scope; the default is the Syncthing `data` directory,
separate from `config`. Both containers use the same numeric process identity
and host-user mapping. Existing synchronized folder paths are not moved.

Real-container CI passed file operations and mapped ownership. The disposable
VM passed LocalAPI authorization, repeat apply and dual-stack backend isolation;
HTTPS browser acceptance is pending external ACME/connectivity checks. See
[the acceptance record](../butane/ACCEPTANCE.md). Broader symlink behavior remains
unverified; denial of an unmounted host target does not prove all container-path
containment.

Disposable acceptance must test
large upload/download, filenames, folder operations, ownership, symlinks/root
containment, reboot, no second login and denial of other users/direct backend
access. Keep native login disabled only when the proxy is the sole entry point.
If different per-user folder rights become required, evaluate Quantum/Copyparty
before introducing another instance or custom authorization in Mikochi.
