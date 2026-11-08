# Skillet source releases

Skillet has one workspace version and annotated `skillet-vX.Y.Z` tags, independent
of other devmachine content. Executables retain `X.Y.Z+git.<revision>` provenance.
The compatibility contract covers CLI commands, credential payloads, image bundles
and retained-state migrations. The workspace is released as one component.

Pinned cargo-release owns version/lockfile edits, the consolidated release commit
and annotated tag; a small launcher verifies its version before delegating with
explicit configuration. This avoids maintaining another versioning implementation.
Releases start on clean `master`, default to preview, and never publish crates or
push automatically. CI exercises actual patch/prerelease cuts in disposable Git
repositories. Identity validation and GitHub publication remain separate gates.

During `0.x`, compatible fixes increment the patch; features or intentional
compatibility breaks increment the minor. Breaking changes require documented
migration/rollback behavior. Starting at `1.0.0`, follow normal
[SemVer](https://semver.org/) major/minor/patch compatibility rules. Prereleases
use explicit identifiers such as `0.2.0-rc.1`.

ucore-images selects an exact version and full commit in `skillet-release.json`.
The tag, Cargo version, source commit, publication status and binary provenance
must agree. The commit remains the build input, so a moved tag causes refusal
rather than changing a daily build's source. One reviewed lock replaces the
earlier commit-only pin; compatible ranges never silently advance builds.

Tag pushes reuse full CI, build matching binaries and validate public bundles
before publishing a source release. Image CI requires that published release and
continues compiling source through the existing bundle pipeline. Published versions
are never replaced; GitHub release immutability
is a repository setting, separate from these workflows. Hosts receive Skillet
through the signed OS image; existing live delivery remains available during migration.

Release identity/build jobs check out the event commit explicitly with full
history. The fetched annotated tag remains a separate object to validate; a
checkout fallback must not replace that ref with its peeled commit SHA.

After successful stable publication, CI proposes the exact tested dependency lock
to ucore-images through a version-specific branch/PR. A repository-limited
fine-grained token supplies Contents/Pull requests write access to that repository;
the source workflow token remains read-only outside publication. Only the lock
file is proposed, retries reuse the release branch, and review/merge remain
explicit. Prereleases are excluded from automatic proposals. A failed proposal
does not undo publication; retry only the failed job after repairing credentials.

Operations: [release guide](../skillet/crates/workstation/docs/releases.md).
