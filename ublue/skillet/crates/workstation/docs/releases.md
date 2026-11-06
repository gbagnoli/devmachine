# Skillet releases

Run commands from `ublue/skillet`. The validator requires Python 3.11+ and Git;
it uses only the Python standard library.

1. Bump `[workspace.package].version` in `Cargo.toml`, update `Cargo.lock` with
   Cargo, run the normal checks and commit. All workspace crates inherit that version.
2. Create an annotated tag on that clean commit, for example:

   ```sh
   git tag -a skillet-v0.1.0 -m 'Skillet 0.1.0'
   python3 tools/release.py --tag skillet-v0.1.0
   ```

   Signed annotated tags (`git tag -s`) are also supported. Never move/reuse a release tag.
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
   python3 tools/release.py --tag skillet-v0.1.0 \
     --write-lock "$HOME/workspace/ucore-images/skillet-release.json"
   ```

   Review and commit the version/commit change there, then push ucore-images.
   Image CI checks the published release, annotated tag and Cargo version before
   building from the locked commit. It verifies matching binary provenance and
   transported bundles before the host image builds/signing.

For a prerelease, set a version such as `0.2.0-rc.1` and tag
`skillet-v0.2.0-rc.1`; the workflow marks its GitHub release as a prerelease.
Image consumers must explicitly select it in their dependency lock.

To check locally built release binaries too, add
`--binary-dir target/x86_64-unknown-linux-musl/release` to the validator command.
Artifact paths follow the explicit Cargo target directory used for that build.

Public bundle commands: [image bundles](image-bundles.md).
Compatibility policy: [design](../../../../design/skillet-releases.md).
