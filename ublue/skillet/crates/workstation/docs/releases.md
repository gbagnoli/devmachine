# Skillet releases

Run commands from `ublue/skillet`. Python 3.11+, Git, Cargo and the exact
`cargo-release` version in `tools/cargo-release-version` must be available on
`PATH`. The launcher checks that version before making changes; it installs nothing.
Use a cargo-release executable compatible with your host's glibc, or run the
tools together in a compatible container/toolbox.
Upstream performs read-only crates.io name lookups even with publishing disabled.

1. Commit the intended source changes, run the normal local checks and start
   from a clean `master` checkout. Preview the release:

   ```sh
   python3 tools/cut_release.py patch
   ```

   Use `minor`, `major`, `rc`, or an explicit version instead of `patch` when
   appropriate. The default is a dry run. All workspace crates inherit one version.
2. Review the preview, then execute locally:

   ```sh
   python3 tools/cut_release.py patch --execute
   python3 tools/release.py --tag skillet-v0.1.1
   ```

   Adjust the example tag to the resulting version. cargo-release updates the
   workspace version and lockfile, creates one release commit and one annotated
   `skillet-vVERSION` tag. It neither publishes to crates.io nor pushes. The
   launcher ignores implicit cargo-release configuration and uses `release.toml`.
   Never move/reuse a release tag. A partial failure may leave edits: inspect
   Git status and tags before retrying rather than automatically deleting them.
3. Before publishing the first release, enable **Settings → General → Releases →
   Enable release immutability** in devmachine. This is a GitHub repository setting;
   the workflow does not configure it. See [GitHub's instructions](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes).
4. Push the commit and then its tag. A tag push starts `Skillet release`: it checks
   the tag/version, reuses full CI without path-based skips, builds static binaries,
   verifies every host bundle, then publishes the source release. Only publication
   receives repository write permission. Existing releases are never overwritten.
   Wait for publication to succeed before updating ucore-images.
5. From a clean checkout of that release, generate the dependency lock in your
   ucore-images checkout:

   ```sh
   python3 tools/release.py --tag skillet-v0.1.1 \
     --write-lock "$HOME/workspace/ucore-images/skillet-release.json"
   ```

   Review and commit the version/commit change there, then push ucore-images.
   Image CI checks the published release, annotated tag and Cargo version before
   building from the locked commit. It verifies matching binary provenance and
   transported bundles before the host image builds/signing.

For a prerelease, use `python3 tools/cut_release.py 0.2.0-rc.1 --execute`;
the workflow marks its GitHub release as a prerelease.
Image consumers must explicitly select it in their dependency lock.

CI tests the pinned tool against temporary Git repositories and a local bare
remote: dry-run preservation, patch/prerelease tags, lockfile consistency,
branch refusal and unchanged remote refs. Run that check explicitly with
`python3 tools/tests/cargo_release_acceptance.py` in a compatible tool environment.

To check locally built release binaries too, add
`--binary-dir target/x86_64-unknown-linux-musl/release` to the validator command.
Artifact paths follow the explicit Cargo target directory used for that build.

Public bundle commands: [image bundles](image-bundles.md).
Compatibility policy: [design](../../../../design/skillet-releases.md).
