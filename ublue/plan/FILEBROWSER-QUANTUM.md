# File Browser Quantum trial

Branch: `feat/filebrowser-quantum`, from master. Production stays unchanged until
the phone/browser trial is accepted and the PR is merged and deliberately applied.

Current status: module, migration, fleet wiring and local checks are implemented.
The real pinned container passed proxy login, file mutations, ownership,
source-escape refusal and PDF byte-range tests. The owned VM reached signed
base/user-environment readiness. Full test-environment provisioning, ACME staging HTTPS, and declared-application
repeat-apply/reboot acceptance passed. PDF downloads work on the phone; inline
PDF viewing exposed Caddy's cross-site request guard. A per-service exact GET
exception for Quantum's short-lived `/api/resources/view` endpoint is implemented
with same-origin/absent-Origin checks and the identity gate retained. Redeploy
and phone preview acceptance remain open.

1. Replace the shared Syncthing file UI with a reusable Quantum module, pinned to
   `2.0.2-stable`. Preserve files scope, mapped ownership, `files` DNS identity,
   private backend and fresh per-request tailnet authorization. Use native proxy
   auth, disable password login and public shares, and grant ordinary file access.
2. Keep SQLite state/cache outside synchronized files. Verify and retire the old
   owned Quadlet without deleting any user files. Preserve old Caddy payload readers.
3. Update monitoring, Homer artwork, requirements documentation and runtime probes.
   CI must test the actual upstream consumer without vault/provider access.
4. Run local required checks. Create owned `clamps` / `quantum` VM, provision test
   Tailscale and Caddy, then run declared-application smoke including reboot.
   Publish only the private test URL to the operator; leave the VM for inspection.
5. Operator accepts phone navigation, uploads/downloads and multipage PDF behavior.
   Quantum intentionally falls back to downloading/opening PDFs on mobile Chromium;
   embedded rendering depends on the browser. Do not count HTTP success as UI acceptance.

Exit: working private test VM with Quantum and no Mikochi container, automated
auth/file/ownership/isolation checks recorded in archive/ACCEPTANCE.md, focused
commits ready for PR, and explicit pending phone acceptance. No real domains,
zone IDs or credentials in source or acceptance logs.
